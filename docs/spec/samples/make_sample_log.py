"""LOG_FORMAT v0.2 の1局分サンプルを生成し、検証する。

使い方:
    python -I docs/spec/samples/make_sample_log.py          # 生成して書き出す
    python -I docs/spec/samples/make_sample_log.py --check  # 生成結果がコミット済みのファイルと一致するか確かめる（CI）

検証内容:
- 配牌＋牌山（pile・表示牌）＝ルールの牌の多重集合（三麻 108 枚、赤 5p/5s 各1）
- 通常のツモが pile の先頭から、嶺上ツモが pile の末尾から順に出ている
- 打牌・北抜き・副露の牌が手牌にあり、打牌後の枚数（門前＋副露×3）が 13
- wall_commit の再計算、点数の合計が不変
- 公開情報版と全情報版で (game_id, kyoku_idx, event_idx) の集合と並びが同じ
- 注釈のキー (game_id, kyoku_idx, event_idx, seat) が一意で、存在するイベントを指す
"""
import copy
import hashlib
import json
import os
import sys
import tomllib
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
GAME_ID = "g-20261122-000001"
FORMAT_VERSION = "0.2"
HIDDEN = "?"

KINDS = ([f"{n}m" for n in range(1, 10)] + [f"{n}p" for n in range(1, 10)]
         + [f"{n}s" for n in range(1, 10)] + ["E", "S", "W", "N", "P", "F", "C"])


def sort_key(t):
    """mj-core の Tile の順序：種類の昇順、同じ種類では赤が先。"""
    red = t.endswith("r")
    return (KINDS.index(t[:-1] if red else t), 0 if red else 1)


def load_rules():
    def read(rel):
        with open(os.path.join(ROOT, rel), "rb") as f:
            return tomllib.load(f)

    def merge(base, over):
        for k, v in over.items():
            if isinstance(v, dict) and isinstance(base.get(k), dict):
                merge(base[k], v)
            else:
                base[k] = v

    rules = read("rules/tenhou-sanma.toml")
    del rules["meta"]
    merge(rules, read("fixtures/rules/tenhou-sanma.test-overrides.toml"))
    return rules


def tile_multiset(rules):
    """mj-core::wall::tile_multiset と同じ規則。"""
    out = []
    reds = rules["tiles"]["red_fives"]
    for k in KINDS:
        if rules["tiles"]["manzu"] == "terminals_only" and k[1:] == "m" and k[0] not in "19":
            continue
        red = reds.get(k[1], 0) if len(k) == 2 and k[0] == "5" else 0
        out += [k + "r" if i < red else k for i in range(4)]
    return Counter(out)


RULES = load_rules()

haipai = [
    ["9m", "9m", "1p", "2p", "3p", "4s", "5s", "6s", "7s", "8s", "W", "C", "C"],
    ["2p", "3p", "4p", "5p", "6p", "7p", "3s", "3s", "5s", "6s", "E", "E", "N"],
    ["4p", "5pr", "6p", "7p", "8p", "9p", "1s", "2s", "3s", "S", "S", "P", "C"],
]
haipai = [sorted(h, key=sort_key) for h in haipai]
draws_used = ["1m", "8p", "P", "E", "2s", "P", "9s"]  # 通常のツモ（pile の先頭から）
rinshan_used = ["1m"]                                 # 嶺上ツモ（pile の末尾から）
dora_first = "1m"

# 残りの牌を正規順に並べて牌山を埋める（サンプルなのでシャッフルしない）
rest = tile_multiset(RULES)
rest.subtract(Counter(sum(haipai, []) + draws_used + rinshan_used + [dora_first]))
assert all(v >= 0 for v in rest.values()), rest
pool = sorted(rest.elements(), key=sort_key)
slots = RULES["wall"]["dora_indicator_slots"]
dora_indicators = [dora_first] + pool[: slots - 1]
pool = pool[slots - 1:]
ura_indicators = pool[:slots]
pool = pool[slots:]
pile = draws_used + pool + list(reversed(rinshan_used))
wall = {"pile": pile, "dora_indicators": dora_indicators, "ura_indicators": ura_indicators}
assert Counter(sum(haipai, []) + pile + dora_indicators + ura_indicators) == tile_multiset(RULES)

# 実際の対局では OS の CSPRNG で作る（LOG_FORMAT §4.6）。サンプルは固定値
SALT = "3f9c1a7be2d04c5590b1e6a2d7c3f018"


def canonical(obj):
    return json.dumps(obj, separators=(",", ":"), ensure_ascii=False)


def wall_commit(tehais, wall, salt):
    return "sha256:" + hashlib.sha256((salt + canonical({"haipai": tehais, "wall": wall})).encode()).hexdigest()


ev = []


def E(kyoku_idx, typ, **kw):
    d = {"game_id": GAME_ID, "kyoku_idx": kyoku_idx, "event_idx": None, "type": typ}
    d.update(kw)
    ev.append(d)


E(-1, "game_start", format_version=FORMAT_VERSION, visibility="full",
  engine={"name": "mj-core", "version": "0.1.0", "git": "0000000"},
  rules=RULES, seed={"game_seed": None, "rng": "manual"},
  players=[{"seat": 0, "name": "ai-ckpt-001200"}, {"seat": 1, "name": "ai-ckpt-000800"},
           {"seat": 2, "name": "rulebot-v1"}])
K = 0
E(K, "kyoku_start", bakaze="E", kyoku=1, honba=0, kyotaku=0, oya=0,
  scores=[35000, 35000, 35000], dora_indicator=dora_indicators[0],
  tehais=haipai, wall=wall, wall_salt=SALT, wall_commit=wall_commit(haipai, wall, SALT))
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
E(K, "dahai", actor=2, pai="2s", tsumogiri=True)
E(K, "tsumo", actor=0, pai="P")
E(K, "dahai", actor=0, pai="P", tsumogiri=True)   # 席2は白を2枚持つがポンを見送る（注釈）
E(K, "tsumo", actor=1, pai="9s")
E(K, "dahai", actor=1, pai="9s", tsumogiri=True)
E(K, "hora", actor=0, target=1, pai="9s",
  hand={"tehai": ["9m", "9m", "1p", "2p", "3p", "4s", "5s", "6s", "7s", "8s"],
        "melds": [{"type": "pon", "pai": "C", "consumed": ["C", "C"], "target": 2}]},
  yaku=[{"id": "yakuhai_chun", "han": 1}, {"id": "dora", "han": 2}],
  han=3, fu=30, yakuman=0, points=5800, ura_indicators=[],
  deltas=[6800, -5800, 0], scores=[41800, 28200, 35000])
E(K, "kyoku_end", reason="hora", scores=[41800, 28200, 35000], kyotaku=0, honba_next=1,
  renchan=True, chips={"deltas": [0, 0, 0], "items": []})
E(-1, "game_end", scores=[41800, 28200, 35000], ranks=[1, 3, 2], chips_total=[0, 0, 0],
  note="サンプルのため1局で打ち切り")

counters = {}
for d in ev:
    k = d["kyoku_idx"]
    d["event_idx"] = counters.get(k, 0)
    counters[k] = d["event_idx"] + 1


def by_idx(kyoku_idx, event_idx):
    return next(d for d in ev if d["kyoku_idx"] == kyoku_idx and d["event_idx"] == event_idx)


assert by_idx(0, 8)["type"] == "dahai" and by_idx(0, 18)["pai"] == "P"
annotations = [
    {"game_id": GAME_ID, "kyoku_idx": 0, "event_idx": 8, "seat": 0, "decision": "pon",
     "model": "ai-ckpt-001200", "policy": [["pon", 0.83], ["pass", 0.17]], "value": 0.21},
    {"game_id": GAME_ID, "kyoku_idx": 0, "event_idx": 18, "seat": 2, "decision": "pass",
     "model": "rulebot-v1"},
    {"game_id": GAME_ID, "kyoku_idx": 0, "event_idx": 20, "seat": 0, "decision": "hora",
     "model": "ai-ckpt-001200", "policy": [["hora", 0.99], ["pass", 0.01]], "value": 0.64},
]


# --- 全情報版を手で再生して検証 ---
def validate(ev):
    hands = [Counter(h) for h in copy.deepcopy(haipai)]
    melds = [0] * 3
    front, back = 0, len(pile) - 1
    scores = [35000] * 3
    ks = by_idx(0, 0)
    assert ks["wall_commit"] == wall_commit(ks["tehais"], ks["wall"], ks["wall_salt"])
    for d in ev:
        t, a = d["type"], d.get("actor")
        if t == "tsumo":
            if d.get("rinshan"):
                assert pile[back] == d["pai"], d
                back -= 1
            else:
                assert pile[front] == d["pai"], d
                front += 1
            hands[a][d["pai"]] += 1
        elif t in ("dahai", "kita"):
            assert hands[a][d["pai"]] > 0, d
            hands[a][d["pai"]] -= 1
            if t == "dahai":
                assert sum(hands[a].values()) + 3 * melds[a] == 13, d
        elif t == "pon":
            for c in d["consumed"]:
                assert hands[a][c] > 0, d
                hands[a][c] -= 1
            melds[a] += 1
        elif t == "reach_accepted":
            scores = d["scores"]
        elif t == "hora":
            # hand.tehai は和了牌を含まない（LOG_FORMAT §4.1）
            assert +hands[a] == Counter(d["hand"]["tehai"]), (hands[a], d["hand"]["tehai"])
            scores = [s + x for s, x in zip(scores, d["deltas"])]
            assert scores == d["scores"] and sum(scores) == 3 * RULES["points"]["starting"]
        elif t == "kyoku_end":
            assert sum(d["chips"]["deltas"]) == 0


def redact_public(ev):
    """mj-log::redact(Viewer::Public) と同じ規則（LOG_FORMAT §5）。"""
    out = []
    for d in ev:
        p = copy.deepcopy(d)
        t = p["type"]
        if t == "game_start":
            p["visibility"] = "public"
            del p["seed"]["game_seed"]
        elif t == "kyoku_start":
            p["tehais"] = [[HIDDEN] * len(h) for h in p["tehais"]]
            del p["wall"], p["wall_salt"]
        elif t == "tsumo":
            p["pai"] = HIDDEN
        elif t == "ankan" and RULES["log_visibility"]["ankan"] == "closed":
            p["consumed"] = [HIDDEN] * 4
        elif t == "ryukyoku":
            reveal = RULES["log_visibility"]["ryukyoku_reveal"]
            p["hands"] = [h if (reveal == "all" or (reveal == "tenpai_only" and tp)) else None
                          for h, tp in zip(p["hands"], p["tenpai"])]
        out.append(p)
    return out


def validate_annotations(annotations, ev):
    keys = [(a["game_id"], a["kyoku_idx"], a["event_idx"], a["seat"]) for a in annotations]
    assert len(set(keys)) == len(keys), "duplicate annotation key"
    ids = {(d["game_id"], d["kyoku_idx"], d["event_idx"]) for d in ev}
    for g, k, e, s in keys:
        assert (g, k, e) in ids, (g, k, e)
        assert 0 <= s < RULES["table"]["num_players"]


validate(ev)
pub = redact_public(ev)
assert [(d["game_id"], d["kyoku_idx"], d["event_idx"]) for d in pub] == \
       [(d["game_id"], d["kyoku_idx"], d["event_idx"]) for d in ev]
validate_annotations(annotations, ev)


def render(rows):
    return "".join(canonical(d) + "\n" for d in rows)


outputs = {
    "sample_full.jsonl": render(ev),
    "sample_public.jsonl": render(pub),
    "sample_annot.jsonl": render(annotations),
}

if "--check" in sys.argv[1:]:
    bad = []
    for name, text in outputs.items():
        with open(os.path.join(HERE, name), encoding="utf-8", newline="") as f:
            if f.read() != text:
                bad.append(name)
    if bad:
        print("sample files are out of date:", ", ".join(bad), "- run make_sample_log.py")
        sys.exit(1)
    print("samples OK:", {n: t.count("\n") for n, t in outputs.items()})
else:
    for name, text in outputs.items():
        with open(os.path.join(HERE, name), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    print("written:", {n: t.count("\n") for n, t in outputs.items()},
          "pile:", len(pile), "commit:", by_idx(0, 0)["wall_commit"][:23])
