"""打ち方の統計（02_RL_DESIGN の7項目）と JSONL の記録。

席ごとの数は Rust（`mj_bots::stats::SeatStats`、mj-py の `finished[i]["stats"]`）が数える。ここでは数を足して割るだけで、
ルールの判定はしない（原則1）。
"""

from __future__ import annotations

import json
import os
from pathlib import Path

import numpy as np

COUNTS = (
    "kyokus",
    "hora",
    "tsumo",
    "hora_points",
    "deal_in",
    "riichi",
    "call",
    "exhaustive",
    "exhaustive_tenpai",
    "score_delta",
)


def finished_arrays(finished: list[dict]) -> dict[str, np.ndarray]:
    """終わった対局の list を、席ごとの数の配列（G × 席）にまとめる"""
    if not finished:
        return {}
    out = {k: np.array([f["stats"][k] for f in finished], dtype=np.int64) for k in COUNTS}
    out["rank"] = np.array([f["ranks"] for f in finished], dtype=np.int64)
    out["table"] = np.array([f["table"] for f in finished], dtype=np.int64)
    out["game_id"] = np.array([f["game_id"] for f in finished], dtype=np.int64)
    out["overflow"] = np.array([f["overflow"] for f in finished], dtype=np.int64)
    return out


class Accum:
    """打ち手の名前ごとに席の数を足していく"""

    def __init__(self) -> None:
        self.c: dict[str, dict[str, float]] = {}

    def add(self, arrays: dict[str, np.ndarray], labels: np.ndarray) -> None:
        """labels：G × 席 の打ち手の名前（str の配列）"""
        if not arrays:
            return
        for lab in np.unique(labels):
            m = labels == lab
            d = self.c.setdefault(str(lab), {k: 0.0 for k in (*COUNTS, "rank_sum", "seat_games")})
            for k in COUNTS:
                d[k] += float(arrays[k][m].sum())
            d["rank_sum"] += float(arrays["rank"][m].sum())
            d["seat_games"] += float(m.sum())

    def take(self) -> dict[str, dict[str, float]]:
        out = {k: rates(v) for k, v in self.c.items()}
        self.c = {}
        return out


def rates(d: dict[str, float]) -> dict[str, float]:
    """数から7項目（＋局収支）を出す"""
    k = max(d["kyokus"], 1.0)

    def div(a: float, b: float) -> float | None:
        return a / b if b > 0 else None

    r = {
        "hora_rate": d["hora"] / k,
        "deal_in_rate": d["deal_in"] / k,
        "riichi_rate": d["riichi"] / k,
        "call_rate": d["call"] / k,
        "avg_hora_points": div(d["hora_points"], d["hora"]),
        "exhaustive_tenpai_rate": div(d["exhaustive_tenpai"], d["exhaustive"]),
        "avg_rank": div(d["rank_sum"], d["seat_games"]),
        "avg_score_delta": d["score_delta"] / k,
        "tsumo_rate": d["tsumo"] / k,
    }
    r["counts"] = dict(d)
    return r


class JsonlLog:
    """1行1記録の追記。再開のときは保存した行数まで切り詰める（同じ更新の行が2回入らないように）"""

    def __init__(self, path: Path) -> None:
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)

    def lines(self) -> int:
        if not self.path.exists():
            return 0
        with open(self.path, "rb") as f:
            return sum(1 for _ in f)

    def truncate(self, n: int) -> None:
        if not self.path.exists():
            if n:
                raise RuntimeError(f"{self.path} missing but checkpoint expects {n} lines")
            return
        with open(self.path, "rb") as f:
            rows = f.readlines()
        if len(rows) < n:
            raise RuntimeError(f"{self.path} has {len(rows)} lines, checkpoint expects {n}")
        tmp = self.path.with_suffix(".tmp")
        with open(tmp, "wb") as f:
            f.writelines(rows[:n])
        os.replace(tmp, self.path)

    def append(self, row: dict) -> None:
        with open(self.path, "a", encoding="utf-8") as f:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    with open(path, encoding="utf-8") as f:
        return [json.loads(x) for x in f if x.strip()]
