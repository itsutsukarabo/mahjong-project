// 牌の見た目。今は文字と簡単な図形（素材の権利の問題を避けるため。docs/03_FRONTEND.md「素材の権利」）。
// 画像に差し替えるときは TileRenderer を別に作って render() に渡す。
export interface TileOptions {
  /** 横向き（立直の宣言牌・鳴いた牌） */
  sideways?: boolean;
  /** 薄く（鳴かれた牌など） */
  dim?: boolean;
  /** 追加の CSS クラス */
  classes?: string[];
}

export interface TileRenderer {
  render(tile: string, opts?: TileOptions): HTMLElement;
}

const SUIT: Record<string, string> = { m: "萬", p: "筒", s: "索" };
const HONOR: Record<string, string> = { E: "東", S: "南", W: "西", N: "北", P: "白", F: "發", C: "中" };

/** 牌の表記（`5pr`・`E`・`?`）→ 表示の文字と分類 */
export function tileFace(tile: string): { top: string; bottom: string; kind: string; red: boolean } {
  if (tile === "?") return { top: "", bottom: "", kind: "back", red: false };
  const honor = HONOR[tile];
  if (honor) return { top: honor, bottom: "", kind: `z-${tile}`, red: false };
  const n = tile[0] ?? "";
  const s = tile[1] ?? "";
  return { top: n, bottom: SUIT[s] ?? s, kind: s, red: tile.endsWith("r") };
}

export const textTiles: TileRenderer = {
  render(tile, opts = {}) {
    const f = tileFace(tile);
    const el = document.createElement("span");
    el.className = ["tile", `tile-${f.kind}`, ...(opts.classes ?? [])].join(" ");
    if (f.red) el.classList.add("tile-red");
    if (opts.sideways) el.classList.add("tile-sideways");
    if (opts.dim) el.classList.add("tile-dim");
    el.dataset.tile = tile;
    el.title = tile;
    const top = document.createElement("span");
    top.className = "tile-top";
    top.textContent = f.top;
    const bottom = document.createElement("span");
    bottom.className = "tile-bottom";
    bottom.textContent = f.bottom;
    el.append(top, bottom);
    return el;
  },
};
