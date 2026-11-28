"""見張り役（D-064）：学習と評価のプロセスを起動し、異常終了したら最新のチェックポイントから再開する。

uv run python -m mjrl.watchdog --config python/mjrl/configs/s1a.toml --run-dir runs/<name> --hours 9 [--set k=v ...]

- 学習（`mjrl.train`）が 0 以外で終わったら（VRAM 不足の CUDA のエラーなど）、数秒待ってから同じ run-dir で起動し直す
  （学習は最新のチェックポイントから再開する。D-058）。評価（`mjrl.evaluate`）も同じ
- 学習の統計（stats.jsonl）が `--hang-minutes` 分のあいだ増えなければ、止まったものとみなしてプロセスを落として再開する
- `--crash-window-min` 分のうちに `--max-crashes` 回を超えて落ちたら、再開をやめて止まる（`give_up`）
- 学習が 0 で終わったら（時間切れ・STOP）、評価の待ち（eval_queue）がなくなるのを待ってから評価を止める
- 記録：runs/<name>/watchdog.jsonl（起動・終了・再開・理由）、pids.txt（プロセスの番号）
- 止め方：runs/<name>/STOP を置く（学習がチェックポイントを書いて終わり、評価の残りを済ませてから終わる）
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from collections import deque
from pathlib import Path

POLL_S = 5.0


def tail(path: Path, n: int = 30) -> list[str]:
    if not path.exists():
        return []
    with open(path, "rb") as f:
        f.seek(0, 2)
        size = f.tell()
        f.seek(max(0, size - 16384))
        lines = f.read().decode("utf-8", "replace").splitlines()
    return lines[-n:]


def reason_of(lines: list[str]) -> str:
    """標準エラーの最後の例外の行（なければ最後の行。見張り役の区切りの行より後に何もなければ、外から落とされたとみなす）"""
    if lines and lines[-1].startswith("===== watchdog start"):
        return "no error output (killed from outside?)"
    for ln in reversed(lines):
        s = ln.strip()
        if s and ("Error" in s or "Exception" in s or s.startswith("RuntimeError") or "error" in s.lower()):
            return s[:500]
    return lines[-1].strip()[:500] if lines else ""


def kill_tree(p: subprocess.Popen) -> None:
    """Windows では uv の仮想環境の python.exe が本物の python.exe を子として起動するので、木ごと落とす"""
    if p.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/PID", str(p.pid), "/T", "/F"], capture_output=True)
    else:
        p.kill()
    try:
        p.wait(timeout=30)
    except subprocess.TimeoutExpired:
        pass


class Child:
    def __init__(self, name: str, run: Path, log: "Log"):
        self.name = name
        self.run = run
        self.log = log
        self.p: subprocess.Popen | None = None
        self.started = 0.0
        self.crashes: deque[float] = deque()
        self.restarts = 0

    def start(self, args: list[str]) -> None:
        out = open(self.run / f"{self.name}.out.log", "ab")
        err = open(self.run / f"{self.name}.err.log", "ab")
        banner = f"\n===== watchdog start {self.name} {time.strftime('%Y-%m-%d %H:%M:%S')} =====\n".encode()
        out.write(banner)
        err.write(banner)
        out.flush()
        err.flush()
        self.err_path = self.run / f"{self.name}.err.log"
        self.p = subprocess.Popen([sys.executable, "-u", *args], stdout=out, stderr=err, cwd=os.getcwd())
        out.close()
        err.close()
        self.started = time.time()
        with open(self.run / "pids.txt", "a", encoding="utf-8") as f:
            f.write(f"{self.name} {self.p.pid} {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
        self.log.write({"event": "start", "proc": self.name, "pid": self.p.pid, "args": args})

    def poll(self) -> int | None:
        return None if self.p is None else self.p.poll()


class Log:
    def __init__(self, path: Path):
        self.path = path

    def write(self, row: dict) -> None:
        row = {"time": time.time(), "at": time.strftime("%Y-%m-%d %H:%M:%S"), **row}
        with open(self.path, "a", encoding="utf-8") as f:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
        print(json.dumps(row, ensure_ascii=False), flush=True)


def stats_lines(run: Path) -> int:
    p = run / "stats.jsonl"
    if not p.exists():
        return 0
    with open(p, "rb") as f:
        return sum(1 for _ in f)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--hours", type=float, required=True, help="学習の時間の合計（再開をまたいで数える）")
    ap.add_argument("--set", action="append", default=[], help="mjrl.train にそのまま渡す")
    ap.add_argument("--no-eval", action="store_true")
    ap.add_argument("--max-crashes", type=int, default=3)
    ap.add_argument("--crash-window-min", type=float, default=30.0)
    ap.add_argument("--hang-minutes", type=float, default=20.0)
    ap.add_argument("--restart-delay", type=float, default=15.0)
    ap.add_argument("--eval-drain-minutes", type=float, default=30.0)
    a = ap.parse_args()
    run = Path(a.run_dir)
    run.mkdir(parents=True, exist_ok=True)
    log = Log(run / "watchdog.jsonl")
    deadline = time.time() + a.hours * 3600
    log.write({"event": "watchdog_start", "pid": os.getpid(), "hours": a.hours, "deadline": deadline, "config": a.config})
    for f in ("STOP", "STOP_EVAL"):
        if (run / f).exists():
            (run / f).unlink()
            log.write({"event": "removed_stale", "file": f})

    def train_args() -> list[str]:
        mins = max((deadline - time.time()) / 60, 0.0)
        args = ["-m", "mjrl.train", "--config", a.config, "--run-dir", str(run), "--max-minutes", f"{mins:.2f}"]
        for s in a.set:
            args += ["--set", s]
        return args

    train = Child("train", run, log)
    ev = Child("eval", run, log)
    train.start(train_args())
    if not a.no_eval:
        ev.start(["-m", "mjrl.evaluate", "--run", str(run)])
    last_lines, last_progress = stats_lines(run), time.time()
    train_done = False
    status = "done"
    try:
        while True:
            time.sleep(POLL_S)
            now = time.time()
            # ---- 学習
            if not train_done:
                code = train.poll()
                n = stats_lines(run)
                if n != last_lines:
                    last_lines, last_progress = n, now
                hung = code is None and now - last_progress > a.hang_minutes * 60
                if hung:
                    log.write({"event": "hang", "proc": "train", "minutes_without_stats": round((now - last_progress) / 60, 1)})
                    kill_tree(train.p)
                    code = train.poll()
                if code is not None:
                    lines = tail(train.err_path)
                    log.write(
                        {
                            "event": "exit",
                            "proc": "train",
                            "code": code,
                            "seconds": round(now - train.started, 1),
                            "reason": "hang" if hung else reason_of(lines) if code != 0 else "",
                            "stderr_tail": lines[-8:] if code != 0 else [],
                        }
                    )
                    if code == 0 and not hung:
                        train_done = True
                        log.write({"event": "train_finished", "stats_lines": n})
                    elif deadline - now < 60:
                        train_done = True
                        status = "deadline_after_crash"
                    else:
                        train.crashes.append(now)
                        while train.crashes and now - train.crashes[0] > a.crash_window_min * 60:
                            train.crashes.popleft()
                        if len(train.crashes) > a.max_crashes:
                            log.write(
                                {
                                    "event": "give_up",
                                    "proc": "train",
                                    "crashes_in_window": len(train.crashes),
                                    "window_min": a.crash_window_min,
                                }
                            )
                            status = "gave_up"
                            train_done = True
                        else:
                            time.sleep(a.restart_delay)
                            train.restarts += 1
                            log.write({"event": "restart", "proc": "train", "restarts": train.restarts})
                            train.start(train_args())
                            last_progress = time.time()
            # ---- 評価
            if not a.no_eval:
                code = ev.poll()
                if code is not None:
                    if code == 0:
                        log.write({"event": "exit", "proc": "eval", "code": 0})
                        break
                    lines = tail(ev.err_path)
                    log.write({"event": "exit", "proc": "eval", "code": code, "reason": reason_of(lines), "stderr_tail": lines[-8:]})
                    ev.crashes.append(now)
                    while ev.crashes and now - ev.crashes[0] > a.crash_window_min * 60:
                        ev.crashes.popleft()
                    if len(ev.crashes) > a.max_crashes:
                        log.write({"event": "give_up", "proc": "eval", "crashes_in_window": len(ev.crashes)})
                        if train_done:
                            break
                        a.no_eval = True
                    else:
                        time.sleep(a.restart_delay)
                        ev.restarts += 1
                        log.write({"event": "restart", "proc": "eval", "restarts": ev.restarts})
                        ev.start(["-m", "mjrl.evaluate", "--run", str(run)])
                if train_done and ev.poll() is None and not (run / "STOP_EVAL").exists():
                    q = run / "eval_queue"
                    waiting = list(q.glob("u*.pt")) if q.exists() else []
                    drain_start = getattr(ev, "drain_start", None) or now
                    ev.drain_start = drain_start
                    if not waiting or now - drain_start > a.eval_drain_minutes * 60:
                        (run / "STOP_EVAL").write_text("watchdog\n", encoding="utf-8")
                        log.write({"event": "stop_eval", "left_in_queue": len(waiting)})
            elif train_done:
                break
    except KeyboardInterrupt:
        status = "interrupted"
        (run / "STOP").write_text("watchdog\n", encoding="utf-8")
        log.write({"event": "interrupted"})
        for c in (train, ev):
            if c.p is not None:
                try:
                    c.p.wait(timeout=300)
                except subprocess.TimeoutExpired:
                    kill_tree(c.p)
    log.write({"event": "watchdog_end", "status": status, "train_restarts": train.restarts, "eval_restarts": ev.restarts})
    sys.exit(0 if status == "done" else 1)


if __name__ == "__main__":
    main()
