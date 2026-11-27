// majiang-core の差分テスト用ハーネス。1行1ケースで読み、1行1ケースで返す。
//
// シャンテン数・待ち（op なし）:
//   入力 {"case_id": "...", "majiang": "m123p456s789z11,z222="}
//   出力 {"case_id": "...", "shanten": 0, "tingpai": ["3s", ...] | null}
//   tingpai は majiang の表記（m1, z1 ...）を MJAI 表記（1m, E ...）に変換して返す。
//   14枚形（ツモ後）では majiang が null を返すので null のまま返す。
// 和了・点数（op = "hule"）:
//   入力 {"case_id": "...", "op": "hule", "shoupai": "...", "rongpai": "m5=" | null, "param": {...}}
//   param は Majiang.Util.hule_param の引数（rule は既定の Majiang.rule()）
//   出力 {"case_id": "...", "hule": {hupai, fu, fanshu, damanguan, defen} | null}
// 対局（op = "game"）:
//   入力 {"case_id": "...", "op": "game", "script": {title, player, qijia, log}, "rule": {...}}
//   script は majiang-core の牌譜の形。Majiang.Dev.Game（牌譜の打牌・副露を返事にして対局をやり直す）で再生する。
//   rule は Majiang.rule() への上書き
//   出力 {"case_id": "...", "game": {log, defen, rank, point}}（majiang-core が作り直した牌譜）
"use strict";

const Majiang = require("@kobalab/majiang-core");
const MajiangDev = require("@kobalab/majiang-core/dev");
const readline = require("node:readline");

const HONORS = ["E", "S", "W", "N", "P", "F", "C"];

function toMjai(p) {
  const s = p[0];
  const n = Number(p[1]);
  return s === "z" ? HONORS[n - 1] : `${n}${s}`;
}

function hule(c) {
  const shoupai = Majiang.Shoupai.fromString(c.shoupai);
  const param = Majiang.Util.hule_param(Object.assign({ rule: Majiang.rule() }, c.param));
  const r = Majiang.Util.hule(shoupai, c.rongpai || null, param);
  if (!r) return null;
  return {
    hupai: r.hupai,
    fu: r.fu ?? null,
    fanshu: r.fanshu ?? null,
    damanguan: r.damanguan ?? null,
    defen: r.defen,
  };
}

function game(c) {
  const g = new MajiangDev.Dev.Game(c.script, Majiang.rule(c.rule || {}));
  g.do_sync();
  const p = g._paipu;
  return { log: p.log, defen: p.defen, rank: p.rank, point: p.point };
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => {
  if (!line.trim()) return;
  const c = JSON.parse(line);
  let out;
  try {
    if (c.op === "hule") {
      out = { case_id: c.case_id, hule: hule(c) };
    } else if (c.op === "game") {
      out = { case_id: c.case_id, game: game(c) };
    } else {
      const shoupai = Majiang.Shoupai.fromString(c.majiang);
      const tingpai = Majiang.Util.tingpai(shoupai);
      out = {
        case_id: c.case_id,
        shanten: Majiang.Util.xiangting(shoupai),
        tingpai: tingpai === null ? null : tingpai.map(toMjai),
      };
    }
  } catch (e) {
    out = { case_id: c.case_id, error: String(e) };
  }
  process.stdout.write(JSON.stringify(out) + "\n");
});
