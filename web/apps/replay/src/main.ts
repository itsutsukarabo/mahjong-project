// 牌譜再生画面。牌譜を WASM（Worker）に渡して卓の状態を受け取り、2D レンダラーで描くだけ（原則1）。
// 配置・帯・アニメーション・結果の窓の決めごとは docs/spec/REPLAY_UI.md。
// URL の引数：log（牌譜の URL）、annot（注釈の URL）、kyoku（局の番号 0〜）、event（event_idx、負なら後ろから）、
// dev（0/1）、seat（下の席）、ai（0/1：AI の判断）、anim（0/1：アニメーション）、layout（auto/landscape/portrait）、speed（ms/手）
import { Engine } from "@mj/engine";
import type { TableView } from "@mj/log-types";
import {
  captureRects, chooseLayout, computeMetrics, css, ensureTileSprite, type LayoutKind, playCues, render, renderSide, sideCss,
} from "@mj/render-2d";
import { ReplayModel, type SelectionMode, cuesBetween, detectSelectionMode, layoutAnnotation } from "@mj/table-model";

const FIXTURES: [string, string, string | null][] = [
  ["サンプル（全情報版、1局）", "fixtures/sample.mjlog.jsonl", "fixtures/sample.annot.jsonl"],
  ["サンプル（公開情報版）", "fixtures/sample.public.mjlog.jsonl", null],
  ["RuleBot 同士の半荘（全情報版＋注釈）", "fixtures/rulebot.mjlog.jsonl", "fixtures/rulebot.annot.jsonl"],
  ["RuleBot 同士の半荘（公開情報版）", "fixtures/rulebot.public.mjlog.jsonl", null],
  ["mj-py の自己対戦（一様な乱数の行動）", "fixtures/mjpy-selfplay.mjlog.jsonl", null],
  ["学習の自己対戦（s1a u157、抽選＋注釈）", "fixtures/rl-u157.mjlog.jsonl", "fixtures/rl-u157.annot.jsonl"],
  ["四麻（ゴールデンの yonma-hanchan-1、チー・槓あり）", "fixtures/yonma.mjlog.jsonl", null],
];
const SPEEDS: [number, string][] = [
  [1000, "1 手/秒"],
  [400, "2.5 手/秒"],
  [150, "6.7 手/秒"],
  [40, "25 手/秒"],
];
/** 手動で1手進めたときの、牌が動く時間 */
const MOVE_MS = 240;

const $ = <T extends HTMLElement>(id: string): T => {
  const e = document.getElementById(id);
  if (!e) throw new Error(`#${id} missing`);
  return e as T;
};

const style = document.createElement("style");
style.textContent = css + sideCss;
document.head.append(style);
ensureTileSprite();

const engine = new Engine();
const params = new URLSearchParams(location.search);
let model: ReplayModel | null = null;
let mode: SelectionMode | null = null;
let annotLoaded = false;
let timer: number | null = null;
/** 閉じた結果の窓（局・和了の数・流局で区別する） */
let closedResult: string | null = null;
let closedGameEnd = false;
/** 計測（テスト用）：描画にかかった時間（ms）と、最後に動かしたアニメーションの数 */
const perf = { renders: 0, totalMs: 0, maxMs: 0, lastAnims: 0 };

const speed = () => Number($<HTMLSelectElement>("speed").value || 400);
const animOn = () => $<HTMLInputElement>("anim").checked;
const viewSeat = () => Number($<HTMLSelectElement>("viewSeat").value || 0);
const resultKey = (v: TableView) => (v.result ? `${v.kyoku_idx}:${v.result.horas.length}:${v.result.ryukyoku ? 1 : 0}` : null);
const resultVisible = () => {
  if (!model) return false;
  const k = resultKey(model.view);
  return k !== null && k !== closedResult;
};
const gameEndVisible = () => !!model && model.atEnd && !!model.meta.game_end && !closedGameEnd;

function showError(e: unknown): void {
  $("error").textContent = e ? String(e instanceof Error ? e.message : e) : "";
}

function layoutKind(): LayoutKind {
  const v = $<HTMLSelectElement>("layout").value;
  if (v === "landscape" || v === "portrait") return v;
  return chooseLayout(window.innerWidth, window.innerHeight);
}

/** 描く。animate なら、前の時点からの差でアニメーションする */
function draw(animate = false): void {
  if (!model) return;
  const t0 = performance.now();
  const v = model.view;
  const kind = layoutKind();
  document.body.dataset.layout = kind;
  if (kind === "landscape" && window.innerWidth < 1200) document.body.dataset.narrow = "1";
  else delete document.body.dataset.narrow;
  const host = $("table");
  const metrics = computeMetrics(host.clientWidth, host.clientHeight, model.meta.num_players, kind);
  const seat = viewSeat();
  const showAi = $<HTMLInputElement>("ai").checked;
  const here = model.annotations;
  const mine = here.find((a) => a.seat === seat);
  const s = v.seats[seat];
  const before = animate && animOn() ? captureRects(host) : null;
  render(host, v, {
    devMode: $<HTMLInputElement>("dev").checked,
    viewSeat: seat,
    metrics,
    meta: model.meta,
    ai: {
      show: showAi,
      loaded: annotLoaded,
      annotation: mine && s ? layoutAnnotation(mine, s) : null,
      mode,
      otherSeats: here.filter((a) => a.seat !== seat).map((a) => a.seat),
    },
    showResult: resultVisible(),
    showGameEnd: gameEndVisible(),
    onCloseResult: () => {
      if (gameEndVisible()) closedGameEnd = true;
      else closedResult = resultKey(model!.view);
      draw();
    },
    onSwitchSeat: (x) => {
      $<HTMLSelectElement>("viewSeat").value = String(x);
      draw();
    },
  });
  if (before) {
    const d = timer !== null ? Math.min(MOVE_MS, Math.round(speed() * 0.6)) : MOVE_MS;
    perf.lastAnims = playCues(host, before, cuesBetween(model.previous, v), { duration: d });
  } else perf.lastAnims = 0;
  if (kind === "landscape") renderSide($("side"), here, v, annotLoaded ? mode : null, showAi);
  $<HTMLSelectElement>("kyoku").value = String(model.kyokuPos);
  for (const [id, label] of [["junme", "junmeLabel"], ["junmeMenu", "junmeMenuLabel"]] as const) {
    const j = $<HTMLInputElement>(id);
    j.max = String(model.maxJunme);
    j.value = String(v.junme);
    $(label).textContent = `${v.junme}/${model.maxJunme}`;
  }
  const k = model.kyoku;
  $("status").textContent = `局 ${model.kyokuPos + 1}/${model.meta.kyokus.length}　イベント ${v.event_idx + 1}/${model.length}（#${v.event_idx} ${v.event_type}）　${k.end}`;
  $("info").textContent = `${model.meta.game_id}（${model.meta.visibility}）${mode ? `　選び方：${mode.detail}` : ""}`;
  document.body.dataset.kyoku = String(v.kyoku_idx);
  document.body.dataset.event = String(v.event_idx);
  const ms = performance.now() - t0;
  perf.renders++;
  perf.totalMs += ms;
  perf.maxMs = Math.max(perf.maxMs, ms);
}

function stop(): void {
  if (timer !== null) window.clearTimeout(timer);
  timer = null;
  $("play").textContent = "▶ 再生";
}

/** 自動再生：結果の窓が開いたら、しばらく止めてから閉じて進む */
function schedule(): void {
  const hold = Math.min(4000, Math.max(1000, speed() * 8));
  timer = window.setTimeout(async () => {
    if (!model) return stop();
    if (gameEndVisible()) return stop();
    if (resultVisible()) closedResult = resultKey(model.view);
    try {
      const moved = await model.next();
      draw(moved);
      if (!moved && !gameEndVisible()) return stop();
    } catch (e) {
      showError(e);
      return stop();
    }
    schedule();
  }, resultVisible() ? hold : speed());
}

function play(): void {
  if (timer !== null) return stop();
  $("play").textContent = "⏸ 停止";
  schedule();
}

async function step(f: (m: ReplayModel) => Promise<unknown> | unknown, animate = false): Promise<void> {
  if (!model) return;
  try {
    await f(model);
    draw(animate);
  } catch (e) {
    showError(e);
  }
}

const forward = () => step(async (m) => m.next(), true);
const backward = () => step((m) => m.prev());

async function openLog(text: string, annot: string | null, annotName: string | null): Promise<void> {
  stop();
  showError(null);
  const meta = await engine.load(text);
  const m = await ReplayModel.open(meta, (k) => engine.kyokuStates(k));
  annotLoaded = false;
  mode = null;
  if (annot) {
    m.setAnnotations(await engine.annotations(annot));
    annotLoaded = true;
    mode = detectSelectionMode(m.allAnnotations, annotName);
  }
  model = m;
  closedResult = null;
  closedGameEnd = false;
  $<HTMLSelectElement>("kyoku").replaceChildren(
    ...meta.kyokus.map((k, i) => new Option(`${i + 1}. ${k.bakaze === "E" ? "東" : k.bakaze === "S" ? "南" : k.bakaze}${k.kyoku}局 ${k.honba}本場（${k.end}）`, String(i))),
  );
  const vs = $<HTMLSelectElement>("viewSeat");
  vs.replaceChildren(...meta.players.map((p, i) => new Option(`視点 ${i}: ${p}`, String(i))));
  vs.value = params.get("seat") ?? "0";
  draw();
}

async function openUrl(log: string, annot: string | null): Promise<void> {
  const text = await (await fetch(log)).text();
  const a = annot ? await (await fetch(annot)).text() : null;
  await openLog(text, a, annot);
}

function setup(): void {
  const fx = $<HTMLSelectElement>("fixture");
  fx.replaceChildren(new Option("サンプルを選ぶ…", ""), ...FIXTURES.map(([name], i) => new Option(name, String(i))));
  fx.onchange = () => {
    const f = FIXTURES[Number(fx.value)];
    if (f) openUrl(f[1], f[2]).catch(showError);
  };
  for (const id of ["speed", "speedMenu"]) {
    const sel = $<HTMLSelectElement>(id);
    sel.replaceChildren(...SPEEDS.map(([ms, label]) => new Option(label, String(ms))));
    sel.value = params.get("speed") ?? "400";
    sel.onchange = () => {
      for (const other of ["speed", "speedMenu"]) $<HTMLSelectElement>(other).value = sel.value;
      if (timer !== null) {
        stop();
        play();
      }
    };
  }
  $<HTMLInputElement>("logFile").onchange = async (ev) => {
    const file = (ev.target as HTMLInputElement).files?.[0];
    if (file) await openLog(await file.text(), null, null).catch(showError);
  };
  $<HTMLInputElement>("annotFile").onchange = async (ev) => {
    const file = (ev.target as HTMLInputElement).files?.[0];
    if (!file || !model) return;
    try {
      model.setAnnotations(await engine.annotations(await file.text()));
      annotLoaded = true;
      mode = detectSelectionMode(model.allAnnotations, file.name);
      draw();
    } catch (e) {
      showError(e);
    }
  };
  $("next").onclick = () => void forward();
  $("prev").onclick = () => void backward();
  $("nextKyoku").onclick = () => step((m) => m.kyokuPos + 1 < m.meta.kyokus.length && m.gotoKyoku(m.kyokuPos + 1));
  $("prevKyoku").onclick = () => step((m) => m.gotoKyoku(Math.max(0, m.kyokuPos - 1)));
  $<HTMLSelectElement>("kyoku").onchange = (ev) => step((m) => m.gotoKyoku(Number((ev.target as HTMLSelectElement).value)));
  for (const id of ["junme", "junmeMenu"]) {
    $<HTMLInputElement>(id).oninput = (ev) => step((m) => m.seekJunme(Number((ev.target as HTMLInputElement).value)));
  }
  for (const id of ["dev", "ai", "anim", "viewSeat", "layout"]) $(id).onchange = () => draw();
  $("play").onclick = () => play();
  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
    if (e.key === "ArrowRight") void forward();
    else if (e.key === "ArrowLeft") void backward();
    else if (e.key === " ") {
      e.preventDefault();
      play();
    } else if (e.key === "Escape" && (resultVisible() || gameEndVisible())) {
      if (gameEndVisible()) closedGameEnd = true;
      else if (model) closedResult = resultKey(model.view);
      draw();
    }
  });
  // スマホ：左右のスワイプで1手進む・戻る（左へはらう＝進む）
  let start: { x: number; y: number; t: number } | null = null;
  const host = $("table");
  host.addEventListener("pointerdown", (e) => {
    start = { x: e.clientX, y: e.clientY, t: e.timeStamp };
  });
  host.addEventListener("pointerup", (e) => {
    if (!start) return;
    const dx = e.clientX - start.x;
    const dy = e.clientY - start.y;
    const quick = e.timeStamp - start.t < 800;
    start = null;
    if (quick && Math.abs(dx) > 40 && Math.abs(dx) > 1.5 * Math.abs(dy)) {
      if (dx < 0) void forward();
      else void backward();
    }
  });
  let resizeTimer: number | null = null;
  window.addEventListener("resize", () => {
    if (resizeTimer !== null) window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => draw(), 60);
  });
  $<HTMLInputElement>("dev").checked = params.get("dev") !== "0";
  $<HTMLInputElement>("ai").checked = params.get("ai") !== "0";
  $<HTMLInputElement>("anim").checked = params.get("anim") !== "0";
  const lay = params.get("layout");
  if (lay === "landscape" || lay === "portrait") $<HTMLSelectElement>("layout").value = lay;
}

/** テスト（Playwright）用の入口 */
declare global {
  interface Window {
    __replay: {
      ready: Promise<void>;
      next(): Promise<boolean>;
      prev(): Promise<boolean>;
      goto(kyokuPos: number, eventIdx: number): Promise<void>;
      position(): { kyoku: number; event: number } | null;
      view(): TableView | null;
      perf: typeof perf;
      play(): void;
      stop(): void;
      playing(): boolean;
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
    draw(moved);
    return moved;
  },
  async prev() {
    if (!model) return false;
    const moved = await model.prev();
    draw();
    return moved;
  },
  async goto(kyokuPos, eventIdx) {
    if (!model) return;
    await model.gotoKyoku(kyokuPos, eventIdx);
    draw();
  },
  position: () => (model ? { kyoku: model.view.kyoku_idx, event: model.view.event_idx } : null),
  view: () => (model ? model.view : null),
  perf,
  play,
  stop,
  playing: () => timer !== null,
};
