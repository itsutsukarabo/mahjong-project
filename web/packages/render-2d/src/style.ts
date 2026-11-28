// 2D レンダラーの CSS（アプリが1回だけ <style> に入れる）。大きさは固定（画面のスナップショットを安定させるため）。
export const css = `
.table {
  display: grid;
  grid-template-columns: 370px 400px 370px;
  grid-template-rows: 240px 300px 240px;
  grid-template-areas: "tl top tr" "left center right" "bl bottom br";
  gap: 6px;
  width: 1152px;
  position: relative;
  font-family: "Noto Sans CJK JP", "Noto Sans JP", "Yu Gothic UI", "Meiryo", sans-serif;
  font-size: 13px;
  color: #1d2330;
  background: #2f6b4f;
  padding: 8px;
  border-radius: 8px;
}
.seat { background: #e9efe9; border-radius: 6px; padding: 6px; overflow: hidden; box-sizing: border-box; }
.seat-active { outline: 3px solid #f2b705; }
.seat-bottom { grid-area: bottom; grid-column: 1 / span 3; }
.seat-top { grid-area: top; grid-column: 1 / span 3; }
.seat-left { grid-area: left; grid-row: 1 / span 3; grid-column: 1; }
.seat-right { grid-area: right; grid-row: 1 / span 3; grid-column: 3; }
.table-3p .seat-left, .table-3p .seat-right { grid-row: 1 / span 2; }
.table-4p .seat-left, .table-4p .seat-right { grid-row: 2; }
.table-3p .center { grid-row: 1 / span 2; }
.seat-head { display: flex; gap: 8px; align-items: baseline; margin-bottom: 4px; }
.seat-wind { font-weight: 700; color: #8a2b2b; }
.seat-name { font-weight: 600; }
.seat-score { margin-left: auto; font-variant-numeric: tabular-nums; }
.seat-riichi { background: #8a2b2b; color: #fff; border-radius: 3px; padding: 0 4px; font-size: 11px; }
.hand-row { display: flex; flex-wrap: wrap; gap: 6px; align-items: flex-end; min-height: 34px; }
.hand, .melds, .kita, .dora, .result-hand { display: flex; gap: 1px; align-items: flex-end; flex-wrap: wrap; }
.melds { gap: 6px; }
.result-hand .melds { margin-left: 10px; }
.meld { display: inline-flex; gap: 1px; align-items: flex-end; }
.kakan-stack { display: inline-flex; flex-direction: column-reverse; gap: 1px; }
.kita { padding-left: 4px; border-left: 2px dashed #9aa79a; }
.river { display: grid; grid-template-columns: repeat(6, 24px); gap: 2px; margin-top: 6px; }
.seat-left .river, .seat-right .river { grid-template-columns: repeat(6, 24px); }
.tile {
  display: inline-flex; flex-direction: column; align-items: center; justify-content: center;
  width: 22px; height: 30px; box-sizing: border-box;
  background: #fffdf6; border: 1px solid #6d6a60; border-radius: 3px;
  line-height: 1; font-weight: 700;
}
.tile-top { font-size: 13px; }
.tile-bottom { font-size: 9px; font-weight: 400; }
.tile-m .tile-top, .tile-m .tile-bottom { color: #1d2330; }
.tile-p .tile-top, .tile-p .tile-bottom { color: #1f4e9c; }
.tile-s .tile-top, .tile-s .tile-bottom { color: #1e7a3c; }
.tile-z-C .tile-top { color: #c0262d; }
.tile-z-F .tile-top { color: #1e7a3c; }
.tile-z-P .tile-top { color: #8c8c8c; }
.tile-red .tile-top, .tile-red .tile-bottom { color: #d0182a; }
.tile-red { background: #fff1f1; }
.tile-back { background: repeating-linear-gradient(45deg, #2b5aa8, #2b5aa8 4px, #3a6cc0 4px, #3a6cc0 8px); border-color: #1d3d75; }
.tile-sideways { width: 30px; height: 22px; }
.tile-dim { opacity: 0.4; }
.tile-drawn { margin-left: 8px; }
.tile-win { margin-left: 8px; outline: 2px solid #d0182a; }
.river-tsumogiri { background: #e3e3dc; }
.river-riichi { outline: 2px solid #8a2b2b; }
.river-ron { outline: 2px solid #d0182a; background: #ffd9d9; }
.center {
  grid-area: center; background: #234f3a; color: #f3f6f3; border-radius: 6px; padding: 10px;
  display: flex; flex-direction: column; gap: 8px; align-items: center; justify-content: center;
}
.center-kyoku { font-size: 20px; font-weight: 700; }
.center .label { margin-right: 6px; font-size: 12px; }
.center-event { font-size: 12px; opacity: 0.8; font-family: ui-monospace, monospace; }
.result {
  position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);
  width: 520px; background: rgba(255, 255, 255, 0.97); border: 2px solid #1d2330; border-radius: 8px;
  padding: 10px; display: flex; flex-direction: column; gap: 8px; box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
}
.result-title { font-weight: 700; font-size: 15px; }
.result-yaku, .result-points, .result-deltas { font-size: 12px; }
.annotations { display: flex; flex-direction: column; gap: 8px; font-size: 12px; }
.annot { background: #f5f7fa; border: 1px solid #c8d0dc; border-radius: 6px; padding: 6px; }
.annot-head { font-weight: 700; margin-bottom: 4px; }
.annot-row { display: grid; grid-template-columns: 140px 1fr 50px; gap: 6px; align-items: center; }
.annot-chosen { font-weight: 700; color: #8a2b2b; }
.annot-bar { height: 8px; background: #e2e6ec; border-radius: 4px; overflow: hidden; }
.annot-fill { display: block; height: 100%; background: #3a6cc0; }
.annot-p { text-align: right; font-variant-numeric: tabular-nums; }
.annot-empty { color: #6b7280; }
`;
