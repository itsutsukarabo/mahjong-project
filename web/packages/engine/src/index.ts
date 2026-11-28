// mj-wasm を Worker で動かす。卓の状態を作るのは Rust（mj_log::view）で、ここは受け渡しだけ（原則1）。
import type { Annotation, ReplayMeta, TableView } from "@mj/log-types";
import type { Request, Response } from "./protocol";

type Pending = { resolve: (json: string) => void; reject: (e: Error) => void };
type Op = Request extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;

export class Engine {
  private worker: Worker;
  private next = 1;
  private pending = new Map<number, Pending>();

  constructor() {
    this.worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
    this.worker.onmessage = (ev: MessageEvent<Response>) => {
      const r = ev.data;
      const p = this.pending.get(r.id);
      if (!p) return;
      this.pending.delete(r.id);
      if (r.ok) p.resolve(r.json);
      else p.reject(new Error(r.error));
    };
  }

  private call(op: Op): Promise<string> {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...op, id } as Request);
    });
  }

  /** 牌譜（JSONL）を読み込み、局の一覧などを返す。読めなければ例外 */
  async load(text: string): Promise<ReplayMeta> {
    return JSON.parse(await this.call({ op: "load", text })) as ReplayMeta;
  }

  /** 1局の、各イベントを当てた直後の卓 */
  async kyokuStates(kyokuIdx: number): Promise<TableView[]> {
    return JSON.parse(await this.call({ op: "kyoku_states", kyoku_idx: kyokuIdx })) as TableView[];
  }

  /** 注釈（*.annot.jsonl）を読む */
  async annotations(text: string): Promise<Annotation[]> {
    return JSON.parse(await this.call({ op: "annotations", text })) as Annotation[];
  }

  terminate(): void {
    this.worker.terminate();
  }
}
