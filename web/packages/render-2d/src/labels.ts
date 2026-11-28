// 表示用の文字列の表（役の id → 日本語名など）。ルールの処理ではない（どの役が付くか・何点かは牌譜に書かれている）。
import type { Hora } from "@mj/log-types";
import type { ShoutKind } from "@mj/table-model";

export const WIND_JA: Record<string, string> = { E: "東", S: "南", W: "西", N: "北" };

export const REASON_JA: Record<string, string> = {
  exhaustive: "荒牌流局",
  kyushu_kyuhai: "九種九牌",
  suukaikan: "四槓散了",
  all_riichi: "全員立直",
  same_wind_discards: "四風連打",
  all_ron: "全員のロン",
  nagashi_mangan: "流し満貫",
};

/** 役の id（crates/mj-core/src/yaku.rs の Yaku::id）→ 名前 */
export const YAKU_JA: Record<string, string> = {
  riichi: "立直",
  double_riichi: "両立直",
  ippatsu: "一発",
  menzen_tsumo: "門前清自摸和",
  pinfu: "平和",
  tanyao: "断幺九",
  iipeikou: "一盃口",
  yakuhai_haku: "役牌 白",
  yakuhai_hatsu: "役牌 發",
  yakuhai_chun: "役牌 中",
  yakuhai_bakaze: "役牌 場風",
  yakuhai_jikaze: "役牌 自風",
  yakuhai_north: "役牌 北",
  haitei: "海底摸月",
  houtei: "河底撈魚",
  rinshan: "嶺上開花",
  chankan: "槍槓",
  chiitoitsu: "七対子",
  chanta: "混全帯幺九",
  ittsu: "一気通貫",
  sanshoku: "三色同順",
  sanshoku_doukou: "三色同刻",
  toitoi: "対々和",
  sanankou: "三暗刻",
  sankantsu: "三槓子",
  shousangen: "小三元",
  honroutou: "混老頭",
  honitsu: "混一色",
  junchan: "純全帯幺九",
  ryanpeikou: "二盃口",
  chinitsu: "清一色",
  tenhou: "天和",
  chiihou: "地和",
  kokushi: "国士無双",
  kokushi_13: "国士無双十三面",
  suuankou: "四暗刻",
  suuankou_tanki: "四暗刻単騎",
  daisangen: "大三元",
  shousuushii: "小四喜",
  daisuushii: "大四喜",
  tsuuiisou: "字一色",
  ryuuiisou: "緑一色",
  chinroutou: "清老頭",
  chuuren: "九蓮宝燈",
  junsei_chuuren: "純正九蓮宝燈",
  suukantsu: "四槓子",
  dora: "ドラ",
  aka: "赤ドラ",
  ura: "裏ドラ",
  kita: "抜きドラ",
};

/** ドラの類（結果の窓で、役と分けて翻数を出す） */
export const DORA_IDS = ["dora", "aka", "ura", "kita"] as const;

export const SHOUT_JA: Record<ShoutKind, string> = {
  chi: "チー",
  pon: "ポン",
  kan: "カン",
  kita: "キタ",
  reach: "リーチ",
  ron: "ロン",
  tsumo: "ツモ",
};

/**
 * 点数の名前。翻数（と役満の倍数）だけで決まるものだけを出す（5翻以上・役満）。
 * 3・4翻の満貫は符と切り上げ満貫のルール（points.kiriage_mangan）で変わり、牌譜にその区分がないので出さない
 * （画面側で点数計算をしないため。REPLAY_UI §8「牌譜に足りない情報」）
 */
export function limitName(h: Hora): string | null {
  if (h.yakuman > 0) {
    if (h.yaku.some((y) => y.yakuman)) return h.yakuman === 1 ? "役満" : `${h.yakuman}倍役満`;
    return "数え役満";
  }
  if (h.han >= 13) return "三倍満";
  if (h.han >= 11) return "三倍満";
  if (h.han >= 8) return "倍満";
  if (h.han >= 6) return "跳満";
  if (h.han === 5) return "満貫";
  return null;
}

const ACT_JA: Record<string, string> = {
  tsumo: "ツモ",
  ron: "ロン",
  kyushu: "九種九牌",
  ankan: "暗槓",
  kakan: "加槓",
  kita: "北抜き",
  daiminkan: "大明槓",
  pon: "ポン",
  chi: "チー",
  reach: "立直",
  dahai: "打",
  declare: "宣言",
  pass: "見送り",
};

/** 行動の表記（DECISION_POINTS §5）→ 短い日本語。牌の部分は牌の表記のまま返す */
export function actionLabel(action: string): { verb: string; tiles: string[]; tsumogiri: boolean; extra: string } {
  const [kind = "", arg = "", flag = ""] = action.split(":");
  const verb = ACT_JA[kind] ?? kind;
  const tiles = arg && kind !== "declare" && kind !== "pass" ? arg.split(",") : [];
  let extra = "";
  if (kind === "declare") extra = arg === "tenpai" ? "聴牌" : "ノーテン";
  if (kind === "pass" && arg === "noten") extra = "ノーテン";
  return { verb, tiles, tsumogiri: flag === "tsumogiri" || arg === "tsumogiri", extra };
}

export const pct = (p: number | null): string => (p === null ? "—" : p >= 0.995 ? "100" : p < 0.005 ? "0" : (p * 100).toFixed(0));

export const signed = (v: number): string => (v > 0 ? `+${v.toLocaleString("ja-JP")}` : v.toLocaleString("ja-JP"));
