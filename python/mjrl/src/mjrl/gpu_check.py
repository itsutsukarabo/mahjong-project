"""PyTorch が GPU で正しく動くかの確認（D-057）：版・GPU の名前と計算能力・CPU との計算の一致・BF16 の行列積・Transformer の学習1歩。

uv run python -m mjrl.gpu_check
"""

import json
import platform
import time

import torch


def main() -> None:
    r: dict = {
        "torch": torch.__version__,
        "cuda_build": torch.version.cuda,
        "cudnn": torch.backends.cudnn.version(),
        "python": platform.python_version(),
        "cuda_available": torch.cuda.is_available(),
    }
    if not torch.cuda.is_available():
        print(json.dumps(r, ensure_ascii=False, indent=1))
        raise SystemExit("CUDA is not available")
    dev = torch.device("cuda")
    p = torch.cuda.get_device_properties(dev)
    r |= {
        "device": p.name,
        "capability": f"{p.major}.{p.minor}",
        "arch_list": torch.cuda.get_arch_list(),
        "vram_total_mb": p.total_memory // 2**20,
    }
    g = torch.Generator().manual_seed(0)
    a = torch.randn(1024, 1024, generator=g, dtype=torch.float64)
    b = torch.randn(1024, 1024, generator=g, dtype=torch.float64)
    # FP32 の行列積：CPU（FP64）との差（TF32 を切って比べる）
    torch.backends.cuda.matmul.allow_tf32 = False
    c = (a.float().to(dev) @ b.float().to(dev)).double().cpu()
    r["fp32_matmul_max_rel_err"] = float(((c - a @ b).abs().max() / (a @ b).abs().max()))
    cb = (a.to(dev, torch.bfloat16) @ b.to(dev, torch.bfloat16)).double().cpu()
    r["bf16_matmul_max_rel_err"] = float(((cb - a @ b).abs().max() / (a @ b).abs().max()))
    # 小さな Transformer の学習1歩：CPU と GPU で損失が一致するか（FP32）
    torch.manual_seed(0)
    layer = torch.nn.TransformerEncoderLayer(64, 4, 128, batch_first=True, norm_first=True, dropout=0.0)
    x = torch.randn(8, 32, 64)
    loss_cpu = layer(x).pow(2).mean()
    loss_cpu.backward()
    grad_cpu = layer.linear1.weight.grad.clone()
    layer.zero_grad()
    layer_g = layer.to(dev)
    loss_gpu = layer_g(x.to(dev)).pow(2).mean()
    loss_gpu.backward()
    r["transformer_loss_cpu_gpu"] = [loss_cpu.item(), loss_gpu.item()]
    r["transformer_grad_max_abs_diff"] = float((layer_g.linear1.weight.grad.cpu() - grad_cpu).abs().max())
    # BF16 の行列積の速さ
    torch.backends.cuda.matmul.allow_tf32 = True
    m = torch.randn(8192, 8192, device=dev, dtype=torch.bfloat16)
    torch.cuda.synchronize()
    for _ in range(3):
        m @ m
    torch.cuda.synchronize()
    t = time.perf_counter()
    k = 20
    for _ in range(k):
        m @ m
    torch.cuda.synchronize()
    r["bf16_matmul_tflops"] = round(2 * 8192**3 * k / (time.perf_counter() - t) / 1e12, 1)
    ok = (
        r["fp32_matmul_max_rel_err"] < 1e-5
        and r["bf16_matmul_max_rel_err"] < 3e-2
        and abs(r["transformer_loss_cpu_gpu"][0] - r["transformer_loss_cpu_gpu"][1]) < 1e-4
        and r["transformer_grad_max_abs_diff"] < 1e-4
    )
    r["ok"] = ok
    print(json.dumps(r, ensure_ascii=False, indent=1))
    if not ok:
        raise SystemExit("GPU results differ from CPU")


if __name__ == "__main__":
    main()
