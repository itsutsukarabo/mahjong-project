// アニメーションの手がかり：「前の卓の状態」と「次の卓の状態」の差から、どの牌がどこからどこへ動いたかを作る
// （docs/spec/REPLAY_UI.md §5）。ルールの判断はしない：イベントの種類と、手牌・河・副露の並びの差を見るだけ。
import type { SeatView, TableView } from "@mj/log-types";

/** 画面の上の牌の置き場所。レンダラーは同じ形のキー（placeKey）を DOM の data-key に付ける */
export type Place =
  | { kind: "hand"; seat: number; index: number }
  | { kind: "drawn"; seat: number }
  | { kind: "river"; seat: number; index: number }
  | { kind: "meld"; seat: number; index: number }
  | { kind: "kita"; seat: number; index: number }
  | { kind: "dora"; index: number };

export const placeKey = (p: Place): string => {
  switch (p.kind) {
    case "hand":
      return `h:${p.seat}:${p.index}`;
    case "drawn":
      return `d:${p.seat}`;
    case "river":
      return `r:${p.seat}:${p.index}`;
    case "meld":
      return `m:${p.seat}:${p.index}`;
    case "kita":
      return `k:${p.seat}:${p.index}`;
    case "dora":
      return `dora:${p.index}`;
  }
};

/** 大きな文字で一瞬出すもの（REPLAY_UI §5.2） */
export type ShoutKind = "chi" | "pon" | "kan" | "kita" | "reach" | "ron" | "tsumo";

export interface Move {
  /** 動いた牌の前の場所（前の卓の DOM のキー） */
  from: Place;
  /** 動いた先（次の卓の DOM のキー） */
  to: Place;
  /** 主な動き（打牌・鳴き・北抜き）か、手牌の並べ直しか */
  role: "main" | "shift";
}

export interface Cues {
  moves: Move[];
  /** 新しく現れた牌（ツモ・槓・ドラ表示牌） */
  appear: Place[];
  shout: { seat: number; kind: ShoutKind } | null;
}

const NONE: Cues = { moves: [], appear: [], shout: null };

/** 手牌＋ツモ牌の並び（表示の順） */
const shown = (s: SeatView): string[] => (s.drawn === null ? s.hand : [...s.hand, s.drawn]);
const slotPlace = (s: SeatView, seat: number, i: number): Place =>
  i < s.hand.length ? { kind: "hand", seat, index: i } : { kind: "drawn", seat };

/** 打牌・北抜きで手から出た牌の前の場所 */
function sourceOf(prev: SeatView, seat: number, pai: string, tsumogiri: boolean): Place {
  if (prev.drawn !== null && (tsumogiri || (prev.drawn === pai && !prev.hand.includes(pai)))) {
    return { kind: "drawn", seat };
  }
  let i = prev.hand.indexOf(pai);
  if (i < 0) i = prev.hand.indexOf("?");
  if (i < 0) return prev.drawn !== null ? { kind: "drawn", seat } : { kind: "hand", seat, index: 0 };
  return { kind: "hand", seat, index: i };
}

/**
 * 手牌の並べ直し：次の手牌の各牌を、前の手牌（＋ツモ牌）の同じ牌に左から対応付け、場所が変わったものを動かす。
 * `taken` は前の手牌のうち、打牌などで出ていった場所（対応付けに使わない）
 */
function shifts(prev: SeatView, next: SeatView, seat: number, taken: Place | null): Move[] {
  const before = shown(prev);
  const used = new Array<boolean>(before.length).fill(false);
  if (taken) {
    const ti = taken.kind === "drawn" ? prev.hand.length : taken.kind === "hand" ? taken.index : -1;
    if (ti >= 0 && ti < used.length) used[ti] = true;
  }
  const out: Move[] = [];
  shown(next).forEach((t, j) => {
    const i = before.findIndex((b, k) => !used[k] && b === t);
    if (i < 0) return;
    used[i] = true;
    const from = slotPlace(prev, seat, i);
    const to = slotPlace(next, seat, j);
    if (placeKey(from) !== placeKey(to)) out.push({ from, to, role: "shift" });
  });
  return out;
}

/** 1手進んだときの手がかり（同じ局の、event_idx が1つ進んだときだけ。それ以外は何も動かさない） */
export function cuesBetween(prev: TableView | null, next: TableView): Cues {
  if (!prev || prev.kyoku_idx !== next.kyoku_idx || next.event_idx !== prev.event_idx + 1) return NONE;
  const a = next.actor;
  const moves: Move[] = [];
  const appear: Place[] = [];
  let shout: Cues["shout"] = null;
  if (next.dora_indicators.length > prev.dora_indicators.length) {
    appear.push({ kind: "dora", index: next.dora_indicators.length - 1 });
  }
  if (a === null) return { moves, appear, shout };
  const p = prev.seats[a];
  const n = next.seats[a];
  if (!p || !n) return { moves, appear, shout };
  switch (next.event_type) {
    case "tsumo":
      if (n.drawn !== null) appear.push({ kind: "drawn", seat: a });
      break;
    case "dahai": {
      const r = n.river[n.river.length - 1];
      if (!r) break;
      const from = sourceOf(p, a, r.pai, r.tsumogiri);
      moves.push({ from, to: { kind: "river", seat: a, index: n.river.length - 1 }, role: "main" }, ...shifts(p, n, a, from));
      break;
    }
    case "reach":
      shout = { seat: a, kind: "reach" };
      break;
    case "chi":
    case "pon":
    case "daiminkan": {
      shout = { seat: a, kind: next.event_type === "daiminkan" ? "kan" : next.event_type };
      const m = n.melds.length - 1;
      const target = n.melds[m]?.target;
      const tr = target === undefined ? undefined : prev.seats[target];
      if (target !== undefined && tr && tr.river.length > 0) {
        moves.push({ from: { kind: "river", seat: target, index: tr.river.length - 1 }, to: { kind: "meld", seat: a, index: m }, role: "main" });
      } else {
        appear.push({ kind: "meld", seat: a, index: m });
      }
      moves.push(...shifts(p, n, a, null));
      break;
    }
    case "ankan":
      shout = { seat: a, kind: "kan" };
      appear.push({ kind: "meld", seat: a, index: n.melds.length - 1 });
      moves.push(...shifts(p, n, a, null));
      break;
    case "kakan": {
      shout = { seat: a, kind: "kan" };
      const i = n.melds.findIndex((m, k) => m.type === "kakan" && p.melds[k]?.type !== "kakan");
      appear.push({ kind: "meld", seat: a, index: i >= 0 ? i : n.melds.length - 1 });
      moves.push(...shifts(p, n, a, null));
      break;
    }
    case "kita": {
      shout = { seat: a, kind: "kita" };
      const pai = n.kita[n.kita.length - 1] ?? "N";
      const from = sourceOf(p, a, pai, false);
      moves.push({ from, to: { kind: "kita", seat: a, index: n.kita.length - 1 }, role: "main" }, ...shifts(p, n, a, from));
      break;
    }
    case "hora": {
      const h = next.result?.horas[next.result.horas.length - 1];
      shout = { seat: a, kind: h && h.actor !== h.target ? "ron" : "tsumo" };
      break;
    }
  }
  return { moves, appear, shout };
}
