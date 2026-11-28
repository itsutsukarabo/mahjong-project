"""mjrl のテスト（CPU で数十秒。`uv run pytest python/mjrl/tests`）。"""

from __future__ import annotations

import json
from pathlib import Path

import mj_py
import numpy as np
import pytest
import torch

from mjrl import checkpoint
from mjrl.config import Config, load
from mjrl.evaluate import Evaluator
from mjrl.model import Dims, MjModel
from mjrl.pool import LATEST, Pool
from mjrl.rating import fit
from mjrl.rollout import Rows, gae
from mjrl.samples import play_and_write
from mjrl.ppo import minibatches
from mjrl.stats import DiscardStats, dahai_tiles, read_jsonl
from mjrl.train import Trainer

CONFIGS = Path(__file__).resolve().parents[1] / "configs"


def tiny(**over) -> Config:
    cfg = load(CONFIGS / "tiny-test.toml")
    for k, v in over.items():
        sec, key = k.split("__")
        setattr(getattr(cfg, sec), key, v)
    return cfg


def env_batch(rows: int = 64) -> dict:
    env = mj_py.VecEnv(num_tables=16, seed=5, overrides="[table]\nlength = 'single'")
    rng = np.random.default_rng(0)
    b = env.observe()
    out = []
    n = 0
    while n < rows:
        out.append(b)
        n += len(b["decision_id"])
        a = (rng.random(b["mask"].shape) * b["mask"]).argmax(1).astype(np.int64)
        b = env.step(b["decision_id"], a)
    keys = ["policy", "events", "events_len", "mask", "critic", "aux"]
    return {k: torch.from_numpy(np.concatenate([x[k] for x in out])[:rows]) for k in keys}


def test_policy_does_not_depend_on_critic_input():
    """D-049：隠れた情報（critic）を乱数に変えても、方策の出力（と補助タスクの頭）は変わらない。価値は変わる"""
    torch.manual_seed(0)
    cfg = tiny()
    t = env_batch()
    m = MjModel(cfg.model, Dims(t["policy"].shape[1], t["critic"].shape[1], 192))
    m.eval()
    with torch.no_grad():
        a = m(t["policy"], t["events"], t["events_len"], t["mask"], t["critic"])
        noise = torch.randn_like(t["critic"]) * 3
        b = m(t["policy"], t["events"], t["events_len"], t["mask"], noise)
        c = m(t["policy"], t["events"], t["events_len"], t["mask"], None)
    assert torch.equal(a.logits, b.logits)
    assert torch.equal(a.logits, c.logits)
    for k in ("hand", "shanten", "tenpai", "waits"):
        assert torch.equal(getattr(a, k), getattr(b, k))
    assert not torch.allclose(a.value, b.value)
    # 勾配でも：方策の損失から critic の入力の枝へ勾配が流れない
    m.zero_grad()
    out = m(t["policy"], t["events"], t["events_len"], t["mask"], t["critic"])
    torch.log_softmax(out.logits, -1).sum().backward()
    assert all(p.grad is None or torch.count_nonzero(p.grad) == 0 for p in m.critic_in.parameters())


def test_illegal_actions_get_zero_probability():
    torch.manual_seed(1)
    cfg = tiny()
    t = env_batch()
    m = MjModel(cfg.model, Dims(t["policy"].shape[1], t["critic"].shape[1], 192))
    with torch.no_grad():
        p = torch.softmax(m(t["policy"], t["events"], t["events_len"], t["mask"], None).logits, -1)
    assert float(p[~t["mask"]].max()) == 0.0


def ref_gae(rows: Rows, v: np.ndarray, gamma: float, lam: float) -> np.ndarray:
    """1行ずつ後ろからたどる素朴な GAE（テストの基準）"""
    adv = np.zeros(len(rows))
    for i in reversed(range(len(rows))):
        nx = rows.next[i]
        if rows.done[i]:
            adv[i] = rows.reward[i] - v[i]
        elif nx >= 0:
            adv[i] = rows.reward[i] + gamma * v[nx] - v[i] + gamma * lam * adv[nx]
    return adv


def test_gae_matches_reference():
    rng = np.random.default_rng(0)
    # 3本の軌跡（step の順に行が並ぶ）。1本目は終わり、2本目は末尾が未解決、3本目は持ち越し（step = -1）から始まる
    step = np.array([-1, 0, 0, 1, 1, 2, 2, 3, 4])
    nxt = np.array([2, 3, 4, 5, 6, 7, -1, 8, -1])
    done = np.array([False, False, False, False, False, True, False, False, True])
    rows = Rows(step=step, next=nxt, done=done, reward=rng.normal(size=9), table=np.zeros(9, np.int64))
    v = rng.normal(size=9)
    adv, ret, train = gae(rows, v, 0.99, 0.9)
    ref = ref_gae(rows, v, 0.99, 0.9)
    assert np.allclose(adv[train], ref[train])
    assert list(train) == [True, True, True, True, True, True, False, True, True]
    assert np.allclose(ret, adv + v)


def test_pool_assignment():
    cfg = tiny()
    p = Pool(cfg.pool, 1)
    g = np.arange(1000)
    assert (p.assign(g, 3) == LATEST).all()
    cfg.pool.self_only = False
    cfg.pool.n = 4
    p = Pool(cfg.pool, 1)
    # 過去がまだない：全員が最新で埋まる
    assert (p.assign(g, 3) == LATEST).all()
    p.past = [type("E", (), {"version": v})() for v in (10, 20)]
    a = p.assign(g, 3)
    assert np.array_equal(a, p.assign(g, 3)), "deterministic"
    assert ((a == LATEST).any(axis=1)).all(), "at least one latest per table"
    frac = (a == LATEST).mean()
    assert 0.5 < frac < 0.85, frac  # 最新 50% ＋ 過去の窓の空き（n=4 のうち 2）を最新で埋める
    # 新しいものほど多い
    assert (a == 20).sum() > (a == 10).sum() > 0


def test_rating_recovers_strength():
    # A が B に 3:1 → Elo の差 ≈ 400 log10(3) ≈ 191
    r = fit([{"a": "A", "b": "B", "wins_a": 3000, "wins_b": 1000, "draws": 0}, {"a": "A", "b": "rulebot", "wins_a": 2000, "wins_b": 2000, "draws": 0}])
    assert r["rulebot"] == 1000.0
    assert abs((r["A"] - r["B"]) - 190.8) < 5
    assert abs(r["A"] - 1000.0) < 5


def test_samples_open_in_replay(tmp_path):
    """自己対戦の牌譜と注釈が、エンジンで再生して判断ポイント・選択肢と一致する（牌譜再生画面でそのまま開ける）"""
    cfg = tiny()
    cfg.samples.games = 2
    t = env_batch(8)
    m = MjModel(cfg.model, Dims(t["policy"].shape[1], t["critic"].shape[1], 192))
    paths = play_and_write(cfg, m, torch.device("cpu"), tmp_path, 0, "tiny@u0")
    assert len(paths) == 4
    seeds: dict[str, list[int]] = {"sample": [], "greedy": []}
    for p in paths:
        mode = p.name.split(".")[-3]
        assert mode in seeds
        ann = p.with_name(p.name.replace(".mjlog.jsonl", ".annot.jsonl"))
        n = mj_py.validate_annotations(p.read_text(encoding="utf-8"), ann.read_text(encoding="utf-8"))
        assert n > 5
        rows = [json.loads(x) for x in ann.read_text(encoding="utf-8").splitlines()]
        assert all(r["selection"] == mode for r in rows)
        if mode == "greedy":
            # 一番確率の高い行動を選んでいる（同じ確率なら番号の小さい方 ＝ policy の並びの先）
            assert all(r["decision"] == max(r["policy"], key=lambda x: x[1])[0] for r in rows)
        first = json.loads(p.read_text(encoding="utf-8").splitlines()[0])
        seeds[mode].append(first["seed"]["game_seed"])
    # 抽選と一番確率の高い行動は同じ牌山（別の game_id）
    assert sorted(seeds["sample"]) == sorted(seeds["greedy"])


def run_updates(run: Path, cfg: Config, n: int) -> Trainer:
    tr = Trainer(cfg, run)
    tr.train(max_updates=n, log_every=0)
    return tr


@pytest.mark.parametrize("self_only", [True, False])
def test_resume_matches_uninterrupted(tmp_path, self_only):
    """途中で止めて再開しても、止めずに回したのと同じ形の記録になる（D-058：数値の完全な再現は求めない）"""
    over = dict(pool__self_only=self_only, pool__save_every=2, pool__n=2, pool__memorial_every=4, samples__every=0)
    a = run_updates(tmp_path / "a", tiny(**over), 6)
    b1 = run_updates(tmp_path / "b", tiny(**over), 3)
    gi = b1.collector.env.next_game_index
    del b1
    b2 = Trainer(tiny(**over), tmp_path / "b")
    assert b2.update_idx == 3
    # 再開した環境は保存した通し番号から卓の数だけ新しい対局を始める（進行中だった対局は捨てる）
    assert b2.collector.env.next_game_index == gi + b2.cfg.env.tables, "env restarts from the saved game index"
    b2.train(max_updates=3, log_every=0)
    sa = read_jsonl(tmp_path / "a" / "stats.jsonl")
    sb = read_jsonl(tmp_path / "b" / "stats.jsonl")
    assert [r["update"] for r in sa] == [r["update"] for r in sb] == list(range(1, 7))
    assert b2.update_idx == a.update_idx == 6
    assert [e.version for e in b2.pool.past] == [e.version for e in a.pool.past]
    assert [e.version for e in b2.pool.memorial] == [e.version for e in a.pool.memorial]
    if not self_only:
        assert [e.version for e in a.pool.past] == [4, 6]
    ev = [json.loads(x)["event"] for x in (tmp_path / "b" / "events.jsonl").read_text(encoding="utf-8").splitlines()]
    assert ev.count("resume") == 1
    # チェックポイントは壊れずに読める・古いものは消える
    ck = checkpoint.latest(tmp_path / "b" / "ckpt")
    assert ck is not None and ck.name == "u0000006.pt"
    assert len(list((tmp_path / "b" / "ckpt").glob("u*.pt"))) <= 2
    # 評価のプロセス（別のプロセスで回す前提。ここでは同じプロセスで1回）
    ev = Evaluator(tmp_path / "b", walls=2)
    rows = [ev.evaluate(p) for p in ev.pending()]
    assert rows and all("rulebot" in r["results"] for r in rows)
    # 抽選（レーティングの既定）と一番確率の高い行動の両方で評価する
    assert all(r["mode"] == "sample" and "rulebot" in r["results_by_mode"]["greedy"] for r in rows)
    rt = read_jsonl(tmp_path / "b" / "rating.jsonl")
    assert rt and set(rt[-1]["by_mode"]) == {"sample", "greedy"}
    assert read_jsonl(tmp_path / "b" / "curriculum.jsonl")
    # 評価の途中で落ちた後：eval.jsonl に行のない候補の組を消し、評価済みなのに残った待ちのファイルを消す
    pairs = tmp_path / "b" / "pairs.jsonl"
    n_pairs = len(read_jsonl(pairs))
    with open(pairs, "a", encoding="utf-8") as f:
        f.write(json.dumps({"a": "u9999999", "b": "rulebot", "update": 9999999, "mode": "sample", "wins_a": 1, "wins_b": 0, "draws": 0}) + "\n")
    q = tmp_path / "b" / "eval_queue"
    q.mkdir(exist_ok=True)
    (q / f"{rows[0]['candidate']}.pt").write_bytes(b"x")
    ev2 = Evaluator(tmp_path / "b", walls=2)
    assert len(read_jsonl(pairs)) == n_pairs
    assert ev2.pending() == [] and not (q / f"{rows[0]['candidate']}.pt").exists()


@pytest.mark.skipif(not torch.cuda.is_available(), reason="GPU がない（CI は CPU だけ）")
def test_resume_on_gpu(tmp_path):
    """GPU に読み込んだチェックポイントから再開できる（乱数の状態の tensor が GPU に移っても戻せる。試運転で見つけた不具合）"""
    over = dict(samples__every=0)
    cfg = tiny(**over)
    cfg.device = "cuda"
    cfg.gpu.memory_fraction = 0.1
    run_updates(tmp_path / "g", cfg, 2)
    cfg2 = tiny(**over)
    cfg2.device = "cuda"
    cfg2.gpu.memory_fraction = 0.1
    tr = Trainer(cfg2, tmp_path / "g")
    assert tr.update_idx == 2
    tr.train(max_updates=1, log_every=0)
    assert tr.update_idx == 3


@pytest.mark.parametrize("grouped", [False, True])
def test_minibatches_cover_every_row_once(grouped):
    """ミニバッチは学習する行をちょうど1回ずつ含む。長さでまとめたときは、ミニバッチの中の長さの幅が混ぜた場合より狭い（D-063）"""
    gen = torch.Generator().manual_seed(0)
    idx = torch.arange(10, 1010)
    lens = torch.randint(1, 192, (1000,), generator=gen)
    mbs = minibatches(idx, lens, 128, grouped, gen)
    allv = torch.cat(mbs)
    assert sorted(allv.tolist()) == idx.tolist()
    assert all(len(m) >= 64 for m in mbs)
    spread = np.mean([float(lens[m - 10].max() - lens[m - 10].min()) for m in mbs])
    assert (spread < 40) if grouped else (spread > 150)


def test_discard_stats_read_rust_shanten():
    """打牌の候補ごとのシャンテン数の読み方が Rust の値と合う：14枚形（和了形を除く）では、候補の中の最小 ＝ 今のシャンテン数。
    最小の候補を選べば悪化は 0、最大の候補（最小より大きいとき）を選べば必ず悪化に数える"""
    env = mj_py.VecEnv(num_tables=16, seed=7, overrides="[table]\nlength = 'single'")
    names = mj_py.action_names()
    ds = DiscardStats(env.policy_layout, names, 3)
    tile = dahai_tiles(names)
    assert names[int(np.nonzero(tile == 0)[0][0])] == "dahai:1m"
    rng = np.random.default_rng(1)
    b = env.observe()
    checked = 0
    for _ in range(300):
        pol = b["policy"]
        cand = pol[:, ds.dc : ds.dc + 37] > 0.5
        csh = np.rint(pol[:, ds.dc + 37 : ds.dc + 74] * 8 - 1)
        has = cand.any(1)
        best = np.where(cand, csh, 99).min(1)
        sh = b["shanten"].astype(np.int64)
        m = has & (sh >= 0)
        assert (best[m] == sh[m]).all()
        checked += int(m.sum())
        # 打牌の行動のうち、切った後のシャンテン数が最小／最大のもの
        acts_best = np.zeros(len(pol), np.int64)
        acts_worst = np.zeros(len(pol), np.int64)
        for i in np.nonzero(has)[0]:
            legal = [a for a in np.nonzero(b["mask"][i])[0] if tile[a] >= 0]
            v = [csh[i, tile[a]] for a in legal]
            acts_best[i] = legal[int(np.argmin(v))]
            acts_worst[i] = legal[int(np.argmax(v))]
        ds.add(pol[has], {"sample": acts_best[has], "greedy": acts_worst[has]})
        a = (rng.random(b["mask"].shape) * b["mask"]).argmax(1).astype(np.int64)
        b = env.step(b["decision_id"], a)
    out = ds.take()
    assert checked > 500
    for c in DiscardStats.CONDS:
        assert out["sample"][c]["worsen"] == 0
    tot = sum(out["greedy"][c]["discards"] for c in DiscardStats.CONDS)
    worse = sum(out["greedy"][c]["worsen"] for c in DiscardStats.CONDS)
    assert tot > 500 and worse > 0.5 * tot
    assert out["greedy"]["opp_riichi"]["discards"] > 0


def test_grad_shares_recorded(tmp_path):
    """共有の幹にかかる勾配の大きさの割合が項ごとに記録され、合計が 1（D-061）。長さでまとめたミニバッチでも学習が回る（D-063）"""
    cfg = tiny(samples__every=0, ppo__length_grouped=True, ppo__adv_norm="batch")
    tr = run_updates(tmp_path / "g", cfg, 2)
    st = read_jsonl(tmp_path / "g" / "stats.jsonl")
    tr_ = st[-1]["train"]
    keys = ["policy", "value", "entropy", "opp_hand", "opp_shanten", "opp_tenpai", "opp_waits"]
    assert abs(sum(tr_[f"grad_share_{k}"] for k in keys) - 1) < 1e-6
    assert all(tr_[f"grad_trunk_{k}"] > 0 for k in keys)
    d = st[-1]["discard"]
    assert d["sample"]["no_riichi"]["discards"] > 0
    assert "slow_update" in st[-1]
    del tr
