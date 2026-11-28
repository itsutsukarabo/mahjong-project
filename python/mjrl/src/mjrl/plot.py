"""学習の推移のグラフ：runs/<name>/plots/*.png と、それを並べた runs/<name>/plots/index.html。

uv run mjrl-plot --run runs/<name>

元のデータは stats.jsonl（1行1更新）・eval.jsonl（1行1評価）・rating.jsonl（評価ごとの全打ち手のレーティング）。
"""

from __future__ import annotations

import argparse
import html
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

from .stats import read_jsonl  # noqa: E402

plt.rcParams["font.family"] = ["Yu Gothic", "Meiryo", "Noto Sans CJK JP", "DejaVu Sans"]
plt.rcParams["axes.grid"] = True
plt.rcParams["grid.alpha"] = 0.3


def series(rows: list[dict], *path: str):
    xs, ys = [], []
    for r in rows:
        v = r
        for p in path:
            v = v.get(p) if isinstance(v, dict) else None
            if v is None:
                break
        if v is not None:
            xs.append(r["update"])
            ys.append(v)
    return xs, ys


def smooth(ys: list[float], k: int) -> list[float]:
    out, s = [], 0.0
    for i, y in enumerate(ys):
        s += y
        if i >= k:
            s -= ys[i - k]
        out.append(s / min(i + 1, k))
    return out


def lines(ax, rows, items, k=5, title="", ylabel=""):
    for label, path in items:
        xs, ys = series(rows, *path)
        if xs:
            ax.plot(xs, ys, alpha=0.25, lw=1)
            ax.plot(xs, smooth(ys, k), lw=2, label=label, color=ax.lines[-1].get_color())
    ax.set_title(title)
    ax.set_xlabel("更新の回数")
    ax.set_ylabel(ylabel)
    if ax.lines:
        ax.legend(fontsize=8)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--smooth", type=int, default=5)
    a = ap.parse_args()
    run = Path(a.run)
    out = run / "plots"
    out.mkdir(exist_ok=True)
    st = read_jsonl(run / "stats.jsonl")
    ev = read_jsonl(run / "eval.jsonl")
    rt = read_jsonl(run / "rating.jsonl")
    k = a.smooth
    figs: list[tuple[str, str]] = []

    def save(fig, name, caption):
        fig.tight_layout()
        fig.savefig(out / f"{name}.png", dpi=110)
        plt.close(fig)
        figs.append((name, caption))

    fig, axs = plt.subplots(2, 2, figsize=(12, 8))
    lines(axs[0, 0], st, [("和了率", ("play", "latest", "hora_rate")), ("放銃率", ("play", "latest", "deal_in_rate"))], k, "和了率・放銃率（学習の対局、最新のモデルの席）")
    lines(axs[0, 1], st, [("立直率", ("play", "latest", "riichi_rate")), ("副露率", ("play", "latest", "call_rate")), ("流局時聴牌率", ("play", "latest", "exhaustive_tenpai_rate"))], k, "立直率・副露率・流局時聴牌率")
    lines(axs[1, 0], st, [("平均打点", ("play", "latest", "avg_hora_points"))], k, "平均打点（点）")
    # 学習の対局は全員が最新のモデル（①-a）だと平均順位は常に 2 なので、局収支だけを描く（順位は評価の図で見る）
    lines(axs[1, 1], st, [("局収支（点、1局あたり）", ("play", "latest", "avg_score_delta"))], k, "局収支")
    save(fig, "play", "打ち方の7項目（学習の対局）。薄い線は更新ごと、濃い線は移動平均")

    fig, axs = plt.subplots(2, 2, figsize=(12, 8))
    lines(axs[0, 0], st, [("方策", ("train", "loss_policy")), ("価値", ("train", "loss_value"))], k, "損失：方策・価値")
    lines(axs[0, 1], st, [(n, ("train", f"loss_{n}")) for n in ("opp_hand", "opp_shanten", "opp_tenpai", "opp_waits")], k, "補助タスクの損失（重みを掛ける前）")
    lines(axs[1, 0], st, [(n, ("train", f"share_{n}")) for n in ("policy", "value", "entropy", "opp_hand", "opp_shanten", "opp_tenpai", "opp_waits")], k, "全体の損失に占める割合（D-050）")
    lines(axs[1, 1], st, [("エントロピー", ("train", "entropy")), ("近似 KL × 100", ("train", "approx_kl")), ("クリップの割合", ("train", "clip_frac")), ("説明分散", ("train", "explained_variance"))], k, "エントロピー・KL・クリップ・価値の説明分散")
    ax = axs[1, 1]
    for ln in ax.lines:
        if ln.get_label() == "近似 KL × 100":
            ln.set_ydata([y * 100 for y in ln.get_ydata()])
    save(fig, "losses", "損失の内訳")

    fig, axs = plt.subplots(1, 2, figsize=(12, 4))
    lines(axs[0], st, [("局の終わりの報酬（和了 +1）", ("reward", "terminal_mean_latest")), ("リターン", ("reward", "return_mean")), ("価値", ("reward", "value_mean"))], k, "報酬")
    lines(axs[1], st, [("上限超え（累計）", ("events_overflow_total",))], 1, "イベント列の上限超え（D-045）")
    save(fig, "reward", "報酬・イベント列の上限超え")

    fig, axs = plt.subplots(1, 2, figsize=(12, 4))
    keys = ["env_advance", "env_observe", "env_pack", "transfer_rust_to_numpy", "h2d", "infer", "bookkeeping", "gae", "train", "gpu_thermal_wait", "samples", "checkpoint"]
    xs = [r["update"] for r in st]
    ys = [[r["timing"].get(kk, 0.0) for r in st] for kk in keys]
    if xs:
        axs[0].stackplot(xs, ys, labels=keys)
        axs[0].legend(fontsize=7, loc="upper left")
    axs[0].set_title("1更新の時間の内訳（秒）")
    axs[0].set_xlabel("更新の回数")
    lines(axs[1], st, [("判断/秒", ("throughput", "decisions_per_s")), ("学習の行/秒", ("throughput", "train_rows_per_s"))], k, "速さ")
    save(fig, "timing", "時間の内訳と速さ")

    fig, axs = plt.subplots(1, 2, figsize=(12, 4))
    lines(axs[0], st, [("GPU 使用率 %", ("gpu", "util_pct_mean")), ("温度 ℃", ("gpu", "temp_c_mean"))], k, "GPU の使用率・温度")
    lines(axs[1], st, [("VRAM（全体）MB", ("gpu", "vram_used_mb_mean")), ("PyTorch の確保 MB", ("gpu", "torch_max_reserved_mb"))], k, "VRAM")
    save(fig, "gpu", "GPU")

    if ev:
        fig, axs = plt.subplots(1, 2, figsize=(12, 4))
        x = [e["update"] for e in ev]
        a_h = [e["results"]["rulebot"]["metrics"]["hora_rate"]["a"] for e in ev]
        b_h = [e["results"]["rulebot"]["metrics"]["hora_rate"]["b"] for e in ev]
        lo = [e["results"]["rulebot"]["metrics"]["hora_rate"]["ci95"][0] for e in ev]
        hi = [e["results"]["rulebot"]["metrics"]["hora_rate"]["ci95"][1] for e in ev]
        axs[0].plot(x, a_h, "o-", label="候補（評価の対局）")
        axs[0].plot(x, b_h, "s--", label="RuleBot（同じ対局）")
        axs[0].set_title("評価：和了率（RuleBot とのデュプリケート）")
        axs[0].set_xlabel("更新の回数")
        axs[0].legend(fontsize=8)
        axs[1].fill_between(x, lo, hi, alpha=0.3)
        axs[1].plot(x, [e["results"]["rulebot"]["metrics"]["hora_rate"]["diff"] for e in ev], "o-")
        axs[1].axhline(0, color="k", lw=1)
        axs[1].set_title("候補 − RuleBot の和了率の差と 95% 区間")
        axs[1].set_xlabel("更新の回数")
        save(fig, "eval", "評価（デュプリケート、席の並び6通り）")
    if rt:
        fig, ax = plt.subplots(figsize=(12, 4))
        last = rt[-1]["ratings"]
        names = sorted(n for n in last if n.startswith("u"))
        ax.plot([int(n[1:]) for n in names], [last[n] for n in names], "o-", label="チェックポイント")
        ax.axhline(last.get("rulebot", 1000.0), color="r", ls="--", label="RuleBot（1000 に固定）")
        ax.set_title("強さの推移（レーティング、最新の評価までの結果で解き直した値。D-054）")
        ax.set_xlabel("更新の回数")
        ax.legend(fontsize=8)
        save(fig, "rating", "レーティング")

    body = "\n".join(
        f'<figure><img src="{n}.png" alt="{html.escape(c)}"><figcaption>{html.escape(c)}</figcaption></figure>' for n, c in figs
    )
    (out / "index.html").write_text(
        f"""<!doctype html><html lang="ja"><meta charset="utf-8"><title>{html.escape(run.name)} の推移</title>
<style>body{{font-family:sans-serif;margin:16px;background:#fff;color:#111}}img{{max-width:100%}}figure{{margin:0 0 24px}}</style>
<h1>{html.escape(run.name)}：学習の推移</h1><p>更新 {st[-1]["update"] if st else 0} まで。元のデータ：stats.jsonl・eval.jsonl・rating.jsonl</p>
{body}</html>""",
        encoding="utf-8",
    )
    print(f"wrote {len(figs)} plots to {out}")


if __name__ == "__main__":
    main()
