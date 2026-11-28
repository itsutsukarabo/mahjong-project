"""VRAM の空きより大きいメモリを確保して、あふれたときの振る舞いを確かめる（D-064）。

uv run python -m mjrl.vram_check [--extra-gb 2]

- 「Prefer No Sysmem Fallback」が効いていれば、空きを超えたところで torch.OutOfMemoryError ですぐに止まる
- 効いていなければ（Windows の既定）、エラーにならずに共有メモリ（メインメモリ）にあふれ、確保と書き込みが遅くなる
学習が動いている間は実行しない（学習の VRAM を奪う）。
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

import torch

CHUNK_MB = 256


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--extra-gb", type=float, default=2.0, help="空きより何 GB 多く確保を試みるか")
    a = ap.parse_args()
    free, total = torch.cuda.mem_get_info()
    target = free + int(a.extra_gb * 1024**3)
    print(f"pid {os.getpid()} base {sys.base_prefix}")
    print(f"free {free / 2**20:.0f}MB total {total / 2**20:.0f}MB target {target / 2**20:.0f}MB", flush=True)
    chunks: list[torch.Tensor] = []
    n = CHUNK_MB * 2**20 // 4
    times: list[float] = []
    result = "no_error"
    t_all = time.perf_counter()
    try:
        while len(chunks) * CHUNK_MB * 2**20 < target:
            t = time.perf_counter()
            x = torch.empty(n, dtype=torch.float32, device="cuda")
            x.fill_(1.0)
            torch.cuda.synchronize()
            chunks.append(x)
            times.append(time.perf_counter() - t)
    except torch.OutOfMemoryError as e:
        result = "oom"
        print("OutOfMemoryError:", str(e).splitlines()[0], flush=True)
    allocated = len(chunks) * CHUNK_MB
    # 確保できた全部に書き込み直す時間（共有メモリにあふれていれば遅い）
    t = time.perf_counter()
    for x in chunks:
        x.add_(1.0)
    torch.cuda.synchronize()
    rewrite = time.perf_counter() - t
    first = sum(times[:8]) / max(1, min(8, len(times)))
    last = sum(times[-8:]) / max(1, min(8, len(times)))
    out = {
        "result": result,
        "free_mb_before": round(free / 2**20),
        "allocated_mb": allocated,
        "beyond_free_mb": round(allocated - free / 2**20),
        "chunk_ms_first8": round(first * 1000, 2),
        "chunk_ms_last8": round(last * 1000, 2),
        "rewrite_s": round(rewrite, 3),
        "rewrite_gb_per_s": round(allocated / 1024 / rewrite, 1) if rewrite > 0 else None,
        "elapsed_s": round(time.perf_counter() - t_all, 2),
    }
    print(json.dumps(out, ensure_ascii=False))
    if target <= free:
        print("判定：空きの範囲内の確保（比べるための基準）")
    elif result == "oom":
        print("判定：VRAM があふれたらすぐにエラーで止まる（Prefer No Sysmem Fallback が効いている）")
    else:
        print("判定：エラーにならなかった（共有メモリにあふれている。設定が効いていない）")


if __name__ == "__main__":
    main()
