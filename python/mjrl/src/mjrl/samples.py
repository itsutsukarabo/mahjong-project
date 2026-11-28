"""最新のモデルの自己対戦の牌譜（全情報版）と注釈（*.annot.jsonl：選んだ行動・各行動の確率・価値）を書き出す。

牌譜再生画面（web/apps/replay）でそのまま開ける形（LOG_FORMAT §6）。学習の環境とは別の小さな環境で打つ
（学習のデータには入れない）。注釈を作るのは書き出す対局（数局）の判断ポイントだけで、学習のループの外にある。

行動の選び方は2つ（`samples.modes`）：
- `sample`：方策の確率どおりに抽選（学習の対局と同じ）。ファイル `<game_id>.sample.mjlog.jsonl`／`.sample.annot.jsonl`
- `greedy`：一番確率の高い行動だけ（AI の「本気の打牌」）。同じ牌山を別の game_id（`GREEDY_OFFSET` を足した番号）で打つ。
  ファイル `<game_id>.greedy.mjlog.jsonl`／`.greedy.annot.jsonl`
注釈の `selection` にもどちらかを書く（LOG_FORMAT §6）
"""

from __future__ import annotations

import json
from pathlib import Path

import mj_py
import numpy as np
import torch

from .config import Config
from .model import MjModel
from .rollout import TENSOR_KEYS, forward, to_device

_NAMES: list[str] | None = None
# greedy の対局の game_id ＝ 同じ牌山の sample の対局の番号 ＋ これ
GREEDY_OFFSET = 1_000_000_000


def action_names() -> list[str]:
    global _NAMES
    if _NAMES is None:
        _NAMES = mj_py.action_names()
    return _NAMES


def play_and_write(cfg: Config, model: MjModel, dev: torch.device, out_dir: Path, round_idx: int, label: str) -> list[Path]:
    """`cfg.samples.games` 局を `cfg.samples.modes` のそれぞれで打って `out_dir` に書く。書いた牌譜のパスを返す"""
    paths = []
    for mode in cfg.samples.modes:
        paths += play_mode(cfg, model, dev, out_dir, round_idx, label, mode)
    return paths


def play_mode(
    cfg: Config, model: MjModel, dev: torch.device, out_dir: Path, round_idx: int, label: str, mode: str
) -> list[Path]:
    if mode not in ("sample", "greedy"):
        raise ValueError(f"samples.modes: {mode!r}")
    s = cfg.samples
    e = cfg.env
    nplayers = 3 if "sanma" in e.rules else 4
    first = round_idx * s.games
    off = GREEDY_OFFSET if mode == "greedy" else 0
    sched = [(off + i, mj_py.game_seed(s.seed, i), [False] * nplayers) for i in range(first, first + s.games)]
    env = mj_py.VecEnv(
        rules=e.rules,
        num_tables=s.games,
        seed=s.seed,
        threads=1,
        reward=e.reward,
        record_logs=True,
        max_events=e.max_events,
        overrides=e.overrides(),
        schedule=sched,
    )
    names = np.array(action_names(), dtype=object)
    annots: dict[int, list[dict]] = {}
    logs: dict[int, str] = {}
    gen = torch.Generator(device=dev).manual_seed(s.seed + round_idx)
    b = env.observe()
    model.eval()
    while not env.is_done:
        n = len(b["decision_id"])
        if n == 0:
            break
        t = to_device(b, TENSOR_KEYS, dev)
        with torch.no_grad():
            out = forward(model, t, cfg.ppo.bf16)
            probs = torch.softmax(out.logits.float(), -1)
            if mode == "greedy":
                a = probs.argmax(-1)
            else:
                a = torch.multinomial(probs, 1, generator=gen).squeeze(1)
        a_np = a.cpu().numpy()
        p_np = probs.cpu().numpy()
        v_np = out.value.float().cpu().numpy()
        for i in range(n):
            legal = np.nonzero(b["mask"][i])[0]
            gid = int(b["game_id"][i])
            annots.setdefault(gid, []).append(
                {
                    "game_id": mj_py.log_game_id(s.seed, gid),
                    "kyoku_idx": int(b["kyoku_idx"][i]),
                    "event_idx": int(b["event_idx"][i]),
                    "seat": int(b["seat"][i]),
                    "decision": names[a_np[i]],
                    "choices": [names[j] for j in legal],
                    "model": label,
                    "policy": [[names[j], round(float(p_np[i, j]), 6)] for j in legal],
                    "value": round(float(v_np[i]), 6),
                    "selection": mode,
                }
            )
        b = env.step(b["decision_id"], a_np.astype(np.int64))
        for f in b["finished"]:
            logs[f["game_id"]] = f["log_jsonl"]
    assert len(logs) == s.games, (len(logs), s.games)
    out_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    for gid, log in sorted(logs.items()):
        stem = f"{mj_py.log_game_id(s.seed, gid)}.{mode}"
        lp = out_dir / f"{stem}.mjlog.jsonl"
        lp.write_text(log, encoding="utf-8")
        (out_dir / f"{stem}.annot.jsonl").write_text(
            "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in annots.get(gid, [])), encoding="utf-8"
        )
        paths.append(lp)
    return paths
