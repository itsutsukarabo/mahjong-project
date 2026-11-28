// 牌の要素を作る。絵は tile-art.ts（種類 → 絵の対応はそこだけ）。画像に差し替えるときは tileArt を変えるか、
// TileRenderer を別に作って render() に渡す。
import { ALL_TILES, ART_H, ART_W, symbolId, tileArt } from "./tile-art";

export interface TileOptions {
  /** 横向き（立直の宣言牌・鳴いた牌） */
  sideways?: boolean;
  /** 薄く（鳴かれた牌など） */
  dim?: boolean;
  /** 追加の CSS クラス */
  classes?: string[];
  /** アニメーションの突き合わせのキー（table-model の placeKey） */
  key?: string;
}

export interface TileRenderer {
  render(tile: string, opts?: TileOptions): HTMLElement;
}

const SVG = "http://www.w3.org/2000/svg";
const SPRITE_ID = "mj-tile-sprite";

/** 牌の絵を <symbol> として文書に1回だけ入れる */
export function ensureTileSprite(doc: Document = document): void {
  if (doc.getElementById(SPRITE_ID)) return;
  const svg = doc.createElementNS(SVG, "svg");
  svg.id = SPRITE_ID;
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("style", "position:absolute;width:0;height:0;overflow:hidden");
  svg.innerHTML =
    "<defs>" +
    ALL_TILES.map((t) => `<symbol id="${symbolId(t)}" viewBox="0 0 ${ART_W} ${ART_H}">${tileArt(t)}</symbol>`).join("") +
    "</defs>";
  doc.body.prepend(svg);
}

/** 牌の表記 → 分類（CSS のクラス・テスト用） */
export function tileKind(tile: string): string {
  if (tile === "?") return "back";
  if (/^[ESWNPFC]$/.test(tile)) return `z-${tile}`;
  return tile[1] ?? "x";
}

export const svgTiles: TileRenderer = {
  render(tile, opts = {}) {
    const known = ALL_TILES.includes(tile) ? tile : "?";
    const el = document.createElement("span");
    el.className = ["tile", `tile-${tileKind(known)}`, ...(opts.classes ?? [])].join(" ");
    if (known.endsWith("r")) el.classList.add("tile-red");
    if (opts.sideways) el.classList.add("tile-sideways");
    if (opts.dim) el.classList.add("tile-dim");
    el.dataset.tile = tile;
    if (opts.key) el.dataset.key = opts.key;
    const svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", opts.sideways ? `0 0 ${ART_H} ${ART_W}` : `0 0 ${ART_W} ${ART_H}`);
    svg.setAttribute("preserveAspectRatio", "none");
    const use = document.createElementNS(SVG, "use");
    use.setAttribute("href", `#${symbolId(known)}`);
    use.setAttribute("width", String(ART_W));
    use.setAttribute("height", String(ART_H));
    // 横向き：左に 90° 倒す（(x, y) → (y, 60 − x)）
    if (opts.sideways) use.setAttribute("transform", `translate(0 ${ART_W}) rotate(-90)`);
    svg.append(use);
    el.append(svg);
    return el;
  },
};
