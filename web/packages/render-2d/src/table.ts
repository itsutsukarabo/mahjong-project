// 卓を描く（DOM）。卓の状態（WASM の TableView）と画面の寸法（layout.ts）から要素を作るだけで、
// 和了判定・点数計算・合法手の判定はしない（原則1）。配置は docs/spec/REPLAY_UI.md §3。
import type { ReplayMeta, SeatView, TableView } from "@mj/log-types";
import { type AnnotationLayout, type ChoiceView, type SelectionMode, bandSlots, placeKey } from "@mj/table-model";
import { WIND_JA, actionLabel, pct } from "./labels";
import { DIR, type Metrics, ROT, relPos, tileH } from "./layout";
import { el, meldEl } from "./dom";
import { gameEndEl, resultEl } from "./result";
import { type TileRenderer, svgTiles } from "./tiles";

export interface AiOptions {
  /** 表示するか（配信で切るため） */
  show: boolean;
  /** 視点の席のこの時点の注釈（なければ null） */
  annotation: AnnotationLayout | null;
  /** 牌譜の選び方（抽選・最大確率） */
  mode: SelectionMode | null;
  /** この時点で注釈のある、視点以外の席 */
  otherSeats: number[];
  /** 注釈ファイルが読み込まれているか */
  loaded: boolean;
}

export interface RenderOptions {
  /** 全員の手牌を表示する（開発者モード）。false なら下の席以外は伏せる */
  devMode: boolean;
  /** 下に置く席（視点の席） */
  viewSeat: number;
  metrics: Metrics;
  meta: ReplayMeta;
  ai: AiOptions;
  /** 局の結果の窓を出すか */
  showResult: boolean;
  /** 対局の終わりの窓を出すか */
  showGameEnd: boolean;
  tiles?: TileRenderer;
  onCloseResult?: () => void;
  onSwitchSeat?: (seat: number) => void;
}

/** 要素の大きさと位置を、盤の座標の中心 (x, y) と回転で決める（回転は要素の中心のまわり） */
function place(e: HTMLElement, x: number, y: number, w: number, h: number, rot = 0): HTMLElement {
  e.style.left = `${Math.round(x - w / 2)}px`;
  e.style.top = `${Math.round(y - h / 2)}px`;
  e.style.width = `${w}px`;
  e.style.height = `${h}px`;
  if (rot) {
    e.style.transform = `rotate(${rot}deg)`;
    e.dataset.rot = String(rot);
  }
  return e;
}

/** 牌の大きさを CSS の変数にする（その中の .tile がこの大きさになる） */
function sized(e: HTMLElement, w: number): HTMLElement {
  e.style.setProperty("--tw", `${w}px`);
  e.style.setProperty("--th", `${tileH(w)}px`);
  return e;
}

/** 抜いた北：1枚だけ置き、枚数を添える（重ねると字の欠けた牌に見えるため）。アニメーションの行き先は最後の1枚のキー */
function kitaEl(s: SeatView, tiles: TileRenderer): HTMLElement | null {
  if (s.kita.length === 0) return null;
  const box = el("span", "kita");
  box.dataset.part = "kita";
  box.dataset.count = String(s.kita.length);
  const last = s.kita.length - 1;
  box.append(tiles.render(s.kita[last]!, { key: placeKey({ kind: "kita", seat: s.seat, index: last }) }));
  box.append(el("span", "kita-count", `×${s.kita.length}`));
  return box;
}

/** 他家の手牌の帯（手牌・ツモ牌・副露・抜いた北） */
function handGroup(s: SeatView, n: number, show: boolean, tiles: TileRenderer): HTMLElement {
  const row = el("div", "hand-group");
  const hand = el("span", "hand");
  hand.dataset.part = "hand";
  s.hand.forEach((t, i) => hand.append(tiles.render(show ? t : "?", { key: placeKey({ kind: "hand", seat: s.seat, index: i }) })));
  if (s.drawn !== null) {
    hand.append(tiles.render(show ? s.drawn : "?", { classes: ["tile-drawn"], key: placeKey({ kind: "drawn", seat: s.seat }) }));
  }
  row.append(hand);
  const melds = el("span", "melds");
  melds.dataset.part = "melds";
  s.melds.forEach((m, i) => melds.append(meldEl(m, s.seat, n, tiles, placeKey({ kind: "meld", seat: s.seat, index: i }))));
  row.append(melds);
  const k = kitaEl(s, tiles);
  if (k) row.append(k);
  return row;
}

function riverEl(s: SeatView, tiles: TileRenderer, perRow = 6): HTMLElement {
  const river = el("div", "river");
  river.dataset.part = "river";
  river.dataset.seat = String(s.seat);
  let row = el("div", "river-row");
  s.river.forEach((r, i) => {
    if (i > 0 && i % perRow === 0) {
      river.append(row);
      row = el("div", "river-row");
    }
    const classes = [r.tsumogiri ? "river-tsumogiri" : "river-tedashi"];
    if (r.called) classes.push("river-called");
    if (r.ron) classes.push("river-ron");
    if (r.riichi) classes.push("river-riichi");
    row.append(tiles.render(r.pai, { sideways: r.riichi, dim: r.called, classes, key: placeKey({ kind: "river", seat: s.seat, index: i }) }));
  });
  river.append(row);
  return river;
}

/** 名前などのプレイヤー情報（控えめに。段位は牌譜にまだないので枠だけ） */
function plateEl(view: TableView, s: SeatView, turn: number | null, arrow = ""): HTMLElement {
  const p = el("div", "plate");
  p.dataset.seat = String(s.seat);
  if (turn === s.seat) p.classList.add("plate-turn");
  const wind = el("span", s.seat === view.oya ? "plate-wind plate-oya" : "plate-wind", WIND_JA[s.wind] ?? s.wind);
  const l1 = el("span", "plate-line");
  l1.append(...(arrow ? [el("span", "plate-arrow", arrow)] : []), wind, el("span", "plate-name", s.name || `席${s.seat}`));
  const l2 = el("span", "plate-line");
  l2.append(el("span", "plate-rank", "段位 —"));
  if (s.riichi) l2.append(el("span", "plate-riichi", "立直"));
  p.append(l1, l2);
  return p;
}

/** 手番の席：この時点で行動した席、なければツモ牌を持つ席 */
function turnSeat(view: TableView): number | null {
  if (view.actor !== null) return view.actor;
  return view.seats.find((s) => s.drawn !== null)?.seat ?? null;
}

function centerEl(view: TableView, m: Metrics, viewSeat: number, turn: number | null): HTMLElement {
  const c = place(el("div", "center"), m.cx, m.cy, m.centerW, m.centerH);
  c.dataset.part = "center";
  c.style.setProperty("--c", `${Math.min(m.centerW, m.centerH)}px`);
  const mid = el("div", "center-mid");
  mid.append(
    el("div", "center-kyoku", `${WIND_JA[view.bakaze] ?? view.bakaze}${view.kyoku}局`),
    el("div", "center-remain", `残り ${view.live_remaining}`),
  );
  const sticks = el("div", "center-sticks");
  sticks.append(el("span", "stick stick-honba"), el("span", "", `${view.honba}`), el("span", "stick stick-riichi"), el("span", "", `${view.kyotaku}`));
  sticks.title = `${view.honba}本場・供託 ${view.kyotaku}`;
  mid.append(sticks);
  c.append(mid);
  const eh = Math.max(16, Math.round(Math.min(m.centerW, m.centerH) * 0.17));
  for (const s of view.seats) {
    const p = relPos(s.seat, viewSeat, view.seats.length);
    const [dx, dy] = DIR[p];
    // 上下の辺は幅 centerW、左右の辺は高さ centerH。角で隣の辺と重ならない長さにする
    const side = dx === 0 ? m.centerW : m.centerH;
    const ew = Math.round(Math.min(side * 0.8, side - 2 * eh - 4));
    const d = (dx === 0 ? m.centerH : m.centerW) / 2 - eh / 2 - 1;
    const edge = place(el("div", "edge"), m.centerW / 2 + dx * d, m.centerH / 2 + dy * d, ew, eh, ROT[p]);
    edge.dataset.seat = String(s.seat);
    edge.style.fontSize = `${Math.round(eh * 0.62)}px`;
    if (turn === s.seat) edge.classList.add("edge-turn");
    const wind = el("span", s.seat === view.oya ? "edge-wind edge-oya" : "edge-wind", WIND_JA[s.wind] ?? s.wind);
    if (s.seat === view.oya) wind.title = "親";
    edge.append(wind, el("span", "edge-score", s.score.toLocaleString("ja-JP")));
    if (s.riichi) edge.append(el("span", "edge-riichi"));
    c.append(edge);
  }
  return c;
}

function doraEl(view: TableView, w: number, tiles: TileRenderer): HTMLElement {
  const d = sized(el("div", "dora"), w);
  d.dataset.part = "dora";
  d.append(el("span", "dora-label", "ドラ表示"));
  const row = el("span", "dora-tiles");
  view.dora_indicators.forEach((t, i) => row.append(tiles.render(t, { key: placeKey({ kind: "dora", index: i }) })));
  d.append(row);
  return d;
}

/** 盤（河・中央パネル・横画面なら他家の手牌の帯と名前） */
function boardEl(view: TableView, opts: RenderOptions, tiles: TileRenderer, turn: number | null): HTMLElement {
  const m = opts.metrics;
  const n = view.seats.length;
  const board = el("div", `board board-${n}p`);
  board.dataset.part = "board";
  board.style.width = `${m.boardW}px`;
  board.style.height = `${m.boardH}px`;
  board.append(centerEl(view, m, opts.viewSeat, turn));
  const cornerSize = m.gap + m.riverH;
  // 十字の四隅（名前を置く）：x は左右の腕の外側、y は上の腕（三麻は詰めた長さ）と下の腕
  const cornerX = (qx: number) => m.cx + qx * (m.cross / 2 - cornerSize / 2);
  const cornerY = (qy: number) => (qy < 0 ? m.cy - m.center / 2 - m.topArm / 2 : m.cy + m.center / 2 + cornerSize / 2);
  const cornerH = (qy: number) => (qy < 0 ? m.topArm : cornerSize) - 8;
  for (const s of view.seats) {
    const p = relPos(s.seat, opts.viewSeat, n);
    const [dx, dy] = DIR[p];
    const vertical = dx === 0;
    const rd = vertical ? m.centerH / 2 + m.gap + m.riverH / 2 : m.centerW / 2 + m.gap + m.sideDepth / 2;
    const [rw, rh] = vertical ? [m.riverW, m.riverH] : [m.centerH, m.sideDepth];
    const river = sized(place(el("div", "river-box"), m.cx + dx * rd, m.cy + dy * rd, rw, rh, ROT[p]), m.rw);
    river.append(riverEl(s, tiles, vertical ? 6 : m.sidePer));
    board.append(river);
    const anchor = place(el("div", "shout-anchor"), m.cx + dx * rd, m.cy + dy * rd, 0, 0);
    anchor.dataset.shout = String(s.seat);
    board.append(anchor);
    if (m.layout === "landscape") {
      if (p !== 0) {
        const sx = m.cross / 2 + m.stripGap + m.od / 2;
        const sy = m.center / 2 + m.topArm + m.stripGap + m.od / 2;
        const [x, y, len] = p === 2 ? [m.cx, m.cy - sy, m.cross] : [m.cx + dx * sx, m.stripCy, m.stripLen];
        const strip = sized(place(el("div", "strip"), x, y, len, m.od, ROT[p]), m.ow);
        strip.dataset.seat = String(s.seat);
        strip.classList.add("seat");
        strip.append(handGroup(s, n, opts.devMode || s.seat === opts.viewSeat, tiles));
        board.append(strip);
      }
      // 名前：その席の手牌の始まる側の角（下＝左下、右＝右下、上＝右上、左＝左上）
      const corner: Record<number, [number, number]> = { 0: [-1, 1], 1: [1, 1], 2: [1, -1], 3: [-1, -1] };
      const [qx, qy] = corner[p] ?? [0, 0];
      const plate = place(plateEl(view, s, turn), cornerX(qx), cornerY(qy), cornerSize - 8, cornerH(qy));
      plate.classList.add(`plate-corner-${qy < 0 ? "t" : "b"}`);
      board.append(plate);
    }
  }
  if (m.layout === "portrait") {
    // ドラ表示牌：盤の左上（四麻は左上の隅、三麻は詰めた上の腕）
    const d = doraEl(view, m.doraW, tiles);
    d.style.left = "2px";
    d.style.top = "2px";
    board.append(d);
  }
  return board;
}

/** 縦画面：他家の段（上家・対面・下家の順。横向きにしない） */
function oppRowsEl(view: TableView, opts: RenderOptions, tiles: TileRenderer, turn: number | null): HTMLElement {
  const m = opts.metrics;
  const n = view.seats.length;
  const box = el("div", "opp-rows");
  const order = view.seats
    .filter((s) => s.seat !== opts.viewSeat)
    .map((s) => ({ s, p: relPos(s.seat, opts.viewSeat, n) }))
    .sort((a, b) => [3, 2, 1].indexOf(a.p) - [3, 2, 1].indexOf(b.p));
  const ARROW: Record<number, string> = { 1: "下家 →", 2: "対面 ↑", 3: "← 上家" };
  for (const { s, p } of order) {
    const row = sized(el("div", "opp-row seat"), m.ow);
    row.dataset.seat = String(s.seat);
    row.style.height = `${m.oppRowH}px`;
    if (turn === s.seat) row.classList.add("opp-turn");
    row.append(plateEl(view, s, turn, ARROW[p]), handGroup(s, n, opts.devMode, tiles));
    box.append(row);
  }
  return box;
}

function bandCell(c: ChoiceView | undefined, show: boolean): HTMLElement {
  const cell = el("div", "band-cell");
  if (!c || !show) return cell;
  cell.dataset.action = c.action;
  if (c.chosen) cell.classList.add("band-chosen");
  if (c.best) cell.classList.add("band-best");
  const bar = el("div", "band-bar");
  bar.style.height = `${Math.round((c.p ?? 0) * 100)}%`;
  cell.append(bar, el("div", "band-p", pct(c.p)));
  if (c.best) cell.append(el("div", "band-star", "★"));
  return cell;
}

function chipEl(c: ChoiceView, tiles: TileRenderer): HTMLElement {
  const chip = el("span", "chip");
  chip.dataset.action = c.action;
  if (c.chosen) chip.classList.add("chip-chosen");
  if (c.best) chip.classList.add("chip-best");
  const l = actionLabel(c.action);
  chip.append(el("span", "chip-verb", l.verb + (l.extra ? ` ${l.extra}` : "")));
  if (l.tiles.length > 0) {
    const t = el("span", "chip-tiles");
    for (const x of l.tiles) t.append(tiles.render(x));
    chip.append(t);
  }
  if (l.tsumogiri) chip.append(el("span", "chip-flag", "ツモ切り"));
  chip.append(el("span", "chip-p", c.p === null ? "—" : `${pct(c.p)}%`));
  if (c.best) chip.append(el("span", "chip-star", "★"));
  return chip;
}

/** 帯の下の一覧：区画に結び付かない選択肢・価値・選び方・他の席の判断 */
function aiListEl(opts: RenderOptions, tiles: TileRenderer): HTMLElement {
  const ai = opts.ai;
  const box = el("div", "ai-list");
  box.dataset.part = "ai-list";
  box.style.height = `${opts.metrics.listH}px`;
  if (!ai.show || !ai.loaded) {
    if (!ai.loaded) box.append(el("span", "ai-none", "注釈なし"));
    else box.append(el("span", "ai-none", "AI の判断の表示：切"));
    return box;
  }
  if (ai.mode) {
    const badge = el("span", `mode mode-${ai.mode.kind}`, ai.mode.kind === "sample" ? "抽選" : ai.mode.kind === "argmax" ? "最大確率" : "選び方不明");
    badge.title = ai.mode.detail;
    box.append(badge);
  }
  const a = ai.annotation;
  if (a) {
    if (a.value !== null) {
      const v = el("span", "value");
      v.title = "価値の推定";
      const meter = el("span", "value-meter");
      const fill = el("span", a.value >= 0 ? "value-fill value-pos" : "value-fill value-neg");
      const w = Math.min(1, Math.abs(a.value)) * 50;
      fill.style.width = `${w}%`;
      fill.style.left = a.value >= 0 ? "50%" : `${50 - w}%`;
      meter.append(fill);
      v.append(el("span", "value-label", "価値"), meter, el("span", "value-num", (a.value >= 0 ? "+" : "") + a.value.toFixed(3)));
      box.append(v);
    }
    if (a.differsFromBest) box.append(el("span", "differs", "★最大確率と違う行動を選んだ"));
    for (const c of a.others) box.append(chipEl(c, tiles));
  } else {
    box.append(el("span", "ai-none", "この時点の判断なし"));
  }
  for (const seat of ai.otherSeats) {
    const b = el("button", "other-seat", `席${seat}の判断 →`) as HTMLButtonElement;
    b.type = "button";
    b.dataset.seat = String(seat);
    b.title = "この席を視点にして判断を見る";
    b.onclick = () => opts.onSwitchSeat?.(seat);
    box.append(b);
  }
  return box;
}

/** 自分（視点の席）の手牌・帯・副露 */
function selfEl(view: TableView, opts: RenderOptions, tiles: TileRenderer, turn: number | null): HTMLElement {
  const m = opts.metrics;
  const s = view.seats[opts.viewSeat];
  const box = el("div", "self seat");
  box.dataset.part = "self";
  if (!s) return box;
  box.dataset.seat = String(s.seat);
  const n = view.seats.length;
  const meldBox = sized(el("div", "self-melds"), m.mw);
  meldBox.dataset.part = "melds";
  s.melds.forEach((x, i) => meldBox.append(meldEl(x, s.seat, n, tiles, placeKey({ kind: "meld", seat: s.seat, index: i }))));
  const k = kitaEl(s, tiles);
  if (k) meldBox.append(k);
  if (m.layout === "portrait") {
    const top = el("div", "self-top");
    top.style.height = `${2 * m.mw + 4}px`;
    top.append(plateEl(view, s, turn), meldBox);
    box.append(top);
  }
  const row = el("div", "self-row");
  row.style.setProperty("--tw", `${m.hw}px`);
  row.style.setProperty("--th", `${m.hh}px`);
  row.style.setProperty("--band", `${m.bandH}px`);
  const hand = el("div", "band");
  hand.dataset.part = "hand";
  const ann = opts.ai.show ? opts.ai.annotation : null;
  for (const slot of bandSlots(s)) {
    if (slot.drawn) hand.append(el("div", "slot-gap"));
    const col = el("div", slot.drawn ? "slot slot-drawn" : "slot");
    col.dataset.slot = String(slot.index);
    col.dataset.action = slot.action;
    const key = slot.drawn ? placeKey({ kind: "drawn", seat: s.seat }) : placeKey({ kind: "hand", seat: s.seat, index: slot.index });
    const t = tiles.render(slot.tile, { key, classes: slot.drawn ? ["tile-drawn"] : [] });
    const cell = bandCell(ann?.bySlot.get(slot.index), opts.ai.show);
    cell.dataset.slot = String(slot.index);
    col.append(t, cell);
    hand.append(col);
  }
  row.append(hand);
  if (m.layout === "landscape") row.append(meldBox);
  box.append(row, aiListEl(opts, tiles));
  return box;
}

/** 卓を描く（container の中身を置き換える） */
export function render(container: HTMLElement, view: TableView, opts: RenderOptions): void {
  const tiles = opts.tiles ?? svgTiles;
  const m = opts.metrics;
  const turn = turnSeat(view);
  const stage = el("div", `stage stage-${m.layout}`);
  stage.dataset.layout = m.layout;
  stage.style.width = `${m.width}px`;
  stage.style.height = `${m.height}px`;
  stage.style.setProperty("--shout", `${Math.round(m.rw * 1.5)}px`);
  if (m.layout === "landscape") {
    const d = doraEl(view, m.rw, tiles);
    d.classList.add("dora-corner");
    stage.append(d);
    stage.append(boardEl(view, opts, tiles, turn));
  } else {
    stage.append(oppRowsEl(view, opts, tiles, turn), boardEl(view, opts, tiles, turn));
  }
  stage.append(selfEl(view, opts, tiles, turn));
  if (opts.showGameEnd && opts.meta.game_end) {
    stage.append(gameEndEl(opts.meta, opts.onCloseResult));
  } else if (opts.showResult && view.result) {
    stage.append(resultEl(view, tiles, opts.onCloseResult));
  }
  container.replaceChildren(stage);
}
