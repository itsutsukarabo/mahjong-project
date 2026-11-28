"""時間の内訳と GPU の状態（RL_TRAINING §4.2・§5.1、02_RL_DESIGN「GPU 温度による待機時間を計測対象に入れる」）。"""

from __future__ import annotations

import time
from collections import defaultdict
from contextlib import contextmanager


class Timer:
    """区間ごとの壁時計の秒を足していく"""

    def __init__(self) -> None:
        self.sec: dict[str, float] = defaultdict(float)

    @contextmanager
    def __call__(self, name: str):
        s = time.perf_counter()
        try:
            yield
        finally:
            self.sec[name] += time.perf_counter() - s

    def add(self, name: str, sec: float) -> None:
        self.sec[name] += sec

    def take(self) -> dict[str, float]:
        out = dict(self.sec)
        self.sec.clear()
        return out


class Gpu:
    """NVML（nvidia-ml-py）で GPU の温度・使用率・VRAM を読む。読めない環境（CI など）では None を返す"""

    def __init__(self, index: int = 0) -> None:
        self.h = None
        try:
            import pynvml

            pynvml.nvmlInit()
            self.nvml = pynvml
            self.h = pynvml.nvmlDeviceGetHandleByIndex(index)
        except Exception:
            self.h = None

    def read(self) -> dict[str, float] | None:
        if self.h is None:
            return None
        n = self.nvml
        try:
            u = n.nvmlDeviceGetUtilizationRates(self.h)
            m = n.nvmlDeviceGetMemoryInfo(self.h)
            return {
                "temp_c": float(n.nvmlDeviceGetTemperature(self.h, n.NVML_TEMPERATURE_GPU)),
                "util_pct": float(u.gpu),
                "mem_util_pct": float(u.memory),
                "vram_used_mb": m.used / 2**20,
                "vram_total_mb": m.total / 2**20,
                "power_w": n.nvmlDeviceGetPowerUsage(self.h) / 1000.0,
            }
        except Exception:
            return None

    def wait_cool(self, limit: int, resume: int, poll: float = 1.0) -> float:
        """温度が limit 以上なら resume 以下に下がるまで待つ。待った秒を返す"""
        r = self.read()
        if r is None or r["temp_c"] < limit:
            return 0.0
        s = time.perf_counter()
        while True:
            time.sleep(poll)
            r = self.read()
            if r is None or r["temp_c"] <= resume:
                return time.perf_counter() - s


class GpuSampler:
    """更新の間に GPU の使用率などを何度か読み、平均と最大を出す（1回だけ読むと、その瞬間の値しか分からない）"""

    def __init__(self, gpu: Gpu) -> None:
        self.gpu = gpu
        self.samples: list[dict[str, float]] = []

    def sample(self) -> None:
        r = self.gpu.read()
        if r is not None:
            self.samples.append(r)

    def take(self) -> dict[str, float]:
        if not self.samples:
            return {}
        out: dict[str, float] = {}
        for k in self.samples[0]:
            v = [s[k] for s in self.samples]
            out[f"{k}_mean"] = sum(v) / len(v)
            out[f"{k}_max"] = max(v)
        self.samples.clear()
        return out
