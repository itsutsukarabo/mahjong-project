// 牌の絵（仮）：「牌の種類 → 絵」の対応はこのファイルの tileArt だけ（docs/spec/REPLAY_UI.md §7）。
// 絵はコードで作る簡単な独自のもの（他のゲームの牌の画像は使わない。docs/03_FRONTEND.md「素材の権利」）。
// 差し替えるときは tileArt を、同じ座標系（幅 60 × 高さ 80）の SVG の中身（<image href="..."> でもよい）を返すものに変える。
// 絵は文書に1回だけ <symbol> として入れ、各牌は <use> で参照する（DOM の数を減らす。テクスチャアトラスの代わり）。

/** 牌の絵の座標系 */
export const ART_W = 60;
export const ART_H = 80;

const INK: Record<string, string> = { m: "#7b2d5b", p: "#1d5aa6", s: "#1b7445" };
const HONOR: Record<string, [string, string]> = {
  E: ["東", "#23304a"],
  S: ["南", "#23304a"],
  W: ["西", "#23304a"],
  N: ["北", "#23304a"],
  F: ["發", "#1b7445"],
  C: ["中", "#c22a33"],
};
const SUIT_JA: Record<string, string> = { m: "萬", p: "筒", s: "索" };
const RED = "#d3192b";

const FONT = `font-family="'Noto Sans CJK JP','Noto Sans JP','Yu Gothic UI','Meiryo',sans-serif"`;

/** 表の地（牌の面と、下の厚み） */
const face = (tint = "#fbf8ef") =>
  `<rect x="1" y="1" width="58" height="78" rx="8" fill="#c9c1ad"/>` +
  `<rect x="1" y="1" width="58" height="73" rx="8" fill="${tint}" stroke="#8f8774" stroke-width="1.5"/>`;

/** 数牌の左上の印（種類ごとに形を変える。色だけに頼らない） */
function suitMark(s: string, color: string): string {
  if (s === "p") return `<circle cx="11" cy="11" r="4.5" fill="none" stroke="${color}" stroke-width="2.2"/>`;
  if (s === "s") return `<rect x="8" y="5" width="5" height="12" rx="2.5" fill="${color}"/>`;
  return `<rect x="7" y="7" width="8" height="8" transform="rotate(45 11 11)" fill="${color}"/>`;
}

/** 牌の表記（`5pr`・`E`・`?`）→ SVG の中身（幅 60 × 高さ 80） */
export function tileArt(tile: string): string {
  if (tile === "?") {
    return (
      `<rect x="1" y="1" width="58" height="78" rx="8" fill="#1d4a52"/>` +
      `<rect x="1" y="1" width="58" height="73" rx="8" fill="#2f7480" stroke="#1d4a52" stroke-width="1.5"/>` +
      `<rect x="9" y="9" width="42" height="57" rx="5" fill="none" stroke="#9fd0d6" stroke-width="1.5" opacity="0.7"/>` +
      `<path d="M30 20 L40 37 L30 54 L20 37 Z" fill="#9fd0d6" opacity="0.55"/>`
    );
  }
  if (tile === "P") {
    return face() + `<rect x="13" y="15" width="34" height="44" rx="4" fill="none" stroke="#5b7fa8" stroke-width="3"/>`;
  }
  const h = HONOR[tile];
  if (h) {
    return face() + `<text x="30" y="51" text-anchor="middle" font-size="36" font-weight="700" fill="${h[1]}" ${FONT}>${h[0]}</text>`;
  }
  const n = tile[0] ?? "";
  const s = tile[1] ?? "";
  const red = tile.endsWith("r");
  const color = red ? RED : (INK[s] ?? "#222");
  return (
    face(red ? "#fff0ee" : undefined) +
    suitMark(s, color) +
    (red ? `<circle cx="49" cy="11" r="4" fill="${RED}"/>` : "") +
    `<text x="30" y="47" text-anchor="middle" font-size="38" font-weight="700" fill="${color}" ${FONT}>${n}</text>` +
    `<text x="30" y="67" text-anchor="middle" font-size="14" fill="${color}" ${FONT}>${SUIT_JA[s] ?? s}</text>`
  );
}

/** 牌の表記の一覧（スプライトに入れるもの）。`?` は裏 */
export const ALL_TILES: string[] = [
  ..."123456789".split("").flatMap((n) => ["m", "p", "s"].map((s) => `${n}${s}`)),
  "5mr", "5pr", "5sr",
  "E", "S", "W", "N", "P", "F", "C",
  "?",
];

/** <symbol> の id */
export const symbolId = (tile: string): string => `mjt-${tile === "?" ? "back" : tile}`;
