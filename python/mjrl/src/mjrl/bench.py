"""モデルの推論・学習の速さ（サンプル/秒）と VRAM の実測（RL_TRAINING §1.5・§4.2）。

入力は mj-py の環境（一様な乱数の行動の1局戦）から集めた本物の観測。
uv run python -m mjrl.bench [--rows 16384] [--batch 512 2048 4096] [--layers 4 --d-model 256]
"""

from __future__ import annotations

import argparse
import json
import time

import mj_py
import numpy as np
import torch

from .config import EnvCfg, ModelCfg
from .model import Dims, MjModel, aux_losses, num_params


def collect(rows: int, seed: int = 1) -> dict[str, np.ndarray]:
    env_cfg = EnvCfg(tables=256)
    env = mj_py.VecEnv(rules=env_cfg.rules, num_tables=env_cfg.tables, seed=seed, overrides=env_cfg.overrides())
    rng = np.random.default_rng(seed)
    b = env.observe()
    keys = ["policy", "events", "events_len", "mask", "critic", "aux"]
    out: dict[str, list[np.ndarray]] = {k: [] for k in keys}
    n = 0
    while n < rows:
        for k in keys:
            out[k].append(b[k])
        n += len(b["decision_id"])
        a = (rng.random(b["mask"].shape) * b["mask"]).argmax(1).astype(np.int64)
        b = env.step(b["decision_id"], a)
    return {k: np.concatenate(v)[:rows] for k, v in out.items()}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rows", type=int, default=16384)
    ap.add_argument("--batch", type=int, nargs="+", default=[512, 1024, 2048, 4096])
    ap.add_argument("--layers", type=int, default=4)
    ap.add_argument("--d-model", type=int, default=256)
    ap.add_argument("--heads", type=int, default=8)
    ap.add_argument("--ff", type=int, default=1024)
    ap.add_argument("--iters", type=int, default=20)
    a = ap.parse_args()
    dev = torch.device("cuda")
    data = collect(a.rows)
    lens = data["events_len"]
    mcfg = ModelCfg(layers=a.layers, d_model=a.d_model, heads=a.heads, ff=a.ff)
    model = MjModel(mcfg, Dims(policy=data["policy"].shape[1], critic=data["critic"].shape[1], max_events=192)).to(dev)
    opt = torch.optim.Adam(model.parameters(), lr=1e-4)
    t = {k: torch.from_numpy(v).to(dev) for k, v in data.items()}
    res: dict = {
        "params": num_params(model),
        "rows": a.rows,
        "events_len_mean": float(lens.mean()),
        "events_len_max": int(lens.max()),
        "device": torch.cuda.get_device_name(),
        "results": [],
    }
    for bs in a.batch:
        idx = [torch.arange(i, i + bs, device=dev) % a.rows for i in range(0, bs * a.iters, bs)]

        def batch(ix):
            ml = int(t["events_len"][ix].max())
            return (t["policy"][ix], t["events"][ix, :ml], t["events_len"][ix], t["mask"][ix], t["critic"][ix], t["aux"][ix])

        # 前の大きさの学習で確保した領域を返す（返さないと VRAM を超えて共有メモリに溢れ、極端に遅くなる）
        opt.zero_grad(set_to_none=True)
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        # 推論
        with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
            for ix in idx[:3]:
                model(*batch(ix)[:5])
            torch.cuda.synchronize()
            s = time.perf_counter()
            for ix in idx:
                model(*batch(ix)[:5])
            torch.cuda.synchronize()
            inf = bs * len(idx) / (time.perf_counter() - s)
        inf_mem = torch.cuda.max_memory_allocated() / 2**20
        torch.cuda.reset_peak_memory_stats()

        def train_step(ix):
            p, e, el, m, c, x = batch(ix)
            with torch.autocast("cuda", dtype=torch.bfloat16):
                out = model(p, e, el, m, c)
                loss = out.logits.logsumexp(-1).mean() * 0 + out.value.pow(2).mean() + sum(aux_losses(out, x, 3).values())
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()

        for ix in idx[:3]:
            train_step(ix)
        torch.cuda.synchronize()
        s = time.perf_counter()
        for ix in idx:
            train_step(ix)
        torch.cuda.synchronize()
        tr = bs * len(idx) / (time.perf_counter() - s)
        res["results"].append(
            {
                "batch": bs,
                "infer_samples_per_s": round(inf),
                "train_samples_per_s": round(tr),
                "infer_peak_mb": round(inf_mem),
                "train_peak_mb": round(torch.cuda.max_memory_allocated() / 2**20),
            }
        )
        print(json.dumps(res["results"][-1]), flush=True)
    print(json.dumps(res, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
