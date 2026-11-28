"""PPO の更新（RL_TRAINING §2.2）。損失の内訳と、補助タスクの損失が全体に占める割合（D-050）を毎回記録する。"""

from __future__ import annotations

import numpy as np
import torch

from .config import Config
from .model import MjModel, aux_losses
from .rollout import Rows, forward

AUX_NAMES = ("opp_hand", "opp_shanten", "opp_tenpai", "opp_waits")


def schedule(cfg: Config, update: int) -> tuple[float, float]:
    """学習率とエントロピー係数（更新の回数で線形に下げる）"""
    f = min(update / max(cfg.ppo.total_updates, 1), 1.0)
    p = cfg.ppo
    return p.lr + (p.lr_final - p.lr) * f, p.ent_coef + (p.ent_coef_final - p.ent_coef) * f


def update(
    cfg: Config,
    model: MjModel,
    opt: torch.optim.Optimizer,
    rows: Rows,
    adv: np.ndarray,
    ret: np.ndarray,
    train: np.ndarray,
    update_idx: int,
    num_players: int,
    gen: torch.Generator,
) -> dict[str, float]:
    p = cfg.ppo
    dev = rows.t["policy"].device
    lr, ent_coef = schedule(cfg, update_idx)
    for g in opt.param_groups:
        g["lr"] = lr
    idx_all = torch.from_numpy(np.nonzero(train)[0]).to(dev)
    adv_t = torch.from_numpy(adv).float().to(dev)
    ret_t = torch.from_numpy(ret).float().to(dev)
    aux_w = {k: getattr(cfg.aux, k) for k in AUX_NAMES}
    acc: dict[str, torch.Tensor] = {}
    skipped = 0
    count = 0
    stopped_epoch = p.epochs
    model.train()
    for epoch in range(p.epochs):
        perm = idx_all[torch.randperm(len(idx_all), generator=gen, device="cpu").to(dev)]
        kls = []
        # 端数の小さなミニバッチは前のものに含める（1行だけのミニバッチはアドバンテージの標準偏差が出ず NaN になる）
        bounds = list(range(0, len(perm), p.minibatch))
        if len(bounds) > 1 and len(perm) - bounds[-1] < p.minibatch // 2:
            bounds.pop()
        ends = bounds[1:] + [len(perm)]
        for s, e in zip(bounds, ends):
            mb = perm[s:e]
            t = {k: v[mb] for k, v in rows.t.items()}
            out = forward(model, t, p.bf16)
            logp_all = torch.log_softmax(out.logits, dim=-1)
            logp = logp_all.gather(1, t["action"].unsqueeze(1)).squeeze(1)
            ratio = torch.exp(logp - t["logp"])
            a = adv_t[mb]
            a = (a - a.mean()) / (a.std(correction=0) + 1e-8)
            pg = -torch.min(ratio * a, ratio.clamp(1 - p.clip, 1 + p.clip) * a).mean()
            v_old = t["value"]
            v_clip = v_old + (out.value - v_old).clamp(-p.value_clip, p.value_clip)
            r = ret_t[mb]
            vf = 0.5 * torch.max((out.value - r) ** 2, (v_clip - r) ** 2).mean()
            probs = logp_all.exp()
            ent = -(probs * logp_all).sum(-1).mean()
            aux = aux_losses(out, t["aux"], num_players)
            aux_total = sum(aux_w[k] * aux[k] for k in AUX_NAMES)
            loss = pg + p.vf_coef * vf - ent_coef * ent + aux_total
            opt.zero_grad(set_to_none=True)
            if not torch.isfinite(loss):
                # 重みを壊さないように飛ばして数える（統計の nonfinite_skipped）
                skipped += 1
                continue
            loss.backward()
            gn = torch.nn.utils.clip_grad_norm_(model.parameters(), p.max_grad_norm)
            opt.step()
            with torch.no_grad():
                kl = ((ratio - 1) - torch.log(ratio)).mean()
                kls.append(kl)
                parts = {
                    "loss_total": loss,
                    "loss_policy": pg,
                    "loss_value": vf,
                    "entropy": ent,
                    "approx_kl": kl,
                    "clip_frac": ((ratio - 1).abs() > p.clip).float().mean(),
                    "grad_norm": gn,
                    "loss_aux_weighted": aux_total,
                }
                for k in AUX_NAMES:
                    parts[f"loss_{k}"] = aux[k]
                    parts[f"loss_{k}_weighted"] = aux_w[k] * aux[k]
                # GPU の上で足しておき、最後に1回だけ CPU に移す（ミニバッチごとに同期しない）
                for k, v in parts.items():
                    acc[k] = acc[k] + v.detach().float() if k in acc else v.detach().float()
                count += 1
        if kls and float(torch.stack(kls).mean()) > p.target_kl:
            stopped_epoch = epoch + 1
            break
    sums = {k: float(v) for k, v in acc.items()}
    out = {k: v / max(count, 1) for k, v in sums.items()}
    # 全体の損失に占める割合（各項の絶対値の和に対して。D-050）
    terms = {
        "policy": abs(out.get("loss_policy", 0.0)),
        "value": p.vf_coef * abs(out.get("loss_value", 0.0)),
        "entropy": ent_coef * abs(out.get("entropy", 0.0)),
        **{k: abs(out.get(f"loss_{k}_weighted", 0.0)) for k in AUX_NAMES},
    }
    tot = sum(terms.values()) or 1.0
    for k, v in terms.items():
        out[f"share_{k}"] = v / tot
    with torch.no_grad():
        v = rows.t["value"][idx_all].float()
        r = ret_t[idx_all]
        out["explained_variance"] = float(1 - (r - v).var() / (r.var() + 1e-8))
    out |= {"nonfinite_skipped": skipped, "lr": lr, "ent_coef": ent_coef, "epochs_run": stopped_epoch, "minibatches": count, "train_rows": len(idx_all)}
    return out
