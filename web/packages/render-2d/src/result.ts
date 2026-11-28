// 局の結果（和了・流局）と対局の終わりの窓。表示する値はすべて牌譜（hora・ryukyoku・game_end）に書かれたもの。
import type { Hora, ReplayMeta, TableView } from "@mj/log-types";
import { DORA_IDS, REASON_JA, WIND_JA, YAKU_JA, limitName, signed } from "./labels";
import { el, meldEl } from "./dom";
import type { TileRenderer } from "./tiles";

function closeButton(onClose?: () => void): HTMLElement {
  const b = el("button", "result-close", "閉じる") as HTMLButtonElement;
  b.type = "button";
  b.onclick = () => onClose?.();
  return b;
}

const seatName = (view: TableView, seat: number) => {
  const s = view.seats[seat];
  return `${WIND_JA[s?.wind ?? ""] ?? ""} ${s?.name || `席${seat}`}`;
};

function tilesRow(list: string[], tiles: TileRenderer, cls = "result-tiles"): HTMLElement {
  const r = el("span", cls);
  for (const t of list) r.append(tiles.render(t));
  return r;
}

/** 各席の点数の増減（増えたら色 A、減ったら色 B） */
function deltasEl(view: TableView, deltas: number[], scores: number[]): HTMLElement {
  const box = el("div", "result-deltas");
  box.dataset.part = "deltas";
  deltas.forEach((d, i) => {
    const row = el("div", "delta-row");
    row.dataset.seat = String(i);
    const cls = d > 0 ? "delta delta-plus" : d < 0 ? "delta delta-minus" : "delta";
    row.append(
      el("span", "delta-name", seatName(view, i)),
      el("span", "delta-before", (scores[i]! - d).toLocaleString("ja-JP")),
      el("span", cls, d === 0 ? "±0" : signed(d)),
      el("span", "delta-after", (scores[i] ?? 0).toLocaleString("ja-JP")),
    );
    box.append(row);
  });
  return box;
}

function horaEl(view: TableView, h: Hora, tiles: TileRenderer): HTMLElement {
  const box = el("section", "result-hora");
  box.dataset.actor = String(h.actor);
  const how = h.actor === h.target ? "ツモ" : `ロン ← ${seatName(view, h.target)}`;
  const head = el("div", "result-head");
  head.append(el("span", "result-who", seatName(view, h.actor)), el("span", "result-how", how));
  box.append(head);
  const name = limitName(h);
  const value = el("div", "result-value");
  if (name) value.append(el("span", "result-limit", name));
  value.append(el("span", "result-points", `${h.points.toLocaleString("ja-JP")}点`));
  if (h.pao !== undefined) value.append(el("span", "result-pao", `包：${seatName(view, h.pao)}`));
  box.append(value);
  // 和了形：手牌＋和了牌＋副露
  const hand = el("div", "result-hand");
  hand.style.setProperty("--tw", "28px");
  hand.style.setProperty("--th", "38px");
  hand.append(tilesRow(h.hand.tehai, tiles), tilesRow([h.pai], tiles, "result-win"));
  const melds = el("span", "melds");
  for (const m of h.hand.melds) melds.append(meldEl(m, h.actor, view.seats.length, tiles));
  hand.append(melds);
  box.append(hand);
  // 役と翻数（ドラの類は分けて出す）
  const yaku = el("div", "result-yaku");
  const doraIds: readonly string[] = DORA_IDS;
  for (const y of h.yaku.filter((x) => !doraIds.includes(x.id))) {
    const row = el("div", "yaku-row");
    row.append(el("span", "yaku-name", YAKU_JA[y.id] ?? y.id), el("span", "yaku-han", y.yakuman ? (y.yakuman === 1 ? "役満" : `${y.yakuman}倍役満`) : `${y.han ?? 0}翻`));
    yaku.append(row);
  }
  const dora = el("div", "result-dora");
  for (const id of DORA_IDS) {
    const y = h.yaku.find((x) => x.id === id);
    dora.append(el("span", y ? "dora-item" : "dora-item dora-zero", `${YAKU_JA[id]} ${y?.han ?? 0}`));
  }
  box.append(yaku, dora);
  const sum = el("div", "result-sum");
  sum.append(el("span", "", h.yakuman > 0 ? `役満 ×${h.yakuman}` : `${h.han}翻 ${h.fu}符`));
  box.append(sum);
  // ドラ表示牌と裏ドラ表示牌
  const ind = el("div", "result-indicators");
  ind.style.setProperty("--tw", "22px");
  ind.style.setProperty("--th", "30px");
  ind.append(el("span", "ind-label", "ドラ表示"), tilesRow(view.dora_indicators, tiles));
  ind.append(el("span", "ind-label", "裏ドラ表示"), h.ura_indicators.length > 0 ? tilesRow(h.ura_indicators, tiles) : el("span", "ind-none", "なし"));
  box.append(ind);
  return box;
}

/** 局の結果の窓 */
export function resultEl(view: TableView, tiles: TileRenderer, onClose?: () => void): HTMLElement {
  const r = view.result;
  const box = el("div", "result");
  box.dataset.part = "result";
  if (!r) return box;
  const n = view.seats.length;
  if (r.horas.length > 0) {
    box.dataset.kind = "hora";
    for (const h of r.horas) box.append(horaEl(view, h, tiles));
    const total = new Array<number>(n).fill(0);
    for (const h of r.horas) h.deltas.forEach((d, i) => (total[i] = (total[i] ?? 0) + d));
    const last = r.horas[r.horas.length - 1]!;
    box.append(deltasEl(view, total, last.scores));
  }
  if (r.ryukyoku) {
    const k = r.ryukyoku;
    box.dataset.kind = "ryukyoku";
    const head = el("div", "result-head");
    head.append(el("span", "result-who", "流局"), el("span", "result-how", REASON_JA[k.reason] ?? k.reason));
    box.append(head);
    const list = el("div", "ryukyoku-hands");
    list.style.setProperty("--tw", "24px");
    list.style.setProperty("--th", "33px");
    k.tenpai.forEach((t, i) => {
      const row = el("div", t ? "ryu-row ryu-tenpai" : "ryu-row");
      row.append(el("span", "ryu-name", seatName(view, i)), el("span", "ryu-state", t ? "聴牌" : "ノーテン"));
      const h = k.hands[i];
      if (h) row.append(tilesRow(h, tiles));
      else row.append(el("span", "ind-none", t ? "（手牌は非公開）" : ""));
      list.append(row);
    });
    box.append(list, deltasEl(view, k.deltas, k.scores));
  }
  box.append(closeButton(onClose));
  return box;
}

/** 対局の終わりの窓：順位・点数・精算 */
export function gameEndEl(meta: ReplayMeta, onClose?: () => void): HTMLElement {
  const g = meta.game_end;
  const box = el("div", "result result-final");
  box.dataset.part = "game-end";
  box.append(el("div", "result-head", "対局終了"));
  if (!g) return box;
  const table = el("div", g.chips_total.some((c) => c !== 0) ? "final-table final-chips-on" : "final-table");
  const head = el("div", "final-row final-headrow");
  head.append(el("span", "", "順位"), el("span", "", "名前"), el("span", "", "点数"), el("span", "", "精算"));
  const chips = g.chips_total.some((c) => c !== 0);
  if (chips) head.append(el("span", "", "祝儀"));
  table.append(head);
  const order = g.ranks.map((r, i) => ({ r, i })).sort((a, b) => a.r - b.r);
  for (const { r, i } of order) {
    const row = el("div", "final-row");
    row.dataset.seat = String(i);
    const st = g.settlement[i] ?? 0;
    row.append(
      el("span", "final-rank", `${r}位`),
      el("span", "final-name", meta.players[i] || `席${i}`),
      el("span", "final-score", (g.scores[i] ?? 0).toLocaleString("ja-JP")),
      el("span", st > 0 ? "delta delta-plus" : st < 0 ? "delta delta-minus" : "delta", signed(st)),
    );
    if (chips) row.append(el("span", "final-chips", signed(g.chips_total[i] ?? 0)));
    if (g.tobi[i]) row.querySelector(".final-rank")?.append(el("span", "final-tobi", " 飛び"));
    table.append(row);
  }
  box.append(table);
  box.append(closeButton(onClose));
  return box;
}
