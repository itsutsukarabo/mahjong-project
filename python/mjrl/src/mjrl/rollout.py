"""自己対戦のデータ集め（RL_TRAINING §2.1・§4.1）。

- mj-py の `VecEnv` の1回の `step` で、すべての卓の判断ポイントをまとめて受け取り、打ち手ごとにまとめて推論する
- 1件ずつの Python のループは使わない：観測・マスクは numpy の配列のまとまりのまま GPU に送り、
  席ごとの軌跡のつながり（次の判断ポイント）は「(卓, 席) → 直前の行」の表で配列のまま付ける
- 報酬（①-a、D-051）：和了 ＝ +1 を局の最後の判断ポイントに、ポテンシャル型のシャンテン報酬
  γΦ(s′) − Φ(s)（Φ = −係数 × シャンテン数、終わりの Φ = 0）を各判断ポイントに足す
- 集めた量の終わりで次の判断ポイントがまだない行（軌跡の末尾）は学習に使わず、次の回に持ち越す
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

import mj_py
import numpy as np
import torch

from .config import Config
from .model import MjModel
from .pool import LATEST, Pool
from .stats import Accum, DiscardStats, finished_arrays
from .timing import GpuSampler, Timer

TENSOR_KEYS = ("policy", "events", "events_len", "mask", "critic", "aux")


@dataclass
class Rows:
    """学習する席の行（GPU の tensor と CPU の numpy）。行の番号はこの回のデータの中の番号"""

    t: dict[str, torch.Tensor] = field(default_factory=dict)
    table: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int64))
    seat: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int64))
    game_id: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int64))
    step: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int64))
    phi: np.ndarray = field(default_factory=lambda: np.zeros(0, np.float64))
    reward: np.ndarray = field(default_factory=lambda: np.zeros(0, np.float64))
    done: np.ndarray = field(default_factory=lambda: np.zeros(0, bool))
    next: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int64))

    def __len__(self) -> int:
        return len(self.table)


def to_device(b: dict, keys, dev: torch.device) -> dict[str, torch.Tensor]:
    out = {}
    for k in keys:
        x = torch.from_numpy(b[k])
        if dev.type == "cuda":
            x = x.pin_memory().to(dev, non_blocking=True)
        out[k] = x
    return out


def take(t: dict[str, torch.Tensor], idx: torch.Tensor) -> dict[str, torch.Tensor]:
    return {k: v[idx] for k, v in t.items()}


# イベント列の長さを切り上げる単位（形の種類を減らし、GPU のメモリの断片化を防ぐ）
LEN_BUCKET = 16


def forward(model: MjModel, t: dict[str, torch.Tensor], bf16: bool, critic: bool = True):
    ml = max(int(t["events_len"].max()), 1) if len(t["events_len"]) else 1
    ml = min(-(-ml // LEN_BUCKET) * LEN_BUCKET, t["events"].shape[1])
    with torch.autocast(t["policy"].device.type, dtype=torch.bfloat16, enabled=bf16):
        return model(
            t["policy"], t["events"][:, :ml], t["events_len"], t["mask"], t["critic"] if critic else None
        )


class Collector:
    """学習の環境を持ち、PPO の1回の更新に使うデータを集める"""

    def __init__(self, cfg: Config, pool: Pool, dev: torch.device, first_game_index: int = 0):
        self.cfg = cfg
        self.dev = dev
        self.pool = pool
        e = cfg.env
        self.env = mj_py.VecEnv(
            rules=e.rules,
            num_tables=e.tables,
            seed=e.seed,
            threads=e.threads,
            reward=e.reward,
            max_events=e.max_events,
            overrides=e.overrides(),
            first_game_index=first_game_index,
        )
        self.seats = 4
        self.num_players = 3 if "sanma" in e.rules else 4
        # (卓, 席) → この回のデータの中の直前の行（-1 はなし）
        self.last = np.full(e.tables * self.seats, -1, dtype=np.int64)
        # 卓ごとの今の対局と、席ごとの打ち手（LATEST か過去の version）
        self.table_game = np.full(e.tables, -1, dtype=np.int64)
        self.seat_model = np.full((e.tables, self.seats), LATEST, dtype=np.int64)
        self.batch = self.env.observe()
        self.carry = Rows()
        self.env_timing = np.array(self.env.timing)
        self.decisions = 0
        self.games = 0
        self.kyokus = 0
        # 終わった対局でイベント列が上限を超えた行の数の和（D-045）
        self.overflow_rows = 0
        # 最新のモデルの席の局の終わりの報酬の和と数（この更新の分。stats の reward.terminal_mean_latest）
        self.term_sum = 0.0
        self.term_n = 0
        # シャンテン数を悪化させる打牌の割合（最新のモデルの席。抽選・一番確率の高い行動）
        self.discard = DiscardStats(self.env.policy_layout, mj_py.action_names(), self.num_players)

    def _assign_new_games(self, table: np.ndarray, game_id: np.ndarray) -> None:
        new = self.table_game[table] != game_id
        if not new.any():
            return
        t, g = table[new], game_id[new]
        # 同じ卓の行が複数あっても同じ対局なので、卓ごとに1つ
        t, first = np.unique(t, return_index=True)
        g = g[first]
        self.table_game[t] = g
        self.seat_model[t, : self.num_players] = self.pool.assign(g, self.num_players)

    def collect(
        self, model: MjModel, opponents: dict[int, MjModel], timer: Timer, gpu: GpuSampler, accum: Accum
    ) -> Rows:
        cfg = self.cfg
        gamma = cfg.ppo.gamma
        coef = cfg.reward.shanten_coef
        parts: list[dict] = []
        rows = self.carry
        base = len(rows)
        # この回のデータの行の配列（容量を見込んで確保する。1回の step で1卓から返る行は席の数まで）
        cap = base + cfg.env.tables * self.num_players * cfg.ppo.steps_per_update
        TB = np.zeros(cap, np.int64)
        SE = np.zeros(cap, np.int64)
        GI = np.zeros(cap, np.int64)
        ST = np.zeros(cap, np.int64)
        P = np.zeros(cap)
        R = np.zeros(cap)
        D = np.zeros(cap, bool)
        N = np.full(cap, -1, np.int64)
        if base:
            TB[:base], SE[:base], GI[:base], ST[:base] = rows.table, rows.seat, rows.game_id, rows.step
            P[:base], R[:base], D[:base], N[:base] = rows.phi, rows.reward, rows.done, rows.next
            parts.append(rows.t)
        n = base
        model.eval()
        for step in range(cfg.ppo.steps_per_update):
            b = self.batch
            with timer("bookkeeping"):
                table = b["table"]
                seat = b["seat"].astype(np.int64)
                game_id = b["game_id"]
                self._assign_new_games(table, game_id)
                who = self.seat_model[table, seat]
                learner = who == LATEST
            with timer("h2d"):
                t = to_device(b, TENSOR_KEYS, self.dev)
            actions = np.zeros(len(table), np.int64)
            with timer("infer"), torch.no_grad():
                li = np.nonzero(learner)[0]
                if len(li):
                    lt = take(t, torch.from_numpy(li).to(self.dev))
                    out = forward(model, lt, cfg.ppo.bf16)
                    dist = torch.distributions.Categorical(logits=out.logits)
                    a = dist.sample()
                    lt["action"] = a
                    lt["logp"] = dist.log_prob(a)
                    lt["value"] = out.value
                    actions[li] = a.cpu().numpy()
                    greedy = out.logits.argmax(-1).cpu().numpy()
                for v in np.unique(who[~learner]):
                    oi = np.nonzero(who == v)[0]
                    ot = take(t, torch.from_numpy(oi).to(self.dev))
                    o = forward(opponents[int(v)], ot, cfg.ppo.bf16, critic=False)
                    actions[oi] = torch.distributions.Categorical(logits=o.logits).sample().cpu().numpy()
            gpu.sample()
            with timer("bookkeeping"):
                k = len(li)
                if k:
                    self.discard.add(b["policy"][li], {"sample": actions[li], "greedy": greedy})
                    parts.append(lt)
                    idx = np.arange(n, n + k)
                    key = table[li] * self.seats + seat[li]
                    sh = b["shanten"][li].astype(np.float64)
                    phi = -coef * sh
                    prev = self.last[key]
                    has = prev >= 0
                    N[prev[has]] = idx[has]
                    R[prev[has]] += gamma * phi[has] - P[prev[has]]
                    self.last[key] = idx
                    TB[idx], SE[idx], GI[idx], ST[idx], P[idx] = table[li], seat[li], game_id[li], step, phi
                    n += k
                self.decisions += len(table)
            s = time.perf_counter()
            self.batch = self.env.step(b["decision_id"], actions)
            el = time.perf_counter() - s
            et = np.array(self.env.timing)
            d = et - self.env_timing
            self.env_timing = et
            timer.add("env_advance", d[0])
            timer.add("env_observe", d[1])
            timer.add("env_pack", d[2])
            timer.add("transfer_rust_to_numpy", max(el - d.sum(), 0.0))
            with timer("bookkeeping"):
                fin = self.batch["finished"]
                if fin:
                    fa = finished_arrays(fin)
                    ft = fa["table"]
                    models = self.seat_model[ft][:, : self.num_players]
                    labels = np.where(models == LATEST, "latest", np.char.add("v", models.astype(str)))
                    accum.add(fa, labels)
                    # 局の終わり：学習する席の最後の行に報酬（①-a：和了 = +1）と終わりの印
                    term = terminal_reward(cfg.reward, fa)
                    lat = models == LATEST
                    self.term_sum += float(term[:, : self.num_players][lat].sum())
                    self.term_n += int(lat.sum())
                    for s_ in range(self.num_players):
                        key = ft * self.seats + s_
                        last = self.last[key]
                        m = (last >= 0) & (models[:, s_] == LATEST)
                        li_ = last[m]
                        D[li_] = True
                        R[li_] += term[m, s_] - P[li_]
                        self.last[key] = -1
                    # 次の対局が同じ卓で始まったら、席の割り当ては次の観測で付け直す
                    self.table_game[ft] = -1
                    self.games += len(fin)
                    self.kyokus += int(fa["kyokus"][:, 0].sum())
                    self.overflow_rows += int(fa["overflow"].sum())
        with timer("bookkeeping"):
            out = Rows(
                t={k: torch.cat([p[k] for p in parts]) for k in (*TENSOR_KEYS, "action", "logp", "value")}
                if parts
                else {},
                table=TB[:n].copy(),
                seat=SE[:n].copy(),
                game_id=GI[:n].copy(),
                step=ST[:n].copy(),
                phi=P[:n].copy(),
                reward=R[:n].copy(),
                done=D[:n].copy(),
                next=N[:n].copy(),
            )
        return out

    def split_carry(self, rows: Rows) -> Rows:
        """次の判断ポイントがまだない行（末尾）を次の回に持ち越す。持ち越す行の番号を付け直す"""
        tail = (~rows.done) & (rows.next < 0)
        ti = np.nonzero(tail)[0]
        remap = np.full(len(rows), -1, np.int64)
        remap[ti] = np.arange(len(ti))
        idx = torch.from_numpy(ti).to(self.dev)
        self.carry = Rows(
            t={k: v[idx] for k, v in rows.t.items()},
            table=rows.table[ti],
            seat=rows.seat[ti],
            game_id=rows.game_id[ti],
            step=np.full(len(ti), -1, np.int64),
            phi=rows.phi[ti],
            reward=rows.reward[ti],
            done=rows.done[ti],
            next=np.full(len(ti), -1, np.int64),
        )
        live = self.last >= 0
        self.last[live] = remap[self.last[live]]
        assert (self.last[live] >= 0).all(), "every live seat has its last row carried over"
        return self.carry


def take_terminal(col: "Collector") -> float | None:
    """この更新の最新のモデルの席の局の終わりの報酬の平均（取ったら 0 に戻す）"""
    m = col.term_sum / col.term_n if col.term_n else None
    col.term_sum, col.term_n = 0.0, 0
    return m


def terminal_reward(rc, fa: dict[str, np.ndarray]) -> np.ndarray:
    """局（対局）の終わりの報酬（G × 席）。rc は RewardCfg"""
    if rc.stage == "1a":
        # 和了 = +1（ダブロンの両者もそれぞれ +1。D-051）
        return fa["hora"].astype(np.float64)
    if rc.stage == "1b":
        # 局収支：sign(v) · log1p(|v| / score_scale)（02_RL_DESIGN ①-b、RL_TRAINING §2.3）。1局戦なので対局 = 局
        if rc.b_source not in ("hora_delta", "score_delta"):
            raise ValueError(f"reward.b_source = {rc.b_source!r}")
        v = fa[rc.b_source].astype(np.float64)
        return np.sign(v) * np.log1p(np.abs(v) / rc.score_scale)
    raise NotImplementedError(f"reward stage {rc.stage}（② 以降は次の段で作る）")


def gae(rows: Rows, value: np.ndarray, gamma: float, lam: float) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """GAE（λ）。行は判断ポイントの順（step の昇順）に並んでいて、次の行は必ず後ろにある。
    step ごとにまとめて後ろから計算する（行ごとのループは使わない）。末尾（次がまだない行）は学習に使わない"""
    n = len(rows)
    nxt = rows.next
    has_next = nxt >= 0
    train = rows.done | has_next
    v_next = np.where(has_next, value[np.where(has_next, nxt, 0)], 0.0)
    delta = rows.reward + gamma * (~rows.done) * v_next - value
    delta[~train] = 0.0
    adv = np.zeros(n)
    steps = rows.step
    for s in np.unique(steps)[::-1]:
        i = np.nonzero(steps == s)[0]
        a_next = np.where(has_next[i], adv[np.where(has_next[i], nxt[i], 0)], 0.0)
        adv[i] = delta[i] + gamma * lam * (~rows.done[i]) * a_next
    adv[~train] = 0.0
    return adv, adv + value, train
