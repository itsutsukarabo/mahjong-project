"""強さの物差し（D-054）：デュプリケートの対戦の結果から、すべての打ち手を1つの数値（レーティング）に載せる。

- 対戦の結果は「同じ対局の中の、違う打ち手の2席の組」ごとの勝ち・負け・引き分け（持ち点の大小。同点は引き分け）。
  順位の同点の決め方（席の順）に左右されないように持ち点で比べる
- Bradley–Terry（引き分けは半分の勝ち）を MM 法で解き、Elo と同じ目盛り（400 × log10）にする。RuleBot を 1000 に固定する
- 結果はすべての評価のぶんを足して解き直す（新しい打ち手が増えても同じ物差し）
"""

from __future__ import annotations

import math
from collections import defaultdict

ANCHOR = "rulebot"
ANCHOR_RATING = 1000.0


def fit(results: list[dict], iters: int = 2000, tol: float = 1e-10) -> dict[str, float]:
    """results：{"a", "b", "wins_a", "wins_b", "draws"} の list。打ち手の名前 → レーティング"""
    w: dict[tuple[str, str], float] = defaultdict(float)
    players: set[str] = set()
    for r in results:
        a, b = r["a"], r["b"]
        players |= {a, b}
        w[(a, b)] += r["wins_a"] + 0.5 * r["draws"]
        w[(b, a)] += r["wins_b"] + 0.5 * r["draws"]
    if not players:
        return {}
    ps = sorted(players)
    # 全勝・全敗の打ち手で発散しないように、全員に仮の対戦（RuleBot と 0.5 勝ずつ）を足す
    prior = 0.5
    for p in ps:
        if p != ANCHOR:
            w[(p, ANCHOR)] += prior
            w[(ANCHOR, p)] += prior
    if ANCHOR not in players:
        ps.append(ANCHOR)
    g = {p: 1.0 for p in ps}
    wins = {p: sum(v for (x, _), v in w.items() if x == p) for p in ps}
    games = defaultdict(float)
    for (x, y), v in w.items():
        games[frozenset((x, y))] += v
    for _ in range(iters):
        new = {}
        for p in ps:
            den = 0.0
            for q in ps:
                if q == p:
                    continue
                n = games.get(frozenset((p, q)), 0.0)
                if n:
                    den += n / (g[p] + g[q])
            new[p] = wins[p] / den if den > 0 else g[p]
        s = new[ANCHOR]
        new = {p: v / s for p, v in new.items()}
        diff = max(abs(math.log(new[p]) - math.log(g[p])) for p in ps)
        g = new
        if diff < tol:
            break
    return {p: ANCHOR_RATING + 400.0 * math.log10(g[p]) for p in ps}
