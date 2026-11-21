"""Generate + validate the 1-kyoku sample for docs/spec/LOG_FORMAT.md.

Writes sample_full.jsonl / sample_public.jsonl next to this script and checks:
- tile multiset == sanma 108 set (red 5p/5s x1 each)
- hand sizes after every event, tiles removed actually exist in hand
- event_idx contiguous, public version has identical (game_id, kyoku_idx, event_idx) set
- score sum invariant
"""
import copy
import hashlib
import json
import os
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
GAME_ID = "g-20261122-000001"

SUITED = [f"{n}p" for n in range(1, 10)] + [f"{n}s" for n in range(1, 10)]
HONORS = ["E", "S", "W", "N", "P", "F", "C"]


def full_set():
    tiles = []
    for t in ["1m", "9m"] + SUITED + HONORS:
        for i in range(4):
            tiles.append(t + "r" if t in ("5p", "5s") and i == 0 else t)
    assert len(tiles) == 108
    return Counter(tiles)


haipai = [
    ["9m", "9m", "1p", "2p", "3p", "4s", "5s", "6s", "7s", "8s", "W", "C", "C"],
    ["2p", "3p", "4p", "5p", "6p", "7p", "3s", "3s", "5s", "6s", "E", "E", "N"],
    ["4p", "5pr", "6p", "7p", "8p", "9p", "1s", "2s", "3s", "S", "S", "F", "C"],
]
live_head = ["1m", "8p", "P", "E", "2s", "P", "9s"]  # draws actually used, in order
rinshan_head = ["1m"]
dora_ind = ["1m"]

# fill the rest of the wall deterministically from the remaining multiset
rest = full_set()
rest.subtract(Counter(sum(haipai, []) + live_head + rinshan_head + dora_ind))
assert all(v >= 0 for v in rest.values()), rest
order = ["1m", "9m"] + [x for t in SUITED for x in (t + "r", t)] + HONORS
pool = [t for t in order for _ in range(rest[t])]
assert len(pool) == 108 - 39 - len(live_head) - 2
DEAD = 14  # provisional (Q-001): rinshan 4 + dora 5 + ura 5
rinshan = rinshan_head + pool[:3]
pool = pool[3:]
dora_indicators = dora_ind + pool[:4]
pool = pool[4:]
ura_indicators = pool[:5]
pool = pool[5:]
live = live_head + pool
assert len(live) + len(rinshan) + 10 == 108 - 39 == 69
wall = {"live": live, "rinshan": rinshan,
        "dora_indicators": dora_indicators, "ura_indicators": ura_indicators}
assert Counter(sum(haipai, []) + live + rinshan + dora_indicators + ura_indicators) == full_set()

SALT = "3f9c1a7be2d04c55"
canon = json.dumps({"haipai": haipai, "wall": wall}, separators=(",", ":"), ensure_ascii=False)
wall_commit = "sha256:" + hashlib.sha256((SALT + canon).encode()).hexdigest()

RULES = {
    "ruleset_id": "sanma-draft-0",
    "num_players": 3,
    "_note": "値は Q-001 未決のため仮。フィールド一覧は SANMA_RULES_DRAFT.md を参照",
    "starting_points": 35000, "kita": "nukidora", "tsumo_loss": True,
    "red_fives": {"m": 0, "p": 1, "s": 1}, "chi": False, "length": "hanchan",
}

ev = []


def E(kyoku_idx, typ, **kw):
    d = {"game_id": GAME_ID, "kyoku_idx": kyoku_idx, "event_idx": None, "type": typ}
    d.update(kw)
    ev.append(d)


E(-1, "game_start", format_version="0.1", visibility="full",
  engine={"name": "mahjong-core", "version": "0.0.0", "git": "0000000"},
  rules=RULES, seed={"game_seed": "0x5eed00000001", "rng": "chacha8-v1"},
  players=[{"seat": 0, "name": "ai-ckpt-001200"}, {"seat": 1, "name": "ai-ckpt-000800"},
           {"seat": 2, "name": "rulebot-v1"}])
K = 0
E(K, "kyoku_start", bakaze="E", kyoku=1, honba=0, kyotaku=0, oya=0,
  scores=[35000, 35000, 35000], dora_indicator=dora_indicators[0],
  tehais=haipai, wall=wall, wall_salt=SALT, wall_commit=wall_commit)
E(K, "tsumo", actor=0, pai="1m")
E(K, "dahai", actor=0, pai="W", tsumogiri=False)
E(K, "tsumo", actor=1, pai="8p")
E(K, "kita", actor=1, pai="N")
E(K, "tsumo", actor=1, pai="1m", rinshan=True)
E(K, "dahai", actor=1, pai="1m", tsumogiri=True)
E(K, "tsumo", actor=2, pai="P")
E(K, "dahai", actor=2, pai="C", tsumogiri=False)
E(K, "pon", actor=0, target=2, pai="C", consumed=["C", "C"])
E(K, "dahai", actor=0, pai="1m", tsumogiri=False)
E(K, "tsumo", actor=1, pai="E")
E(K, "reach", actor=1)
E(K, "dahai", actor=1, pai="8p", tsumogiri=False)
E(K, "reach_accepted", actor=1, scores=[35000, 34000, 35000], kyotaku=1)
E(K, "tsumo", actor=2, pai="2s")
E(K, "dahai", actor=2, pai="F", tsumogiri=False)
E(K, "tsumo", actor=0, pai="P")
E(K, "dahai", actor=0, pai="P", tsumogiri=True)
E(K, "tsumo", actor=1, pai="9s")
E(K, "dahai", actor=1, pai="9s", tsumogiri=True)
E(K, "hora", actor=0, target=1, pai="9s",
  hand={"tehai": ["9m", "9m", "1p", "2p", "3p", "4s", "5s", "6s", "7s", "8s"],
        "melds": [{"type": "pon", "pai": "C", "consumed": ["C", "C"], "target": 2}]},
  yaku=[{"id": "yakuhai_chun", "han": 1}, {"id": "dora", "han": 2}],
  han=3, fu=30, yakuman=0, points=5800, ura_indicators=[],
  deltas=[6800, -5800, 0], scores=[41800, 28200, 35000])
E(K, "kyoku_end", reason="hora", scores=[41800, 28200, 35000], kyotaku=0, honba_next=1,
  renchan=True)
E(-1, "game_end", scores=[41800, 28200, 35000], ranks=[1, 3, 2],
  note="サンプルのため1局で打ち切り")

# event_idx: 0.. per kyoku; game-level (kyoku_idx=-1) events numbered 0,1
counters = {}
for d in ev:
    k = d["kyoku_idx"]
    d["event_idx"] = counters.get(k, 0)
    counters[k] = d["event_idx"] + 1

# --- replay validation on the full log ---
hands = [Counter(h) for h in copy.deepcopy(haipai)]
melds = [[] for _ in range(3)]
pos = {"live": 0, "rinshan": 0}
scores = [35000] * 3
for d in ev:
    t = d["type"]
    if t == "tsumo":
        src = "rinshan" if d.get("rinshan") else "live"
        assert wall[src][pos[src]] == d["pai"], (d, wall[src][pos[src]])
        pos[src] += 1
        hands[d["actor"]][d["pai"]] += 1
    elif t in ("dahai", "kita"):
        assert hands[d["actor"]][d["pai"]] > 0, d
        hands[d["actor"]][d["pai"]] -= 1
    elif t == "pon":
        for c in d["consumed"]:
            assert hands[d["actor"]][c] > 0, d
            hands[d["actor"]][c] -= 1
        melds[d["actor"]].append(d)
    elif t == "reach_accepted":
        scores = d["scores"]
    elif t == "hora":
        h = Counter(d["hand"]["tehai"])
        assert +hands[d["actor"]] == h, (hands[d["actor"]], h)
        scores = [s + x for s, x in zip(scores, d["deltas"])]
        assert scores == d["scores"] and sum(scores) == 105000
    if t in ("dahai",):
        n = sum(hands[d["actor"]].values()) + 3 * len(melds[d["actor"]])
        assert n == 13, (d, n)

# --- public (spectator) view: same events, fields masked ---
HIDDEN = "?"


def to_public(d):
    p = copy.deepcopy(d)
    t = p["type"]
    if t == "game_start":
        p["visibility"] = "public"
        p["seed"] = {"rng": p["seed"]["rng"]}  # game_seed withheld
    elif t == "kyoku_start":
        p["tehais"] = [[HIDDEN] * 13 for _ in p["tehais"]]
        del p["wall"], p["wall_salt"]
    elif t == "tsumo":
        p["pai"] = HIDDEN
    return p


pub = [to_public(d) for d in ev]
assert [(d["game_id"], d["kyoku_idx"], d["event_idx"]) for d in pub] == \
       [(d["game_id"], d["kyoku_idx"], d["event_idx"]) for d in ev]

for name, rows in (("sample_full.jsonl", ev), ("sample_public.jsonl", pub)):
    with open(os.path.join(HERE, name), "w", encoding="utf-8") as f:
        for d in rows:
            f.write(json.dumps(d, ensure_ascii=False, separators=(",", ":")) + "\n")
print("events:", len(ev), "kyoku events:", counters[0], "live wall:", len(live),
      "commit:", wall_commit[:23])
