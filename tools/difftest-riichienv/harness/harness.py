"""RiichiEnv（riichienv 0.4.11）の差分テスト用ハーネス。1行1ケースで読み、1行1ケースで返す。

本体の依存には入れない（D-010）。隔離した venv で動かす（docs/research/DEV_SETUP.md §4）。

op:
  shanten  {"tiles": [mjai...]}                         -> {"shanten", "waits": [mjai 種類] | null}
  hora     {"tiles": [...], "melds": [...], "win", "dora", "ura", "cond": {...}}
                                                        -> {"is_win", "han", "fu", "yaku": [名前], "yakuman",
                                                            "ron", "tsumo_oya", "tsumo_ko"}
  game_new {"seed", "mode"}                             -> 局の状態（牌山 "wall"・配牌 "hands" を含む）
  game_step {"actions": {席: 行動}}                       -> 局の状態
局の状態：{"done", "seats": {席: [合法手の正規形]}, "drawn", "events": [新しい MJAI イベント], "scores"}
合法手の正規形：dahai:<牌>（手出し・ツモ切りは区別しない）, reach, pon:<牌>,<牌>, chi:<牌>,<牌>, daiminkan,
  ankan:<種類>, kakan:<牌>, kita, tsumo, ron, kyushu, pass
"""

import json
import sys

import riichienv as r
from riichienv.hand import Conditions, HandEvaluator3P

AT = r.ActionType
HONORS = ["E", "S", "W", "N", "P", "F", "C"]
RED = {"5mr": 16, "5pr": 52, "5sr": 88}


def kind_of(mjai):
    """MJAI 表記 → 牌の種類（0..33）"""
    if mjai in HONORS:
        return 27 + HONORS.index(mjai)
    n = int(mjai[0])
    s = "mps".index(mjai[1])
    return s * 9 + n - 1


def mjai_of(tid):
    if tid in (16, 52, 88):
        return {16: "5mr", 52: "5pr", 88: "5sr"}[tid]
    k = tid // 4
    if k >= 27:
        return HONORS[k - 27]
    return f"{k % 9 + 1}{'mps'[k // 9]}"


def kind_mjai(k):
    return HONORS[k - 27] if k >= 27 else f"{k % 9 + 1}{'mps'[k // 9]}"


class Ids:
    """MJAI 表記の牌に、重ならない牌 ID（136 形式）を振る。赤は決まった ID、それ以外は種類 × 4 ＋ 1〜3（5 以外は 0 も使う）"""

    def __init__(self):
        self.used = set()

    def get(self, mjai):
        if mjai in RED:
            t = RED[mjai]
            self.used.add(t)
            return t
        k = kind_of(mjai)
        for c in range(4):
            t = k * 4 + c
            if t in (16, 52, 88) or t in self.used:
                continue
            self.used.add(t)
            return t
        raise ValueError(f"no id left for {mjai}")


def shanten(c):
    ids = Ids()
    tiles = [ids.get(t) for t in c["tiles"]]
    sh = r.calculate_shanten_3p(tiles)
    waits = None
    if len(tiles) % 3 == 1:
        waits = [kind_mjai(k) for k in HandEvaluator3P(tiles, []).get_waits()]
    return {"shanten": sh, "waits": waits}


MELD = {
    "pon": r.MeldType.Pon,
    "chi": r.MeldType.Chi,
    "daiminkan": r.MeldType.Daiminkan,
    "ankan": r.MeldType.Ankan,
    "kakan": r.MeldType.Kakan,
}


def hora(c):
    ids = Ids()
    melds = []
    for m in c["melds"]:
        tiles = [ids.get(t) for t in m["tiles"]]
        melds.append(r.Meld(MELD[m["type"]], tiles, m["type"] != "ankan", m.get("from", -1)))
    tiles = [ids.get(t) for t in c["tiles"]]
    win = ids.get(c["win"])
    # 表示牌は手牌と重なってもよい（種類だけが効く）
    dora = [kind_of(t) * 4 + 3 if t not in RED else RED[t] for t in c["dora"]]
    ura = [kind_of(t) * 4 + 3 if t not in RED else RED[t] for t in c["ura"]]
    cond = Conditions(**c["cond"])
    res = HandEvaluator3P(tiles, melds).calc(win, dora_indicators=dora, conditions=cond, ura_indicators=ura)
    if not res.is_win:
        return {"is_win": False}
    return {
        "is_win": True,
        "han": res.han,
        "fu": res.fu,
        "yaku": sorted(y.name_en for y in res.yaku_list()),
        "yakuman": res.yakuman,
        "ron": res.ron_agari,
        "tsumo_oya": res.tsumo_agari_oya,
        "tsumo_ko": res.tsumo_agari_ko,
    }


def canon(a, drawn):
    t = a.action_type
    if t == AT.DISCARD:
        return f"dahai:{mjai_of(a.tile)}"
    if t == AT.RIICHI:
        return "reach"
    if t in (AT.PON, AT.CHI):
        c = ",".join(sorted(mjai_of(x) for x in a.consume_tiles))
        return f"{'pon' if t == AT.PON else 'chi'}:{c}"
    if t == AT.DAIMINKAN:
        return "daiminkan"
    if t == AT.ANKAN:
        return f"ankan:{kind_mjai(a.consume_tiles[0] // 4)}"
    if t == AT.KAKAN:
        return f"kakan:{mjai_of(a.tile)}"
    if t == AT.KITA:
        return "kita"
    if t == AT.TSUMO:
        return "tsumo"
    if t == AT.RON:
        return "ron"
    if t == AT.KYUSHU_KYUHAI:
        return "kyushu"
    if t == AT.PASS:
        return "pass"
    return f"other:{a.to_mjai()}"


GAMES = {}


class G:
    def __init__(self, seed, mode):
        self.env = r.RiichiEnv(game_mode=mode, seed=seed)
        self.obs = self.env.reset()
        self.seen = 0

    def state(self, first=False):
        env = self.env
        drawn = env.drawn_tile
        seats = {}
        for pid, o in self.obs.items():
            la = o.legal_actions()
            if la:
                seats[str(pid)] = sorted({canon(a, drawn) for a in la})
        log = env.mjai_log
        events = [e if isinstance(e, dict) else json.loads(e) for e in log[self.seen:]]
        self.seen = len(log)
        out = {
            "done": env.done(),
            "seats": seats,
            "drawn": None if drawn is None else mjai_of(drawn),
            "events": events,
            "scores": list(env.scores()),
        }
        if first:
            out["wall"] = list(env.wall)
            out["drawn_id"] = drawn
            out["hands"] = [list(h) for h in env.hands]
            out["oya"] = env.oya
        return out

    def pick(self, pid, want):
        o = self.obs[pid]
        la = o.legal_actions()
        drawn = self.env.drawn_tile
        ty = want["type"]
        cands = []
        for a in la:
            t = a.action_type
            if ty == "dahai" and t == AT.DISCARD and mjai_of(a.tile) == want["pai"]:
                cands.append(a)
            elif ty == "reach" and t == AT.RIICHI:
                cands.append(a)
            elif ty in ("pon", "chi") and t == (AT.PON if ty == "pon" else AT.CHI):
                if sorted(mjai_of(x) for x in a.consume_tiles) == sorted(want["consumed"]):
                    cands.append(a)
            elif ty == "daiminkan" and t == AT.DAIMINKAN:
                cands.append(a)
            elif ty == "ankan" and t == AT.ANKAN and kind_mjai(a.consume_tiles[0] // 4) == want["kind"]:
                cands.append(a)
            elif ty == "kakan" and t == AT.KAKAN and mjai_of(a.tile) == want["pai"]:
                cands.append(a)
            elif (ty, t) in (("kita", AT.KITA), ("tsumo", AT.TSUMO), ("ron", AT.RON),
                             ("kyushu", AT.KYUSHU_KYUHAI), ("pass", AT.PASS)):
                cands.append(a)
        if not cands:
            return None
        if ty == "dahai":
            # ツモ切りならツモった牌の ID、手出しならそれ以外の ID を選ぶ
            pref = [a for a in cands if (a.tile == drawn) == bool(want.get("tsumogiri"))]
            if pref:
                return pref[0]
        return cands[0]

    def step(self, actions):
        acts = {}
        for pid, want in actions.items():
            a = self.pick(int(pid), want)
            if a is None:
                return {"error": f"not legal in RiichiEnv: seat {pid} {json.dumps(want, ensure_ascii=False)}"}
            acts[int(pid)] = a
        self.obs = self.env.step(acts)
        return self.state()


def handle(c):
    op = c["op"]
    if op == "shanten":
        return shanten(c)
    if op == "hora":
        return hora(c)
    if op == "game_new":
        g = G(c["seed"], c.get("mode", "3p-red-single"))
        GAMES[c["game"]] = g
        return g.state(first=True)
    if op == "game_step":
        return GAMES[c["game"]].step(c["actions"])
    if op == "game_end":
        GAMES.pop(c["game"], None)
        return {}
    raise ValueError(f"unknown op {op}")


def main():
    for line in sys.stdin:
        if not line.strip():
            continue
        c = json.loads(line)
        try:
            out = handle(c)
        except Exception as e:  # noqa: BLE001 -- 差分テストの側で失敗として数える
            out = {"error": f"{type(e).__name__}: {e}"}
        out["case_id"] = c.get("case_id")
        sys.stdout.write(json.dumps(out, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
