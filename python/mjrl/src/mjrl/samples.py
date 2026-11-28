"""最新のモデルの自己対戦の牌譜（全情報版）と注釈（*.annot.jsonl：選んだ行動・各行動の確率・価値）を書き出す。

牌譜再生画面（web/apps/replay）でそのまま開ける形（LOG_FORMAT §6）。学習の環境とは別の小さな環境で打つ
（学習のデータには入れない）。注釈を作るのは書き出す対局（数局）の判断ポイントだけで、学習のループの外にある。
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


def action_names() -> list[str]:
    global _NAMES
    if _NAMES is None:
        _NAMES = mj_py.action_names()
    return _NAMES


def play_and_write(cfg: Config, model: MjModel, dev: torch.device, out_dir: Path, round_idx: int, label: str) -> list[Path]:
    """`cfg.samples.games` 局を打って `out_dir` に書く。書いた牌譜のパスを返す"""
    s = cfg.samples
    e = cfg.env
    env = mj_py.VecEnv(
        rules=e.rules,
        num_tables=s.games,
        seed=s.seed,
        threads=1,
        reward=e.reward,
        record_logs=True,
        max_events=e.max_events,
        overrides=e.overrides(),
        first_game_index=round_idx * s.games,
    )
    names = np.array(action_names(), dtype=object)
    annots: dict[int, list[dict]] = {}
    logs: dict[int, str] = {}
    gen = torch.Generator(device=dev).manual_seed(s.seed + round_idx)
    b = env.observe()
    model.eval()
    while len(logs) < s.games:
        n = len(b["decision_id"])
        if n == 0:
            break
        t = to_device(b, TENSOR_KEYS, dev)
        with torch.no_grad():
            out = forward(model, t, cfg.ppo.bf16)
            probs = torch.softmax(out.logits, -1)
            a = torch.multinomial(probs, 1, generator=gen).squeeze(1)
        a_np = a.cpu().numpy()
        p_np = probs.float().cpu().numpy()
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
                }
            )
        b = env.step(b["decision_id"], a_np.astype(np.int64))
        for f in b["finished"]:
            if len(logs) < s.games and f["game_id"] not in logs:
                logs[f["game_id"]] = f["log_jsonl"]
    out_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    for gid, log in sorted(logs.items()):
        stem = mj_py.log_game_id(s.seed, gid)
        lp = out_dir / f"{stem}.mjlog.jsonl"
        lp.write_text(log, encoding="utf-8")
        (out_dir / f"{stem}.annot.jsonl").write_text(
            "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in annots.get(gid, [])), encoding="utf-8"
        )
        paths.append(lp)
    return paths
