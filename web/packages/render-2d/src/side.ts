// 横画面の右の欄（開発者向けの詳細）：この時点の全席の判断ポイントの注釈を、確率の高い順に全部出す
import type { Annotation, TableView } from "@mj/log-types";
import type { SelectionMode } from "@mj/table-model";
import { el } from "./dom";
import { WIND_JA, actionLabel, pct } from "./labels";

export function renderSide(container: HTMLElement, list: Annotation[], view: TableView, mode: SelectionMode | null, show: boolean): void {
  const box = el("div", "side");
  box.dataset.part = "side";
  box.append(el("div", "side-title", "判断ポイントの注釈（この時点）"));
  if (mode) box.append(el("div", "side-mode", `選び方：${mode.kind === "sample" ? "抽選" : mode.kind === "argmax" ? "最大確率" : "不明"}（${mode.detail}）`));
  if (!show) {
    box.append(el("div", "side-empty", "AI の判断の表示：切"));
    container.replaceChildren(box);
    return;
  }
  if (list.length === 0) box.append(el("div", "side-empty", "この時点の注釈はない"));
  for (const a of list) {
    const s = view.seats[a.seat];
    const card = el("div", "side-card");
    card.dataset.seat = String(a.seat);
    const head = el("div", "side-head");
    head.append(
      el("span", "", `${WIND_JA[s?.wind ?? ""] ?? ""} ${s?.name || `席${a.seat}`}`),
      el("span", "side-model", a.model ?? ""),
      el("span", "side-value", a.value === undefined ? "" : `価値 ${a.value >= 0 ? "+" : ""}${a.value.toFixed(3)}`),
    );
    card.append(head);
    const probs = new Map((a.policy ?? []).map(([act, p]) => [act, p] as const));
    const acts = [...(a.choices ?? [...probs.keys()])];
    if (probs.size > 0) acts.sort((x, y) => (probs.get(y) ?? 0) - (probs.get(x) ?? 0));
    for (const act of acts) {
      const p = probs.get(act) ?? null;
      const row = el("div", act === a.decision ? "side-row side-chosen" : "side-row");
      const l = actionLabel(act);
      row.append(el("span", "side-act", `${l.verb}${l.tiles.length ? ` ${l.tiles.join(",")}` : ""}${l.tsumogiri ? " ツモ切り" : ""}${l.extra ? ` ${l.extra}` : ""}`));
      const bar = el("span", "side-bar");
      const fill = el("span", "side-fill");
      fill.style.width = `${Math.round((p ?? 0) * 100)}%`;
      bar.append(fill);
      row.append(bar, el("span", "side-p", p === null ? "—" : `${pct(p)}%`));
      card.append(row);
    }
    box.append(card);
  }
  container.replaceChildren(box);
}

export const sideCss = `
.side { display: flex; flex-direction: column; gap: 8px; font-size: 12px; color: #dfe7f0; }
.side-title { font-weight: 700; }
.side-mode, .side-empty { color: #8d9bad; font-size: 11px; }
.side-card { background: #172230; border: 1px solid #33445a; border-radius: 6px; padding: 6px; display: flex; flex-direction: column; gap: 2px; }
.side-head { display: flex; gap: 6px; font-weight: 700; margin-bottom: 2px; }
.side-model { color: #8d9bad; font-weight: 400; }
.side-value { margin-left: auto; font-variant-numeric: tabular-nums; }
.side-row { display: grid; grid-template-columns: 110px 1fr 40px; gap: 6px; align-items: center; }
.side-chosen { color: #f0b429; font-weight: 700; }
.side-bar { height: 7px; background: #26364a; border-radius: 4px; overflow: hidden; }
.side-fill { display: block; height: 100%; background: #4e8fd6; }
.side-chosen .side-fill { background: #f0b429; }
.side-p { text-align: right; font-variant-numeric: tabular-nums; }
`;
