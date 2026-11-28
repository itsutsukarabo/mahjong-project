// 牌譜再生画面のテスト（PC の横画面・スマホの縦画面の2つの project で回す。playwright.config.ts）：
// 1. サンプル・ボット対局・mj-py の自己対戦・学習の自己対戦・四麻の牌譜を最後まで1手ずつ再生し、各時点で
//    例外が出ない、各席の「手牌（ツモ牌を含む） ＋ 3 × 副露」が 13 か 14、牌が画面の枠からはみ出さない、
//    手牌の下の帯の区画と手牌の各牌の左右がそろう、を確かめる
// 2. 帯の区画と牌の対応（全席を視点にして、ツモ牌・副露・抜いた北・手牌の枚数が変わる場面を含む）
// 3. AI の判断の表示（区画ごとの確率・区画外の一覧・選んだ行動と最大確率の違い・選び方）
// 4. アニメーション（入れると動く、切ると何も動かない）
// 5. 決まった時点の画面のスナップショット（アニメーションを切って撮る。ubuntu の CI でだけ比べる）。
//    スナップショットは CI の「web-snapshots」ワークフロー（--update-snapshots）で撮り、成果物を取り込んでコミットする。
//    まだコミットされていないスナップショットの比較は飛ばす（UPDATE_SNAPSHOTS=1 のときは撮る）
// 6. アニメーションの性能（PERF=1 のときだけ。docs/spec/REPLAY_UI.md §10）
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { type Page, expect, test } from "@playwright/test";

const LOGS: [string, string | null][] = [
  ["fixtures/sample.mjlog.jsonl", "fixtures/sample.annot.jsonl"],
  ["fixtures/sample.public.mjlog.jsonl", null],
  ["fixtures/rulebot.mjlog.jsonl", "fixtures/rulebot.annot.jsonl"],
  ["fixtures/rulebot.public.mjlog.jsonl", null],
  ["fixtures/rulebot-v04.mjlog.jsonl", null],
  ["fixtures/mjpy-selfplay.mjlog.jsonl", null],
  ["fixtures/rl-u157.mjlog.jsonl", "fixtures/rl-u157.annot.jsonl"],
  ["fixtures/yonma.mjlog.jsonl", null],
];

async function open(page: Page, query: string): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto(`/?${query}`);
  await page.evaluate(() => window.__replay.ready);
  await expect(page.locator("#error")).toHaveText("");
  return errors;
}

/** 画面の中で確かめる（page.evaluate に渡す。牌の枚数・はみ出し・帯の区画） */
function installChecks(page: Page): Promise<void> {
  return page.evaluate(() => {
    (window as unknown as { __check: () => string[] }).__check = () => {
      const bad: string[] = [];
      const pos = JSON.stringify(window.__replay.position());
      const stage = document.querySelector<HTMLElement>(".stage");
      if (!stage) return [`${pos}: no stage`];
      const sr = stage.getBoundingClientRect();
      const vw = document.documentElement.clientWidth;
      if (sr.right > vw + 1) bad.push(`${pos}: stage wider than the viewport (${sr.right} > ${vw})`);
      // 牌の枚数
      for (const seat of document.querySelectorAll<HTMLElement>(".seat[data-seat]")) {
        const hand = seat.querySelectorAll("[data-part=hand] .tile").length;
        const melds = seat.querySelectorAll("[data-part=melds] .meld").length;
        const n = hand + 3 * melds;
        if (n !== 13 && n !== 14) bad.push(`${pos} seat ${seat.dataset.seat}: ${n} tiles`);
      }
      // はみ出し（結果の窓と、横に送れる一覧の中は除く）
      for (const t of stage.querySelectorAll<HTMLElement>(".tile")) {
        if (t.closest(".result, .ai-list")) continue;
        const r = t.getBoundingClientRect();
        if (r.left < sr.left - 1 || r.right > sr.right + 1 || r.top < sr.top - 1 || r.bottom > sr.bottom + 1) {
          bad.push(`${pos}: tile ${t.dataset.key ?? t.dataset.tile} outside the stage`);
          break;
        }
      }
      // 帯の区画と、その真上の牌の左右がそろう
      const slots = [...stage.querySelectorAll<HTMLElement>(".self .slot")];
      const v = window.__replay.view();
      const seatNo = Number(stage.querySelector<HTMLElement>(".self")?.dataset.seat ?? 0);
      const sv = v?.seats[seatNo];
      if (sv && slots.length !== sv.hand.length + (sv.drawn === null ? 0 : 1)) bad.push(`${pos}: ${slots.length} slots`);
      slots.forEach((s, i) => {
        const tile = s.querySelector(".tile")!.getBoundingClientRect();
        const cell = s.querySelector(".band-cell")!.getBoundingClientRect();
        if (Number(s.dataset.slot) !== i) bad.push(`${pos}: slot ${i} numbered ${s.dataset.slot}`);
        if (Math.abs(tile.left - cell.left) > 0.5 || Math.abs(tile.right - cell.right) > 0.5 || cell.top < tile.bottom - 0.5) {
          bad.push(`${pos}: slot ${i} not under its tile`);
        }
      });
      return bad;
    };
  });
}

for (const [log, annot] of LOGS) {
  test(`plays ${log} to the end without layout breaks`, async ({ page }) => {
    const errors = await open(page, `log=${log}${annot ? `&annot=${annot}` : ""}&anim=0`);
    await installChecks(page);
    const r = await page.evaluate(async () => {
      const check = (window as unknown as { __check: () => string[] }).__check;
      const bad: string[] = [...check()];
      let steps = 0;
      while (await window.__replay.next()) {
        steps++;
        bad.push(...check());
        if (bad.length > 20) break;
      }
      return { steps, bad };
    });
    expect(r.bad).toEqual([]);
    expect(r.steps).toBeGreaterThan(10);
    expect(errors).toEqual([]);
  });
}

test("band slots line up with the hand for every seat (calls, kakan, kita, drawn tile)", async ({ page }) => {
  for (const [log, kyoku, seats] of [
    ["fixtures/yonma.mjlog.jsonl", 5, 4],
    ["fixtures/rulebot.mjlog.jsonl", 0, 3],
  ] as const) {
    for (let seat = 0; seat < seats; seat++) {
      await open(page, `log=${log}&anim=0&seat=${seat}&kyoku=${kyoku}&event=0`);
      await installChecks(page);
      const r = await page.evaluate(async (k) => {
        const check = (window as unknown as { __check: () => string[] }).__check;
        const bad: string[] = [];
        const sizes = new Set<number>();
        do {
          const p = window.__replay.position()!;
          if (p.kyoku !== k) break;
          bad.push(...check());
          sizes.add(document.querySelectorAll(".self .slot").length);
        } while (await window.__replay.next());
        return { bad, sizes: [...sizes] };
      }, kyoku);
      expect(r.bad).toEqual([]);
      // 手牌の枚数が変わる場面（副露で減る・ツモ牌で増える）を通っている
      expect(r.sizes.length).toBeGreaterThan(1);
    }
  }
});

test("AI annotations go under their tiles and into the list", async ({ page }) => {
  await open(page, "log=fixtures/rl-u157.mjlog.jsonl&annot=fixtures/rl-u157.annot.jsonl&kyoku=0&event=1&anim=0");
  // 席0 の第1打：12 択のうち、同じ牌の2枚目（3s・E）を除く区画に確率が出る
  const cells = page.locator(".self .band-cell[data-action]");
  expect(await cells.count()).toBeGreaterThan(8);
  await expect(page.locator(".self .band-chosen")).toHaveCount(1);
  await expect(page.locator(".self .band-chosen")).toHaveAttribute("data-action", "dahai:F");
  await expect(page.locator(".self .band-best")).toHaveAttribute("data-action", "dahai:9m");
  await expect(page.locator(".differs")).toBeVisible();
  await expect(page.locator(".mode")).toHaveText("抽選");
  await expect(page.locator(".value-num")).toHaveText("+0.381");
  // 区画の確率の行動と、真上の牌の行動が同じ
  const mismatched = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>(".self .band-cell[data-action]")]
      .filter((c) => c.closest<HTMLElement>(".slot")?.dataset.action !== c.dataset.action)
      .map((c) => c.dataset.action),
  );
  expect(mismatched).toEqual([]);
  // 鳴きの判断（区画に結び付かない）：一覧に出る
  await open(page, "log=fixtures/sample.mjlog.jsonl&annot=fixtures/sample.annot.jsonl&kyoku=0&event=8&anim=0");
  await expect(page.locator(".ai-list .chip")).toHaveCount(2);
  await expect(page.locator(".ai-list .chip-chosen")).toHaveAttribute("data-action", "pon:C,C");
  // 他の席の判断：視点を切り替えると、その席の帯に出る
  await open(page, "log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&kyoku=0&event=31&anim=0&seat=0");
  await page.locator(".other-seat[data-seat='1']").click();
  await expect(page.locator(".self")).toHaveAttribute("data-seat", "1");
  expect(await page.locator(".self .band-cell[data-action]").count()).toBeGreaterThan(5);
  // 表示を切る
  await open(page, "log=fixtures/rl-u157.mjlog.jsonl&annot=fixtures/rl-u157.annot.jsonl&kyoku=0&event=1&anim=0&ai=0");
  await expect(page.locator(".self .band-cell[data-action]")).toHaveCount(0);
});

test("animations run when enabled and nothing animates when disabled", async ({ page }) => {
  await open(page, "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=7");
  // 次は席0 のポン（event 9 は pon。8 は dahai）
  const moved = await page.evaluate(async () => {
    const out: { anims: number; shout: boolean; type: string }[] = [];
    for (let i = 0; i < 3; i++) {
      await window.__replay.next();
      out.push({ anims: window.__replay.perf.lastAnims, shout: !!document.querySelector(".shout"), type: window.__replay.view()!.event_type });
    }
    return out;
  });
  expect(moved.some((m) => m.anims > 0)).toBe(true);
  expect(moved.some((m) => m.type === "pon" && m.shout)).toBe(true);
  await open(page, "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=7&anim=0");
  const still = await page.evaluate(async () => {
    for (let i = 0; i < 3; i++) await window.__replay.next();
    return { anims: window.__replay.perf.lastAnims, running: document.getAnimations().length, shout: !!document.querySelector(".shout") };
  });
  expect(still).toEqual({ anims: 0, running: 0, shout: false });
});

test("result window closes and autoplay holds then continues", async ({ page }) => {
  await open(page, "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=21&anim=0");
  await expect(page.locator("[data-part=result]")).toBeVisible();
  await page.locator(".result-close").click();
  await expect(page.locator("[data-part=result]")).toHaveCount(0);
  // 自動再生（最速）：和了の窓で一度止まり、自動で閉じて対局の終わりまで進む
  await open(page, "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=19&anim=0&speed=40");
  await page.evaluate(() => window.__replay.play());
  await expect(page.locator("[data-part=result]")).toBeVisible();
  await expect(page.locator("[data-part=game-end]")).toBeVisible({ timeout: 10_000 });
  expect(await page.evaluate(() => window.__replay.playing())).toBe(false);
});

test("result window names the hand from hora.limit, and from han only in old logs", async ({ page }) => {
  // LOG_FORMAT v0.5（limit あり）：6翻 → 跳満（limit = haneman）、4翻20符 → 名前なし（limit = none）
  await open(page, "log=fixtures/rulebot.mjlog.jsonl&kyoku=0&event=82&anim=0");
  await expect(page.locator(".result-limit")).toHaveText("跳満");
  await expect(page.locator(".result-points")).toHaveText("12,000点");
  // 窓の中身が横にはみ出さない（スマホの縦画面を含む）
  const fits = await page.evaluate(() => {
    const r = document.querySelector<HTMLElement>("[data-part=result]")!;
    const box = r.getBoundingClientRect();
    return [...r.querySelectorAll(".tile")].every((t) => t.getBoundingClientRect().right <= box.right - 4);
  });
  expect(fits).toBe(true);
  await open(page, "log=fixtures/rulebot.mjlog.jsonl&kyoku=4&event=102&anim=0");
  await expect(page.locator("[data-part=result]")).toBeVisible();
  await expect(page.locator(".result-limit")).toHaveCount(0);
  // v0.4（limit なし）：翻数だけで決まるもの（5翻以上）だけ名前を出す
  await open(page, "log=fixtures/rulebot-v04.mjlog.jsonl&kyoku=3&event=94&anim=0");
  await expect(page.locator(".result-limit")).toHaveText("満貫");
  await open(page, "log=fixtures/rulebot-v04.mjlog.jsonl&kyoku=4&event=102&anim=0");
  await expect(page.locator("[data-part=result]")).toBeVisible();
  await expect(page.locator(".result-limit")).toHaveCount(0);
});

test("kita is shown as one tile with its count", async ({ page }) => {
  await open(page, "log=fixtures/rulebot.mjlog.jsonl&kyoku=0&event=60&anim=0&seat=1");
  const kita = page.locator(".self [data-part=kita]");
  await expect(kita.locator(".tile")).toHaveCount(1);
  const n = Number(await kita.getAttribute("data-count"));
  expect(n).toBeGreaterThan(1);
  await expect(kita.locator(".kita-count")).toHaveText(`×${n}`);
});

const SNAPSHOTS: [string, string][] = [
  ["sample-start", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=0"],
  ["sample-pon-decision", "log=fixtures/sample.mjlog.jsonl&annot=fixtures/sample.annot.jsonl&kyoku=0&event=8"],
  ["sample-pon", "log=fixtures/sample.mjlog.jsonl&annot=fixtures/sample.annot.jsonl&kyoku=0&event=9"],
  ["sample-riichi", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=14"],
  ["sample-hora", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=21"],
  ["sample-public", "log=fixtures/sample.public.mjlog.jsonl&kyoku=0&event=14&dev=0"],
  ["rulebot-kita", "log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&kyoku=0&event=4&seat=1"],
  ["rulebot-annotated", "log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&kyoku=0&event=31&seat=1"],
  ["rulebot-game-end", "log=fixtures/rulebot.mjlog.jsonl&kyoku=9&event=-1"],
  ["rulebot-haneman", "log=fixtures/rulebot.mjlog.jsonl&kyoku=0&event=82"],
  ["rl-annotated", "log=fixtures/rl-u157.mjlog.jsonl&annot=fixtures/rl-u157.annot.jsonl&kyoku=0&event=1"],
  ["rl-ryukyoku", "log=fixtures/rl-u157.mjlog.jsonl&annot=fixtures/rl-u157.annot.jsonl&kyoku=0&event=-2"],
  ["yonma-calls", "log=fixtures/yonma.mjlog.jsonl&kyoku=5&event=92"],
];

for (const [name, query] of SNAPSHOTS) {
  test(`screenshot ${name}`, async ({ page }, info) => {
    test.skip(process.platform !== "linux", "snapshots are taken on the ubuntu CI");
    const file = fileURLToPath(new URL(`./__screenshots__/${name}-${info.project.name}.png`, import.meta.url));
    test.skip(!existsSync(file) && !process.env.UPDATE_SNAPSHOTS, `no committed snapshot ${name}-${info.project.name}.png yet`);
    await open(page, `${query}&anim=0`);
    await expect(page).toHaveScreenshot(`${name}.png`);
  });
}

test("animation performance (PERF=1)", async ({ page }, info) => {
  test.skip(!process.env.PERF, "set PERF=1 to measure");
  await open(page, "log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&speed=40");
  const r = await page.evaluate(async () => {
    const frames: number[] = [];
    let last = performance.now();
    let on = true;
    const loop = (t: number) => {
      frames.push(t - last);
      last = t;
      if (on) requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    window.__replay.perf.renders = 0;
    window.__replay.perf.totalMs = 0;
    window.__replay.perf.maxMs = 0;
    window.__replay.play();
    await new Promise((res) => setTimeout(res, 10_000));
    window.__replay.stop();
    on = false;
    frames.shift();
    const sorted = [...frames].sort((a, b) => a - b);
    const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))]!;
    const p = window.__replay.perf;
    return {
      frames: frames.length,
      fps: frames.length / 10,
      p50: q(0.5),
      p95: q(0.95),
      p99: q(0.99),
      over20ms: frames.filter((f) => f > 20).length,
      over33ms: frames.filter((f) => f > 33.4).length,
      renders: p.renders,
      renderAvgMs: p.totalMs / Math.max(1, p.renders),
      renderMaxMs: p.maxMs,
    };
  });
  console.log(`PERF ${info.project.name} ${JSON.stringify(r)}`);
});
