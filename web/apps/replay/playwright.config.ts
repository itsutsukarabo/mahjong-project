import { defineConfig, devices } from "@playwright/test";

// 画面のスナップショット（tests/__screenshots__）は CI の ubuntu-24.04 で撮ったものを正とする（フォントで画素が変わるため、
// ほかの OS ではスナップショットの比較を飛ばす。tests/replay.spec.ts）。作り直しは docs/research/DEV_SETUP.md
export default defineConfig({
  testDir: "tests",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}{ext}",
  timeout: 120_000,
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.002, animations: "disabled" } },
  reporter: [["list"], ["html", { open: "never" }]],
  use: { ...devices["Desktop Chrome"], baseURL: "http://localhost:4173", viewport: { width: 1600, height: 900 }, deviceScaleFactor: 1 },
  webServer: {
    command: "pnpm preview",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
