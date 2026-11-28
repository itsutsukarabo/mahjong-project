// 2D レンダラーの CSS（アプリが1回だけ <style> に入れる）。大きさは layout.ts が決めた px を CSS の変数で受け取る
// （--tw・--th：その中の牌の幅・高さ、--band：帯の高さ、--c：中央パネルの一辺）。配色はこのプロジェクト独自のもの。
export const css = `
.stage {
  position: relative; margin: 0 auto; box-sizing: border-box; overflow: hidden;
  display: flex; flex-direction: column; align-items: center; gap: 4px;
  font-family: "Noto Sans CJK JP", "Noto Sans JP", "Yu Gothic UI", "Meiryo", sans-serif;
  font-size: 13px; color: #e6ebf2; user-select: none; -webkit-user-select: none;
  --accent: #f0b429; --plus: #3ec27a; --minus: #e5534b; --band-bg: #172230; --line: #3a4a5e;
}
.stage-portrait { gap: 2px; justify-content: space-between; }
.board { position: relative; flex: none; border-radius: 14px; background: radial-gradient(ellipse at center, #1c2938 0%, #141e2a 70%, #111923 100%); }
.board > * { position: absolute; box-sizing: border-box; }

/* 牌 */
.tile { display: inline-block; flex: none; position: relative; width: var(--tw); height: var(--th); }
.tile svg { display: block; width: 100%; height: 100%; overflow: visible; }
.tile-sideways { width: var(--th); height: var(--tw); }
.tile-dim { opacity: 0.42; }

/* 河（6 枚ずつ。局所座標で中央から外へ段が増える） */
.river { display: flex; flex-direction: column; gap: 2px; }
.river-row { display: flex; gap: 1px; align-items: flex-end; height: var(--th); }
.river-tsumogiri svg { filter: brightness(0.86); }
.river-called { outline: 1px dashed #9fb3c8; outline-offset: -1px; }
.river-ron { outline: 2px solid var(--minus); outline-offset: 0; border-radius: 3px; }

/* 他家の手牌の帯（横画面）・段（縦画面） */
.strip { display: flex; align-items: flex-end; }
.hand-group { display: flex; align-items: flex-end; gap: calc(var(--tw) * 0.3); }
.hand { display: flex; align-items: flex-end; gap: 1px; }
.tile-drawn { margin-left: calc(var(--tw) * 0.35); }
.melds { display: flex; gap: 4px; align-items: flex-end; }
.meld { display: inline-flex; align-items: flex-end; }
.kakan-stack { display: inline-flex; flex-direction: column; }
.kita { display: inline-flex; align-items: flex-end; position: relative; padding-right: 2px; }
.kita .tile + .tile { margin-left: calc(var(--tw) * -0.6); }
.kita-count {
  align-self: flex-start; margin-left: 3px; font-size: 11px; font-weight: 700; line-height: 1.3;
  background: rgba(0, 0, 0, 0.55); color: #fff; padding: 0 4px; border-radius: 3px;
}

/* 中央の情報パネル */
.center {
  background: #111a25; border: 1px solid var(--line); border-radius: 10px;
  box-shadow: inset 0 0 0 3px #0b121a;
}
.center-mid {
  position: absolute; left: 20%; right: 20%; top: 20%; bottom: 20%;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3%;
  font-variant-numeric: tabular-nums;
}
.center-kyoku { font-size: calc(var(--c) * 0.13); font-weight: 700; letter-spacing: 0.04em; }
.center-remain { font-size: calc(var(--c) * 0.085); color: #b9c6d6; }
.center-sticks { display: flex; gap: 4px; align-items: center; font-size: calc(var(--c) * 0.075); color: #b9c6d6; }
.stick { display: inline-block; width: calc(var(--c) * 0.11); height: calc(var(--c) * 0.026); border-radius: 2px; background: #e9edf2; }
.stick-honba { background: repeating-linear-gradient(90deg, #e9edf2 0 3px, #3c4a5c 3px 5px); }
.stick-riichi { background: radial-gradient(circle, #d3192b 0 22%, #e9edf2 26%); }
.edge {
  position: absolute; display: flex; align-items: center; justify-content: center; gap: 6px;
  border-bottom: 3px solid transparent; box-sizing: border-box; color: #c9d4e1; line-height: 1;
}
.edge-turn { border-bottom-color: var(--accent); color: #fff; }
.edge-wind { font-weight: 700; padding: 1px 3px; border-radius: 3px; border: 1px solid #6f8399; }
.edge-oya { background: #c8463d; border-color: #c8463d; color: #fff; }
.edge-score { font-variant-numeric: tabular-nums; font-weight: 600; }
.edge-riichi {
  position: absolute; top: -1px; left: 30%; right: 30%; height: 4px; border-radius: 2px;
  background: radial-gradient(circle, #d3192b 0 20%, #e9edf2 24%);
}

/* 名前などのプレイヤー情報（控えめに） */
.plate { display: flex; flex-direction: column; gap: 2px; padding: 4px 6px; font-size: 12px; color: #aebccc; line-height: 1.25; }
.plate-corner-b { justify-content: flex-end; }
.plate-line { display: flex; gap: 4px; align-items: center; min-width: 0; }
.plate-wind { font-weight: 700; color: #dfe7f0; flex: none; }
.plate-oya::after { content: "親"; margin-left: 4px; font-size: 10px; background: #c8463d; color: #fff; padding: 0 3px; border-radius: 3px; }
.plate-name { color: #dfe7f0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.plate-rank { font-size: 10px; color: #7f8fa3; }
.plate-riichi { font-size: 10px; font-weight: 700; color: #fff; background: #8a2b2b; padding: 0 4px; border-radius: 3px; }
.plate-turn .plate-name { color: var(--accent); }
.plate-arrow { font-size: 10px; color: #7f8fa3; }

/* ドラ表示牌 */
.dora { display: flex; flex-direction: column; gap: 3px; font-size: 11px; color: #aebccc; }
.dora-corner { position: absolute; left: 6px; top: 6px; z-index: 2; }
.dora-tiles { display: flex; gap: 1px; }

/* 縦画面：他家の段 */
.opp-rows { display: flex; flex-direction: column; gap: 2px; width: 100%; flex: none; }
.opp-row { display: flex; flex-direction: column; justify-content: space-between; padding: 0 4px; border-left: 3px solid transparent; box-sizing: border-box; }
.opp-turn { border-left-color: var(--accent); }
.opp-row .plate, .self-top .plate { flex-direction: row; align-items: center; gap: 6px; padding: 0; height: 15px; }

/* 自分の手牌と、その下の帯 */
.self { display: flex; flex-direction: column; align-items: center; gap: 4px; width: 100%; flex: none; }
.self-top { display: flex; justify-content: space-between; align-items: flex-end; width: 100%; box-sizing: border-box; padding: 0 4px; }
.self-row { display: flex; align-items: flex-start; gap: 14px; }
.self-melds { display: flex; gap: 6px; align-items: flex-end; }
.band { display: flex; align-items: flex-start; }
.slot { display: flex; flex-direction: column; width: var(--tw); flex: none; }
.slot .tile-drawn { margin-left: 0; }
.slot-gap { width: calc(var(--tw) * 0.4); flex: none; padding-top: calc(var(--th) + 4px); }
.slot-gap::after { content: ""; display: block; height: var(--band); background: var(--band-bg); }
.band-cell {
  position: relative; height: var(--band); margin-top: 4px; box-sizing: border-box;
  background: var(--band-bg); border-left: 1px solid var(--line);
}
.slot:first-child .band-cell { border-left-color: transparent; border-radius: 6px 0 0 6px; }
.slot:last-child .band-cell { border-radius: 0 6px 6px 0; }
.band-bar { position: absolute; left: 18%; right: 18%; bottom: 0; background: #4e8fd6; border-radius: 2px 2px 0 0; }
.band-p { position: absolute; top: 2px; left: 0; right: 0; text-align: center; font-size: 10px; color: #cfd8e3; font-variant-numeric: tabular-nums; }
.band-chosen { box-shadow: inset 0 0 0 2px var(--accent); }
.band-chosen .band-bar { background: var(--accent); }
.band-star { position: absolute; top: 1px; right: 2px; font-size: 9px; line-height: 1; color: #ffd666; }
.band-best .band-p { color: #ffd666; font-weight: 700; }

/* 帯の下の一覧（区画に結び付かない選択肢・価値・選び方） */
.ai-list {
  display: flex; gap: 6px; align-items: center; justify-content: safe center; overflow-x: auto; overflow-y: hidden;
  width: 100%; white-space: nowrap; font-size: 12px; box-sizing: border-box; padding: 0 4px; flex: none; scrollbar-width: thin;
}
.ai-none { color: #7f8fa3; }
.chip {
  display: inline-flex; align-items: center; gap: 4px; padding: 2px 7px; border-radius: 12px; flex: none;
  background: #1f2c3d; border: 1px solid var(--line); --tw: 15px; --th: 20px;
}
.chip-chosen { border-color: var(--accent); background: #3a3320; }
.chip-tiles { display: inline-flex; gap: 1px; }
.chip-flag { font-size: 10px; color: #9fb0c3; }
.chip-p { font-variant-numeric: tabular-nums; font-weight: 700; }
.chip-star { color: #ffd666; }
.mode { padding: 2px 6px; border-radius: 4px; font-weight: 700; flex: none; color: #fff; }
.mode-sample { background: #5a43a0; }
.mode-argmax { background: #1f7a5a; }
.mode-unknown { background: #4b5565; }
.value { display: inline-flex; align-items: center; gap: 4px; flex: none; }
.value-meter { position: relative; width: 56px; height: 8px; background: #2a3646; border-radius: 4px; overflow: hidden; }
.value-meter::after { content: ""; position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: #8899aa; }
.value-fill { position: absolute; top: 0; bottom: 0; }
.value-pos { background: var(--plus); }
.value-neg { background: var(--minus); }
.value-num { font-variant-numeric: tabular-nums; }
.differs { color: #ffb4a9; font-weight: 700; flex: none; }
.other-seat { font-size: 12px; padding: 2px 8px; border-radius: 12px; border: 1px solid #5d7089; background: #263447; color: #dfe7f0; flex: none; cursor: pointer; }

/* 大きな文字（ポン・カン・リーチ…） */
.shout-anchor { overflow: visible; z-index: 30; pointer-events: none; }
.shout {
  position: absolute; left: 0; top: 0; transform: translate(-50%, -50%); opacity: 0;
  font-size: var(--shout, 40px); font-weight: 900; letter-spacing: 0.06em; white-space: nowrap; pointer-events: none;
  color: #fff; -webkit-text-stroke: 2px rgba(0, 0, 0, 0.55); text-shadow: 0 2px 10px rgba(0, 0, 0, 0.7);
}
.shout-reach { color: #ffd166; }
.shout-ron, .shout-tsumo { color: #ff7b72; }
.shout-pon, .shout-chi, .shout-kan, .shout-kita { color: #8fd0ff; }

/* 結果の窓 */
.result {
  position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); z-index: 40;
  width: min(560px, calc(100% - 16px)); max-height: calc(100% - 16px); overflow: auto; box-sizing: border-box;
  background: #f6f3ea; color: #1c2430; border-radius: 10px; padding: 12px 14px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.55); display: flex; flex-direction: column; gap: 8px;
}
.result-hora { display: flex; flex-direction: column; gap: 6px; padding-bottom: 8px; border-bottom: 1px solid #d8d2c2; }
.result-head { display: flex; gap: 10px; align-items: baseline; font-weight: 700; font-size: 16px; }
.result-how { font-size: 13px; color: #5a6270; }
.result-value { display: flex; gap: 12px; align-items: baseline; }
.result-limit { font-size: 22px; font-weight: 800; color: #b3261e; }
.result-points { font-size: 22px; font-weight: 800; font-variant-numeric: tabular-nums; }
.result-pao { font-size: 12px; color: #8a2b2b; }
.result-hand { display: flex; flex-wrap: wrap; gap: 6px; align-items: flex-end; }
.result-tiles { display: inline-flex; gap: 1px; }
.result-win { display: inline-flex; outline: 2px solid #d3192b; outline-offset: 1px; border-radius: 4px; }
.result-yaku { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 2px 18px; font-size: 13px; }
.yaku-row { display: flex; justify-content: space-between; gap: 8px; }
.yaku-han { font-variant-numeric: tabular-nums; }
.result-dora { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 12px; }
.dora-zero { opacity: 0.45; }
.result-sum { font-weight: 700; }
.result-indicators { display: flex; gap: 6px; align-items: center; font-size: 11px; flex-wrap: wrap; color: #5a6270; }
.ind-none { font-size: 11px; color: #8a8f98; }
.result-deltas { display: grid; gap: 2px; font-size: 13px; font-variant-numeric: tabular-nums; }
.delta-row { display: grid; grid-template-columns: minmax(0, 1fr) 72px 72px 72px; text-align: right; gap: 4px; }
.delta-name { text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.delta-before { color: #7a808a; }
.delta-plus { color: #1d8f50; font-weight: 700; }
.delta-minus { color: #c62f28; font-weight: 700; }
.ryukyoku-hands { display: flex; flex-direction: column; gap: 6px; }
.ryu-row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; font-size: 13px; }
.ryu-name { min-width: 90px; }
.ryu-state { color: #7a808a; }
.ryu-tenpai .ryu-state { color: #1d8f50; font-weight: 700; }
.final-table { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-variant-numeric: tabular-nums; }
.final-row { display: grid; grid-template-columns: 40px minmax(0, 1fr) 76px 84px; gap: 6px; align-items: center; }
.final-chips-on .final-row { grid-template-columns: 40px minmax(0, 1fr) 76px 84px 56px; }
.final-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.final-row > .delta, .final-score, .final-chips { text-align: right; }
.final-headrow { font-size: 11px; color: #7a808a; }
.final-rank { font-weight: 800; }
.final-tobi { color: #c62f28; font-size: 11px; }
.result-close {
  align-self: center; font-size: 14px; padding: 6px 26px; border-radius: 6px; cursor: pointer;
  border: 1px solid #9aa3ad; background: #fff; color: #1c2430;
}
`;
