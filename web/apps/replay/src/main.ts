// 牌譜再生画面。牌譜を WASM（Worker）に渡して卓の状態を受け取り、2D レンダラーで描くだけ（原則1）。
// URL の引数：log（牌譜の URL）、annot（注釈の URL）、kyoku（局の番号 0〜）、event（event_idx）、dev（0/1）、seat（下の席）
import { Engine } from "@mj/engine";
import { css, render, renderAnnotations } from "@mj/render-2d";
import { ReplayModel } from "@mj/table-model";

const FIXTURES: [string, string, string | null][] = [
  ["サンプル（全情報版、1局）", "fixtures/sample.mjlog.jsonl", "fixtures/sample.annot.jsonl"],
  ["サンプル（公開情報版）", "fixtures/sample.public.mjlog.jsonl", null],
  ["RuleBot 同士の半荘（全情報版＋注釈）", "fixtures/rulebot.mjlog.jsonl", "fixtures/rulebot.annot.jsonl"],
  ["RuleBot 同士の半荘（公開情報版）", "fixtures/rulebot.public.mjlog.jsonl", null],
  ["mj-py の自己対戦（一様な乱数の行動）", "fixtures/mjpy-selfplay.mjlog.jsonl", null],
];

const $ = <T extends HTMLElement>(id: string): T => {
  const e = document.getElementById(id);
  if (!e) throw new Error(`#${id} missing`);
  return e as T;
};

const style = document.createElement("style");
style.textContent = css;
document.head.append(style);

const engine = new Engine();
let model: ReplayModel | null = null;
let timer: number | null = null;
const params = new URLSearchParams(location.search);

function showError(e: unknown): void {
  $("error").textContent = e ? String(e instanceof Error ? e.message : e) : "";
}

function draw(): void {
  if (!model) return;
  const v = model.view;
  const devMode = $<HTMLInputElement>("dev").checked;
  const viewSeat = Number($<HTMLSelectElement>("viewSeat").value || 0);
  render($("table"), v, { devMode, viewSeat });
  renderAnnotations($("side"), model.annotations, model.meta.players);
  $<HTMLSelectElement>("kyoku").value = String(model.kyokuPos);
  const j = $<HTMLInputElement>("junme");
  j.max = String(model.maxJunme);
  j.value = String(v.junme);
  $("junmeLabel").textContent = `${v.junme} / ${model.maxJunme}`;
  const k = model.kyoku;
  $("status").textContent =
    `${model.meta.game_id}（${model.meta.visibility}）　局 ${model.kyokuPos + 1} / ${model.meta.kyokus.length}` +
    `　イベント ${v.event_idx + 1} / ${model.length}（${k.end}）`;
  document.body.dataset.kyoku = String(v.kyoku_idx);
  document.body.dataset.event = String(v.event_idx);
}

function stop(): void {
  if (timer !== null) window.clearInterval(timer);
  timer = null;
  $("play").textContent = "▶ 再生";
}

async function step(f: (m: ReplayModel) => Promise<unknown> | unknown): Promise<void> {
  if (!model) return;
  try {
    await f(model);
    draw();
  } catch (e) {
    showError(e);
  }
}

async function openLog(text: string, annot: string | null): Promise<void> {
  stop();
  showError(null);
  const meta = await engine.load(text);
  const m = await ReplayModel.open(meta, (k) => engine.kyokuStates(k));
  if (annot) m.setAnnotations(await engine.annotations(annot));
  model = m;
  $<HTMLSelectElement>("kyoku").replaceChildren(
    ...meta.kyokus.map((k, i) => new Option(`${i + 1}. ${k.bakaze}${k.kyoku}局 ${k.honba}本場（${k.end}）`, String(i))),
  );
  const vs = $<HTMLSelectElement>("viewSeat");
  vs.replaceChildren(...meta.players.map((p, i) => new Option(`${i}: ${p}`, String(i))));
  vs.value = params.get("seat") ?? "0";
  draw();
}

async function openUrl(log: string, annot: string | null): Promise<void> {
  const text = await (await fetch(log)).text();
  const a = annot ? await (await fetch(annot)).text() : null;
  await openLog(text, a);
}

function setup(): void {
  const fx = $<HTMLSelectElement>("fixture");
  fx.replaceChildren(new Option("サンプルを選ぶ…", ""), ...FIXTURES.map(([name], i) => new Option(name, String(i))));
  fx.onchange = () => {
    const f = FIXTURES[Number(fx.value)];
    if (f) openUrl(f[1], f[2]).catch(showError);
  };
  $<HTMLInputElement>("logFile").onchange = async (ev) => {
    const file = (ev.target as HTMLInputElement).files?.[0];
    if (file) await openLog(await file.text(), null).catch(showError);
  };
  $<HTMLInputElement>("annotFile").onchange = async (ev) => {
    const file = (ev.target as HTMLInputElement).files?.[0];
    if (!file || !model) return;
    try {
      model.setAnnotations(await engine.annotations(await file.text()));
      draw();
    } catch (e) {
      showError(e);
    }
  };
  $("next").onclick = () => step((m) => m.next());
  $("prev").onclick = () => step((m) => m.prev());
  $("nextKyoku").onclick = () => step((m) => m.kyokuPos + 1 < m.meta.kyokus.length && m.gotoKyoku(m.kyokuPos + 1));
  $("prevKyoku").onclick = () => step((m) => m.gotoKyoku(Math.max(0, m.kyokuPos - 1)));
  $<HTMLSelectElement>("kyoku").onchange = (ev) => step((m) => m.gotoKyoku(Number((ev.target as HTMLSelectElement).value)));
  $<HTMLInputElement>("junme").oninput = (ev) => step((m) => m.seekJunme(Number((ev.target as HTMLInputElement).value)));
  $("dev").onchange = () => draw();
  $("viewSeat").onchange = () => draw();
  $("play").onclick = () => {
    if (timer !== null) return stop();
    $("play").textContent = "⏸ 停止";
    timer = window.setInterval(() => {
      void step(async (m) => {
        if (!(await m.next())) stop();
      });
    }, Number($<HTMLSelectElement>("speed").value));
  };
  $("speed").onchange = () => {
    if (timer !== null) {
      stop();
      $("play").click();
    }
  };
  window.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") $("next").click();
    if (e.key === "ArrowLeft") $("prev").click();
  });
  $<HTMLInputElement>("dev").checked = params.get("dev") !== "0";
}

/** テスト（Playwright）用の入口：最後まで1手ずつ進め、各時点の描画を確かめる */
declare global {
  interface Window {
    __replay: {
      ready: Promise<void>;
      next(): Promise<boolean>;
      position(): { kyoku: number; event: number } | null;
    };
  }
}

setup();
const ready = (async () => {
  const log = params.get("log");
  if (!log) return;
  await openUrl(log, params.get("annot"));
  const k = params.get("kyoku");
  const e = params.get("event");
  // openUrl の中で model が入る（TS は関数呼び出しの先の代入を追えないので、型を付け直して読む）
  const m = model as ReplayModel | null;
  if (m && k !== null) {
    await m.gotoKyoku(Number(k), e === null ? 0 : Number(e));
    draw();
  }
})().catch((e) => {
  showError(e);
  throw e;
});
window.__replay = {
  ready,
  async next() {
    if (!model) return false;
    const moved = await model.next();
    draw();
    return moved;
  },
  position: () => (model ? { kyoku: model.view.kyoku_idx, event: model.view.event_idx } : null),
};
