//! シャンテン数と有効牌（聴牌なら待ち）。
//!
//! 数え方の定義（`docs/spec/LOG_FORMAT.md` とは独立した、エンジンの仕様）：
//! - 一般形：手牌を面子 m・搭子 d・孤立牌 g に分け、`13 - 3m - 2d - g` の最小値。
//!   副露は面子に数える。上限：m ≤ 4（超過分は搭子へ）、m + d ≤ 4（超過分は孤立牌へ）、
//!   m + d + g ≤ 5（雀頭を確定した場合は 4、そのうえで d に雀頭 1 を足す）
//! - 字牌（三麻の 1m・9m も同じ扱い）は種類ごとに：3枚以上 = 面子（4枚目は数えない）、2枚 = 搭子、1枚 = 孤立牌
//! - 数牌は面子・搭子（対子・両面/辺張・嵌張）・孤立牌への分け方を全探索する
//! - 七対子：`13 - 2×対子の種類数 - 孤立牌`（対子 ≤ 7、対子＋孤立 ≤ 7）。副露があれば対象外
//! - 国士無双：么九牌の種類数 n、么九の対子があれば `12 - n`、なければ `13 - n`。副露があれば対象外
//! - 有効牌：13枚形（3n+1枚）で、その牌を1枚足すとシャンテン数が下がる種類。
//!   ルールで使わない種類と、手牌に4枚ある種類は除く
//!
//! この定義は @kobalab/majiang-core の `Util.xiangting` / `Util.tingpai` と同じ数え方で、
//! 差分テスト（`tools/difftest-majiang`）で突き合わせる。実装は独立（数牌の分解を全探索する）。
//! 1色の分解の結果は [`ShantenCalc`] の中に覚えておく（同じ形を2回目からは探索しない）。
//! そのため `ShantenCalc` はスレッドをまたいで共有できない（スレッドごとに作る）。

use crate::rules::Manzu;
use crate::tile::{KindCounts, NUM_KINDS, Suit, TileKind};
use crate::wall::kind_in_use;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// シャンテン数の計算に要るルールの情報。
#[derive(Debug, Clone)]
pub struct ShantenCalc {
    /// 順子を作れない種類（字牌、三麻の 1m・9m）
    honor_like: [bool; NUM_KINDS],
    in_use: [bool; NUM_KINDS],
    /// 1色の枚数（5進数の符号）→ 分解の候補（パレート最小）
    cache: RefCell<HashMap<u32, Arc<[Mdg]>>>,
}

/// 面子・搭子・孤立牌の数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Mdg {
    m: u8,
    d: u8,
    g: u8,
}

impl Mdg {
    fn add(self, o: Mdg) -> Mdg {
        Mdg {
            m: self.m + o.m,
            d: self.d + o.d,
            g: self.g + o.g,
        }
    }

    fn dominated_by(self, o: Mdg) -> bool {
        self != o && self.m <= o.m && self.d <= o.d && self.g <= o.g
    }
}

fn formula(x: Mdg, with_pair: bool) -> i8 {
    let (mut m, mut d, mut g) = (i32::from(x.m), i32::from(x.d), i32::from(x.g));
    let n = if with_pair { 4 } else { 5 };
    if m > 4 {
        d += m - 4;
        m = 4;
    }
    if m + d > 4 {
        g += m + d - 4;
        d = 4 - m;
    }
    if m + d + g > n {
        g = n - m - d;
    }
    if with_pair {
        d += 1;
    }
    (13 - m * 3 - d * 2 - g) as i8
}

fn pareto(mut v: Vec<Mdg>) -> Vec<Mdg> {
    v.sort();
    v.dedup();
    let keep: Vec<Mdg> = v
        .iter()
        .copied()
        .filter(|a| !v.iter().any(|b| a.dominated_by(*b)))
        .collect();
    keep
}

/// 1色（9種）の数牌の分け方をすべて列挙する。
fn suit_partitions(c: &mut [u8; 9]) -> Vec<Mdg> {
    fn rec(c: &mut [u8; 9], i: usize, acc: Mdg, out: &mut Vec<Mdg>) {
        let Some(i) = (i..9).find(|&k| c[k] > 0) else {
            out.push(acc);
            return;
        };
        let one = |m, d, g| Mdg { m, d, g };
        // 孤立牌
        c[i] -= 1;
        rec(c, i, acc.add(one(0, 0, 1)), out);
        c[i] += 1;
        // 刻子
        if c[i] >= 3 {
            c[i] -= 3;
            rec(c, i, acc.add(one(1, 0, 0)), out);
            c[i] += 3;
        }
        // 順子
        if i <= 6 && c[i + 1] > 0 && c[i + 2] > 0 {
            c[i] -= 1;
            c[i + 1] -= 1;
            c[i + 2] -= 1;
            rec(c, i, acc.add(one(1, 0, 0)), out);
            c[i] += 1;
            c[i + 1] += 1;
            c[i + 2] += 1;
        }
        // 対子の搭子
        if c[i] >= 2 {
            c[i] -= 2;
            rec(c, i, acc.add(one(0, 1, 0)), out);
            c[i] += 2;
        }
        // 両面・辺張、嵌張
        for gap in [1, 2] {
            if i + gap <= 8 && c[i + gap] > 0 {
                c[i] -= 1;
                c[i + gap] -= 1;
                rec(c, i, acc.add(one(0, 1, 0)), out);
                c[i] += 1;
                c[i + gap] += 1;
            }
        }
    }
    let mut out = Vec::new();
    rec(c, 0, Mdg { m: 0, d: 0, g: 0 }, &mut out);
    pareto(out)
}

impl ShantenCalc {
    pub fn new(manzu: Manzu) -> Self {
        let mut honor_like = [false; NUM_KINDS];
        let mut in_use = [false; NUM_KINDS];
        for k in TileKind::all() {
            in_use[k.index()] = kind_in_use(manzu, k);
            honor_like[k.index()] =
                k.is_honor() || (manzu == Manzu::TerminalsOnly && k.suit() == Some(Suit::Man));
        }
        Self {
            honor_like,
            in_use,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// 一般形・七対子・国士無双の最小。`melds` は副露（暗槓を含む）の数。
    pub fn shanten(&self, hand: &KindCounts, melds: u8) -> i8 {
        let mut s = self.shanten_standard(hand, melds);
        if melds == 0 {
            s = s.min(shanten_chiitoi(hand)).min(shanten_kokushi(hand));
        }
        s
    }

    pub fn shanten_standard(&self, hand: &KindCounts, melds: u8) -> i8 {
        let mut c = *hand;
        let mut best = self.standard_with(&mut c, melds, false);
        for k in 0..NUM_KINDS {
            if c[k] >= 2 {
                c[k] -= 2;
                best = best.min(self.standard_with(&mut c, melds, true));
                c[k] += 2;
            }
        }
        best
    }

    fn standard_with(&self, c: &mut KindCounts, melds: u8, with_pair: bool) -> i8 {
        // 字牌扱いの種類は数え方が決まっている
        let mut fixed = Mdg {
            m: melds,
            d: 0,
            g: 0,
        };
        for k in 0..NUM_KINDS {
            if self.honor_like[k] {
                match c[k] {
                    0 => {}
                    1 => fixed.g += 1,
                    2 => fixed.d += 1,
                    _ => fixed.m += 1,
                }
            }
        }
        // 数牌は色ごとに全探索し、組み合わせの最小を取る
        let mut groups: [Arc<[Mdg]>; 3] = [Arc::from([]), Arc::from([]), Arc::from([])];
        for (s, group) in groups.iter_mut().enumerate() {
            let mut suit = [0u8; 9];
            for (n, slot) in suit.iter_mut().enumerate() {
                let k = s * 9 + n;
                if !self.honor_like[k] {
                    *slot = c[k];
                }
            }
            // 覚える鍵は5進数の符号なので、5枚以上の種類がある（ありえない手）ときは覚えない（鍵が重なるため）
            if suit.iter().any(|&x| x > 4) {
                *group = Arc::from(suit_partitions(&mut suit));
                continue;
            }
            let code = suit.iter().fold(0u32, |a, &x| a * 5 + u32::from(x));
            // 覚えた分解は Arc で共有する（引くたびに Vec を複製しない）
            let cached = self.cache.borrow().get(&code).cloned();
            *group = match cached {
                Some(p) => p,
                None => {
                    let p: Arc<[Mdg]> = Arc::from(suit_partitions(&mut suit));
                    self.cache.borrow_mut().insert(code, p.clone());
                    p
                }
            };
        }
        let mut best = i8::MAX;
        for a in groups[0].iter() {
            for b in groups[1].iter() {
                for d in groups[2].iter() {
                    let x = fixed.add(*a).add(*b).add(*d);
                    best = best.min(formula(x, with_pair));
                }
            }
        }
        best
    }

    /// 13枚形（3n+1）で、1枚足すとシャンテン数が下がる牌の種類（昇順）。
    ///
    /// 手の中に同じ種類がなく、同じ色の ±2 以内にも牌がない種類（孤立する牌）は、一般形のシャンテン数を変えない：
    /// 13枚形はどの分け方でも 面子＋搭子＋孤立牌 ≥ 上限（雀頭なし 5、あり 4）なので、足した孤立牌は式の上限で切られ、
    /// 雀頭にもならない。その種類は一般形の計算を省き、七対子・国士無双だけを数え直す
    /// （全種類を数え直す [`Self::improving_kinds_naive`] と同じ結果になることをテストで確かめる）。
    pub fn improving_kinds(&self, hand: &KindCounts, melds: u8) -> Vec<TileKind> {
        let base_std = self.shanten_standard(hand, melds);
        let other = |c: &KindCounts| shanten_chiitoi(c).min(shanten_kokushi(c));
        let base = if melds == 0 {
            base_std.min(other(hand))
        } else {
            base_std
        };
        let mut c = *hand;
        let mut out = Vec::new();
        for k in TileKind::all() {
            let i = k.index();
            if !self.in_use[i] || c[i] >= 4 {
                continue;
            }
            let near = c[i] > 0
                || (!self.honor_like[i] && {
                    let (lo, hi) = (i / 9 * 9, i / 9 * 9 + 8);
                    (i.saturating_sub(2).max(lo)..=(i + 2).min(hi)).any(|j| c[j] > 0)
                });
            c[i] += 1;
            let std = if near {
                self.shanten_standard(&c, melds)
            } else {
                base_std
            };
            let s = if melds == 0 { std.min(other(&c)) } else { std };
            if s < base {
                out.push(k);
            }
            c[i] -= 1;
        }
        out
    }

    /// [`Self::improving_kinds`] の素直な形（全種類でシャンテン数を数え直す）。テストの基準
    pub fn improving_kinds_naive(&self, hand: &KindCounts, melds: u8) -> Vec<TileKind> {
        let base = self.shanten(hand, melds);
        let mut c = *hand;
        let mut out = Vec::new();
        for k in TileKind::all() {
            let i = k.index();
            if !self.in_use[i] || c[i] >= 4 {
                continue;
            }
            c[i] += 1;
            if self.shanten(&c, melds) < base {
                out.push(k);
            }
            c[i] -= 1;
        }
        out
    }
}

pub fn shanten_chiitoi(hand: &KindCounts) -> i8 {
    let pairs = hand.iter().filter(|&&n| n >= 2).count() as i32;
    let singles = hand.iter().filter(|&&n| n == 1).count() as i32;
    let pairs = pairs.min(7);
    let singles = singles.min(7 - pairs);
    (13 - pairs * 2 - singles) as i8
}

pub fn shanten_kokushi(hand: &KindCounts) -> i8 {
    let yaochu = TileKind::all().filter(|k| k.is_terminal_or_honor());
    let (mut kinds, mut pair) = (0, false);
    for k in yaochu {
        let n = hand[k.index()];
        if n >= 1 {
            kinds += 1;
        }
        if n >= 2 {
            pair = true;
        }
    }
    if pair { 12 - kinds } else { 13 - kinds }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, kind_counts};

    /// テスト用の簡易表記："123m456p11z" 形式（z は 1..7 = 東南西北白發中）。
    fn hand(s: &str) -> KindCounts {
        let mut tiles = Vec::new();
        let mut digits = Vec::new();
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                digits.push(ch);
                continue;
            }
            for d in digits.drain(..) {
                let t = match ch {
                    'z' => {
                        ["E", "S", "W", "N", "P", "F", "C"][d as usize - '1' as usize].to_string()
                    }
                    _ => format!("{d}{ch}"),
                };
                tiles.push(t.parse::<Tile>().unwrap());
            }
        }
        kind_counts(&tiles)
    }

    fn names(v: &[TileKind]) -> Vec<String> {
        v.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn complete_and_tenpai_hands() {
        let c = ShantenCalc::new(Manzu::All);
        assert_eq!(c.shanten(&hand("123m456p789s11122z"), 0), -1);
        assert_eq!(c.shanten(&hand("123m456p789s1112z"), 0), 0);
        assert_eq!(c.shanten(&hand("19m19p19s1234567z"), 0), 0);
        assert_eq!(c.shanten(&hand("1133557799m1122z"), 0), -1);
        // LOG_FORMAT のサンプル：99m123p45678s + 中ポン → 3s6s9s 待ち
        let h = hand("99m123p45678s");
        assert_eq!(c.shanten(&h, 1), 0);
        assert_eq!(names(&c.improving_kinds(&h, 1)), ["3s", "6s", "9s"]);
    }

    #[test]
    fn worst_cases() {
        let c = ShantenCalc::new(Manzu::All);
        // 么九牌がばらばら：国士 13-? / 一般形
        assert_eq!(c.shanten(&hand("147m258p369s1234z"), 0), 6);
        assert_eq!(c.shanten_standard(&hand("147m258p369s1234z"), 0), 8);
    }

    #[test]
    fn exhausted_waits_still_count_like_majiang_core() {
        // この定義は牌の残り枚数を考えない（majiang-core と同じ）。
        // 3面子＋1111z：雀頭 11z ＋搭子 11z と数えるので聴牌（待ちの 1z は残っていない）
        let c = ShantenCalc::new(Manzu::All);
        let h = hand("123m456p789s1111z");
        assert_eq!(c.shanten(&h, 0), 0);
        assert!(c.improving_kinds(&h, 0).is_empty());
        let h = hand("1111m456p789s111z");
        assert_eq!(c.shanten(&h, 0), 0);
        assert!(c.improving_kinds(&h, 0).is_empty());
    }

    #[test]
    fn sanma_terminals_and_missing_kinds() {
        // RiichiEnv README の例 1111m111122233z は、RiichiEnv（残り枚数を考える）では 4麻 1 / 3麻 2。
        // この定義ではどちらも 1（雀頭 11m＋搭子 11m と数える）。三麻の差分テストではこの差を扱う必要がある
        let h = hand("1111m111122233z");
        assert_eq!(ShantenCalc::new(Manzu::All).shanten(&h, 0), 1);
        assert_eq!(ShantenCalc::new(Manzu::TerminalsOnly).shanten(&h, 0), 1);
        // 三麻では 2m〜8m は有効牌にならない
        let s = ShantenCalc::new(Manzu::TerminalsOnly);
        let h = hand("19m123p456s11223z");
        assert!(
            s.improving_kinds(&h, 0)
                .iter()
                .all(|k| !(1..=7).contains(&k.index()))
        );
    }

    #[test]
    fn improving_kinds_pruned_equals_naive() {
        // 乱数の 3n+1 枚の手（副露 0〜4）で、枝刈りした受け入れの種類が全種類の数え直しと一致する
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = |m: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % m
        };
        for manzu in [Manzu::All, Manzu::TerminalsOnly] {
            let c = ShantenCalc::new(manzu);
            let kinds: Vec<usize> = TileKind::all()
                .filter(|k| kind_in_use(manzu, *k))
                .map(|k| k.index())
                .collect();
            for n in 0..4_000 {
                let melds = (n % 5) as u8;
                let len = 13 - 3 * usize::from(melds);
                let mut h = [0u8; NUM_KINDS];
                let mut placed = 0;
                // 半分は1色に寄せた手（搭子・面子が多い形）
                let narrow = n % 2 == 0;
                while placed < len {
                    let k = if narrow {
                        kinds[(next(9) as usize + 9 * (n % 3)).min(kinds.len() - 1)]
                    } else {
                        kinds[next(kinds.len() as u64) as usize]
                    };
                    if h[k] < 4 {
                        h[k] += 1;
                        placed += 1;
                    }
                }
                assert_eq!(
                    c.improving_kinds(&h, melds),
                    c.improving_kinds_naive(&h, melds),
                    "{h:?} melds {melds}"
                );
            }
        }
    }
}
