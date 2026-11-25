//! 和了形の判定・待ち・面子への分解。
//!
//! 局進行で毎打牌に使うので速さを優先する：
//! - 数牌は色ごとに、9種の枚数を5進数にした番号で表を引く（`5^9` 通り）。
//!   表には「面子だけに分けられる」「面子＋雀頭1つに分けられる」と、
//!   「1枚足すとそうなる牌」を持たせ、和了・聴牌・待ちを色の組み合わせだけで求める
//! - 字牌（三麻で使わない 2m〜8m は枚数が常に0なので、萬子の表がそのまま 1m・9m を字牌と同じに扱う）は種類ごとに数える
//! - 役と符を数えるための面子の分解は、和了した手だけで全列挙する（[`decompose`]）
//!
//! 待ちの定義（D-018：残り枚数を考えない）：[`waits`] は「1枚足すと和了形になる種類」で、
//! 手の中に4枚ある種類（5枚目）は含めない（majiang-core の `Util.tingpai` と同じ）。
//! 5枚目を待つ形も聴牌とするかは [`is_tenpai`]（形式聴牌、`draws.keishiki_tenpai`）。

use crate::tile::{KindCounts, NUM_KINDS, TileKind};
use std::sync::OnceLock;

const SUIT_STATES: usize = 1_953_125; // 5^9

/// 面子だけに分けられる
const F_M: u32 = 1;
/// 面子＋雀頭に分けられる
const F_P: u32 = 2;
/// 1枚足すと面子だけになる牌（9 ビット）
const W_M_SHIFT: u32 = 2;
/// 1枚足すと面子＋雀頭になる牌（9 ビット）
const W_P_SHIFT: u32 = 11;

struct Tables {
    suit: Vec<u32>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(build_tables)
}

fn encode(c: &[u8]) -> usize {
    c.iter()
        .rev()
        .fold(0usize, |acc, &n| acc * 5 + usize::from(n))
}

fn build_tables() -> Tables {
    let mut suit = vec![0u32; SUIT_STATES];
    // 面子（刻子 9 種・順子 7 種）を最大4つ、昇順に選んで並べた形を全部作る
    let mut melds: Vec<[u8; 9]> = Vec::new();
    for i in 0..9 {
        let mut m = [0u8; 9];
        m[i] = 3;
        melds.push(m);
    }
    for i in 0..7 {
        let mut m = [0u8; 9];
        m[i] = 1;
        m[i + 1] = 1;
        m[i + 2] = 1;
        melds.push(m);
    }
    fn rec(melds: &[[u8; 9]], start: usize, depth: usize, c: &mut [u8; 9], suit: &mut [u32]) {
        suit[encode(c)] |= F_M;
        // 雀頭を足した形
        for p in 0..9 {
            if c[p] + 2 <= 4 {
                c[p] += 2;
                suit[encode(c)] |= F_P;
                c[p] -= 2;
            }
        }
        if depth == 4 {
            return;
        }
        for (mi, m) in melds.iter().enumerate().skip(start) {
            if (0..9).all(|k| c[k] + m[k] <= 4) {
                for k in 0..9 {
                    c[k] += m[k];
                }
                rec(melds, mi, depth + 1, c, suit);
                for k in 0..9 {
                    c[k] -= m[k];
                }
            }
        }
    }
    rec(&melds, 0, 0, &mut [0u8; 9], &mut suit);
    // 1枚足すと和了形の部分になる牌
    let mut c = [0u8; 9];
    for idx in 0..SUIT_STATES {
        let mut x = idx;
        let mut total = 0;
        for slot in c.iter_mut() {
            *slot = (x % 5) as u8;
            x /= 5;
            total += *slot;
        }
        if total > 13 {
            continue;
        }
        let mut wm = 0u32;
        let mut wp = 0u32;
        let mut pow = 1usize;
        for (k, &ck) in c.iter().enumerate() {
            if ck < 4 {
                let f = suit[idx + pow];
                if f & F_M != 0 {
                    wm |= 1 << k;
                }
                if f & F_P != 0 {
                    wp |= 1 << k;
                }
            }
            pow *= 5;
        }
        suit[idx] |= (wm << W_M_SHIFT) | (wp << W_P_SHIFT);
    }
    Tables { suit }
}

/// 色ごと・字牌の種類ごとの「部分」の状態（`F_M`/`F_P` と、1枚足したときの待ち）。
#[derive(Clone, Copy)]
struct Part {
    flags: u32,
    /// 1枚足すと面子だけ（34 種のビット）
    wait_m: u64,
    /// 1枚足すと面子＋雀頭
    wait_p: u64,
}

fn parts(c: &KindCounts) -> [Part; 10] {
    let t = tables();
    let mut out = [Part {
        flags: 0,
        wait_m: 0,
        wait_p: 0,
    }; 10];
    for (s, slot) in out.iter_mut().enumerate().take(3) {
        let v = t.suit[encode(&c[s * 9..s * 9 + 9])];
        *slot = Part {
            flags: v & (F_M | F_P),
            wait_m: u64::from((v >> W_M_SHIFT) & 0x1ff) << (s * 9),
            wait_p: u64::from((v >> W_P_SHIFT) & 0x1ff) << (s * 9),
        };
    }
    // 字牌は7種を1つの部分にまとめる（雀頭は高々1つ）
    let mut m_ok = true;
    let mut pairs = 0;
    let mut singles = 0u64;
    let mut doubles = 0u64;
    for (k, &ck) in c.iter().enumerate().skip(27) {
        match ck {
            0 | 3 => {}
            2 => {
                pairs += 1;
                doubles |= 1 << k;
            }
            1 => {
                m_ok = false;
                singles |= 1 << k;
            }
            _ => {
                m_ok = false;
                singles |= 1 << 63; // 4枚：字牌では和了形にならない（印だけ付ける）
            }
        }
    }
    let four = singles & (1 << 63) != 0;
    let singles = singles & !(1 << 63);
    let mut h = Part {
        flags: 0,
        wait_m: 0,
        wait_p: 0,
    };
    if !four {
        let n_single = singles.count_ones();
        if m_ok && pairs == 0 {
            h.flags |= F_M;
        }
        if m_ok && pairs == 1 {
            h.flags |= F_P;
        }
        // 1枚足す：対子 → 刻子、孤立 → 対子
        if n_single == 0 && pairs == 1 {
            h.wait_m |= doubles;
        }
        if n_single == 0 && pairs == 2 {
            h.wait_p |= doubles;
        }
        if n_single == 1 && pairs == 0 {
            h.wait_p |= singles;
        }
    }
    out[3] = h;
    // 残りの枠は使わない（面子だけ扱い）
    for slot in out.iter_mut().skip(4) {
        slot.flags = F_M;
    }
    out
}

/// 一般形（4面子1雀頭。副露の分は面子が少なくてよい）の和了形か。
pub fn is_standard_agari(c: &KindCounts) -> bool {
    let p = parts(c);
    let mut pair_used = false;
    for x in &p[..4] {
        if x.flags & F_M != 0 {
            continue;
        }
        if x.flags & F_P != 0 && !pair_used {
            pair_used = true;
            continue;
        }
        return false;
    }
    pair_used
}

pub fn is_chiitoi(c: &KindCounts) -> bool {
    c.iter().filter(|&&n| n == 2).count() == 7
}

pub fn is_kokushi(c: &KindCounts) -> bool {
    let mut pair = false;
    for k in TileKind::all() {
        let n = c[k.index()];
        if k.is_terminal_or_honor() {
            match n {
                1 => {}
                2 if !pair => pair = true,
                _ => return false,
            }
        } else if n != 0 {
            return false;
        }
    }
    pair
}

/// 和了形か（門前なら七対子・国士無双も）。`c` は 3n+2 枚。
pub fn is_agari(c: &KindCounts, closed: bool) -> bool {
    is_standard_agari(c) || (closed && (is_chiitoi(c) || is_kokushi(c)))
}

/// 一般形の待ち（34 種のビット）。`c` は 3n+1 枚。
pub fn standard_waits(c: &KindCounts) -> u64 {
    let p = parts(c);
    let mut waits = 0u64;
    for s in 0..4 {
        // 部分 s に1枚足し、ほかの部分はそのまま
        let others_m = (0..4).filter(|&t| t != s).all(|t| p[t].flags & F_M != 0);
        if others_m {
            waits |= p[s].wait_p;
        }
        for q in (0..4).filter(|&q| q != s) {
            if p[q].flags & F_P != 0
                && (0..4)
                    .filter(|&t| t != s && t != q)
                    .all(|t| p[t].flags & F_M != 0)
            {
                waits |= p[s].wait_m;
            }
        }
    }
    waits
}

/// 待ち（34 種のビット）。門前なら七対子・国士無双の待ちも含む。手の中に4枚ある種類は含まない。
pub fn waits(c: &KindCounts, closed: bool) -> u64 {
    let mut w = standard_waits(c);
    if closed {
        let total: u32 = c.iter().map(|&n| u32::from(n)).sum();
        if total == 13 {
            // 七対子：対子6つ＋孤立1つ（4枚使いは認めない）
            let pairs = c.iter().filter(|&&n| n == 2).count();
            let singles: Vec<usize> = (0..NUM_KINDS).filter(|&k| c[k] == 1).collect();
            if pairs == 6 && singles.len() == 1 {
                w |= 1 << singles[0];
            }
            // 国士無双
            let yao: Vec<usize> = TileKind::all()
                .filter(|k| k.is_terminal_or_honor())
                .map(|k| k.index())
                .collect();
            if (0..NUM_KINDS).all(|k| yao.contains(&k) || c[k] == 0) {
                let missing: Vec<usize> = yao.iter().copied().filter(|&k| c[k] == 0).collect();
                let has_pair = yao.iter().any(|&k| c[k] == 2);
                if yao.iter().all(|&k| c[k] <= 2) {
                    if missing.is_empty() {
                        for k in &yao {
                            w |= 1 << k;
                        }
                    } else if missing.len() == 1 && has_pair {
                        w |= 1 << missing[0];
                    }
                }
            }
        }
    }
    w
}

/// 聴牌か（`c` は 3n+1 枚）。[`waits`] に加え、手の中に4枚ある種類の5枚目を待つ形も聴牌とする
/// （形式聴牌。4枚のうち3枚を刻子とし、残り1枚と5枚目で雀頭か面子を作る）。
pub fn is_tenpai(c: &KindCounts, closed: bool) -> bool {
    if waits(c, closed) != 0 {
        return true;
    }
    (0..NUM_KINDS).filter(|&k| c[k] == 4).any(|k| {
        let mut d = *c;
        d[k] = 2;
        is_standard_agari(&d)
    })
}

/// 面子（手の中の部分。副露は別に持つ）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Set {
    /// 順子（最小の牌）
    Shuntsu(TileKind),
    Koutsu(TileKind),
}

/// 一般形の分解の1つ：雀頭と手の中の面子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decomp {
    pub pair: TileKind,
    pub sets: Vec<Set>,
}

/// 一般形の分解をすべて列挙する（`c` は和了形）。
pub fn decompose(c: &KindCounts) -> Vec<Decomp> {
    fn rec(
        c: &mut KindCounts,
        i: usize,
        acc: &mut Vec<Set>,
        pair: TileKind,
        out: &mut Vec<Decomp>,
    ) {
        let Some(i) = (i..NUM_KINDS).find(|&k| c[k] > 0) else {
            out.push(Decomp {
                pair,
                sets: acc.clone(),
            });
            return;
        };
        let k = TileKind::new(i as u8).expect("in range");
        if c[i] >= 3 {
            c[i] -= 3;
            acc.push(Set::Koutsu(k));
            rec(c, i, acc, pair, out);
            acc.pop();
            c[i] += 3;
        }
        let seq_ok = i < 27 && i % 9 <= 6 && c[i + 1] > 0 && c[i + 2] > 0;
        if seq_ok {
            c[i] -= 1;
            c[i + 1] -= 1;
            c[i + 2] -= 1;
            acc.push(Set::Shuntsu(k));
            rec(c, i, acc, pair, out);
            acc.pop();
            c[i] += 1;
            c[i + 1] += 1;
            c[i + 2] += 1;
        }
    }
    let mut out = Vec::new();
    let mut c = *c;
    for p in 0..NUM_KINDS {
        if c[p] >= 2 {
            c[p] -= 2;
            let pair = TileKind::new(p as u8).expect("in range");
            rec(&mut c, 0, &mut Vec::new(), pair, &mut out);
            c[p] += 2;
        }
    }
    out
}

pub fn mask_kinds(mask: u64) -> impl Iterator<Item = TileKind> {
    (0..NUM_KINDS as u8)
        .filter(move |&k| mask & (1 << k) != 0)
        .map(|k| TileKind::new(k).expect("in range"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Manzu;
    use crate::shanten::ShantenCalc;
    use crate::tile::{Tile, kind_counts};
    use rand_chacha::ChaCha8Rng;
    use rand_core::{RngCore, SeedableRng};

    fn counts(s: &str) -> KindCounts {
        let tiles: Vec<Tile> = s.split_whitespace().map(|t| t.parse().unwrap()).collect();
        kind_counts(&tiles)
    }

    #[test]
    fn agari_examples() {
        assert!(is_agari(
            &counts("1m 2m 3m 4p 5p 6p 7s 8s 9s E E E C C"),
            true
        ));
        assert!(!is_agari(
            &counts("1m 2m 3m 4p 5p 6p 7s 8s 9s E E E C F"),
            true
        ));
        assert!(is_agari(
            &counts("1m 1m 2p 2p 3s 3s 4s 4s E E S S C C"),
            true
        ));
        assert!(!is_agari(
            &counts("1m 1m 2p 2p 3s 3s 4s 4s E E S S C C"),
            false
        ));
        assert!(is_agari(&counts("1m 9m 1p 9p 1s 9s E S W N P F C C"), true));
        // 副露1つ（11枚）
        assert!(is_agari(&counts("2p 3p 4p 5s 5s 5s 7s 8s 9s P P"), false));
    }

    #[test]
    fn waits_examples() {
        let k = |s: &str| s.parse::<Tile>().unwrap().kind().index();
        let w = waits(&counts("1m 1m 1m 2m 3m 4m 5m 6m 7m 8m 9m 9m 9m"), true);
        let want: u64 = (0..9).map(|i| 1u64 << i).sum();
        assert_eq!(w, want); // 九蓮宝燈
        let w = waits(&counts("1m 9m 1p 9p 1s 9s E S W N P F C"), true);
        assert_eq!(w.count_ones(), 13);
        // 5枚目待ち（4枚使いの単騎）は待ちに含めないが、聴牌ではある
        let c = counts("1p 1p 1p 1p 5s 6s 7s E E E C C C");
        assert_eq!(waits(&c, false), 0);
        assert!(is_tenpai(&c, false));
        assert!(!is_tenpai(
            &counts("1p 1p 1p 1p 5s 6s 7s E E E C C F"),
            false
        ));
        let w = waits(&counts("1p 1p 1p 1p 2p 3p 4p 5s 6s 7s E E E"), false);
        assert_eq!(w, 1 << k("4p"));
        // 七対子
        let w = waits(&counts("1m 1m 2p 2p 3s 3s 4s 4s E E S S C"), true);
        assert_eq!(w, 1 << k("C"));
    }

    /// 無作為な手で、表を引く判定と、シャンテン数（全探索）の判定が一致すること。
    #[test]
    fn agrees_with_shanten_on_random_hands() {
        let calc = ShantenCalc::new(Manzu::All);
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        for _ in 0..20_000 {
            let mut c = [0u8; NUM_KINDS];
            let n = if rng.next_u32() % 2 == 0 { 13 } else { 14 };
            let mut placed = 0;
            // 偏りを付けて聴牌・和了を多めに出す
            let pool: Vec<usize> = (0..6).map(|_| (rng.next_u32() % 34) as usize).collect();
            while placed < n {
                let k = if rng.next_u32() % 3 == 0 {
                    (rng.next_u32() % 34) as usize
                } else {
                    let base = pool[(rng.next_u32() % 6) as usize];
                    if base < 27 && rng.next_u32() % 2 == 0 {
                        (base / 9) * 9 + ((base % 9 + (rng.next_u32() % 3) as usize).min(8))
                    } else {
                        base
                    }
                };
                if c[k] < 4 {
                    c[k] += 1;
                    placed += 1;
                }
            }
            if n == 14 {
                assert_eq!(is_agari(&c, true), calc.shanten(&c, 0) == -1, "{c:?}");
                assert_eq!(
                    is_standard_agari(&c),
                    calc.shanten_standard(&c, 0) == -1,
                    "{c:?}"
                );
            } else {
                let w = waits(&c, true);
                assert_eq!(is_tenpai(&c, true), calc.shanten(&c, 0) == 0, "{c:?}");
                for k in 0..NUM_KINDS {
                    if c[k] < 4 {
                        let mut d = c;
                        d[k] += 1;
                        assert_eq!(w & (1 << k) != 0, is_agari(&d, true), "{c:?} + {k}");
                    }
                }
            }
        }
    }

    #[test]
    fn decompositions_are_complete() {
        // 111222333m：刻子3つと順子3つの2通り（×雀頭）
        let d = decompose(&counts("1m 1m 1m 2m 2m 2m 3m 3m 3m 5p 6p 7p E E"));
        assert_eq!(d.len(), 2);
        let d = decompose(&counts("1m 1m 2m 2m 3m 3m 4p 5p 6p 7s 8s 9s E E"));
        assert_eq!(d.len(), 1);
    }
}
