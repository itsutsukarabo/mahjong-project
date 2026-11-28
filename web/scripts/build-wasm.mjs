// mj-wasm を WASM にビルドし、wasm-bindgen で web/packages/engine/pkg に JS の包みを書き出す。
// 必要なもの：rustup target add wasm32-unknown-unknown、cargo install wasm-bindgen-cli --version 0.2.130
// （crates/mj-wasm/Cargo.toml の wasm-bindgen と同じ版。docs/research/DEV_SETUP.md）
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: "inherit" });
run("cargo", ["build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "mj-wasm"]);
run("wasm-bindgen", [
  "--target", "web",
  "--out-dir", join("web", "packages", "engine", "pkg"),
  join("target", "wasm32-unknown-unknown", "release", "mj_wasm.wasm"),
]);
