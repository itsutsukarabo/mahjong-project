// DOM の小さな部品（卓と結果の窓で共有する）
import type { Meld } from "@mj/log-types";
import type { TileRenderer } from "./tiles";

export function el(tag: string, cls: string, text?: string): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

export function meldEl(m: Meld, seat: number, n: number, tiles: TileRenderer, key?: string): HTMLElement {
  const box = el("span", `meld meld-${m.type}`);
  box.dataset.meld = m.type;
  if (key) box.dataset.key = key;
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
  const called = tiles.render(m.pai ?? "?", { sideways: true, classes: ["tile-called"] });
  if (m.type === "kakan") {
    // 加槓：横にした牌の上に加えた牌を重ねる
    const stack = el("span", "kakan-stack");
    stack.append(tiles.render(m.consumed[3] ?? m.consumed[2] ?? "?", { sideways: true }), called);
    parts.splice(pos, 0, stack);
  } else {
    parts.splice(pos, 0, called);
  }
  box.append(...parts);
  return box;
}

