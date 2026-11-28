"""評価（RL_TRAINING §6、D-052〜D-056）。学習とは別のプロセスで、学習が書いたモデル（runs/<name>/eval_queue/）を読んで回す。

uv run mjrl-eval --run runs/<name> [--once] [--walls N]

- 三麻のデュプリケート：評価用の牌山（学習の game_seed とは別の名前空間）を、2つの打ち手 A（候補）・B（相手）で
  席の並び6通り（AAB・ABA・BAA・ABB・BAB・BBA）に打つ。指標の差は牌山ごとにまとめてから平均と 95% の区間を出す
- 相手：直近の最良のモデル（runs/<name>/best.pt）・記念のチェックポイント（新しい順に eval.memorials 個）・RuleBot（最低ライン）
- 行動は方策から引く（乱数は評価のシードから。同じモデルなら同じ結果）
- 書くもの：eval.jsonl（評価ごとの結果）・pairs.jsonl（レーティングのもとの勝ち負け）・rating.jsonl（全打ち手のレーティング）・
  curriculum.jsonl（D-055 の目安の判定。達しても切り替えない、D-056）
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import time
from pathlib import Path

import mj_py
import numpy as np
import torch

from .config import Config, from_dict
from .model import MjModel
from .pool import _mix
from .rating import fit
from .rollout import TENSOR_KEYS, forward, take, to_device
from .stats import COUNTS, JsonlLog, read_jsonl

# 席の並び（三麻）：A が座る席
ARRANGEMENTS_3 = [
    (True, True, False),
    (True, False, True),
    (False, True, True),
    (True, False, False),
    (False, True, False),
    (False, False, True),
]
METRICS = (
    "hora_rate",
    "deal_in_rate",
    "riichi_rate",
    "call_rate",
    "avg_rank",
    "score_delta",
    "reward",
)
# D-055 の「極端な値」：この範囲の外なら報酬の抜け道の兆候
EXTREME = {"deal_in_rate": (0.005, 0.6), "call_rate": (0.005, 0.95), "riichi_rate": (0.005, 0.9)}


def eval_seed(base: int, w: int) -> int:
    """評価用の牌山のシード（学習は splitmix(seed ^ splitmix(i)) なので名前空間が違う）"""
    return int(_mix(np.array([base * 1_000_003 + w], dtype=np.uint64))[0] >> np.uint64(1))


class Player:
    def __init__(self, name: str, model: MjModel | None):
        self.name = name
        self.model = model  # None は RuleBot（Rust の中で打つ）


def play_duplicate(cfg: Config, a: Player, opps: list[Player], walls: int, dev: torch.device, seed: int) -> dict:
    """A と各相手のデュプリケート。返り値：相手の名前 → 牌山ごとの指標・2席の組の勝ち負け"""
    e = cfg.env
    nseat = 3
    arr = ARRANGEMENTS_3
    sched = []
    for oi, b in enumerate(opps):
        for w in range(walls):
            gs = eval_seed(cfg.eval.seed, w)
            for ai, pat in enumerate(arr):
                gid = (oi * walls + w) * len(arr) + ai
                bots = [(not x) and b.model is None for x in pat]
                sched.append((gid, gs, bots))
    env = mj_py.VecEnv(
        rules=e.rules,
        num_tables=cfg.eval.tables,
        seed=cfg.eval.seed,
        threads=e.threads,
        reward=e.reward,
        max_events=e.max_events,
        overrides=e.overrides(),
        schedule=sched,
    )
    gen = torch.Generator(device=dev).manual_seed(seed)
    fins: list[dict] = []
    b_ = env.observe()
    fins += b_["finished"]
    while not env.is_done:
        gid = b_["game_id"]
        seat = b_["seat"].astype(np.int64)
        oi = gid // (walls * len(arr))
        ai = gid % len(arr)
        is_a = np.array(arr)[ai, seat]
        actions = np.zeros(len(gid), np.int64)
        t = to_device(b_, TENSOR_KEYS, dev)
        with torch.no_grad():
            groups = [(np.nonzero(is_a)[0], a.model)]
            for k, p in enumerate(opps):
                groups.append((np.nonzero((~is_a) & (oi == k))[0], p.model))
            for idx, m in groups:
                if len(idx) == 0:
                    continue
                assert m is not None, "RuleBot seats are played in Rust"
                o = forward(m, take(t, torch.from_numpy(idx).to(dev)), cfg.ppo.bf16, critic=False)
                pr = torch.softmax(o.logits.float(), -1)
                actions[idx] = torch.multinomial(pr, 1, generator=gen).squeeze(1).cpu().numpy()
        b_ = env.step(b_["decision_id"], actions)
        fins += b_["finished"]
    return summarize(fins, opps, walls, arr, nseat)


def summarize(fins: list[dict], opps: list[Player], walls: int, arr, nseat: int) -> dict:
    out = {}
    na = len(arr)
    by_gid = {f["game_id"]: f for f in fins}
    assert len(by_gid) == len(opps) * walls * na, (len(by_gid), len(opps) * walls * na)
    for oi, b in enumerate(opps):
        per_wall = {m: np.zeros((walls, 2)) for m in METRICS}
        counts = {"a": {k: 0.0 for k in COUNTS}, "b": {k: 0.0 for k in COUNTS}}
        ranks = {"a": [], "b": []}
        wins_a = wins_b = draws = 0
        for w in range(walls):
            acc = {m: [[], []] for m in METRICS}
            for ai, pat in enumerate(arr):
                f = by_gid[(oi * walls + w) * na + ai]
                st = f["stats"]
                for s in range(nseat):
                    side = 0 if pat[s] else 1
                    k = max(st["kyokus"][s], 1)
                    acc["hora_rate"][side].append(st["hora"][s] / k)
                    acc["deal_in_rate"][side].append(st["deal_in"][s] / k)
                    acc["riichi_rate"][side].append(st["riichi"][s] / k)
                    acc["call_rate"][side].append(st["call"][s] / k)
                    acc["avg_rank"][side].append(f["ranks"][s])
                    acc["score_delta"][side].append(st["score_delta"][s] / k)
                    acc["reward"][side].append(f["rewards_normalized"][s])
                    sd = "a" if pat[s] else "b"
                    for c in COUNTS:
                        counts[sd][c] += st[c][s]
                    ranks[sd].append(f["ranks"][s])
                sc = f["scores"]
                for i in range(nseat):
                    for j in range(i + 1, nseat):
                        if pat[i] == pat[j]:
                            continue
                        ia, ib = (i, j) if pat[i] else (j, i)
                        if sc[ia] > sc[ib]:
                            wins_a += 1
                        elif sc[ia] < sc[ib]:
                            wins_b += 1
                        else:
                            draws += 1
            for m in METRICS:
                per_wall[m][w] = [np.mean(acc[m][0]), np.mean(acc[m][1])]
        res = {"walls": walls, "games": walls * na, "metrics": {}, "pairs": {"wins_a": wins_a, "wins_b": wins_b, "draws": draws}}
        for m in METRICS:
            d = per_wall[m][:, 0] - per_wall[m][:, 1]
            sd = float(d.std(ddof=1)) if walls > 1 else float("nan")
            half = 1.96 * sd / math.sqrt(walls) if walls > 1 else float("nan")
            res["metrics"][m] = {
                "a": float(per_wall[m][:, 0].mean()),
                "b": float(per_wall[m][:, 1].mean()),
                "diff": float(d.mean()),
                "ci95": [float(d.mean() - half), float(d.mean() + half)],
                "sd_per_wall": sd,
            }
        res["counts"] = counts
        res["sd_per_seat_game"] = {
            "avg_rank_a": float(np.std(ranks["a"], ddof=1)) if len(ranks["a"]) > 1 else None,
        }
        out[b.name] = res
    return out


def required_walls(sd: float, delta: float) -> float | None:
    """差 delta を両側 5%・検出力 80% で見分けるのに要る牌山の数（1牌山 = 6対局）"""
    if not sd or not math.isfinite(sd) or delta <= 0:
        return None
    return ((1.96 + 0.8416) * sd / delta) ** 2


def load_model(path: Path, dev: torch.device) -> MjModel:
    from .train import load_model_file

    return load_model_file(path, dev)


def significant(m: dict) -> bool:
    return m["ci95"][0] > 0


class Evaluator:
    def __init__(self, run: Path, walls: int | None = None):
        self.run = run
        cfg = from_dict(json.loads((run / "config.json").read_text(encoding="utf-8")))
        if walls:
            cfg.eval.walls = walls
        self.cfg = cfg
        self.dev = torch.device("cuda" if torch.cuda.is_available() and cfg.device == "cuda" else "cpu")
        if self.dev.type == "cuda":
            torch.cuda.set_per_process_memory_fraction(0.15)
        self.eval_log = JsonlLog(run / "eval.jsonl")
        self.pairs_log = JsonlLog(run / "pairs.jsonl")
        self.rating_log = JsonlLog(run / "rating.jsonl")
        self.cur_log = JsonlLog(run / "curriculum.jsonl")
        # 段ごとの主な指標（①-a は和了率。①-b 以降は局収支、②以降は報酬）
        self.key = {"1a": "hora_rate", "1b": "score_delta"}.get(cfg.reward.stage, "reward")

    def pending(self) -> list[Path]:
        q = self.run / "eval_queue"
        return sorted(q.glob("u*.pt")) if q.exists() else []

    def evaluate(self, path: Path) -> dict:
        cfg = self.cfg
        upd = int(path.stem[1:])
        name = f"u{upd:07d}"
        t0 = time.time()
        cand = Player(name, load_model(path, self.dev))
        opps: list[Player] = []
        best_meta = self.run / "best.json"
        best = json.loads(best_meta.read_text(encoding="utf-8")) if best_meta.exists() else None
        if best is not None:
            opps.append(Player(best["name"], load_model(self.run / "best.pt", self.dev)))
        mem = sorted((self.run / "memorial").glob("v*.pt")) if (self.run / "memorial").exists() else []
        seen = {p.name for p in opps}
        for mp in mem[::-1][: cfg.eval.memorials]:
            mn = f"u{int(mp.stem[1:8]):07d}"
            if mn not in seen and mn != name:
                opps.append(Player(mn, load_model(mp, self.dev)))
                seen.add(mn)
        opps.append(Player("rulebot", None))
        res = play_duplicate(cfg, cand, opps, cfg.eval.walls, self.dev, cfg.eval.seed + upd)
        for o, r in res.items():
            self.pairs_log.append({"a": name, "b": o, "update": upd, **r["pairs"]})
        ratings = fit(read_jsonl(self.run / "pairs.jsonl"))
        key = self.key
        became_best = best is None or (best["name"] in res and significant(res[best["name"]]["metrics"][key]))
        if became_best:
            shutil.copyfile(path, self.run / "best.pt")
            best_meta.write_text(json.dumps({"name": name, "update": upd}), encoding="utf-8")
        row = {
            "update": upd,
            "candidate": name,
            "time": time.time(),
            "seconds": time.time() - t0,
            "walls": cfg.eval.walls,
            "best_before": best["name"] if best else None,
            "became_best": became_best,
            "results": res,
            "rating": ratings.get(name),
            "required_walls": {
                o: {
                    "hora_rate_0.01": required_walls(r["metrics"]["hora_rate"]["sd_per_wall"], 0.01),
                    "avg_rank_0.05": required_walls(r["metrics"]["avg_rank"]["sd_per_wall"], 0.05),
                    "score_delta_200": required_walls(r["metrics"]["score_delta"]["sd_per_wall"], 200.0),
                }
                for o, r in res.items()
            },
        }
        self.eval_log.append(row)
        self.rating_log.append({"update": upd, "time": time.time(), "ratings": ratings})
        self.curriculum(row)
        path.unlink()
        return row

    def curriculum(self, row: dict) -> None:
        """D-055 の目安に達したかの判定（切り替えはしない。D-056）"""
        stage = self.cfg.reward.stage
        res = row["results"]
        rb = res["rulebot"]["metrics"]
        hist = read_jsonl(self.run / "eval.jsonl")
        last3 = hist[-3:]
        # 伸びが止まった：直近3回の評価で、候補が直近の最良のモデルに有意に勝てなかった
        plateau = len(last3) == 3 and all(
            h["best_before"] is not None and not h["became_best"] for h in last3
        )
        flags = []
        for m, (lo, hi) in EXTREME.items():
            v = rb[m]["a"]
            if not (lo <= v <= hi):
                flags.append(f"{m}={v:.4f} outside [{lo}, {hi}]")
        cond: dict = {"plateau_last3": plateau, "not_extreme": not flags, "extreme_flags": flags}
        if stage == "1a":
            cond["hora_above_rulebot"] = significant(rb["hora_rate"])
            reached = cond["hora_above_rulebot"] and plateau and not flags
            target = "1b"
        elif stage == "1b":
            cond["score_delta_plateau"] = plateau
            reached = plateau and not flags
            target = "2"
        else:
            cond["beats_rulebot"] = significant(rb["reward"])
            reached = cond["beats_rulebot"] and plateau and not flags
            target = "3"
        out = {"update": row["update"], "stage": stage, "target": target, "reached": reached, "conditions": cond, "time": time.time()}
        self.cur_log.append(out)
        if reached:
            print(f"*** curriculum: stage {stage} -> {target} criteria reached at {row['candidate']} (D-056: not switching) ***", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--once", action="store_true", help="待っているモデルを評価したら終わる")
    ap.add_argument("--walls", type=int)
    ap.add_argument("--poll", type=float, default=20.0)
    a = ap.parse_args()
    ev = Evaluator(Path(a.run), a.walls)
    while True:
        p = ev.pending()
        if not p:
            if a.once:
                return
            if (Path(a.run) / "STOP_EVAL").exists():
                return
            time.sleep(a.poll)
            continue
        r = ev.evaluate(p[0])
        rb = r["results"]["rulebot"]["metrics"]
        print(
            f"eval {r['candidate']}: rating={r['rating']:.0f} best={r['became_best']} "
            f"hora a={rb['hora_rate']['a']:.4f} rulebot={rb['hora_rate']['b']:.4f} "
            f"avg_rank a={rb['avg_rank']['a']:.3f} ({r['seconds']:.0f}s)",
            flush=True,
        )


if __name__ == "__main__":
    main()
