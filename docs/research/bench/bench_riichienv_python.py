"""RiichiEnv (PyPI wheel) throughput / determinism micro-benchmark.

Random legal-action agent, single process, single thread, Python driving loop.
Usage: python -I bench_riichienv.py <game_mode> <n_games> <skip_log 0|1>
"""
import hashlib
import json
import random
import sys
import time

from riichienv import RiichiEnv


def run_game(mode, seed, skip_log):
    env = RiichiEnv(game_mode=mode, skip_mjai_logging=skip_log, seed=seed)
    rng = random.Random(seed)
    obs = env.reset()
    steps = 0
    while not env.done():
        acts = {pid: rng.choice(sorted(obs[pid].legal_actions(), key=repr)) for pid in sorted(obs)}
        obs = env.step(acts)
        steps += 1
    return env, steps


def main():
    mode, n, skip = sys.argv[1], int(sys.argv[2]), sys.argv[3] == "1"
    # 1) kyoku count & determinism check with logging on (small sample)
    kyoku_total = 0
    for seed in range(20):
        env, _ = run_game(mode, seed, False)
        log = [json.loads(e) if isinstance(e, str) else e for e in env.mjai_log]
        kyoku_total += sum(1 for e in log if e.get("type") == "start_kyoku")
        if seed < 5:
            h1 = hashlib.sha256(json.dumps(log).encode()).hexdigest()
            env2, _ = run_game(mode, seed, False)
            log2 = [json.loads(e) if isinstance(e, str) else e for e in env2.mjai_log]
            h2 = hashlib.sha256(json.dumps(log2).encode()).hexdigest()
            assert h1 == h2, f"non-deterministic at seed {seed}"
    kyoku_per_game = kyoku_total / 20
    # 2) timed run
    t0 = time.perf_counter()
    steps = 0
    for seed in range(1000, 1000 + n):
        _, s = run_game(mode, seed, skip)
        steps += s
    dt = time.perf_counter() - t0
    print(json.dumps({
        "mode": mode, "games": n, "skip_mjai_logging": skip,
        "kyoku_per_game_sample20": kyoku_per_game,
        "sec": round(dt, 3), "games_per_sec": round(n / dt, 2),
        "kyoku_per_sec_est": round(n * kyoku_per_game / dt, 1),
        "steps_per_sec": round(steps / dt, 1),
        "determinism_check_seeds_0_4": "ok",
    }))


if __name__ == "__main__":
    main()
