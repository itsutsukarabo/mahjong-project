//! 牌の集合と牌山の生成。
//!
//! 出力は牌譜の正規化した形（`docs/spec/LOG_FORMAT.md` §4「牌山」）：
//! 配牌（席ごとに整列）、ツモ山 `pile`（先頭からツモ、末尾から嶺上）、ドラ表示牌、裏ドラ表示牌。
//!
//! RNG `chacha8-v1` の定義：
//! - 鍵 32 バイト = `b"mjwallv1"` ‖ game_seed (u64 LE) ‖ kyoku_idx (u64 LE) ‖ 0 × 8
//! - `rand_chacha::ChaCha8Rng::from_seed(鍵)`（rand_chacha =0.9.0 に固定）
//! - 正規順（[`tile_multiset`]）の牌を Fisher–Yates（末尾から、[`uniform_below`] で一様に選ぶ）で並べ替える
//! - 並べ替えた列の先頭から 13 枚ずつ席 0, 1, … の配牌、続いてドラ表示 `slots` 枚、裏 `slots` 枚、残りがツモ山
//!
//! 牌譜は牌山そのものを持つので、再生にこの定義は要らない。定義を変えるときは rng 名の版を上げる。

use crate::rules::{Manzu, TileRules};
use crate::tile::{Suit, Tile, TileKind};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

pub const RNG_NAME: &str = "chacha8-v1";
pub const HAND_SIZE: usize = 13;

/// 牌山の生成に要るルールの部分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallSpec {
    pub num_players: u8,
    pub tiles: TileRules,
    pub dora_indicator_slots: u8,
}

/// その種類の牌がこのルールで使われるか。
pub fn kind_in_use(manzu: Manzu, k: TileKind) -> bool {
    match (manzu, k.suit(), k.num()) {
        (Manzu::TerminalsOnly, Some(Suit::Man), Some(n)) => n == 1 || n == 9,
        _ => true,
    }
}

/// 使う牌の多重集合を正規順（種類の昇順、赤が先）で返す。
pub fn tile_multiset(tiles: &TileRules) -> Vec<Tile> {
    let mut out = Vec::with_capacity(136);
    for k in TileKind::all().filter(|&k| kind_in_use(tiles.manzu, k)) {
        let red = match (k.suit(), k.num()) {
            (Some(Suit::Man), Some(5)) => tiles.red_fives.m,
            (Some(Suit::Pin), Some(5)) => tiles.red_fives.p,
            (Some(Suit::Sou), Some(5)) => tiles.red_fives.s,
            _ => 0,
        };
        for i in 0..4 {
            out.push(Tile::new(k, i < red).expect("red only on fives"));
        }
    }
    out
}

/// ドラ表示牌からドラを求める（使われている種類の中で回る。三麻の 1m→9m→1m）。
pub fn dora_from_indicator(manzu: Manzu, indicator: TileKind) -> TileKind {
    let (start, len) = match indicator.index() {
        0..=26 => (indicator.index() / 9 * 9, 9),
        27..=30 => (27, 4),
        _ => (31, 3),
    };
    let mut i = indicator.index();
    loop {
        i = start + (i - start + 1) % len;
        let k = TileKind::new(i as u8).expect("in range");
        if kind_in_use(manzu, k) {
            return k;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wall {
    pub haipai: Vec<Vec<Tile>>,
    pub pile: Vec<Tile>,
    pub dora_indicators: Vec<Tile>,
    pub ura_indicators: Vec<Tile>,
}

impl Wall {
    /// すべての牌（配牌・ツモ山・表示牌）。
    pub fn all_tiles(&self) -> impl Iterator<Item = &Tile> {
        self.haipai
            .iter()
            .flatten()
            .chain(&self.pile)
            .chain(&self.dora_indicators)
            .chain(&self.ura_indicators)
    }
}

fn rng_for(game_seed: u64, kyoku_idx: u64) -> ChaCha8Rng {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(b"mjwallv1");
    key[8..16].copy_from_slice(&game_seed.to_le_bytes());
    key[16..24].copy_from_slice(&kyoku_idx.to_le_bytes());
    ChaCha8Rng::from_seed(key)
}

/// `0..bound` から一様に1つ選ぶ（剰余の偏りを棄却で除く）。
pub fn uniform_below(rng: &mut impl RngCore, bound: u32) -> u32 {
    assert!(bound > 0);
    let reject_below = bound.wrapping_neg() % bound; // (2^32 - bound) mod bound
    loop {
        let r = rng.next_u32();
        if r >= reject_below {
            return r % bound;
        }
    }
}

pub fn generate_wall(spec: &WallSpec, game_seed: u64, kyoku_idx: u64) -> Wall {
    let mut tiles = tile_multiset(&spec.tiles);
    let mut rng = rng_for(game_seed, kyoku_idx);
    for i in (1..tiles.len()).rev() {
        let j = uniform_below(&mut rng, (i + 1) as u32) as usize;
        tiles.swap(i, j);
    }
    let n = usize::from(spec.num_players);
    let slots = usize::from(spec.dora_indicator_slots);
    let mut rest = tiles.into_iter();
    let haipai = (0..n)
        .map(|_| {
            let mut h: Vec<Tile> = rest.by_ref().take(HAND_SIZE).collect();
            h.sort();
            h
        })
        .collect();
    let dora_indicators = rest.by_ref().take(slots).collect();
    let ura_indicators = rest.by_ref().take(slots).collect();
    let pile = rest.collect();
    Wall {
        haipai,
        pile,
        dora_indicators,
        ura_indicators,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RedFives;
    use crate::tile::kind_counts;

    fn sanma() -> WallSpec {
        WallSpec {
            num_players: 3,
            tiles: TileRules {
                manzu: Manzu::TerminalsOnly,
                red_fives: RedFives { m: 0, p: 1, s: 1 },
            },
            dora_indicator_slots: 5,
        }
    }

    fn yonma() -> WallSpec {
        WallSpec {
            num_players: 4,
            tiles: TileRules {
                manzu: Manzu::All,
                red_fives: RedFives { m: 1, p: 1, s: 1 },
            },
            dora_indicator_slots: 5,
        }
    }

    #[test]
    fn multiset_sizes() {
        assert_eq!(tile_multiset(&sanma().tiles).len(), 108);
        assert_eq!(tile_multiset(&yonma().tiles).len(), 136);
        let reds = tile_multiset(&sanma().tiles)
            .iter()
            .filter(|t| t.is_red())
            .count();
        assert_eq!(reds, 2);
    }

    #[test]
    fn wall_is_a_permutation_of_the_multiset() {
        for spec in [sanma(), yonma()] {
            for seed in 0..50 {
                let w = generate_wall(&spec, seed, seed % 7);
                let mut all: Vec<Tile> = w.all_tiles().copied().collect();
                all.sort();
                let mut want = tile_multiset(&spec.tiles);
                want.sort();
                assert_eq!(all, want);
                assert_eq!(w.haipai.len(), usize::from(spec.num_players));
                assert!(w.haipai.iter().all(|h| h.len() == HAND_SIZE));
                assert_eq!(w.dora_indicators.len(), 5);
                assert_eq!(w.ura_indicators.len(), 5);
            }
        }
        assert_eq!(generate_wall(&sanma(), 1, 0).pile.len(), 108 - 39 - 10);
        assert_eq!(generate_wall(&yonma(), 1, 0).pile.len(), 136 - 52 - 10);
    }

    #[test]
    fn deterministic_and_seed_sensitive() {
        let a = generate_wall(&sanma(), 42, 3);
        assert_eq!(a, generate_wall(&sanma(), 42, 3));
        assert_ne!(a, generate_wall(&sanma(), 42, 4));
        assert_ne!(a, generate_wall(&sanma(), 43, 3));
    }

    #[test]
    fn shuffle_is_roughly_uniform() {
        // 1m（正規順の先頭4枚）が配牌の席0に入る回数：期待値 4*13/108*N
        let n = 20_000u64;
        let hits: u64 = (0..n)
            .map(|s| {
                let w = generate_wall(&sanma(), s, 0);
                u64::from(kind_counts(&w.haipai[0])[0])
            })
            .sum();
        let expect = (n as f64) * 4.0 * 13.0 / 108.0;
        assert!(
            (hits as f64 - expect).abs() < expect * 0.05,
            "{hits} vs {expect}"
        );
    }

    #[test]
    fn sanma_dora_wraps_1m_9m() {
        let m = Manzu::TerminalsOnly;
        let k = |s: &str| s.parse::<Tile>().unwrap().kind();
        assert_eq!(dora_from_indicator(m, k("1m")), k("9m"));
        assert_eq!(dora_from_indicator(m, k("9m")), k("1m"));
        assert_eq!(dora_from_indicator(Manzu::All, k("9m")), k("1m"));
        assert_eq!(dora_from_indicator(Manzu::All, k("1m")), k("2m"));
        assert_eq!(dora_from_indicator(m, k("9p")), k("1p"));
        assert_eq!(dora_from_indicator(m, k("N")), k("E"));
        assert_eq!(dora_from_indicator(m, k("C")), k("P"));
    }
}
