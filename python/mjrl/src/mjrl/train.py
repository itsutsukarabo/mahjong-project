"""学習のループ（PPO、RL_TRAINING）。

uv run mjrl-train --config python/mjrl/configs/s1a.toml [--run-dir runs/<name>] [--max-updates N] [--max-minutes M] [--no-resume]

- runs/<name>/ にチェックポイント（ckpt/）・統計（stats.jsonl）・評価用のモデル（models/）・プール（pool/）・
  記念のチェックポイント（memorial/）・自己対戦の牌譜と注釈（samples/）を書く
- 同じ run-dir で起動し直すと、最新のチェックポイントから再開する（D-058）
- 止め方：Ctrl+C、または runs/<name>/STOP を置く（どちらもチェックポイントを書いてから終わる）
"""

from __future__ import annotations

import argparse
import json
import random
import shutil
import signal
import time
from pathlib import Path

import numpy as np
import torch

from . import checkpoint
from .config import Config, from_dict, load
from .model import Dims, MjModel, num_params
from .pool import Pool
from .ppo import update as ppo_update
from .rollout import Collector, gae
from .samples import play_and_write
from .stats import Accum, JsonlLog
from .timing import Gpu, GpuSampler, Timer


def build_model(cfg: Config, dims: Dims, dev: torch.device) -> MjModel:
    return MjModel(cfg.model, dims).to(dev)


def model_file(model: MjModel, cfg: Config, update: int) -> dict:
    """評価・プール用のモデルだけのファイル"""
    d = model.dims
    return {
        "update": update,
        "model": model.state_dict(),
        "model_cfg": cfg.to_dict()["model"],
        "dims": {"policy": d.policy, "critic": d.critic, "max_events": d.max_events},
        "name": cfg.name,
    }


def load_model_file(path: Path, dev: torch.device) -> MjModel:
    from .config import ModelCfg

    d = torch.load(path, map_location=dev, weights_only=False)
    m = MjModel(ModelCfg(**d["model_cfg"]), Dims(**d["dims"])).to(dev)
    m.load_state_dict(d["model"])
    m.eval()
    return m


class Trainer:
    def __init__(self, cfg: Config, run_dir: Path, resume: bool = True):
        self.cfg = cfg
        self.run = run_dir
        run_dir.mkdir(parents=True, exist_ok=True)
        self.dev = torch.device(cfg.device if (cfg.device != "cuda" or torch.cuda.is_available()) else "cpu")
        if self.dev.type == "cuda":
            torch.cuda.set_per_process_memory_fraction(cfg.gpu.memory_fraction)
        random.seed(cfg.seed)
        np.random.seed(cfg.seed)
        torch.manual_seed(cfg.seed)
        self.gen = torch.Generator().manual_seed(cfg.seed)
        self.stats_log = JsonlLog(run_dir / "stats.jsonl")
        self.events_log = JsonlLog(run_dir / "events.jsonl")
        self.pool = Pool(cfg.pool, cfg.seed)
        self.update_idx = 0
        self.elapsed = 0.0
        self.totals = {"games": 0, "kyokus": 0, "decisions": 0, "train_rows": 0}
        self.overflow_total = 0
        self.stage = cfg.reward.stage
        first_game_index = 0
        ck = checkpoint.latest(run_dir / "ckpt") if resume else None
        self.state = checkpoint.load(ck, self.dev) if ck else None
        if self.state is None:
            (run_dir / "config.json").write_text(json.dumps(cfg.to_dict(), ensure_ascii=False, indent=1), encoding="utf-8")
            if self.stats_log.lines():
                raise RuntimeError(f"{run_dir} has stats but no checkpoint; use a new run dir")
        else:
            first_game_index = self.state["env"]["next_game_index"]
        self.collector = Collector(cfg, self.pool, self.dev, first_game_index)
        env = self.collector.env
        self.dims = Dims(policy=env.policy_dim, critic=env.critic_dim, max_events=env.max_events)
        self.model = build_model(cfg, self.dims, self.dev)
        self.opt = torch.optim.Adam(self.model.parameters(), lr=cfg.ppo.lr, eps=1e-5)
        self.opponents: dict[int, MjModel] = {}
        if self.state is not None:
            self._restore(self.state, ck)
        self.gpu = Gpu()
        self.sampler = GpuSampler(self.gpu)
        self.timer = Timer()
        self.accum = Accum()
        self.stop_requested = False

    def _restore(self, s: dict, path: Path) -> None:
        self.model.load_state_dict(s["model"])
        self.opt.load_state_dict(s["optimizer"])
        checkpoint.set_rng_state(s["rng"])
        self.gen.set_state(s["gen"])
        self.pool.load_state(s["pool"])
        self.update_idx = s["update"]
        self.elapsed = s["elapsed"]
        self.totals = s["totals"]
        self.overflow_total = s["overflow_total"]
        self.stage = s["stage"]
        self.stats_log.truncate(s["stats_lines"])
        self.events_log.truncate(s["events_lines"])
        self.events_log.append({"event": "resume", "update": self.update_idx, "from": str(path), "time": time.time()})

    def state_dict(self) -> dict:
        return {
            "update": self.update_idx,
            "model": self.model.state_dict(),
            "optimizer": self.opt.state_dict(),
            "rng": checkpoint.rng_state(),
            "gen": self.gen.get_state(),
            "env": {"next_game_index": self.collector.env.next_game_index},
            "pool": self.pool.state(),
            "stats_lines": self.stats_log.lines(),
            "events_lines": self.events_log.lines(),
            "totals": self.totals,
            "overflow_total": self.overflow_total,
            "elapsed": self.elapsed,
            "stage": self.stage,
            "config": self.cfg.to_dict(),
            "git": checkpoint.git_commit(),
        }

    def save(self) -> Path:
        with self.timer("checkpoint"):
            return checkpoint.save(self.run / "ckpt", self.update_idx, self.state_dict(), self.cfg.ckpt.keep)

    def _opponents(self) -> None:
        """プールの窓の中のモデルを読み込む（使わなくなったものは外す）"""
        want = {e.version: e for e in self.pool.window()}
        for v in list(self.opponents):
            if v not in want and not (self.collector.seat_model == v).any():
                del self.opponents[v]
        for v, e in want.items():
            if v not in self.opponents:
                self.opponents[v] = load_model_file(Path(e.path), self.dev)

    def periodic(self) -> None:
        c = self.cfg
        u = self.update_idx
        models = self.run / "models"
        if u % c.eval.every == 0 or u % c.pool.save_every == 0 or u % c.pool.memorial_every == 0:
            models.mkdir(parents=True, exist_ok=True)
            mf = models / f"u{u:07d}.pt"
            checkpoint.atomic_save(model_file(self.model, c, u), mf)
            if u % c.pool.save_every == 0 and not c.pool.self_only:
                self.pool.add(u, mf, self.run / "pool")
            if u % c.pool.memorial_every == 0:
                self.pool.add_memorial(u, mf, self.run / "memorial", "interval")
                self.events_log.append({"event": "memorial", "update": u, "reason": "interval", "time": time.time()})
            if u % c.eval.every != 0:
                mf.unlink()
            else:
                # 評価のプロセスが読む（読んだら消すか、最良のモデルとして残す）
                q = self.run / "eval_queue"
                q.mkdir(exist_ok=True)
                shutil.move(str(mf), q / mf.name)
        if c.samples.every and u % c.samples.every == 0:
            with self.timer("samples"):
                rnd = u // c.samples.every
                play_and_write(c, self.model, self.dev, self.run / "samples" / f"u{u:07d}", rnd, f"{c.name}@u{u}")
        if u % c.ckpt.every == 0:
            self.save()

    def step(self) -> dict:
        c = self.cfg
        t0 = time.perf_counter()
        wait = self.gpu.wait_cool(c.gpu.temp_limit, c.gpu.temp_resume)
        self.timer.add("gpu_thermal_wait", wait)
        self._opponents()
        col = self.collector
        g0, k0, d0 = col.games, col.kyokus, col.decisions
        rows = col.collect(self.model, self.opponents, self.timer, self.sampler, self.accum)
        with self.timer("gae"):
            value = rows.t["value"].float().cpu().numpy().astype(np.float64)
            adv, ret, train = gae(rows, value, c.ppo.gamma, c.ppo.lam)
        with self.timer("train"):
            losses = ppo_update(
                c, self.model, self.opt, rows, adv, ret, train, self.update_idx, col.num_players, self.gen
            )
            if self.dev.type == "cuda":
                torch.cuda.synchronize()
        self.sampler.sample()
        with self.timer("bookkeeping"):
            col.split_carry(rows)
        self.update_idx += 1
        el = time.perf_counter() - t0
        self.elapsed += el
        games, kyokus, decs = col.games - g0, col.kyokus - k0, col.decisions - d0
        self.totals["games"] += games
        self.totals["kyokus"] += kyokus
        self.totals["decisions"] += decs
        self.totals["train_rows"] += int(train.sum())
        self.overflow_total = col.env.overflow_total
        play = self.accum.take()
        timing = self.timer.take()
        timing["total"] = el
        row = {
            "update": self.update_idx,
            "time": time.time(),
            "elapsed": self.elapsed,
            "stage": self.stage,
            "totals": dict(self.totals),
            "next_game_index": col.env.next_game_index,
            "events_overflow_total": self.overflow_total,
            "events_overflow_finished_rows": col.overflow_rows,
            "reward": {
                "terminal_mean_latest": play.get("latest", {}).get("hora_rate"),
                "step_reward_mean": float(rows.reward[train].mean()) if train.any() else None,
                "adv_mean": float(adv[train].mean()) if train.any() else None,
                "return_mean": float(ret[train].mean()) if train.any() else None,
                "value_mean": float(value[train].mean()) if train.any() else None,
            },
            "train": losses,
            "play": play,
            "timing": timing,
            "gpu": self.sampler.take(),
            "throughput": {
                "decisions_per_s": decs / el,
                "train_rows_per_s": float(train.sum()) / el,
                "kyokus_per_s": kyokus / el,
                "games_this_update": games,
            },
        }
        if self.dev.type == "cuda":
            row["gpu"]["torch_max_allocated_mb"] = torch.cuda.max_memory_allocated() / 2**20
            row["gpu"]["torch_max_reserved_mb"] = torch.cuda.max_memory_reserved() / 2**20
            torch.cuda.reset_peak_memory_stats()
        self.stats_log.append(row)
        self.periodic()
        return row

    def train(self, max_updates: int | None = None, max_seconds: float | None = None, log_every: int = 1) -> None:
        start = time.perf_counter()
        done = 0
        if self.update_idx == 0:
            self.events_log.append({"event": "start", "update": 0, "time": time.time(), "params": num_params(self.model)})
            self.periodic()
        prev = signal.signal(signal.SIGINT, self._on_sigint)
        try:
            while True:
                if max_updates is not None and done >= max_updates:
                    break
                if max_seconds is not None and time.perf_counter() - start >= max_seconds:
                    break
                if self.stop_requested or (self.run / "STOP").exists():
                    break
                row = self.step()
                done += 1
                if log_every and self.update_idx % log_every == 0:
                    lat = row["play"].get("latest", {})
                    print(
                        f"u{self.update_idx} hora={fmt(lat.get('hora_rate'))} deal_in={fmt(lat.get('deal_in_rate'))} "
                        f"riichi={fmt(lat.get('riichi_rate'))} call={fmt(lat.get('call_rate'))} "
                        f"pg={row['train'].get('loss_policy', 0):.4f} vf={row['train'].get('loss_value', 0):.4f} "
                        f"ent={row['train'].get('entropy', 0):.3f} kl={row['train'].get('approx_kl', 0):.4f} "
                        f"dec/s={row['throughput']['decisions_per_s']:.0f} t={row['timing']['total']:.1f}s",
                        flush=True,
                    )
        finally:
            signal.signal(signal.SIGINT, prev)
            c = self.cfg
            if done and c.samples.every and self.update_idx % c.samples.every != 0:
                # 終わりの時点の自己対戦の牌譜も残す（途中の分は periodic が書く）
                u = self.update_idx
                with self.timer("samples"):
                    play_and_write(c, self.model, self.dev, self.run / "samples" / f"u{u:07d}", 10_000 + u, f"{c.name}@u{u}")
            p = self.save()
            self.events_log.append({"event": "stop", "update": self.update_idx, "checkpoint": str(p), "time": time.time()})

    def _on_sigint(self, *_):
        self.stop_requested = True


def fmt(x) -> str:
    return "-" if x is None else f"{x:.3f}"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--run-dir")
    ap.add_argument("--max-updates", type=int)
    ap.add_argument("--max-minutes", type=float)
    ap.add_argument("--no-resume", action="store_true")
    ap.add_argument("--set", action="append", default=[], help="上書き：section.key=value（TOML の値）")
    a = ap.parse_args()
    cfg = load(a.config)
    if a.set:
        import tomllib

        d = cfg.to_dict()
        for kv in a.set:
            k, v = kv.split("=", 1)
            *path, last = k.split(".")
            cur = d
            for p in path:
                cur = cur[p]
            cur[last] = tomllib.loads(f"x = {v}")["x"]
        cfg = from_dict(d)
    run = Path(a.run_dir or f"runs/{cfg.name}")
    tr = Trainer(cfg, run, resume=not a.no_resume)
    print(f"run={run} device={tr.dev} params={num_params(tr.model)} update={tr.update_idx}", flush=True)
    tr.train(max_updates=a.max_updates, max_seconds=a.max_minutes * 60 if a.max_minutes else None)


if __name__ == "__main__":
    main()
