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


# 牌の番号（ACTION_SPACE §2）
TILE_NAMES = (
    *(f"{i}m" for i in (1, 2, 3, 4)), "5mr", *(f"{i}m" for i in (5, 6, 7, 8, 9)),
    *(f"{i}p" for i in (1, 2, 3, 4)), "5pr", *(f"{i}p" for i in (5, 6, 7, 8, 9)),
    *(f"{i}s" for i in (1, 2, 3, 4)), "5sr", *(f"{i}s" for i in (5, 6, 7, 8, 9)),
    "E", "S", "W", "N", "P", "F", "C",
)


def dahai_tiles(action_names: list[str]) -> np.ndarray:
    """行動の番号 → 打牌（手出し・ツモ切り）なら牌の番号、ほかは -1"""
    idx = {n: i for i, n in enumerate(TILE_NAMES)}
    out = np.full(len(action_names), -1, np.int64)
    for a, name in enumerate(action_names):
        p = name.split(":")
        if p[0] == "dahai":
            out[a] = idx[p[1]]
    return out


class DiscardStats:
    """シャンテン数を悪化させる打牌の割合（打牌の判断のうち、選んだ打牌の後のシャンテン数が、打牌の候補の中で最も小さい
    シャンテン数より大きいもの）。シャンテン数は普通の手・七対子・国士無双のうち最も小さいもの（mj-features が観測の
    `discard_candidates` に入れた値を読むだけで、ここでは計算しない。原則1）。他家の立直がない局面とある局面、
    抽選した行動（`sample`）と一番確率の高い行動（`greedy`）を分けて数える。立直の打牌は数えない（立直は聴牌なので悪化しない）
    """

    MODES = ("sample", "greedy")
    CONDS = ("no_riichi", "opp_riichi")

    def __init__(self, layout: list[tuple[str, int, int]], action_names: list[str], num_players: int) -> None:
        off = {n: (o, d) for n, o, d in layout}
        self.dc = off["discard_candidates"][0]
        self.seats = off["seats"][0]
        self.num_players = num_players
        self.tile = dahai_tiles(action_names)
        self.reset()

    def reset(self) -> None:
        self.c = {m: {c: [0, 0] for c in self.CONDS} for m in self.MODES}

    def add(self, policy: np.ndarray, actions: dict[str, np.ndarray]) -> None:
        if len(policy) == 0:
            return
        cand = policy[:, self.dc : self.dc + 37] > 0.5
        csh = np.rint(policy[:, self.dc + 37 : self.dc + 74] * 8.0 - 1.0)
        best = np.where(cand, csh, 99.0).min(axis=1)
        # 相対の席 1〜（人数 − 1）の「立直」（OBSERVATION §2 の seats：1席7つの4つ目）
        riichi = np.zeros(len(policy), bool)
        for r in range(1, self.num_players):
            riichi |= policy[:, self.seats + r * 7 + 3] > 0.5
        rows = np.arange(len(policy))
        for m in self.MODES:
            t = self.tile[actions[m]]
            is_d = t >= 0
            worse = is_d & (csh[rows, np.maximum(t, 0)] > best)
            for c, sel in (("no_riichi", ~riichi), ("opp_riichi", riichi)):
                self.c[m][c][0] += int((is_d & sel).sum())
                self.c[m][c][1] += int((worse & sel).sum())

    def take(self) -> dict:
        out = {
            m: {
                c: {"discards": d, "worsen": w, "rate": (w / d) if d else None}
                for c, (d, w) in self.c[m].items()
            }
            for m in self.MODES
        }
        self.reset()
        return out
