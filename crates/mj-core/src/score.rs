//! 点数・祝儀の移動（和了・荒牌流局）。局の状態に依らない純粋な計算なので、局進行から切り出してある。
//!
//! ルールの項目：`points.tsumo_payment`・`honba_ron`・`honba_tsumo_each`・`noten_penalty_total`、
//! `yakuman.pao_ron_split`、`chips.*`。

use crate::event::ChipItem;
use crate::rules::{Rules, TsumoPayment};
use crate::yaku::{WinResult, Yaku, round_up_100};

/// 支払いの場面
#[derive(Debug, Clone, Copy)]
pub struct PayCtx {
    pub n: u8,
    pub oya: u8,
    pub honba: u32,
}

/// 和了の点数の移動（供託を除く）と、和了者が受け取る手の値（本場を除く）。
///
/// - ロン：放銃者が 基本点 ×（親 6・子 4）を100点単位に切り上げて払う。本場は `honba_ron` × 本場
/// - ツモ：親は子が 基本点 ×2 ずつ、子のツモは親が ×2・子が ×1 を払う（各100点単位に切り上げ）。
///   本場は支払う人1人あたり `honba_tsumo_each` × 本場。三麻（いない北家1人分）は `tsumo_payment` に従う：
///   `loss` は払わない、`split` はその分を支払う人で等分して上乗せ（100点単位に切り上げ）
/// - 包（`pao = Some((包の人, 包の役満の基本点))`）：ツモは包の人が全額（その分はロンと同じ倍率）、
///   ロンは `pao_ron_split` なら半分を包の人が払う（majiang-core と同じ）。ツモの全額が包なら本場も包の人が払う
/// - `honba_on = false`：本場を払わない（ダブロンの2人目の上家取り、流し満貫）
pub fn hora_deltas(
    rules: &Rules,
    ctx: PayCtx,
    w: u8,
    loser: Option<u8>,
    base: i64,
    pao: Option<(u8, i64)>,
    honba_on: bool,
) -> (Vec<i32>, i64) {
    let n = usize::from(ctx.n);
    let mut d = vec![0i64; n];
    let wi = usize::from(w);
    let oya_w = w == ctx.oya;
    let honba = if honba_on { i64::from(ctx.honba) } else { 0 };
    let mul = if oya_w { 6 } else { 4 };
    let mut base = base;
    let mut points = 0i64;
    let mut honba_paid = false;
    if let Some((ps, full)) = pao {
        let base2 = match loser {
            Some(_) if rules.yakuman.pao_ron_split => full / 2,
            Some(_) => 0,
            None => full,
        };
        if base2 > 0 {
            base -= base2;
            let amt = base2 * mul;
            d[wi] += amt;
            d[usize::from(ps)] -= amt;
            points += amt;
            if base == 0 && loser.is_none() {
                let h = i64::from(rules.points.honba_ron) * honba;
                d[wi] += h;
                d[usize::from(ps)] -= h;
                honba_paid = true;
            }
        }
    }
    if base > 0 {
        match loser {
            Some(l) => {
                let amt = round_up_100(base * mul);
                let h = i64::from(rules.points.honba_ron) * honba;
                d[wi] += amt + h;
                d[usize::from(l)] -= amt + h;
                points += amt;
                honba_paid = true;
            }
            None => {
                let payers: Vec<usize> = (0..n).filter(|&s| s != wi).collect();
                let missing = 4 - n; // 三麻でいない北家の人数
                let missing_share = round_up_100(base * if oya_w { 2 } else { 1 }) * missing as i64;
                let extra = match rules.points.tsumo_payment {
                    TsumoPayment::Split if missing > 0 => round_up_100(
                        (missing_share + payers.len() as i64 - 1) / payers.len() as i64,
                    ),
                    _ => 0,
                };
                let he = i64::from(rules.points.honba_tsumo_each) * honba;
                for &ps in &payers {
                    let share = round_up_100(
                        base * if oya_w || ps == usize::from(ctx.oya) {
                            2
                        } else {
                            1
                        },
                    ) + extra;
                    d[ps] -= share + he;
                    d[wi] += share + he;
                    points += share;
                }
                honba_paid = true;
            }
        }
    }
    if !honba_paid && let Some(l) = loser {
        let h = i64::from(rules.points.honba_ron) * honba;
        d[wi] += h;
        d[usize::from(l)] -= h;
    }
    (d.into_iter().map(|x| x as i32).collect(), points)
}

/// 荒牌流局の聴牌料：ノーテン者が等分して払い、聴牌者が等分して受け取る（全員聴牌・全員ノーテンなら移動なし）。
pub fn noten_deltas(rules: &Rules, tenpai: &[bool]) -> Vec<i32> {
    let n = tenpai.len() as i32;
    let t = tenpai.iter().filter(|&&b| b).count() as i32;
    let total = rules.points.noten_penalty_total;
    tenpai
        .iter()
        .map(|&tp| {
            if t == 0 || t == n {
                0
            } else if tp {
                total / t
            } else {
                -total / (n - t)
            }
        })
        .collect()
}

/// 祝儀の移動と内訳（`chips.enabled = true` のとき）。`closed` は和了者が門前か。
pub fn hora_chips(
    rules: &Rules,
    n: u8,
    w: u8,
    loser: Option<u8>,
    r: &WinResult,
    closed: bool,
) -> (Vec<i32>, Vec<ChipItem>) {
    let v = &rules.chips.values;
    let mut per = Vec::new();
    if r.count(Yaku::Ippatsu) > 0 {
        per.push(("ippatsu", u32::from(v.ippatsu)));
    }
    let aka = u32::from(r.count(Yaku::Aka));
    if aka > 0 && (closed || v.aka_when_open) {
        per.push(("aka", aka * u32::from(v.aka)));
    }
    let ura = u32::from(r.count(Yaku::Ura));
    if ura > 0 {
        per.push(("ura", ura * u32::from(v.ura)));
    }
    if r.yakuman > 0 && (!r.kazoe || v.kazoe_yakuman_counts) {
        let c = if loser.is_some() {
            v.yakuman_ron
        } else {
            v.yakuman_tsumo_each
        };
        per.push(("yakuman", u32::from(c)));
    }
    let payers: Vec<u8> = match loser {
        Some(l) => vec![l],
        None => (0..n).filter(|&s| s != w).collect(),
    };
    let mut deltas = vec![0i32; usize::from(n)];
    let mut items = Vec::new();
    for (kind, count) in per {
        if count == 0 {
            continue;
        }
        for &ps in &payers {
            deltas[usize::from(ps)] -= count as i32;
            deltas[usize::from(w)] += count as i32;
        }
        items.push(ChipItem {
            seat: w,
            kind,
            count,
        });
    }
    (deltas, items)
}

/// 祝儀の枚数の点数換算（表示用。牌譜の `scores` には混ぜない、LOG_FORMAT §4.5）。
pub fn chips_to_points(rules: &Rules, chips: i32) -> i32 {
    chips * rules.chips.values.point_value
}
