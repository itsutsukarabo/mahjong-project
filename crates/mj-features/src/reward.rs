//! RL の報酬（`docs/spec/REWARD_CONFIG.md`）：プリセット・抽選・報酬の計算。
//!
//! ルールの順位点（`points.uma`）とは別物（D-019）。報酬 ＝ 順位点[順位] ＋ 係数 ×（終局時の持ち点 − 基準点）（D-020）、
//! 正規化した報酬 ＝ 報酬 ÷ scale（§4）。抽選は対局のシードと席から決まる（§5、D-030・D-036・D-037）。

use crate::obs::RewardInput;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

/// 報酬のプリセット（東南戦。REWARD_CONFIG §3）
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    /// 1位から順（三麻は3つ、4つ目は 0）
    pub rank_points: [f32; 4],
    /// 持ち点1点あたり
    pub score_coef: f32,
    pub score_base: i32,
}

impl Preset {
    /// §4：(rank_points[0] − rank_points[人数−1]) / 2
    pub fn scale(&self, num_players: u8) -> f32 {
        (self.rank_points[0] - self.rank_points[usize::from(num_players) - 1]) / 2.0
    }

    /// 報酬（正規化の前）
    pub fn reward(&self, rank: u8, final_score: i32) -> f32 {
        self.rank_points[usize::from(rank) - 1]
            + self.score_coef * (final_score - self.score_base) as f32
    }

    /// 観測に入れる形（REWARD_CONFIG §5）
    pub fn input(&self, num_players: u8) -> RewardInput {
        let s = self.scale(num_players);
        let mut rp = [0f32; 4];
        for (i, x) in rp.iter_mut().enumerate().take(usize::from(num_players)) {
            *x = self.rank_points[i] / s;
        }
        RewardInput {
            rank_points: rp,
            score_coef: self.score_coef / s,
            score_base: self.score_base,
        }
    }
}

const fn p(id: &'static str, rp: [f32; 4], coef: f32, base: i32) -> Preset {
    Preset {
        id,
        rank_points: rp,
        score_coef: coef,
        score_base: base,
    }
}

/// 東南戦のプリセット（REWARD_CONFIG §3.1・§3.2）
pub const PRESETS: &[Preset] = &[
    p(
        "tenhou-sanma-7dan-tonnan",
        [135.0, 0.0, -135.0, 0.0],
        0.0,
        0,
    ),
    p(
        "tenhou-sanma-8dan-tonnan",
        [135.0, 0.0, -150.0, 0.0],
        0.0,
        0,
    ),
    p(
        "tenhou-sanma-9dan-tonnan",
        [135.0, 0.0, -165.0, 0.0],
        0.0,
        0,
    ),
    p(
        "tenhou-sanma-10dan-tonnan",
        [135.0, 0.0, -180.0, 0.0],
        0.0,
        0,
    ),
    p(
        "mjsoul-sanma-jansei1-ouza-tonnan",
        [255.0, 0.0, -255.0, 0.0],
        0.001,
        35000,
    ),
    p(
        "mjsoul-sanma-jansei2-ouza-tonnan",
        [255.0, 0.0, -280.0, 0.0],
        0.001,
        35000,
    ),
    p(
        "mjsoul-sanma-jansei3-ouza-tonnan",
        [255.0, 0.0, -305.0, 0.0],
        0.001,
        35000,
    ),
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// 抽選の分布（REWARD_CONFIG §5 の表。重みは §6 の人数）
pub fn distribution(name: &str) -> Option<Vec<(&'static Preset, u32)>> {
    let w = |id: &str, n: u32| (preset(id).expect("preset"), n);
    match name {
        "tenhou-sanma-7dan+" => Some(vec![
            w("tenhou-sanma-7dan-tonnan", 1075 + 23),
            w("tenhou-sanma-8dan-tonnan", 287),
            w("tenhou-sanma-9dan-tonnan", 55),
            w("tenhou-sanma-10dan-tonnan", 18),
        ]),
        "mjsoul-sanma-jansei+" => Some(vec![
            w("mjsoul-sanma-jansei1-ouza-tonnan", 12903 + 1722),
            w("mjsoul-sanma-jansei2-ouza-tonnan", 4231),
            w("mjsoul-sanma-jansei3-ouza-tonnan", 1472),
        ]),
        _ => None,
    }
}

/// 報酬の設定：固定のプリセットか、分布からの抽選
#[derive(Debug, Clone)]
pub enum RewardSpec {
    Fixed(&'static Preset),
    Sampled(Vec<(&'static Preset, u32)>),
}

impl RewardSpec {
    /// `"tenhou-sanma-7dan+"` などの分布の名前、または固定のプリセットの id
    pub fn parse(s: &str) -> Option<Self> {
        distribution(s)
            .map(RewardSpec::Sampled)
            .or_else(|| preset(s).map(RewardSpec::Fixed))
    }

    /// 対局 `game_seed` の席 `seat` のプリセット（牌山の乱数とは別の系列。同じシードなら同じ）
    pub fn pick(&self, game_seed: u64, seat: u8) -> &'static Preset {
        match self {
            RewardSpec::Fixed(p) => p,
            RewardSpec::Sampled(v) => {
                let total: u32 = v.iter().map(|x| x.1).sum();
                let mut rng = ChaCha8Rng::seed_from_u64(
                    game_seed ^ 0x7265_7761_7264_0000 ^ (u64::from(seat) << 56),
                );
                let mut r = mj_core::wall::uniform_below(&mut rng, total);
                for (p, w) in v {
                    if r < *w {
                        return p;
                    }
                    r -= w;
                }
                v.last().expect("non-empty").0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewards_and_scales() {
        let p7 = preset("tenhou-sanma-7dan-tonnan").unwrap();
        assert_eq!(p7.scale(3), 135.0);
        assert_eq!(p7.reward(1, 50000), 135.0);
        let j1 = preset("mjsoul-sanma-jansei1-ouza-tonnan").unwrap();
        assert_eq!(j1.reward(3, 25000), -255.0 - 10.0);
    }

    #[test]
    fn sampling_is_deterministic_and_follows_weights() {
        let s = RewardSpec::parse("tenhou-sanma-7dan+").unwrap();
        assert_eq!(s.pick(42, 1).id, s.pick(42, 1).id);
        let n = 20000;
        let sevens = (0..n)
            .filter(|&g| s.pick(g, 0).id == "tenhou-sanma-7dan-tonnan")
            .count();
        let rate = sevens as f64 / n as f64;
        assert!((rate - 0.7531).abs() < 0.02, "{rate}");
    }
}
