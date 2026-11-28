import { defineConfig } from "vite";

// WASM は @mj/engine の pkg/（pnpm build:wasm で作る）を Worker から読む
export default defineConfig({
  base: "./",
  worker: { format: "es" },
  build: { target: "es2022" },
  server: { fs: { allow: ["../.."] } },
});
