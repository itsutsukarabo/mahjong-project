"""実験の設定（1実験1ファイル、`python/mjrl/configs/*.toml`。冒頭の `purpose` に「何を確かめる実験か」を書く）。

値の意味と初期値の根拠は docs/spec/RL_TRAINING.md（§2.2 PPO、§3 プール、§5 統計、§6 評価）。
"""

from __future__ import annotations

import dataclasses
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class EnvCfg:
    rules: str = "tenhou-sanma"
    # 1局戦（①・②）は single、③は hanchan
    length: str = "single"
    tables: int = 512
    threads: int = 0
    seed: int = 1
    reward: str = "tenhou-sanma-7dan+"
    max_events: int = 192

    def overrides(self) -> str:
        return "" if self.length == "hanchan" else f"[table]\nlength = '{self.length}'"


@dataclass
class ModelCfg:
    # D-048：中の案（4層・256次元）
    layers: int = 4
    d_model: int = 256
    heads: int = 8
    ff: int = 1024
    critic_hidden: int = 128


@dataclass
class PpoCfg:
    steps_per_update: int = 128
    minibatch: int = 4096
    epochs: int = 3
    clip: float = 0.2
    gamma: float = 0.995
    lam: float = 0.95
    lr: float = 3e-4
    lr_final: float = 3e-5
    ent_coef: float = 0.01
    ent_coef_final: float = 0.001
    vf_coef: float = 0.5
    value_clip: float = 0.2
    max_grad_norm: float = 0.5
    target_kl: float = 0.02
    # 学習率・エントロピー係数を下げきる更新の回数
    total_updates: int = 20000
    # BF16 の自動混合精度（Transformer の部分）
    bf16: bool = True


@dataclass
class RewardCfg:
    # 1a：和了 = +1（D-051）。1b・2・3 は次の段で作る
    stage: str = "1a"
    # ポテンシャル Φ = −シャンテン数 × shanten_coef（γ は ppo.gamma と同じ、D-051）
    shanten_coef: float = 0.1


@dataclass
class AuxCfg:
    # D-050：重み 0.1 から（タスクごと）
    opp_hand: float = 0.1
    opp_shanten: float = 0.1
    opp_tenpai: float = 0.1
    opp_waits: float = 0.1


@dataclass
class PoolCfg:
    # D-052。①-a は self_only = true（自分自身とだけ打つ）
    self_only: bool = True
    latest_frac: float = 0.5
    # 「最新から n バージョン以内」の n と、プールに入れる間隔（更新の回数）
    n: int = 10
    save_every: int = 50
    # 新しいものほど重く：重み ∝ decay^(新しい方からの順番)
    decay: float = 0.8
    # 記念のチェックポイント（D-053）を残す間隔
    memorial_every: int = 1000


@dataclass
class CkptCfg:
    every: int = 10
    keep: int = 3


@dataclass
class SamplesCfg:
    # 一定間隔で最新のモデルの自己対戦の牌譜（全情報版）と注釈を runs/<name>/samples/ に書く
    every: int = 50
    games: int = 2
    seed: int = 777


@dataclass
class GpuCfg:
    # 温度による待機（02_RL_DESIGN）：limit 以上なら resume 以下に下がるまで待つ
    temp_limit: int = 83
    temp_resume: int = 78
    # PyTorch が使ってよい VRAM の割合（ほかのアプリと合わせて VRAM を超えると、Windows は共有メモリに溢れさせて極端に遅くなる）
    memory_fraction: float = 0.6


@dataclass
class EvalCfg:
    # 評価用の牌山（学習の game_seed とは別の名前空間）。三麻はデュプリケートで席の並び6通り
    seed: int = 900001
    walls: int = 500
    tables: int = 256
    # 何更新ごとのチェックポイントを評価するか
    every: int = 50
    memorials: int = 3


@dataclass
class Config:
    name: str = "exp"
    purpose: str = ""
    device: str = "cuda"
    seed: int = 1
    env: EnvCfg = field(default_factory=EnvCfg)
    model: ModelCfg = field(default_factory=ModelCfg)
    ppo: PpoCfg = field(default_factory=PpoCfg)
    reward: RewardCfg = field(default_factory=RewardCfg)
    aux: AuxCfg = field(default_factory=AuxCfg)
    pool: PoolCfg = field(default_factory=PoolCfg)
    ckpt: CkptCfg = field(default_factory=CkptCfg)
    samples: SamplesCfg = field(default_factory=SamplesCfg)
    gpu: GpuCfg = field(default_factory=GpuCfg)
    eval: EvalCfg = field(default_factory=EvalCfg)

    def to_dict(self) -> dict[str, Any]:
        return dataclasses.asdict(self)


def _merge(obj: Any, d: dict[str, Any], path: str = "") -> Any:
    names = {f.name: f for f in dataclasses.fields(obj)}
    for k, v in d.items():
        if k not in names:
            raise ValueError(f"unknown config key {path}{k}")
        cur = getattr(obj, k)
        if dataclasses.is_dataclass(cur):
            if not isinstance(v, dict):
                raise ValueError(f"{path}{k} must be a table")
            _merge(cur, v, f"{path}{k}.")
        else:
            setattr(obj, k, type(cur)(v) if not isinstance(cur, bool) else bool(v))
    return obj


def from_dict(d: dict[str, Any]) -> Config:
    return _merge(Config(), d)


def load(path: str | Path) -> Config:
    with open(path, "rb") as f:
        return from_dict(tomllib.load(f))
