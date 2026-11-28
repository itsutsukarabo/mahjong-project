// 牌譜再生画面のテスト：
// 1. サンプル・ボット対局・mj-py の自己対戦の牌譜を最後まで1手ずつ再生し、各時点で表示が崩れないこと
//    （例外が出ない、各席の「手牌 ＋ 3 × 副露」が 13 か 14、席の枠から牌がはみ出さない）
// 2. 決まった時点の画面のスナップショット（ubuntu の CI でだけ比べる。playwright.config.ts）。
//    スナップショットは CI の「web-snapshots」ワークフロー（--update-snapshots）で撮り、成果物を取り込んでコミットする。
//    まだコミットされていないスナップショットの比較は飛ばす（UPDATE_SNAPSHOTS=1 のときは撮る）
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { type Page, expect, test } from "@playwright/test";

const LOGS = [
  "fixtures/sample.mjlog.jsonl",
  "fixtures/sample.public.mjlog.jsonl",
  "fixtures/rulebot.mjlog.jsonl",
  "fixtures/rulebot.public.mjlog.jsonl",
  "fixtures/mjpy-selfplay.mjlog.jsonl",
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

for (const log of LOGS) {
  test(`plays ${log} to the end without layout breaks`, async ({ page }) => {
    const errors = await open(page, `log=${log}`);
    const r = await page.evaluate(async () => {
      const bad: string[] = [];
      let steps = 0;
      const check = () => {
        const pos = window.__replay.position();
        for (const seat of document.querySelectorAll<HTMLElement>(".seat")) {
          const hand = seat.querySelectorAll("[data-part=hand] .tile").length;
          const melds = seat.querySelectorAll("[data-part=melds] .meld").length;
          const n = hand + 3 * melds;
          if (n !== 13 && n !== 14) bad.push(`${JSON.stringify(pos)} seat ${seat.dataset.seat}: ${n} tiles`);
          if (seat.scrollWidth > seat.clientWidth + 1 || seat.scrollHeight > seat.clientHeight + 1) {
            bad.push(`${JSON.stringify(pos)} seat ${seat.dataset.seat}: overflow`);
          }
        }
      };
      check();
      while (await window.__replay.next()) {
        steps++;
        check();
        if (bad.length > 20) break;
      }
      return { steps, bad };
    });
    expect(r.bad).toEqual([]);
    expect(r.steps).toBeGreaterThan(10);
    expect(errors).toEqual([]);
  });
}

const SNAPSHOTS: [string, string][] = [
  ["sample-start", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=0"],
  ["sample-pon-decision", "log=fixtures/sample.mjlog.jsonl&annot=fixtures/sample.annot.jsonl&kyoku=0&event=8"],
  ["sample-riichi", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=14"],
  ["sample-hora", "log=fixtures/sample.mjlog.jsonl&kyoku=0&event=21"],
  ["sample-public", "log=fixtures/sample.public.mjlog.jsonl&kyoku=0&event=14"],
  ["rulebot-annotated", "log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&kyoku=0&event=30"],
  ["rulebot-end-of-kyoku", "log=fixtures/rulebot.mjlog.jsonl&kyoku=0&event=-1"],
  ["mjpy-selfplay", "log=fixtures/mjpy-selfplay.mjlog.jsonl&kyoku=1&event=40"],
];

for (const [name, query] of SNAPSHOTS) {
  test(`screenshot ${name}`, async ({ page }) => {
    test.skip(process.platform !== "linux", "snapshots are taken on the ubuntu CI");
    const file = fileURLToPath(new URL(`./__screenshots__/${name}.png`, import.meta.url));
    test.skip(!existsSync(file) && !process.env.UPDATE_SNAPSHOTS, `no committed snapshot ${name}.png yet`);
    await open(page, query);
    await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
  });
}
