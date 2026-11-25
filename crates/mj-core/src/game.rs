//! 対局：局の移り変わり（連荘・親流れ・延長戦・和了止め・飛び）と終局の精算。
//!
//! 終了の判定（ルールの項目）：
//! - `table.length`：`single` は1局で終わる。`tonpuu` は東場、`hanchan` は南場が規定の最後の場
//! - `table.tobi`：局の終わりに条件に当たる人がいれば終わる（`single` では使わない。D-012）
//! - 規定の最後の局が親流れで終わったとき：`table.extension = none` なら終了。`sudden_death` なら、
//!   終了点数（`table.extension_threshold`、`extension_threshold_counts_kyotaku` ならトップに供託を足して判定）
//!   以上の人がいれば終了、いなければ次の場（延長戦）へ
//! - 延長戦では、どの局の終わりでも終了点数以上の人がいれば終了。`table.extension_last_wind` の最後の局が親流れで終われば終了
//! - 和了止め・聴牌止め（`table.oorasu_stop = auto`）：規定の最後の局か延長戦の局で親が連荘したとき、
//!   親が単独トップ（同点は `table.tie_break` の順）で、延長戦ありなら親が終了点数以上なら終了
//!
//! 精算：残った供託（`table.leftover_kyotaku`）→ 順位（`table.tie_break`）→
//! 持ち点 − 返し点 ＋ ウマ ＋ オカ（トップに（返し点 − 配給原点）× 人数）。同点で `split` なら、同点の人どうしでウマ・オカを等分する。

use crate::action::Action;
use crate::event::Event;
use crate::kyoku::{Decision, EngineError, Kyoku, KyokuOutcome, KyokuSetup};
use crate::rules::*;
use crate::wall::{Wall, WallSpec, generate_wall};
use std::sync::Arc;

/// 各局の牌山の作り方。
#[derive(Debug, Clone)]
pub enum WallSource {
    /// `chacha8-v1`（`game_seed`, `kyoku_idx`）
    Seed(u64),
    /// 手で組んだ牌山（局の順）
    Manual(Vec<Wall>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameEnd {
    /// 残った供託の精算後の持ち点
    pub scores: Vec<i32>,
    /// 席ごとの順位（1 始まり）
    pub ranks: Vec<u8>,
    /// 持ち点 − 返し点 ＋ ウマ ＋ オカ（点）
    pub settlement: Vec<i32>,
    pub chips_total: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct Game {
    rules: Arc<Rules>,
    walls: WallSource,
    spec: WallSpec,
    n: u8,
    kyoku_idx: u64,
    bakaze: u8,
    kyoku_num: u8,
    honba: u32,
    kyotaku: u32,
    oya: u8,
    scores: Vec<i32>,
    chips_total: Vec<i32>,
    current: Kyoku,
    finished: Vec<Vec<Event>>,
    end: Option<GameEnd>,
}

impl Game {
    pub fn new(rules: Rules, walls: WallSource) -> Self {
        let n = rules.table.num_players;
        let spec = WallSpec {
            num_players: n,
            tiles: rules.tiles.clone(),
            dora_indicator_slots: rules.wall.dora_indicator_slots,
        };
        let rules = Arc::new(rules);
        let scores = vec![rules.points.starting; usize::from(n)];
        let wall = Self::wall_for(&walls, &spec, 0);
        let current = Kyoku::new(
            rules.clone(),
            KyokuSetup {
                bakaze: 0,
                kyoku: 0,
                honba: 0,
                kyotaku: 0,
                oya: 0,
                scores: scores.clone(),
                wall,
            },
        );
        let mut g = Game {
            rules,
            walls,
            spec,
            n,
            kyoku_idx: 0,
            bakaze: 0,
            kyoku_num: 0,
            honba: 0,
            kyotaku: 0,
            oya: 0,
            scores,
            chips_total: vec![0; usize::from(n)],
            current,
            finished: Vec::new(),
            end: None,
        };
        g.settle_done_kyoku();
        g
    }

    fn wall_for(src: &WallSource, spec: &WallSpec, kyoku_idx: u64) -> Wall {
        match src {
            WallSource::Seed(seed) => generate_wall(spec, *seed, kyoku_idx),
            WallSource::Manual(walls) => walls
                .get(kyoku_idx as usize)
                .cloned()
                .unwrap_or_else(|| panic!("no manual wall for kyoku {kyoku_idx}")),
        }
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    pub fn wall_source(&self) -> &WallSource {
        &self.walls
    }

    pub fn decisions(&self) -> &[Decision] {
        if self.end.is_some() {
            &[]
        } else {
            self.current.decisions()
        }
    }

    pub fn is_over(&self) -> bool {
        self.end.is_some()
    }

    pub fn end(&self) -> Option<&GameEnd> {
        self.end.as_ref()
    }

    /// 局ごとのイベント（`kyoku_idx` の順。進行中の局を含む）
    pub fn kyokus(&self) -> impl Iterator<Item = &[Event]> {
        self.finished
            .iter()
            .map(Vec::as_slice)
            .chain((self.end.is_none()).then(|| self.current.events()))
    }

    /// 進行中の局（終局後は最後の局）
    pub fn current_kyoku(&self) -> &Kyoku {
        &self.current
    }

    pub fn step(&mut self, actions: &[(u8, Action)]) -> Result<(), EngineError> {
        if self.end.is_some() {
            return Err(EngineError("game is over".into()));
        }
        self.current.step(actions)?;
        self.settle_done_kyoku();
        Ok(())
    }

    /// 局が終わっていれば、次の局を始めるか終局する（判断ポイントのある局まで）。
    fn settle_done_kyoku(&mut self) {
        while self.current.is_done() && self.end.is_none() {
            let o = self
                .current
                .outcome()
                .expect("done kyoku has outcome")
                .clone();
            let over = self.after_kyoku(&o);
            self.finished.push(self.current.events().to_vec());
            if over {
                self.finish_game();
                return;
            }
            self.kyoku_idx += 1;
            let wall = Self::wall_for(&self.walls, &self.spec, self.kyoku_idx);
            self.current = Kyoku::new(
                self.rules.clone(),
                KyokuSetup {
                    bakaze: self.bakaze,
                    kyoku: self.kyoku_num,
                    honba: self.honba,
                    kyotaku: self.kyotaku,
                    oya: self.oya,
                    scores: self.scores.clone(),
                    wall,
                },
            );
        }
    }

    /// 席の順位（1 始まり）。持ち点の高い順、同点は起家に近い席が上。
    fn ranks(scores: &[i32]) -> Vec<u8> {
        let n = scores.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| scores[b].cmp(&scores[a]).then(a.cmp(&b)));
        let mut ranks = vec![0u8; n];
        for (r, &s) in order.iter().enumerate() {
            ranks[s] = r as u8 + 1;
        }
        ranks
    }

    fn top_seat(&self) -> usize {
        Self::ranks(&self.scores)
            .iter()
            .position(|&r| r == 1)
            .expect("someone is top")
    }

    /// 終了点数に届いた人がいるか（供託を含める設定ならトップに足して判定）
    fn someone_reached(&self) -> bool {
        let t = &self.rules.table;
        let top = self.top_seat();
        self.scores.iter().enumerate().any(|(s, &x)| {
            let bonus = if t.extension_threshold_counts_kyotaku && s == top {
                self.kyotaku as i32 * self.rules.points.riichi_deposit
            } else {
                0
            };
            x + bonus >= t.extension_threshold
        })
    }

    fn after_kyoku(&mut self, o: &KyokuOutcome) -> bool {
        let t = self.rules.table.clone();
        self.scores = o.scores.clone();
        for (c, d) in self.chips_total.iter_mut().zip(&o.chip_deltas) {
            *c += d;
        }
        self.kyotaku = o.kyotaku_next;
        if t.length == GameLength::Single {
            return true;
        }
        let tobi = match t.tobi {
            Tobi::None => false,
            Tobi::BelowZero => self.scores.iter().any(|&x| x < 0),
            Tobi::ZeroOrBelow => self.scores.iter().any(|&x| x <= 0),
        };
        if tobi {
            return true;
        }
        let regular_last_wind = if t.length == GameLength::Tonpuu { 0 } else { 1 };
        let in_extension = self.bakaze > regular_last_wind;
        let regular_last = self.bakaze == regular_last_wind && self.kyoku_num + 1 == self.n;
        let extension_on = t.extension == Extension::SuddenDeath;
        if o.renchan {
            if (regular_last || in_extension) && t.oorasu_stop == OorasuStop::Auto {
                let oya = usize::from(self.oya);
                let sole_top = self.top_seat() == oya;
                let reached = !extension_on || {
                    let bonus = if t.extension_threshold_counts_kyotaku {
                        self.kyotaku as i32 * self.rules.points.riichi_deposit
                    } else {
                        0
                    };
                    self.scores[oya] + bonus >= t.extension_threshold
                };
                if sole_top && reached {
                    return true;
                }
            }
            if in_extension && extension_on && self.someone_reached() {
                return true;
            }
            self.honba = o.honba_next;
            return false;
        }
        if in_extension && self.someone_reached() {
            return true;
        }
        if regular_last && (!extension_on || self.someone_reached()) {
            return true;
        }
        self.honba = o.honba_next;
        self.oya = (self.oya + 1) % self.n;
        self.kyoku_num += 1;
        if self.kyoku_num == self.n {
            self.kyoku_num = 0;
            self.bakaze += 1;
        }
        let last_wind = t.extension_last_wind as u8;
        self.bakaze > last_wind.max(regular_last_wind)
    }

    fn finish_game(&mut self) {
        let rules = self.rules.clone();
        let n = usize::from(self.n);
        let mut scores = self.scores.clone();
        if rules.table.leftover_kyotaku == LeftoverKyotaku::Top {
            let top = self.top_seat();
            scores[top] += self.kyotaku as i32 * rules.points.riichi_deposit;
        }
        let ranks = Self::ranks(&scores);
        let oka = (rules.points.returning - rules.points.starting) * n as i32;
        let bonus_of_rank =
            |r: u8| rules.points.uma[usize::from(r - 1)] + if r == 1 { oka } else { 0 };
        let mut settlement: Vec<i32> = (0..n)
            .map(|s| scores[s] - rules.points.returning + bonus_of_rank(ranks[s]))
            .collect();
        if rules.table.tie_break == TieBreak::Split {
            // 同点の人どうしでウマ・オカを等分（余りは起家に近い席から）
            let mut seen = vec![false; n];
            for s in 0..n {
                if seen[s] {
                    continue;
                }
                let group: Vec<usize> = (0..n).filter(|&t| scores[t] == scores[s]).collect();
                for &g in &group {
                    seen[g] = true;
                }
                if group.len() < 2 {
                    continue;
                }
                let total: i32 = group.iter().map(|&g| bonus_of_rank(ranks[g])).sum();
                let m = group.len() as i32;
                let (q, r) = (total.div_euclid(m), total.rem_euclid(m));
                for (i, &g) in group.iter().enumerate() {
                    let share = q + i32::from((i as i32) < r);
                    settlement[g] = scores[g] - rules.points.returning + share;
                }
            }
        }
        self.end = Some(GameEnd {
            scores,
            ranks,
            settlement,
            chips_total: self.chips_total.clone(),
        });
    }
}
