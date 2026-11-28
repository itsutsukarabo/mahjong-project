import { defineConfig, devices } from "@playwright/test";

// 2つの画面の大きさで回す（docs/spec/REPLAY_UI.md §9）：PC の横画面 1440×900 と、スマホの縦画面 390×844（devicePixelRatio 2、タッチ）。
// 画面のスナップショット（tests/__screenshots__/<名前>-<project>.png）は CI の ubuntu-24.04 で撮ったものを正とする
// （フォントで画素が変わるため、ほかの OS ではスナップショットの比較を飛ばす。tests/replay.spec.ts）。作り直しは docs/research/DEV_SETUP.md
// 手元（学習が GPU を使っている PC）では GPU を使わずに回す。
export default defineConfig({
  testDir: "tests",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}-{projectName}{ext}",
  timeout: 180_000,
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.002, animations: "disabled" } },
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://localhost:4173",
    launchOptions: { args: process.env.CI ? [] : ["--disable-gpu"] },
  },
  projects: [
    { name: "pc", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 } },
    {
      name: "phone",
      use: { ...devices["Desktop Chrome"], viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
    },
  ],
  webServer: {
    command: "pnpm preview",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
