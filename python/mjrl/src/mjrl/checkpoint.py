"""チェックポイントと完全な再開（RL_TRAINING §3.2、D-058）。

保存するもの：モデル・最適化器・乱数（Python・NumPy・PyTorch の CPU と CUDA）・環境の「何局目まで始めたか」
（`next_game_index`）・対戦相手のプール・記念のチェックポイント・統計の行数・カリキュラムの段・更新の回数・経過時間・設定・git のコミット。
進行中の対局は保存しない（再開したら次の通し番号から新しい対局を始める）。
書き込みは一時ファイルに書いてから名前を変える（途中で落ちても壊れたチェックポイントを残さない）。
"""

from __future__ import annotations

import os
import random
import re
import subprocess
from pathlib import Path

import numpy as np
import torch

PATTERN = re.compile(r"^u(\d{7})\.pt$")


def git_commit() -> str:
    try:
        return subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True, timeout=10
        ).stdout.strip()
    except Exception:
        return "unknown"


def rng_state() -> dict:
    return {
        "python": random.getstate(),
        "numpy": np.random.get_state(),
        "torch": torch.get_rng_state(),
        "cuda": torch.cuda.get_rng_state_all() if torch.cuda.is_available() else [],
    }


def set_rng_state(s: dict) -> None:
    random.setstate(s["python"])
    np.random.set_state(s["numpy"])
    torch.set_rng_state(s["torch"])
    if s["cuda"] and torch.cuda.is_available():
        torch.cuda.set_rng_state_all(s["cuda"])


def atomic_save(obj: dict, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    torch.save(obj, tmp)
    os.replace(tmp, path)


def save(ckpt_dir: Path, update: int, state: dict, keep: int) -> Path:
    path = ckpt_dir / f"u{update:07d}.pt"
    atomic_save(state, path)
    olds = sorted(p for p in ckpt_dir.iterdir() if PATTERN.match(p.name))
    for p in olds[:-keep]:
        p.unlink()
    return path


def latest(ckpt_dir: Path) -> Path | None:
    if not ckpt_dir.exists():
        return None
    c = sorted(p for p in ckpt_dir.iterdir() if PATTERN.match(p.name))
    return c[-1] if c else None


def load(path: Path, dev: torch.device) -> dict:
    return torch.load(path, map_location=dev, weights_only=False)
