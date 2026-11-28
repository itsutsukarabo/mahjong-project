// 2D レンダラー（DOM）：卓の状態（mj_log::view の TableView）を描く。描くだけで、和了判定・点数計算・合法手の判定は
// しない（原則1）。配置・帯・アニメーションの決めごとは docs/spec/REPLAY_UI.md。牌の絵は tile-art.ts（1か所）。
export { type AiOptions, type RenderOptions, render } from "./table";
export { captureRects, playCues, type PlayOptions, type Rects } from "./anim";
export { type LayoutKind, type Metrics, chooseLayout, computeMetrics, relPos } from "./layout";
export { type TileOptions, type TileRenderer, ensureTileSprite, svgTiles, tileKind } from "./tiles";
export { tileArt, ALL_TILES } from "./tile-art";
export { YAKU_JA, REASON_JA, actionLabel, limitName } from "./labels";
export { css } from "./style";
export { renderSide, sideCss } from "./side";
