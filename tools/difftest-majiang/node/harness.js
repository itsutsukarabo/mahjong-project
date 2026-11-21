// majiang-core の差分テスト用ハーネス。
// 入力（1行1ケース）: {"case_id": "...", "majiang": "m123p456s789z11,z222="}
// 出力（1行1ケース）: {"case_id": "...", "shanten": 0, "tingpai": ["3s", ...] | null}
// tingpai は majiang の表記（m1, z1 ...）を MJAI 表記（1m, E ...）に変換して返す。
// 14枚形（ツモ後）では majiang が null を返すので null のまま返す。
"use strict";

const Majiang = require("@kobalab/majiang-core");
const readline = require("node:readline");

const HONORS = ["E", "S", "W", "N", "P", "F", "C"];

function toMjai(p) {
  const s = p[0];
  const n = Number(p[1]);
  return s === "z" ? HONORS[n - 1] : `${n}${s}`;
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => {
  if (!line.trim()) return;
  const c = JSON.parse(line);
  let out;
  try {
    const shoupai = Majiang.Shoupai.fromString(c.majiang);
    const tingpai = Majiang.Util.tingpai(shoupai);
    out = {
      case_id: c.case_id,
      shanten: Majiang.Util.xiangting(shoupai),
      tingpai: tingpai === null ? null : tingpai.map(toMjai),
    };
  } catch (e) {
    out = { case_id: c.case_id, error: String(e) };
  }
  process.stdout.write(JSON.stringify(out) + "\n");
});
