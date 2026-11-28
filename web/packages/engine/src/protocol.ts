// メインスレッド ↔ Worker のやり取り（id で突き合わせる）
export type Request =
  | { id: number; op: "load"; text: string }
  | { id: number; op: "kyoku_states"; kyoku_idx: number }
  | { id: number; op: "annotations"; text: string };

export type Response = { id: number; ok: true; json: string } | { id: number; ok: false; error: string };
