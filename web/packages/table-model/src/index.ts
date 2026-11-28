// 牌譜の再生の位置を持つ（どの局の何番目のイベントか）。卓の状態（TableView）は WASM が作ったものを受け取るだけで、
// ここでは和了判定・点数計算・合法手の判定をしない（原則1）。注釈は (kyoku_idx, event_idx, seat) で突き合わせる（CLAUDE.md）。
import type { Annotation, KyokuSummary, ReplayMeta, TableView } from "@mj/log-types";

/** 1局の卓の状態を返す（実際は Engine.kyokuStates） */
export type StatesLoader = (kyokuIdx: number) => Promise<TableView[]>;

export const annotationKey = (kyokuIdx: number, eventIdx: number, seat: number): string =>
  `${kyokuIdx}:${eventIdx}:${seat}`;

export class ReplayModel {
  readonly meta: ReplayMeta;
  private loader: StatesLoader;
  private cache = new Map<number, TableView[]>();
  private annots = new Map<string, Annotation>();
  /** meta.kyokus の何番目か */
  kyokuPos = 0;
  /** その局の何番目のイベントか（event_idx と同じ。読み込み時に確かめる） */
  eventIdx = 0;
  private states: TableView[] = [];

  private constructor(meta: ReplayMeta, loader: StatesLoader) {
    this.meta = meta;
    this.loader = loader;
  }

  static async open(meta: ReplayMeta, loader: StatesLoader): Promise<ReplayModel> {
    if (meta.kyokus.length === 0) throw new Error("the log has no kyoku");
    const m = new ReplayModel(meta, loader);
    await m.gotoKyoku(0);
    return m;
  }

  /** 注釈を読み込む。game_id が違う行・キーの重複は誤り */
  setAnnotations(list: Annotation[]): void {
    const map = new Map<string, Annotation>();
    for (const a of list) {
      if (a.game_id !== this.meta.game_id) {
        throw new Error(`annotation game_id ${a.game_id} differs from ${this.meta.game_id}`);
      }
      const k = annotationKey(a.kyoku_idx, a.event_idx, a.seat);
      if (map.has(k)) throw new Error(`duplicate annotation ${k}`);
      map.set(k, a);
    }
    this.annots = map;
  }

  get kyoku(): KyokuSummary {
    const k = this.meta.kyokus[this.kyokuPos];
    if (!k) throw new Error("no kyoku");
    return k;
  }

  get view(): TableView {
    const v = this.states[this.eventIdx];
    if (!v) throw new Error("no state");
    return v;
  }

  get length(): number {
    return this.states.length;
  }

  get maxJunme(): number {
    return this.states.reduce((m, s) => Math.max(m, s.junme), 1);
  }

  /** この時点（このイベントの直後）の判断ポイントの注釈（席の順） */
  get annotations(): Annotation[] {
    const v = this.view;
    const out: Annotation[] = [];
    for (let s = 0; s < this.meta.num_players; s++) {
      const a = this.annots.get(annotationKey(v.kyoku_idx, v.event_idx, s));
      if (a) out.push(a);
    }
    return out;
  }

  get atStart(): boolean {
    return this.kyokuPos === 0 && this.eventIdx === 0;
  }

  get atEnd(): boolean {
    return this.kyokuPos === this.meta.kyokus.length - 1 && this.eventIdx === this.states.length - 1;
  }

  async gotoKyoku(pos: number, eventIdx = 0): Promise<void> {
    const k = this.meta.kyokus[pos];
    if (!k) throw new Error(`no kyoku at ${pos}`);
    let states = this.cache.get(k.kyoku_idx);
    if (!states) {
      states = await this.loader(k.kyoku_idx);
      states.forEach((s, i) => {
        if (s.kyoku_idx !== k.kyoku_idx || s.event_idx !== i) {
          throw new Error(`state ${i} of kyoku ${k.kyoku_idx} has key (${s.kyoku_idx}, ${s.event_idx})`);
        }
      });
      this.cache.set(k.kyoku_idx, states);
    }
    this.kyokuPos = pos;
    this.states = states;
    this.eventIdx = Math.max(0, Math.min(eventIdx < 0 ? states.length + eventIdx : eventIdx, states.length - 1));
  }

  /** 1手進む（局の終わりなら次の局の始め）。進めなければ false */
  async next(): Promise<boolean> {
    if (this.eventIdx < this.states.length - 1) {
      this.eventIdx++;
      return true;
    }
    if (this.kyokuPos < this.meta.kyokus.length - 1) {
      await this.gotoKyoku(this.kyokuPos + 1, 0);
      return true;
    }
    return false;
  }

  /** 1手戻る（局の始めなら前の局の終わり） */
  async prev(): Promise<boolean> {
    if (this.eventIdx > 0) {
      this.eventIdx--;
      return true;
    }
    if (this.kyokuPos > 0) {
      await this.gotoKyoku(this.kyokuPos - 1, -1);
      return true;
    }
    return false;
  }

  /** その巡目の最初のツモ（なければその巡目以降で最初の時点）へ */
  seekJunme(j: number): void {
    const i = this.states.findIndex((s) => s.junme >= j && s.event_type === "tsumo");
    this.eventIdx = i >= 0 ? i : this.states.length - 1;
  }
}
