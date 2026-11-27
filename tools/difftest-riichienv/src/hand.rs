//! 手牌の層：三麻の手牌のシャンテン数と待ち（`calculate_shanten_3p`・`HandEvaluator3P.get_waits`）。

use mj_core::agari;
use mj_core::rules::Manzu;
use mj_core::shanten::ShantenCalc;
use mj_core::tile::{KindCounts, NUM_KINDS, Tile, TileKind};
use mj_core::wall::uniform_below;
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};

/// 三麻で使う牌の種類（1m・9m・筒子・索子・字牌）
pub fn sanma_kinds() -> Vec<usize> {
    let mut v = vec![0, 8];
    v.extend(9..NUM_KINDS);
    v
}

pub fn tile_str(k: usize, red: bool) -> String {
    Tile::new(TileKind::new(k as u8).expect("kind"), red)
        .expect("tile")
        .to_string()
}

/// 種類の枚数 → MJAI 表記の牌（5p・5s は1枚目を赤にすることがある）
pub fn tiles_of(c: &KindCounts, red: &[bool; NUM_KINDS]) -> Vec<String> {
    let mut out = Vec::new();
    for (k, &n) in c.iter().enumerate() {
        for i in 0..n {
            out.push(tile_str(k, red[k] && i == 0));
        }
    }
    out
}

pub struct Gen {
    pub rng: ChaCha8Rng,
}

impl Gen {
    pub fn below(&mut self, n: usize) -> usize {
        uniform_below(&mut self.rng, n as u32) as usize
    }

    fn pick_kind(&mut self, c: &KindCounts, kinds: &[usize]) -> Option<usize> {
        let free: Vec<usize> = kinds.iter().copied().filter(|&k| c[k] < 4).collect();
        if free.is_empty() {
            None
        } else {
            Some(free[self.below(free.len())])
        }
    }

    /// 面子（順子は筒子・索子だけ）を1つ足す。足せなければ false
    pub fn add_set(&mut self, c: &mut KindCounts, kinds: &[usize]) -> bool {
        for _ in 0..20 {
            if self.below(3) == 0 {
                let k = kinds[self.below(kinds.len())];
                if c[k] + 3 <= 4 {
                    c[k] += 3;
                    return true;
                }
            } else {
                let suit = 1 + self.below(2); // 筒子・索子
                let start = suit * 9 + self.below(7);
                if (0..3).all(|d| c[start + d] < 4) {
                    for d in 0..3 {
                        c[start + d] += 1;
                    }
                    return true;
                }
            }
        }
        false
    }

    /// 13枚か14枚の手牌と、作り方の名前
    pub fn hand(&mut self) -> (KindCounts, &'static str) {
        let kinds = sanma_kinds();
        let mut c = [0u8; NUM_KINDS];
        let strategy = match self.below(10) {
            0..=2 => "random",
            3..=6 => "near_tenpai",
            7 => "four_held",
            8 => "chiitoi",
            _ => "fourteen",
        };
        let target = if strategy == "fourteen" { 14 } else { 13 };
        match strategy {
            "near_tenpai" | "fourteen" | "four_held" => {
                if strategy == "four_held" {
                    let k = kinds[self.below(kinds.len())];
                    c[k] = 4;
                }
                while c.iter().map(|&x| u32::from(x)).sum::<u32>() + 3 <= 12
                    && self.add_set(&mut c, &kinds)
                {}
                // 雀頭
                if let Some(k) = self.pick_kind(&c, &kinds)
                    && c[k] + 2 <= 4
                {
                    c[k] += 2;
                }
            }
            "chiitoi" => {
                for _ in 0..6 {
                    if let Some(k) = self.pick_kind(&c, &kinds)
                        && c[k] == 0
                    {
                        c[k] = 2;
                    }
                }
            }
            _ => {}
        }
        // 多すぎれば減らし、足りなければ足す
        while c.iter().map(|&x| u32::from(x)).sum::<u32>() > target {
            let held: Vec<usize> = (0..NUM_KINDS).filter(|&k| c[k] > 0).collect();
            c[held[self.below(held.len())]] -= 1;
        }
        while c.iter().map(|&x| u32::from(x)).sum::<u32>() < target {
            let k = self.pick_kind(&c, &kinds).expect("room");
            c[k] += 1;
        }
        (c, strategy)
    }

    pub fn reds(&mut self, c: &KindCounts) -> [bool; NUM_KINDS] {
        let mut red = [false; NUM_KINDS];
        for k in [13usize, 22] {
            // 4枚あれば1枚は赤（赤は 5p・5s 各1枚。4枚とも赤でない手は作れない）
            red[k] = c[k] == 4 || (c[k] > 0 && self.below(2) == 0);
        }
        red
    }
}

/// こちらの答え：シャンテン数と（13枚形なら）待ち
pub fn ours(calc: &ShantenCalc, c: &KindCounts) -> (i8, Option<Vec<String>>) {
    let s = calc.shanten(c, 0);
    let n: u32 = c.iter().map(|&x| u32::from(x)).sum();
    let waits = (n % 3 == 1).then(|| {
        agari::mask_kinds(agari::waits(c, true))
            .map(|k| tile_str(k.index(), false))
            .collect()
    });
    (s, waits)
}

pub fn calc() -> ShantenCalc {
    ShantenCalc::new(Manzu::TerminalsOnly)
}

pub fn request(c: &KindCounts, red: &[bool; NUM_KINDS]) -> Value {
    json!({"op": "shanten", "tiles": tiles_of(c, red)})
}
