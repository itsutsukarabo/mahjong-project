//! 探索用ボット（D-047）：差分テストで珍しい局面を起こすために、槓・立直・北抜き・鳴き・和了を積極的に選ぶ。
//!
//! 選ばないときは通常のボット（[`crate::RuleBot`]）と同じ。乱数は使うが、局面（イベントの数・席）と
//! 設定のシードから決まるので、同じ対局なら同じ行動になる（決定論）。

use crate::{Player, RuleBot};
use mj_core::action::Action;
use mj_core::kyoku::{Decision, Kyoku};
use mj_core::rules::Rules;

/// 行動を選ぶ割合（0〜100）
#[derive(Debug, Clone, Copy)]
pub struct ExploreConfig {
    pub seed: u64,
    /// 暗槓・加槓・大明槓
    pub kan: u32,
    pub riichi: u32,
    pub kita: u32,
    /// ポン・チー（役牌に限らない）
    pub call: u32,
    /// ツモ・ロン（見逃しも試す）
    pub win: u32,
    /// 九種九牌
    pub kyushu: u32,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            kan: 90,
            riichi: 95,
            kita: 100,
            call: 35,
            win: 97,
            kyushu: 50,
        }
    }
}

pub struct ExplorerBot {
    base: RuleBot,
    cfg: ExploreConfig,
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl ExplorerBot {
    pub fn new(rules: &Rules, cfg: ExploreConfig) -> Self {
        Self {
            base: RuleBot::new(rules),
            cfg,
        }
    }

    pub fn choose(&self, k: &Kyoku, d: &Decision) -> Action {
        let ch = &d.choices;
        // 局面ごとに決まる乱数（イベントの数・席・設定のシード）
        let mut r = mix(self.cfg.seed ^ mix(k.events().len() as u64) ^ (u64::from(d.seat) << 48));
        let mut roll = |p: u32| {
            r = mix(r);
            (r % 100) < u64::from(p)
        };
        let find = |f: &dyn Fn(&Action) -> bool| ch.iter().copied().find(|a| f(a));
        if let Some(a) = find(&|a| matches!(a, Action::Tsumo | Action::Ron)) {
            if roll(self.cfg.win) {
                return a;
            }
            // 見逃し（同巡フリテン・立直後のフリテンを起こす）
            if let Some(p) = find(&|a| *a == Action::Pass) {
                return p;
            }
        }
        if let Some(a) =
            find(&|a| matches!(a, Action::Ankan(_) | Action::Kakan(_) | Action::Daiminkan))
            && roll(self.cfg.kan)
        {
            return a;
        }
        if let Some(a) = find(&|a| *a == Action::Kita)
            && roll(self.cfg.kita)
        {
            return a;
        }
        if let Some(a) = find(&|a| *a == Action::Kyushu)
            && roll(self.cfg.kyushu)
        {
            return a;
        }
        if let Some(a) = find(&|a| matches!(a, Action::Reach { .. }))
            && roll(self.cfg.riichi)
        {
            return a;
        }
        if let Some(a) = find(&|a| matches!(a, Action::Pon(..) | Action::Chi(..)))
            && roll(self.cfg.call)
        {
            return a;
        }
        self.base.choose(k, d)
    }
}

/// 卓の席ごとに通常のボットと探索用ボットを混ぜる打ち手（差分テストの対局用）
pub struct MixedBots {
    normal: RuleBot,
    explorer: ExplorerBot,
    /// 席ごとに探索用か
    pub explore_seats: Vec<bool>,
}

impl MixedBots {
    /// `game_seed` から席ごとに探索用かを決める（おおむね半分）
    pub fn new(rules: &Rules, game_seed: u64) -> Self {
        let n = usize::from(rules.table.num_players);
        let m = mix(game_seed ^ 0x6578_706c_6f72_6500);
        Self {
            normal: RuleBot::new(rules),
            explorer: ExplorerBot::new(
                rules,
                ExploreConfig {
                    seed: game_seed,
                    ..Default::default()
                },
            ),
            explore_seats: (0..n).map(|s| (m >> s) & 1 == 1).collect(),
        }
    }

    pub fn choose(&self, k: &Kyoku, d: &Decision) -> Action {
        if self.explore_seats[usize::from(d.seat)] {
            self.explorer.choose(k, d)
        } else {
            self.normal.choose(k, d)
        }
    }
}

impl Player for MixedBots {
    fn act(&mut self, k: &Kyoku, ds: &[Decision]) -> Vec<(u8, Action)> {
        ds.iter().map(|d| (d.seat, self.choose(k, d))).collect()
    }
}
