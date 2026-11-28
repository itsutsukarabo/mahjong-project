// 画面の寸法（docs/spec/REPLAY_UI.md §3）。画面の大きさと人数から、牌の大きさと各部の位置を決める。
// すべて整数の CSS px にする（高 DPI でにじませないため。拡大縮小の transform は使わない）。

export type LayoutKind = "landscape" | "portrait";

export interface Metrics {
  layout: LayoutKind;
  /** 使える領域の幅・高さ */
  width: number;
  height: number;
  n: number;
  /** 河の牌の幅・高さ */
  rw: number;
  rh: number;
  /** 他家の手牌の牌と、手牌の帯の奥行き（加槓は横向きの牌を2枚重ねるので、牌の幅の2倍を取る） */
  ow: number;
  oh: number;
  od: number;
  /** 自分（視点の席）の手牌の牌 */
  hw: number;
  hh: number;
  /** 自分の副露の牌（手牌より小さく） */
  mw: number;
  mh: number;
  /** 手牌の下の帯の高さ */
  bandH: number;
  /** 帯の下の、区画に結び付かない選択肢の一覧の高さ */
  listH: number;
  /** 中央の情報パネルの一辺（横画面は正方形。縦画面は centerW × centerH の縦長） */
  center: number;
  centerW: number;
  centerH: number;
  /** 上下の席の河の枠（局所座標：幅 = 6 枚＋横向き1枚分、高さ = 4 段） */
  riverW: number;
  riverH: number;
  /** 左右の席の河：1段の枚数と枠の奥行き（横画面は上下と同じ 6 枚 × 4 段。縦画面は幅を詰めるため 8 枚 × 3 段） */
  sidePer: number;
  sideDepth: number;
  /** ドラ表示牌の牌の幅 */
  doraW: number;
  /** 中央パネルと河の間 */
  gap: number;
  /** 河・中央パネルの十字の一辺 */
  cross: number;
  /** 盤（河・中央・他家の手牌。自分の手牌は含めない）の幅・高さと、十字の中心 */
  boardW: number;
  boardH: number;
  cx: number;
  cy: number;
  /** 横画面：他家の手牌の帯と十字との間 */
  stripGap: number;
  /** 十字の上の腕の長さ（四麻は河＋間。三麻は上の席が空くので、左の席の名前が入るだけに詰める） */
  topArm: number;
  /** 横画面：左右の他家の手牌の帯の長さと、その中心の y */
  stripLen: number;
  stripCy: number;
  /** 縦画面：他家の1段の高さ */
  oppRowH: number;
  /** 自分の手牌の段（副露の段・手牌・帯・一覧）の高さ */
  selfH: number;
}

/** 牌の高さ（幅の 1.36 倍、整数） */
export const tileH = (w: number): number => Math.round(w * 1.36);

const RIVER_ROWS = 4;

function base(width: number, height: number, n: number, layout: LayoutKind, rw: number) {
  const rh = tileH(rw);
  const riverW = 6 * rw + (rh - rw) + 6;
  const riverH = RIVER_ROWS * rh + (RIVER_ROWS - 1) * 2;
  const gap = Math.max(3, Math.round(rw * 0.15));
  const center = riverW;
  const cross = center + 2 * (gap + riverH);
  return { width, height, n, layout, rw, rh, riverW, riverH, gap, center, centerW: center, centerH: center, sidePer: 6, sideDepth: riverH, doraW: rw, cross };
}

/**
 * 横画面：盤の左右（と四麻なら上）に他家の手牌の帯、盤の下に自分の手牌と帯。
 * 河の牌の幅を大きい方から試し、全体が入る最大のものを使う
 */
function landscape(width: number, height: number, n: number): Metrics {
  for (let rw = 40; rw >= 12; rw--) {
    const b = base(width, height, n, "landscape", rw);
    const topArm = n === 4 ? b.gap + b.riverH : Math.min(b.gap + b.riverH, 72);
    const armsH = topArm + b.center + b.gap + b.riverH;
    const ow = Math.min(rw, Math.floor(armsH / 21));
    const oh = tileH(ow);
    const od = Math.max(oh, 2 * ow);
    const stripGap = Math.max(4, Math.round(rw * 0.3));
    const top = n === 4 ? od + stripGap : 0;
    const boardW = b.cross + 2 * (od + stripGap);
    const boardH = top + armsH + 4;
    const hw = Math.min(Math.round(rw * 1.5), Math.floor(width / 21));
    const hh = tileH(hw);
    const mw = Math.round(hw * 0.78);
    const mh = tileH(mw);
    const bandH = Math.max(40, Math.round(hh * 0.62));
    const listH = 34;
    const selfH = hh + bandH + listH + 10;
    if (boardW <= width && boardH + selfH <= height) {
      const cy = top + topArm + Math.round(b.center / 2);
      const stripLen = n === 4 ? b.cross : armsH;
      const stripCy = n === 4 ? cy : top + Math.round(armsH / 2);
      return { ...b, ow, oh, od, hw, hh, mw, mh, bandH, listH, boardW, boardH, cx: Math.round(boardW / 2), cy, stripGap, topArm, stripLen, stripCy, oppRowH: 0, selfH };
    }
  }
  return portrait(width, height, n);
}

/**
 * 縦画面：上に他家の段（1席1段、横向きにしない）、真ん中に河と中央パネルの十字、下に自分の副露の段・手牌・帯・一覧。
 * 幅を決めるのは左右の河の奥行きなので、左右の河は 8 枚 × 3 段にして浅くし、中央パネルを縦長にする（REPLAY_UI §3.2）。
 * 三麻は上の席がないので、上の腕はドラ表示牌の高さだけにする。河の牌を大きい方から試し、入る最大のものを使い、
 * 余った高さは帯に回す（72px まで）
 */
function portrait(width: number, height: number, n: number): Metrics {
  const ow = Math.max(12, Math.min(24, Math.floor((width - 8) / 20)));
  const oh = tileH(ow);
  const od = Math.max(oh, 2 * ow);
  const oppRowH = 16 + od + 2;
  const hw = Math.max(16, Math.min(40, Math.floor((width - 8) / 14.8)));
  const hh = tileH(hw);
  const mw = Math.round(hw * 0.78);
  const mh = tileH(mw);
  const band0 = Math.max(48, Math.round(hh * 1.3));
  const listH = 34;
  const self0 = 2 * mw + 4 + hh + band0 + listH + 10;
  const fit = (rw: number, sidePer: 6 | 8) => {
    const b = base(width, height, n, "portrait", rw);
    const sideDepth = sidePer === 8 ? 3 * b.rh + 2 * 2 : b.riverH;
    const centerW = b.riverW;
    const centerH = Math.max(centerW, sidePer * rw + (b.rh - rw) + 6);
    const boardW = centerW + 2 * (b.gap + sideDepth);
    const corner = b.gap + sideDepth;
    // ドラ表示牌：四麻は左上の隅（5 枚が1段に入る幅）、三麻は詰めた上の腕
    const doraW = n === 4 ? Math.min(rw, Math.floor((corner - 6) / 5)) : rw;
    const topArm = n === 4 ? b.gap + b.riverH : 16 + tileH(doraW) + 4;
    const boardH = topArm + centerH + b.gap + b.riverH + 4;
    const used = (n - 1) * oppRowH + boardH + self0 + 2 * (n + 1);
    const bandH = Math.min(72, band0 + Math.max(0, height - used));
    const selfH = self0 - band0 + bandH;
    const m: Metrics = {
      ...b, centerW, centerH, sidePer, sideDepth, doraW, ow, oh, od, hw, hh, mw, mh, bandH, listH, boardW, boardH,
      cx: Math.round(boardW / 2), cy: topArm + Math.round(centerH / 2), stripGap: 0, topArm, stripLen: 0, stripCy: 0, oppRowH, selfH,
    };
    return { m, ok: boardW <= width && used <= height };
  };
  // 左右の河を 8 枚 × 3 段（中央パネルが縦長）にするか、6 枚 × 4 段（正方形）にするかは、河の牌が大きくなる方
  // （四麻は上の河と他家 3 段で高さが足りないので、多くは正方形になる）
  for (let rw = 40; rw > 10; rw--) {
    const square = fit(rw, 6);
    if (square.ok) return square.m;
    const tall = fit(rw, 8);
    if (tall.ok) return tall.m;
  }
  return fit(10, 6).m;
}

/** 縦長（幅 < 高さ × 0.8）か幅 700 未満なら縦画面の配置 */
export function chooseLayout(width: number, height: number): LayoutKind {
  return width < 700 || width < height * 0.8 ? "portrait" : "landscape";
}

export function computeMetrics(width: number, height: number, n: number, layout: LayoutKind = chooseLayout(width, height)): Metrics {
  return layout === "landscape" ? landscape(width, height, n) : portrait(width, height, n);
}

/** 席の位置（視点の席からの相対。0 = 下、1 = 右（下家）、2 = 上（対面）、3 = 左（上家））。三麻は 0・1・3 を使う（上を空ける） */
export function relPos(seat: number, viewSeat: number, n: number): 0 | 1 | 2 | 3 {
  const r = (seat - viewSeat + n) % n;
  if (n === 3) return r === 0 ? 0 : r === 1 ? 1 : 3;
  return r as 0 | 1 | 2 | 3;
}

/** 位置ごとの回転（その席から見て正しい向きになるように） */
export const ROT: Record<0 | 1 | 2 | 3, number> = { 0: 0, 1: -90, 2: 180, 3: 90 };
/** 位置ごとの、十字の中心から見た向き */
export const DIR: Record<0 | 1 | 2 | 3, [number, number]> = { 0: [0, 1], 1: [1, 0], 2: [0, -1], 3: [-1, 0] };
