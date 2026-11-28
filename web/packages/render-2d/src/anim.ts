// アニメーション（docs/spec/REPLAY_UI.md §5）：描き直す前の牌の位置（data-key ごとの画面上の矩形）を取っておき、
// 描き直した後の同じ牌を、前の位置から今の位置へ動かす（FLIP）。どの牌が動いたかは table-model の cuesBetween
// （前後の卓の状態の差）が決める。ルールの判断はしない。Web Animations API だけを使い、毎フレームの JS を書かない。
import { type Cues, placeKey } from "@mj/table-model";
import { SHOUT_JA } from "./labels";

export type Rects = Map<string, DOMRect>;

/** 描き直す前に、data-key の付いた要素の画面上の矩形を取る */
export function captureRects(root: HTMLElement): Rects {
  const out: Rects = new Map();
  for (const e of root.querySelectorAll<HTMLElement>("[data-key]")) out.set(e.dataset.key!, e.getBoundingClientRect());
  return out;
}

/** 要素の局所座標の回転（祖先の data-rot。回転した帯・河の中の牌） */
function rotationOf(e: HTMLElement): number {
  const r = e.closest<HTMLElement>("[data-rot]");
  return r ? Number(r.dataset.rot) : 0;
}

export interface PlayOptions {
  /** 牌が動く時間（ms） */
  duration: number;
}

/** 手がかりどおりに動かす。動かしたアニメーションの数を返す（テスト・計測用） */
export function playCues(root: HTMLElement, before: Rects, cues: Cues, opts: PlayOptions): number {
  let count = 0;
  const d = Math.max(40, opts.duration);
  for (const mv of cues.moves) {
    const from = before.get(placeKey(mv.from));
    const target = root.querySelector<HTMLElement>(`[data-key="${placeKey(mv.to)}"]`);
    if (!from || !target) continue;
    const to = target.getBoundingClientRect();
    if (to.width === 0 || to.height === 0) continue;
    const dx = from.left + from.width / 2 - (to.left + to.width / 2);
    const dy = from.top + from.height / 2 - (to.top + to.height / 2);
    // 画面の向きの差を、要素の局所座標（回転した帯・河の中）に直す
    const t = (-rotationOf(target) * Math.PI) / 180;
    const lx = dx * Math.cos(t) - dy * Math.sin(t);
    const ly = dx * Math.sin(t) + dy * Math.cos(t);
    const s = Math.sqrt((from.width * from.height) / (to.width * to.height));
    target.animate(
      [
        { transform: `translate(${lx.toFixed(1)}px, ${ly.toFixed(1)}px) scale(${s.toFixed(3)})`, zIndex: 5 },
        { transform: "none", zIndex: 5 },
      ],
      { duration: mv.role === "main" ? d : Math.round(d * 0.8), easing: "cubic-bezier(.2,.7,.3,1)" },
    );
    count++;
  }
  for (const p of cues.appear) {
    const target = root.querySelector<HTMLElement>(`[data-key="${placeKey(p)}"]`);
    if (!target) continue;
    target.animate(
      [
        { opacity: 0, transform: "translateY(-35%)" },
        { opacity: 1, transform: "none" },
      ],
      { duration: d, easing: "ease-out" },
    );
    count++;
  }
  if (cues.shout) {
    const anchor = root.querySelector<HTMLElement>(`[data-shout="${cues.shout.seat}"]`);
    if (anchor) {
      const sh = document.createElement("div");
      sh.className = `shout shout-${cues.shout.kind}`;
      sh.textContent = SHOUT_JA[cues.shout.kind];
      anchor.append(sh);
      const a = sh.animate(
        [
          { opacity: 0, transform: "translate(-50%, -50%) scale(0.6)" },
          { opacity: 1, transform: "translate(-50%, -50%) scale(1.08)", offset: 0.18 },
          { opacity: 1, transform: "translate(-50%, -50%) scale(1)", offset: 0.7 },
          { opacity: 0, transform: "translate(-50%, -50%) scale(1)" },
        ],
        { duration: Math.max(500, d * 3.5), easing: "ease-out", fill: "forwards" },
      );
      a.onfinish = () => sh.remove();
      count++;
    }
  }
  return count;
}
