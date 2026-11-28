"""PPO の更新（RL_TRAINING §2.2）。損失の内訳と、補助タスクの損失が全体に占める割合（D-050）、
共有の幹にかかる勾配の大きさの項ごとの割合（D-061）を毎回記録する。
ミニバッチは乱数で混ぜるか、イベント列の長さの近い行でまとめる（D-063 案1、`ppo.length_grouped`）。"""

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


def minibatches(
    idx_all: torch.Tensor, lens: torch.Tensor, size: int, grouped: bool, gen: torch.Generator
) -> list[torch.Tensor]:
    """1エポックのミニバッチ（学習する行の番号の列）。grouped なら長さ（同じ長さの中は乱数）で並べて切り、ミニバッチの順番を混ぜる。
    端数の小さなミニバッチは前のものに含める（1行だけのミニバッチはアドバンテージの標準偏差が出ず NaN になる）"""
    n = len(idx_all)
    if grouped:
        order = torch.argsort(lens.float() + torch.rand(n, generator=gen))
    else:
        order = torch.randperm(n, generator=gen)
    bounds = list(range(0, n, size))
    if len(bounds) > 1 and n - bounds[-1] < size // 2:
        bounds.pop()
    ends = bounds[1:] + [n]
    chunks = [order[s:e] for s, e in zip(bounds, ends)]
    if grouped:
        chunks = [chunks[i] for i in torch.randperm(len(chunks), generator=gen).tolist()]
    dev = idx_all.device
    return [idx_all[c.to(dev)] for c in chunks]


def grad_norms(terms: dict[str, torch.Tensor], params: list[torch.nn.Parameter]) -> dict[str, torch.Tensor]:
    """項ごとに、共有の幹のパラメータにかかる勾配の大きさ（L2）。重みと係数を掛けた後の項で測る（D-061）"""
    out = {}
    for k, v in terms.items():
        gs = torch.autograd.grad(v, params, retain_graph=True, allow_unused=True)
        sq = [g.float().pow(2).sum() for g in gs if g is not None]
        out[k] = torch.stack(sq).sum().sqrt() if sq else torch.zeros((), device=v.device)
    return out


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
    lens = rows.t["events_len"][idx_all].cpu()
    if p.adv_norm == "batch":
        a_all = adv_t[idx_all]
        a_mean, a_std = a_all.mean(), a_all.std(correction=0) + 1e-8
    elif p.adv_norm != "minibatch":
        raise ValueError(f"ppo.adv_norm = {p.adv_norm!r}")
    trunk = model.trunk_parameters()
    gacc: dict[str, torch.Tensor] = {}
    gcount = 0
    acc: dict[str, torch.Tensor] = {}
    skipped = 0
    count = 0
    stopped_epoch = p.epochs
    model.train()
    for epoch in range(p.epochs):
        kls = []
        for mb in minibatches(idx_all, lens, p.minibatch, p.length_grouped, gen):
            t = {k: v[mb] for k, v in rows.t.items()}
            out = forward(model, t, p.bf16)
            logp_all = torch.log_softmax(out.logits, dim=-1)
            logp = logp_all.gather(1, t["action"].unsqueeze(1)).squeeze(1)
            ratio = torch.exp(logp - t["logp"])
            a = adv_t[mb]
            if p.adv_norm == "batch":
                a = (a - a_mean) / a_std
            else:
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
            if gcount < p.grad_share_minibatches and torch.isfinite(loss):
                terms = {"policy": pg, "value": p.vf_coef * vf, "entropy": -ent_coef * ent}
                terms |= {k: aux_w[k] * aux[k] for k in AUX_NAMES}
                for k, v in grad_norms(terms, trunk).items():
                    gacc[k] = gacc[k] + v.detach() if k in gacc else v.detach()
                gcount += 1
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
    # 共有の幹にかかる勾配の大きさの項ごとの割合（D-061。項ごとの大きさの和に対して）
    if gcount:
        gn = {k: float(v) / gcount for k, v in gacc.items()}
        gtot = sum(gn.values()) or 1.0
        for k, v in gn.items():
            out[f"grad_trunk_{k}"] = v
            out[f"grad_share_{k}"] = v / gtot
    with torch.no_grad():
        v = rows.t["value"][idx_all].float()
        r = ret_t[idx_all]
        out["explained_variance"] = float(1 - (r - v).var() / (r.var() + 1e-8))
    out |= {"nonfinite_skipped": skipped, "lr": lr, "ent_coef": ent_coef, "epochs_run": stopped_epoch, "minibatches": count, "train_rows": len(idx_all), "grad_share_minibatches": gcount}
    return out
