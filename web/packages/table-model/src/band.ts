// 手牌の下の帯（打牌を選びやすくするための領域、docs/spec/REPLAY_UI.md §4）の区画と牌の対応。
// 区画 i は手牌の i 枚目（ツモ牌があれば最後の区画）。各区画の「打牌の行動の表記」（DECISION_POINTS §5）も持つ。
// 将来のゲームモードでは、エンジンが返す合法手の一覧にこの表記があるかで区画を押せるかを決める
// （ここでは合法手の判定はしない。原則1）。
import type { Annotation, SeatView } from "@mj/log-types";

export interface BandSlot {
  /** 区画の番号（0 始まり、左から） */
  index: number;
  /** その真上の牌（見えない牌は `?`） */
  tile: string;
  /** ツモ牌の区画か */
  drawn: boolean;
  /** 手牌の何枚目か（ツモ牌なら null） */
  handIndex: number | null;
  /** この区画の牌を切る行動の表記（`dahai:5p`・`dahai:5p:tsumogiri`） */
  action: string;
}

/** 席の手牌から帯の区画を作る（手牌の並び＋ツモ牌。副露・抜いた北は含めない） */
export function bandSlots(seat: Pick<SeatView, "hand" | "drawn">): BandSlot[] {
  const slots: BandSlot[] = seat.hand.map((tile, i) => ({
    index: i,
    tile,
    drawn: false,
    handIndex: i,
    action: `dahai:${tile}`,
  }));
  if (seat.drawn !== null) {
    slots.push({
      index: slots.length,
      tile: seat.drawn,
      drawn: true,
      handIndex: null,
      action: `dahai:${seat.drawn}:tsumogiri`,
    });
  }
  return slots;
}

/**
 * 行動の表記 → 区画の番号。同じ牌が手に2枚以上あれば、左から最初の区画（REPLAY_UI §4.3）。
 * 見えない牌（`?`）の区画には結び付けない
 */
export function slotOfAction(slots: BandSlot[], action: string): number | null {
  const s = slots.find((x) => x.action === action && x.tile !== "?");
  return s ? s.index : null;
}

/** 区画の番号 → 行動の表記（ゲームモードで、押した区画の打牌を送るため） */
export function actionOfSlot(slots: BandSlot[], index: number): string | null {
  return slots[index]?.action ?? null;
}

export interface ChoiceView {
  action: string;
  /** 方策の確率（注釈に policy がなければ null） */
  p: number | null;
  /** 帯の区画（牌の区画に結び付かない行動は null） */
  slot: number | null;
  /** 実際に選んだ行動 */
  chosen: boolean;
  /** 一番確率の高い行動（同じ確率なら選択肢の並びで先のもの） */
  best: boolean;
}

export interface AnnotationLayout {
  seat: number;
  decision: string;
  model: string | null;
  value: number | null;
  choices: ChoiceView[];
  /** 帯の区画に出す選択肢（区画の番号 → 選択肢） */
  bySlot: Map<number, ChoiceView>;
  /** 区画に結び付かない選択肢（鳴き・立直・槓・北抜き・見送りなど）。選択肢の並び（DECISION_POINTS §5） */
  others: ChoiceView[];
  /** 一番確率の高い行動（policy がなければ null） */
  bestAction: string | null;
  /** 選んだ行動が一番確率の高い行動と違う（policy がなければ false） */
  differsFromBest: boolean;
}

/** 注釈を、その席の帯の区画と区画外の一覧に振り分ける */
export function layoutAnnotation(a: Annotation, seat: Pick<SeatView, "hand" | "drawn">): AnnotationLayout {
  const slots = bandSlots(seat);
  const probs = new Map((a.policy ?? []).map(([act, p]) => [act, p] as const));
  const acts = a.choices ?? (probs.size > 0 ? [...probs.keys()] : [a.decision]);
  if (!acts.includes(a.decision)) acts.push(a.decision);
  let bestAction: string | null = null;
  let bestP = -Infinity;
  for (const act of acts) {
    const p = probs.get(act);
    if (p !== undefined && p > bestP) {
      bestP = p;
      bestAction = act;
    }
  }
  const used = new Set<number>();
  const choices: ChoiceView[] = acts.map((action) => {
    let slot = action.startsWith("dahai:") ? slotOfAction(slots, action) : null;
    if (slot !== null && used.has(slot)) slot = null;
    if (slot !== null) used.add(slot);
    return {
      action,
      p: probs.get(action) ?? null,
      slot,
      chosen: action === a.decision,
      best: action === bestAction,
    };
  });
  const bySlot = new Map<number, ChoiceView>();
  for (const c of choices) if (c.slot !== null) bySlot.set(c.slot, c);
  return {
    seat: a.seat,
    decision: a.decision,
    model: a.model ?? null,
    value: a.value ?? null,
    choices,
    bySlot,
    others: choices.filter((c) => c.slot === null),
    bestAction,
    differsFromBest: bestAction !== null && bestAction !== a.decision && (probs.get(a.decision) ?? -1) < bestP,
  };
}

/** 注釈の牌譜が「確率どおりに抽選した」か「一番確率の高い行動だけを選んだ」か（REPLAY_UI §6.4） */
export interface SelectionMode {
  kind: "sample" | "argmax" | "unknown";
  /** 何から判断したか */
  source: "filename" | "model" | "inferred" | "none";
  /** 判断の根拠（画面に出す） */
  detail: string;
}

const ARGMAX_RE = /greedy|argmax|best[-_]?action|deterministic/i;
const SAMPLE_RE = /sampl|stochastic/i;

/**
 * 注釈に「選び方」のフィールドがない（LOG_FORMAT §6）ので、ファイル名 → model の名前 → 中身の順に推定する。
 * 中身からの推定：選んだ行動の確率が最大より小さい判断が1つでもあれば抽選。1つもなく、確率が一様でない判断が
 * 10 個以上あれば最大確率（推定）。それ以外は不明
 */
export function detectSelectionMode(list: Annotation[], fileName?: string | null): SelectionMode {
  if (fileName) {
    if (ARGMAX_RE.test(fileName)) return { kind: "argmax", source: "filename", detail: `ファイル名（${fileName}）` };
    if (SAMPLE_RE.test(fileName)) return { kind: "sample", source: "filename", detail: `ファイル名（${fileName}）` };
  }
  const models = [...new Set(list.map((a) => a.model).filter((m): m is string => !!m))];
  for (const m of models) {
    if (ARGMAX_RE.test(m)) return { kind: "argmax", source: "model", detail: `model（${m}）` };
    if (SAMPLE_RE.test(m)) return { kind: "sample", source: "model", detail: `model（${m}）` };
  }
  let withPolicy = 0;
  let differs = 0;
  let nonUniform = 0;
  for (const a of list) {
    if (!a.policy || a.policy.length < 2) continue;
    withPolicy++;
    const ps = a.policy.map(([, p]) => p);
    const max = Math.max(...ps);
    const min = Math.min(...ps);
    if (max - min > 1e-9) nonUniform++;
    const mine = a.policy.find(([act]) => act === a.decision)?.[1];
    if (mine !== undefined && mine < max - 1e-9) differs++;
  }
  if (withPolicy === 0) return { kind: "unknown", source: "none", detail: "注釈に確率がない" };
  if (differs > 0) {
    return { kind: "sample", source: "inferred", detail: `推定：最大確率でない行動を選んだ判断が ${differs} / ${withPolicy}` };
  }
  if (nonUniform >= 10) {
    return { kind: "argmax", source: "inferred", detail: `推定：${withPolicy} の判断すべてで最大確率の行動` };
  }
  return { kind: "unknown", source: "inferred", detail: `判断できない（確率のある判断 ${withPolicy}、一様でないもの ${nonUniform}）` };
}
