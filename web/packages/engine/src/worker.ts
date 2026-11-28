// Worker の中で mj-wasm を読み込み、牌譜の再生を行う（メインスレッドを止めない。docs/03_FRONTEND.md）。
import init, { Replay, parse_annotations } from "../pkg/mj_wasm.js";
import type { Request, Response } from "./protocol";

let ready: Promise<unknown> | null = null;
let replay: Replay | null = null;

self.onmessage = async (ev: MessageEvent<Request>) => {
  const req = ev.data;
  const reply = (r: Response) => (self as unknown as Worker).postMessage(r);
  try {
    ready ??= init();
    await ready;
    switch (req.op) {
      case "load":
        replay?.free();
        replay = new Replay(req.text);
        reply({ id: req.id, ok: true, json: replay.meta() });
        break;
      case "kyoku_states":
        if (!replay) throw new Error("no log loaded");
        reply({ id: req.id, ok: true, json: replay.kyoku_states(req.kyoku_idx) });
        break;
      case "annotations":
        reply({ id: req.id, ok: true, json: parse_annotations(req.text) });
        break;
    }
  } catch (e) {
    reply({ id: req.id, ok: false, error: String(e) });
  }
};
