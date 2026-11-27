//! 点数・祝儀の移動（和了・荒牌流局）。局の状態に依らない純粋な計算なので、局進行から切り出してある。
//!
//! ルールの項目：`points.tsumo_payment`・`honba_ron`・`honba_tsumo_each`・`noten_penalty_total`、
//! `yakuman.pao_ron_split`・`pao_honba`・`pao_tsumo_honba`、`chips.*`。

use crate::event::ChipItem;
use crate::rules::{PaoHonba, PaoTsumoHonba, Rules, TsumoPayment};
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
/// - 包（`pao = Some((包の人, 責任払いの対象の基本点))`。対象の額は `yakuman.pao_scope` で呼び出し側が決める）：
///   ツモは対象の額を、ふつうのツモの支払いの合計（ツモ損なら北家の分を除いた額）で包の人が全額払い、
///   残りの額はふつうのツモとして包の人も含めて分担する。ロンは `pao_ron_split` なら対象の額の半分を包の人が払い、
///   残りは放銃者が払う。積み棒は `yakuman.pao_honba`・`pao_tsumo_honba`
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
    let honba = if honba_on { i64::from(ctx.honba) } else { 0 };
    let mul = if w == ctx.oya { 6 } else { 4 };
    let mut points = 0i64;
    let pay = |d: &mut Vec<i64>, from: usize, amt: i64| {
        d[from] -= amt;
        d[wi] += amt;
    };
    let payers = n - 1;
    match loser {
        Some(l) => {
            let total = round_up_100(base * mul);
            let pao_amt = match pao {
                Some((ps, pb)) if rules.yakuman.pao_ron_split => {
                    let a = round_up_100(pb * mul) / 2;
                    pay(&mut d, usize::from(ps), a);
                    a
                }
                _ => 0,
            };
            pay(&mut d, usize::from(l), total - pao_amt);
            points += total;
            let h = i64::from(rules.points.honba_ron) * honba;
            let honba_payer = match pao {
                Some((ps, _)) if rules.yakuman.pao_honba == PaoHonba::Pao => ps,
                _ => l,
            };
            pay(&mut d, usize::from(honba_payer), h);
        }
        None => {
            let mut rest = base;
            if let Some((ps, pb)) = pao {
                let a: i64 = tsumo_shares(rules, ctx, w, pb)
                    .iter()
                    .map(|&(_, x)| x)
                    .sum();
                pay(&mut d, usize::from(ps), a);
                points += a;
                rest -= pb;
            }
            if rest > 0 {
                for (s, x) in tsumo_shares(rules, ctx, w, rest) {
                    pay(&mut d, s, x);
                    points += x;
                }
            }
            match pao {
                Some((ps, _)) if rules.yakuman.pao_honba == PaoHonba::Pao || rest == 0 => {
                    let h = match rules.yakuman.pao_tsumo_honba {
                        PaoTsumoHonba::TsumoTotal => {
                            i64::from(rules.points.honba_tsumo_each) * honba * payers as i64
                        }
                        PaoTsumoHonba::RonAmount => i64::from(rules.points.honba_ron) * honba,
                    };
                    pay(&mut d, usize::from(ps), h);
                }
                _ => {
                    let he = i64::from(rules.points.honba_tsumo_each) * honba;
                    for s in (0..n).filter(|&s| s != wi) {
                        pay(&mut d, s, he);
                    }
                }
            }
        }
    }
    (d.into_iter().map(|x| x as i32).collect(), points)
}

/// ふつうのツモの支払い（本場を除く）：支払う人ごとの額。
fn tsumo_shares(rules: &Rules, ctx: PayCtx, w: u8, base: i64) -> Vec<(usize, i64)> {
    let n = usize::from(ctx.n);
    let wi = usize::from(w);
    let oya_w = w == ctx.oya;
    let payers: Vec<usize> = (0..n).filter(|&s| s != wi).collect();
    let missing = 4 - n; // 三麻でいない北家の人数
    let missing_share = round_up_100(base * if oya_w { 2 } else { 1 }) * missing as i64;
    let extra = match rules.points.tsumo_payment {
        TsumoPayment::Split if missing > 0 => {
            round_up_100((missing_share + payers.len() as i64 - 1) / payers.len() as i64)
        }
        _ => 0,
    };
    payers
        .into_iter()
        .map(|ps| {
            let m = if oya_w || ps == usize::from(ctx.oya) {
                2
            } else {
                1
            };
            (ps, round_up_100(base * m) + extra)
        })
        .collect()
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
