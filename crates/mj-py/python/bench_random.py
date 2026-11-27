"""mj_py の速さ：一様な乱数の行動で回し、判断ポイント/秒と局/秒を測る（学習のコードではない）。

python crates/mj-py/python/bench_random.py [--tables 256] [--threads 0] [--seconds 10] [--rules tenhou-sanma] [--length hanchan|single]
"""

import argparse
import time

import numpy as np

import mj_py


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tables", type=int, default=256)
    ap.add_argument("--threads", type=int, default=0)
    ap.add_argument("--seconds", type=float, default=10.0)
    ap.add_argument("--rules", default="tenhou-sanma")
    ap.add_argument("--length", default="hanchan")
    ap.add_argument("--seed", type=int, default=1)
    a = ap.parse_args()
    overrides = "" if a.length == "hanchan" else f"[table]\nlength = '{a.length}'"
    reward = "tenhou-sanma-7dan+" if a.rules == "tenhou-sanma" else "tenhou-sanma-7dan-tonnan"
    env = mj_py.VecEnv(rules=a.rules, num_tables=a.tables, seed=a.seed, threads=a.threads,
                       reward=reward, overrides=overrides)
    rng = np.random.default_rng(a.seed)
    b = env.observe()
    decisions = 0
    kyokus = 0
    games = 0
    t_env = 0.0
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < a.seconds:
        mask = b["mask"]
        # 行ごとに合法手から一様に1つ
        r = rng.random(mask.shape) * mask
        actions = r.argmax(axis=1).astype(np.int64)
        decisions += len(actions)
        s = time.perf_counter()
        b = env.step(b["decision_id"], actions)
        t_env += time.perf_counter() - s
        for f in b["finished"]:
            games += 1
            kyokus += f["kyokus"]
    el = time.perf_counter() - t0
    print(f"rules={a.rules} length={a.length} tables={a.tables} threads={a.threads or 'all'} "
          f"elapsed={el:.1f}s (in env.step {t_env:.1f}s)")
    print(f"decisions/s={decisions / el:.0f} kyoku/s={kyokus / el:.0f} games/s={games / el:.1f} "
          f"games={games} kyokus={kyokus}")


if __name__ == "__main__":
    main()
