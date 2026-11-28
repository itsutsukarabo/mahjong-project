"""モデル（D-048・D-049・D-050、RL_TRAINING §1）。

- イベント列（L × 6）：欄ごとの埋め込みの和 ＋ 位置の埋め込み（欄5、0〜191）
- 状態トークン：方策の入力ベクトル（policy）を MLP で1トークンにして先頭に置く（案 A）
- Transformer エンコーダ（pre-LN、パディングはマスク）→ 先頭トークンの出力 h
- 方策の頭（325、合法手マスク）・補助タスクの頭は h から。価値の頭だけが隠れた情報（critic）を見る（案1）
"""

from __future__ import annotations

from dataclasses import dataclass

import torch
import torch.nn.functional as F
from torch import nn

from .config import ModelCfg

NUM_ACTIONS = 325
EVENT_FIELDS = 6
# 欄ごとの値の数（-1 のパディングを 0 にずらすので +1）。OBSERVATION §3
FIELD_SIZES = (12 + 1, 4 + 1, 39 + 1, 8 + 1, 8 + 1)
NUM_KINDS = 34
# 補助タスクのラベルの並び（mj_py.aux_sections()、OBSERVATION §5.1）
AUX_HAND = slice(0, 3 * NUM_KINDS)
AUX_SHANTEN = slice(3 * NUM_KINDS, 3 * NUM_KINDS + 3)
AUX_TENPAI = slice(3 * NUM_KINDS + 3, 3 * NUM_KINDS + 6)
AUX_WAITS = slice(3 * NUM_KINDS + 6, 6 * NUM_KINDS + 6)
AUX_DIM = 6 * NUM_KINDS + 6
SHANTEN_CLASSES = 10  # −1〜8


@dataclass
class Dims:
    policy: int
    critic: int
    max_events: int


@dataclass
class Output:
    logits: torch.Tensor  # (B, 325)、使えない行動は大きな負の値
    value: torch.Tensor  # (B,)
    hand: torch.Tensor  # (B, 3, 34, 5)
    shanten: torch.Tensor  # (B, 3, 10)
    tenpai: torch.Tensor  # (B, 3)
    waits: torch.Tensor  # (B, 3, 34)


def mlp(i: int, h: int, o: int) -> nn.Sequential:
    return nn.Sequential(nn.Linear(i, h), nn.GELU(), nn.Linear(h, o))


class MjModel(nn.Module):
    def __init__(self, cfg: ModelCfg, dims: Dims):
        super().__init__()
        d = cfg.d_model
        self.dims = dims
        self.field_emb = nn.ModuleList(nn.Embedding(n, d) for n in FIELD_SIZES)
        self.pos_emb = nn.Embedding(dims.max_events + 1, d)
        self.state = mlp(dims.policy, d, d)
        self.state_type = nn.Parameter(torch.zeros(d))
        layer = nn.TransformerEncoderLayer(
            d, cfg.heads, cfg.ff, dropout=0.0, activation="gelu", batch_first=True, norm_first=True
        )
        self.encoder = nn.TransformerEncoder(layer, cfg.layers, enable_nested_tensor=False)
        self.norm = nn.LayerNorm(d)
        self.policy_head = mlp(d, d, NUM_ACTIONS)
        self.critic_in = mlp(dims.critic, cfg.critic_hidden, cfg.critic_hidden)
        self.value_head = mlp(d + cfg.critic_hidden, d, 1)
        self.hand_head = nn.Linear(d, 3 * NUM_KINDS * 5)
        self.shanten_head = nn.Linear(d, 3 * SHANTEN_CLASSES)
        self.tenpai_head = nn.Linear(d, 3)
        self.waits_head = nn.Linear(d, 3 * NUM_KINDS)

    def trunk(self, policy: torch.Tensor, events: torch.Tensor, events_len: torch.Tensor) -> torch.Tensor:
        """先頭トークンの出力 h（B, d）。events は (B, L, 6) の整数（-1 はパディング）"""
        b, length, _ = events.shape
        ev = events.long() + 1
        x = self.pos_emb(ev[..., 5].clamp(0, self.dims.max_events))
        for i, emb in enumerate(self.field_emb):
            x = x + emb(ev[..., i].clamp(0, emb.num_embeddings - 1))
        s = (self.state(policy) + self.state_type).unsqueeze(1)
        x = torch.cat([s, x], dim=1)
        ar = torch.arange(length + 1, device=events.device)
        pad = ar.unsqueeze(0) > events_len.unsqueeze(1)  # 先頭（状態トークン）は常に有効
        y = self.encoder(x, src_key_padding_mask=pad)
        return self.norm(y[:, 0])

    def forward(
        self,
        policy: torch.Tensor,
        events: torch.Tensor,
        events_len: torch.Tensor,
        mask: torch.Tensor,
        critic: torch.Tensor | None,
    ) -> Output:
        h = self.trunk(policy, events, events_len).float()
        logits = self.policy_head(h).float().masked_fill(~mask, -1e9)
        b = h.shape[0]
        if critic is None:
            value = torch.zeros(b, device=h.device)
        else:
            value = self.value_head(torch.cat([h, self.critic_in(critic)], dim=-1)).squeeze(-1).float()
        return Output(
            logits=logits,
            value=value,
            hand=self.hand_head(h).view(b, 3, NUM_KINDS, 5),
            shanten=self.shanten_head(h).view(b, 3, SHANTEN_CLASSES),
            tenpai=self.tenpai_head(h),
            waits=self.waits_head(h).view(b, 3, NUM_KINDS),
        )


def num_params(m: nn.Module) -> int:
    return sum(p.numel() for p in m.parameters())


def aux_losses(out: Output, aux: torch.Tensor, seats: int) -> dict[str, torch.Tensor]:
    """補助タスクの損失（いない席は除く。三麻は相対の席 1・2 だけ）"""
    b = aux.shape[0]
    hand = aux[:, AUX_HAND].view(b, 3, NUM_KINDS).long().clamp(0, 4)
    shanten = (aux[:, AUX_SHANTEN].long() + 1).clamp(0, SHANTEN_CLASSES - 1)
    tenpai = aux[:, AUX_TENPAI]
    waits = aux[:, AUX_WAITS].view(b, 3, NUM_KINDS)
    k = seats - 1
    return {
        "opp_hand": F.cross_entropy(out.hand[:, :k].reshape(-1, 5), hand[:, :k].reshape(-1)),
        "opp_shanten": F.cross_entropy(out.shanten[:, :k].reshape(-1, SHANTEN_CLASSES), shanten[:, :k].reshape(-1)),
        "opp_tenpai": F.binary_cross_entropy_with_logits(out.tenpai[:, :k], tenpai[:, :k]),
        "opp_waits": F.binary_cross_entropy_with_logits(out.waits[:, :k], waits[:, :k]),
    }
