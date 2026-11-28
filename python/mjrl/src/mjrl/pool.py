"""対戦相手のプール（D-052）と記念のチェックポイント（D-053）。RL_TRAINING §3.1。

- ①-a（`self_only`）：全員が最新のモデル
- ①-b 以降：席ごとに 最新 `latest_frac`・過去 残り。過去は「最新から n バージョン以内」から、新しいものほど重く
  （重み ∝ decay^j、j = 0 が一番新しい）。引いた j のバージョンがまだなければ（n 個たまるまで）最新で埋める
- 1卓に最新が1人もいない対局は作らない（最初の席を最新にする）
- 席の割り当ては対局ごとに game_id から決める（乱数の状態を持たないので、再開しても同じ割り当てになる）
"""

from __future__ import annotations

import hashlib
import shutil
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from .config import PoolCfg

LATEST = -1


def _mix(x: np.ndarray) -> np.ndarray:
    """splitmix64（numpy の uint64 で、行ごとに独立）"""
    with np.errstate(over="ignore"):
        z = x.astype(np.uint64) + np.uint64(0x9E3779B97F4A7C15)
        z = (z ^ (z >> np.uint64(30))) * np.uint64(0xBF58476D1CE4E5B9)
        z = (z ^ (z >> np.uint64(27))) * np.uint64(0x94D049BB133111EB)
        return z ^ (z >> np.uint64(31))


def _unit(game_id: np.ndarray, seat: int, salt: int, seed: int) -> np.ndarray:
    """(game_id, 席, 用途) ごとの [0, 1) の一様な値"""
    with np.errstate(over="ignore"):
        k = _mix(game_id.astype(np.uint64) ^ _mix(np.uint64(seed * 131 + seat * 7 + salt)))
    return (k >> np.uint64(11)).astype(np.float64) / float(1 << 53)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


@dataclass
class Entry:
    version: int  # 保存した更新の番号
    path: str
    sha256: str

    def to_dict(self) -> dict:
        return {"version": self.version, "path": self.path, "sha256": self.sha256}


@dataclass
class Pool:
    cfg: PoolCfg
    seed: int
    # 古い順
    past: list[Entry] = field(default_factory=list)
    memorial: list[Entry] = field(default_factory=list)

    def assign(self, game_ids: np.ndarray, seats: int) -> np.ndarray:
        """新しい対局の席ごとの打ち手（G × 席）。LATEST か、past の version"""
        g = len(game_ids)
        out = np.full((g, seats), LATEST, dtype=np.int64)
        if self.cfg.self_only or g == 0:
            return out
        n = self.cfg.n
        w = self.cfg.decay ** np.arange(n)
        cdf = np.cumsum(w) / w.sum()
        newest = self.past[::-1][:n]
        versions = np.array([e.version for e in newest] + [LATEST] * (n - len(newest)), dtype=np.int64)
        for s in range(seats):
            u_latest = _unit(game_ids, s, 1, self.seed)
            u_pick = _unit(game_ids, s, 2, self.seed)
            j = np.searchsorted(cdf, u_pick, side="right").clip(0, n - 1)
            out[:, s] = np.where(u_latest < self.cfg.latest_frac, LATEST, versions[j])
        none_latest = (out != LATEST).all(axis=1)
        out[none_latest, 0] = LATEST
        return out

    def window(self) -> list[Entry]:
        return self.past[::-1][: self.cfg.n]

    def add(self, version: int, src: Path, pool_dir: Path) -> Entry:
        """最新のモデルの重みをプールに入れる。窓（n）から外れたものは消す（記念に残したものは別のファイル）"""
        pool_dir.mkdir(parents=True, exist_ok=True)
        dst = pool_dir / f"v{version:07d}.pt"
        shutil.copyfile(src, dst)
        e = Entry(version, str(dst), sha256(dst))
        self.past.append(e)
        keep = {x.version for x in self.window()}
        for x in [x for x in self.past if x.version not in keep]:
            Path(x.path).unlink(missing_ok=True)
        self.past = [x for x in self.past if x.version in keep]
        return e

    def add_memorial(self, version: int, src: Path, mem_dir: Path, reason: str) -> Entry:
        mem_dir.mkdir(parents=True, exist_ok=True)
        dst = mem_dir / f"v{version:07d}-{reason}.pt"
        shutil.copyfile(src, dst)
        e = Entry(version, str(dst), sha256(dst))
        self.memorial.append(e)
        return e

    def state(self) -> dict:
        return {
            "past": [e.to_dict() for e in self.past],
            "memorial": [e.to_dict() for e in self.memorial],
        }

    def load_state(self, d: dict) -> None:
        self.past = [Entry(**e) for e in d["past"]]
        self.memorial = [Entry(**e) for e in d["memorial"]]
        for e in self.past + self.memorial:
            if sha256(Path(e.path)) != e.sha256:
                raise RuntimeError(f"pool file changed: {e.path}")
