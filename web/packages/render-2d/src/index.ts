// 2D レンダラー（開発者向け）：卓の状態（mj_log::view の TableView）を DOM に描く。描くだけで、
// 和了判定・点数計算・合法手の判定はしない（原則1）。牌の見た目は TileRenderer（tiles.ts）で差し替えられる。
import type { Annotation, Meld, SeatView, TableView } from "@mj/log-types";
import { type TileRenderer, textTiles } from "./tiles";

export { type TileOptions, type TileRenderer, textTiles, tileFace } from "./tiles";
export { css } from "./style";

export interface RenderOptions {
  /** 全員の手牌を表示する（開発者モード）。false なら下の席（viewSeat）以外は伏せる */
  devMode: boolean;
  /** 下に置く席 */
  viewSeat: number;
  tiles?: TileRenderer;
}

const POS_4 = ["bottom", "right", "top", "left"] as const;
const POS_3 = ["bottom", "right", "left"] as const;
const WIND_JA: Record<string, string> = { E: "東", S: "南", W: "西", N: "北" };
const REASON_JA: Record<string, string> = {
  exhaustive: "荒牌平局",
  kyushu_kyuhai: "九種九牌",
  suukaikan: "四槓散了",
  all_riichi: "四家立直",
  same_wind_discards: "四風連打",
  all_ron: "三家和",
  nagashi_mangan: "流し満貫",
};

function el(tag: string, cls: string, text?: string): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function meldEl(m: Meld, seat: number, n: number, tiles: TileRenderer): HTMLElement {
  const box = el("span", `meld meld-${m.type}`);
  box.dataset.meld = m.type;
  if (m.type === "ankan") {
    // 暗槓：両端を伏せる
    m.consumed.forEach((t, i) => box.append(tiles.render(i === 0 || i === 3 ? "?" : t)));
    return box;
  }
  // 鳴いた牌（pai）を、鳴いた相手の向きに横にして置く（上家＝左、対面＝中、下家＝右）
  const own = m.type === "kakan" ? m.consumed.slice(0, 2) : m.consumed.slice();
  const rel = m.target === undefined ? 0 : (m.target - seat + n) % n;
  const pos = rel === n - 1 ? 0 : rel === 1 ? own.length : Math.floor(own.length / 2);
  const parts = own.map((t) => tiles.render(t));
  const called = tiles.render(m.pai ?? "?", { sideways: true });
  if (m.type === "kakan") {
    // 加槓：横にした牌の上に加えた牌を重ねる
    const stack = el("span", "kakan-stack");
    stack.append(called, tiles.render(m.consumed[3] ?? m.consumed[2] ?? "?", { sideways: true }));
    parts.splice(pos, 0, stack);
  } else {
    parts.splice(pos, 0, called);
  }
  box.append(...parts);
  return box;
}

function seatEl(view: TableView, s: SeatView, pos: string, opts: RenderOptions, tiles: TileRenderer): HTMLElement {
  const n = view.seats.length;
  const box = el("section", `seat seat-${pos}`);
  box.dataset.seat = String(s.seat);
  if (view.actor === s.seat) box.classList.add("seat-active");
  const head = el("div", "seat-head");
  head.append(
    el("span", "seat-wind", WIND_JA[s.wind] ?? s.wind),
    el("span", "seat-name", s.name || `席${s.seat}`),
    el("span", "seat-score", s.score.toLocaleString("ja-JP")),
  );
  if (s.riichi) head.append(el("span", "seat-riichi", "立直"));
  box.append(head);

  const show = opts.devMode || s.seat === opts.viewSeat;
  const hand = el("div", "hand");
  hand.dataset.part = "hand";
  for (const t of s.hand) hand.append(tiles.render(show ? t : "?"));
  if (s.drawn !== null) {
    hand.append(tiles.render(show ? s.drawn : "?", { classes: ["tile-drawn"] }));
  }
  const melds = el("div", "melds");
  melds.dataset.part = "melds";
  for (const m of s.melds) melds.append(meldEl(m, s.seat, n, tiles));
  const kita = el("div", "kita");
  kita.dataset.part = "kita";
  for (const t of s.kita) kita.append(tiles.render(t));
  const row = el("div", "hand-row");
  row.append(hand, melds, kita);
  box.append(row);

  const river = el("div", "river");
  river.dataset.part = "river";
  for (const r of s.river) {
    const classes = [r.tsumogiri ? "river-tsumogiri" : "river-tedashi"];
    if (r.called) classes.push("river-called");
    if (r.ron) classes.push("river-ron");
    if (r.riichi) classes.push("river-riichi");
    river.append(tiles.render(r.pai, { sideways: r.riichi, dim: r.called, classes }));
  }
  box.append(river);
  return box;
}

function centerEl(view: TableView, tiles: TileRenderer): HTMLElement {
  const c = el("div", "center");
  c.append(
    el("div", "center-kyoku", `${WIND_JA[view.bakaze] ?? view.bakaze}${view.kyoku}局 ${view.honba}本場`),
    el("div", "center-sub", `供託 ${view.kyotaku}　残り ${view.live_remaining}　${view.junme}巡目`),
  );
  const dora = el("div", "dora");
  dora.dataset.part = "dora";
  dora.append(el("span", "label", "ドラ表示"));
  for (const t of view.dora_indicators) dora.append(tiles.render(t));
  c.append(dora);
  c.append(el("div", "center-event", `#${view.event_idx} ${view.event_type}`));
  return c;
}

function resultEl(view: TableView, tiles: TileRenderer): HTMLElement | null {
  const r = view.result;
  if (!r) return null;
  const box = el("div", "result");
  box.dataset.part = "result";
  for (const h of r.horas) {
    const name = view.seats[h.actor]?.name ?? `席${h.actor}`;
    const line = el("div", "result-hora");
    const how = h.actor === h.target ? "ツモ" : `ロン（席${h.target}から）`;
    line.append(el("div", "result-title", `${name} ${how}`));
    const hand = el("div", "result-hand");
    for (const t of h.hand.tehai) hand.append(tiles.render(t));
    hand.append(tiles.render(h.pai, { classes: ["tile-win"] }));
    const melds = el("span", "melds");
    for (const m of h.hand.melds) melds.append(meldEl(m, h.actor, view.seats.length, tiles));
    hand.append(melds);
    line.append(hand);
    const yaku = h.yaku.map((y) => (y.yakuman ? `${y.id}（役満×${y.yakuman}）` : `${y.id} ${y.han ?? 0}翻`)).join("・");
    line.append(el("div", "result-yaku", yaku));
    const value = h.yakuman > 0 ? `役満×${h.yakuman}` : `${h.han}翻${h.fu}符`;
    const pao = h.pao === undefined ? "" : `　包：席${h.pao}`;
    line.append(el("div", "result-points", `${value}　${h.points}点${pao}`));
    line.append(el("div", "result-deltas", `点数の移動 ${h.deltas.join(" / ")}`));
    box.append(line);
  }
  if (r.ryukyoku) {
    const k = r.ryukyoku;
    box.append(el("div", "result-title", `流局：${REASON_JA[k.reason] ?? k.reason}`));
    const tenpai = k.tenpai.map((t) => (t ? "○" : "×")).join(" ");
    box.append(el("div", "result-deltas", `聴牌 ${tenpai}　点数の移動 ${k.deltas.join(" / ")}`));
  }
  return box;
}

/** 判断ポイントの注釈：選んだ行動・各行動の確率・価値（D-022） */
export function renderAnnotations(container: HTMLElement, list: Annotation[], seatNames: string[]): void {
  const box = el("div", "annotations");
  box.dataset.part = "annotations";
  if (list.length === 0) {
    box.append(el("div", "annot-empty", "この時点の判断ポイントの注釈はない"));
  }
  for (const a of list) {
    const card = el("div", "annot");
    card.dataset.seat = String(a.seat);
    const value = a.value === undefined ? "" : `　価値 ${a.value.toFixed(3)}`;
    const model = a.model ? `（${a.model}）` : "";
    card.append(el("div", "annot-head", `${seatNames[a.seat] ?? `席${a.seat}`}：${a.decision}${value}${model}`));
    const probs = new Map((a.policy ?? []).map(([act, p]) => [act, p] as const));
    const acts = a.choices ?? [...probs.keys()];
    const table = el("div", "annot-policy");
    for (const act of acts) {
      const row = el("div", act === a.decision ? "annot-row annot-chosen" : "annot-row");
      const p = probs.get(act);
      row.append(el("span", "annot-act", act));
      const bar = el("span", "annot-bar");
      const fill = el("span", "annot-fill");
      fill.style.width = `${Math.round((p ?? 0) * 100)}%`;
      bar.append(fill);
      row.append(bar, el("span", "annot-p", p === undefined ? "—" : p.toFixed(3)));
      table.append(row);
    }
    card.append(table);
    box.append(card);
  }
  container.replaceChildren(box);
}

/** 卓を描く（container の中身を置き換える） */
export function render(container: HTMLElement, view: TableView, opts: RenderOptions): void {
  const tiles = opts.tiles ?? textTiles;
  const n = view.seats.length;
  const layout = n === 3 ? POS_3 : POS_4;
  const table = el("div", `table table-${n}p`);
  for (const s of view.seats) {
    const pos = layout[(s.seat - opts.viewSeat + n) % n] ?? "bottom";
    table.append(seatEl(view, s, pos, opts, tiles));
  }
  table.append(centerEl(view, tiles));
  const r = resultEl(view, tiles);
  if (r) table.append(r);
  container.replaceChildren(table);
}
