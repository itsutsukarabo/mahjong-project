//! 観測（`docs/spec/OBSERVATION.md`、D-042・D-043）。
//!
//! 観測を作る実装はここだけ（D-043）。学習（mj-py）・対戦サーバー・FE（WASM）はすべてこれを呼ぶ。
//!
//! - **方策の入力**（[`Observation::policy`]・[`Observation::events`]）は、局のイベント列から作った
//!   **席視点のイベント列**（[`seat_view`]。他家のツモ・他家の配牌・牌山・裏ドラを含まない）だけから作る
//! - **価値関数だけの入力**（[`Observation::critic`]）は全情報のイベント列から作る（学習時だけ使う）
//! - 同じ入力からは常に同じ観測になる（乱数・時刻・ハッシュの順序に依らない）

use crate::{NUM_ACTIONS, legal_mask, tile_index};
use mj_core::action::Action;
use mj_core::agari;
use mj_core::catalog::{CATALOG, Kind};
use mj_core::event::Event;
use mj_core::rules::{AnkanVisibility, GameLength, Rules, lookup, rules_to_table};
use mj_core::shanten::ShantenCalc;
use mj_core::tile::{KindCounts, NUM_KINDS, Tile, TileKind};
use mj_core::wall::{dora_from_indicator, tile_multiset};

const NORTH: usize = 30;
const UNKNOWN_TILE: i16 = 37;
const NO_TILE: i16 = 38;

/// イベント列の1つ（トークン）の欄の数：[種類, 席（自分から見た相対 0〜3）, 牌（0〜36、37 = 見えない、38 = なし）,
/// 付加1, 付加2, 位置（局の中の順番 0〜）]
pub const EVENT_FIELDS: usize = 6;

/// イベント列の長さの上限（D-045：1局 192 件）
pub const MAX_EVENTS: usize = 192;

/// イベントの種類（トークンの 0 欄）
pub mod ev {
    pub const DORA: i16 = 1;
    pub const DRAW: i16 = 2;
    pub const DISCARD: i16 = 3;
    pub const REACH: i16 = 4;
    pub const REACH_ACCEPTED: i16 = 5;
    pub const CHI: i16 = 6;
    pub const PON: i16 = 7;
    pub const DAIMINKAN: i16 = 8;
    pub const ANKAN: i16 = 9;
    pub const KAKAN: i16 = 10;
    pub const KITA: i16 = 11;
}

/// 報酬設定の特徴（REWARD_CONFIG §5）：順位点（正規化、4人分）、持ち点の係数（正規化）、基準点
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RewardInput {
    pub rank_points: [f32; 4],
    pub score_coef: f32,
    /// 基準点（点）。係数 0 なら使わない
    pub score_base: i32,
}

impl Default for RewardInput {
    fn default() -> Self {
        Self {
            rank_points: [0.0; 4],
            score_coef: 0.0,
            score_base: 0,
        }
    }
}

/// 席視点のイベント（LOG_FORMAT §5 の席視点版と同じ範囲）
#[derive(Debug, Clone, PartialEq)]
pub enum ViewEvent {
    Start {
        bakaze: TileKind,
        kyoku: u8,
        honba: u32,
        kyotaku: u32,
        oya: u8,
        scores: Vec<i32>,
        dora_indicator: Tile,
        own_tehai: Vec<Tile>,
    },
    /// 他家のツモは牌が見えない（`None`）
    Draw {
        actor: u8,
        tile: Option<Tile>,
        rinshan: bool,
    },
    Discard {
        actor: u8,
        tile: Tile,
        tsumogiri: bool,
    },
    Reach {
        actor: u8,
    },
    ReachAccepted {
        actor: u8,
        scores: Vec<i32>,
        kyotaku: u32,
    },
    Call {
        kind: i16,
        actor: u8,
        target: u8,
        tile: Tile,
        consumed: Vec<Tile>,
    },
    /// 暗槓：公開しないルール（`log_visibility.ankan = closed`）では他家の暗槓の牌は見えない
    Ankan {
        actor: u8,
        consumed: Option<Vec<Tile>>,
    },
    Kakan {
        actor: u8,
        tile: Tile,
    },
    Kita {
        actor: u8,
        tile: Tile,
    },
    Dora {
        indicator: Tile,
    },
}

/// 局の全情報のイベント列 → 席 `seat` の視点のイベント列。隠れた情報はここで落とす（方策の入力はこれだけから作る）
pub fn seat_view(rules: &Rules, events: &[Event], seat: u8) -> Vec<ViewEvent> {
    let mut out = Vec::new();
    for e in events {
        let v = match e {
            Event::KyokuStart(s) => ViewEvent::Start {
                bakaze: s.bakaze.kind(),
                kyoku: s.kyoku,
                honba: s.honba,
                kyotaku: s.kyotaku,
                oya: s.oya,
                scores: s.scores.clone(),
                dora_indicator: s.dora_indicator,
                own_tehai: s.tehais[usize::from(seat)].clone(),
            },
            Event::Tsumo {
                actor,
                pai,
                rinshan,
            } => ViewEvent::Draw {
                actor: *actor,
                tile: (*actor == seat).then_some(*pai),
                rinshan: *rinshan,
            },
            Event::Dahai {
                actor,
                pai,
                tsumogiri,
            } => ViewEvent::Discard {
                actor: *actor,
                tile: *pai,
                tsumogiri: *tsumogiri,
            },
            Event::Reach { actor } => ViewEvent::Reach { actor: *actor },
            Event::ReachAccepted {
                actor,
                scores,
                kyotaku,
            } => ViewEvent::ReachAccepted {
                actor: *actor,
                scores: scores.clone(),
                kyotaku: *kyotaku,
            },
            Event::Chi {
                actor,
                target,
                pai,
                consumed,
            } => call(ev::CHI, *actor, *target, *pai, consumed),
            Event::Pon {
                actor,
                target,
                pai,
                consumed,
            } => call(ev::PON, *actor, *target, *pai, consumed),
            Event::Daiminkan {
                actor,
                target,
                pai,
                consumed,
            } => call(ev::DAIMINKAN, *actor, *target, *pai, consumed),
            Event::Ankan { actor, consumed } => ViewEvent::Ankan {
                actor: *actor,
                consumed: (*actor == seat || rules.log_visibility.ankan == AnkanVisibility::Open)
                    .then(|| consumed.clone()),
            },
            Event::Kakan { actor, pai, .. } => ViewEvent::Kakan {
                actor: *actor,
                tile: *pai,
            },
            Event::Kita { actor, pai } => ViewEvent::Kita {
                actor: *actor,
                tile: *pai,
            },
            Event::Dora { dora_indicator } => ViewEvent::Dora {
                indicator: *dora_indicator,
            },
            // 和了・流局・局の終わりは判断ポイントの後にしか来ない
            Event::Hora(_) | Event::Ryukyoku(_) | Event::KyokuEnd(_) => continue,
        };
        out.push(v);
    }
    out
}

fn call(kind: i16, actor: u8, target: u8, tile: Tile, consumed: &[Tile]) -> ViewEvent {
    ViewEvent::Call {
        kind,
        actor,
        target,
        tile,
        consumed: consumed.to_vec(),
    }
}

/// 観測1つ
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// 方策の入力（固定長の数値。区画の並びは [`Layout`]）
    pub policy: Vec<f32>,
    /// イベント列（局の始めから。トークンの欄は [`EVENT_FIELDS`]）
    pub events: Vec<[i16; EVENT_FIELDS]>,
    /// 合法手マスク（325）
    pub mask: Vec<bool>,
    /// 価値関数だけの入力（隠れた情報。学習時だけ使う）
    pub critic: Vec<f32>,
}

/// 区画ごとの次元（方策の入力 `policy` の並び）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub sections: Vec<(&'static str, usize)>,
    pub critic: usize,
}

impl Layout {
    pub fn policy(&self) -> usize {
        self.sections.iter().map(|s| s.1).sum()
    }
}

/// 観測を作るもの（シャンテン数の計算器を中に持つ。スレッドごとに1つ作る）
pub struct Observer {
    rules: Rules,
    calc: ShantenCalc,
    rule_features: Vec<f32>,
    live0: usize,
}

const N_HAND: usize = 34 + 3 + 37;
const N_SELF: usize = 4;
const N_CAND: usize = 37 * 3;
const N_TABLE: usize = 4 + 4 + 4 + 5;
const N_SEATS: usize = 4 * 7 + 4;
const N_TILES: usize = 34 + 34;
const N_REWARD: usize = 6;
const N_CRITIC: usize = 3 * 34 + 34 + 34 + 3;

impl Observer {
    pub fn new(rules: &Rules) -> Self {
        let n = usize::from(rules.table.num_players);
        Self {
            rules: rules.clone(),
            calc: ShantenCalc::new(rules.tiles.manzu),
            rule_features: rule_features(rules),
            live0: tile_multiset(&rules.tiles).len() - 13 * n - usize::from(rules.wall.dead_wall),
        }
    }

    pub fn layout(&self) -> Layout {
        Layout {
            sections: vec![
                ("hand", N_HAND),
                ("self", N_SELF),
                ("discard_candidates", N_CAND),
                ("table", N_TABLE),
                ("seats", N_SEATS),
                ("tiles", N_TILES),
                ("rules", self.rule_features.len()),
                ("reward", N_REWARD),
            ],
            critic: N_CRITIC,
        }
    }

    /// 局の全情報のイベント列 `events`（`kyoku_start` から判断ポイントまで）と、その席の選択肢から観測を作る
    pub fn observe(
        &self,
        events: &[Event],
        seat: u8,
        choices: &[Action],
        reward: &RewardInput,
    ) -> Observation {
        let view = seat_view(&self.rules, events, seat);
        let (policy, tokens) = self.policy(&view, seat, choices, reward);
        Observation {
            policy,
            events: tokens,
            mask: {
                let m = legal_mask(choices);
                debug_assert_eq!(m.len(), NUM_ACTIONS);
                m
            },
            critic: self.critic(events, seat),
        }
    }

    fn policy(
        &self,
        view: &[ViewEvent],
        seat: u8,
        choices: &[Action],
        reward: &RewardInput,
    ) -> (Vec<f32>, Vec<[i16; EVENT_FIELDS]>) {
        let n = self.rules.table.num_players;
        let rel = |s: u8| i16::from((s + n - seat) % n);
        let mut st = State::new(n);
        let mut tokens: Vec<[i16; EVENT_FIELDS]> = Vec::new();
        let push = |t: [i16; 5], tokens: &mut Vec<[i16; EVENT_FIELDS]>| {
            let pos = tokens.len() as i16;
            tokens.push([t[0], t[1], t[2], t[3], t[4], pos]);
        };
        let ti = |t: Tile| tile_index(t) as i16;
        for v in view {
            match v {
                ViewEvent::Start {
                    bakaze,
                    kyoku,
                    honba,
                    kyotaku,
                    oya,
                    scores,
                    dora_indicator,
                    own_tehai,
                } => {
                    st.bakaze = bakaze.index();
                    st.kyoku = *kyoku;
                    st.honba = *honba;
                    st.kyotaku = *kyotaku;
                    st.oya = *oya;
                    st.scores = scores.clone();
                    for t in own_tehai {
                        st.add_own(*t);
                    }
                    st.indicators.push(*dora_indicator);
                    st.visible[dora_indicator.kind().index()] += 1;
                    push([ev::DORA, 0, ti(*dora_indicator), 0, 0], &mut tokens);
                }
                ViewEvent::Draw {
                    actor,
                    tile,
                    rinshan,
                } => {
                    st.draws += 1;
                    if *actor == seat {
                        let t = tile.expect("own draw is visible");
                        st.add_own(t);
                        st.drawn = Some(t);
                    }
                    let t = tile.map_or(UNKNOWN_TILE, ti);
                    push(
                        [ev::DRAW, rel(*actor), t, i16::from(*rinshan), 0],
                        &mut tokens,
                    );
                }
                ViewEvent::Discard {
                    actor,
                    tile,
                    tsumogiri,
                } => {
                    if *actor == seat {
                        st.remove_own(*tile);
                        st.drawn = None;
                    }
                    st.discards[usize::from(*actor)].push(*tile);
                    st.visible[tile.kind().index()] += 1;
                    let reach = i16::from(st.pending_reach == Some(*actor));
                    st.pending_reach = None;
                    push(
                        [
                            ev::DISCARD,
                            rel(*actor),
                            ti(*tile),
                            i16::from(*tsumogiri),
                            reach,
                        ],
                        &mut tokens,
                    );
                }
                ViewEvent::Reach { actor } => {
                    st.riichi[usize::from(*actor)] = true;
                    st.pending_reach = Some(*actor);
                    push([ev::REACH, rel(*actor), NO_TILE, 0, 0], &mut tokens);
                }
                ViewEvent::ReachAccepted {
                    actor,
                    scores,
                    kyotaku,
                } => {
                    st.scores = scores.clone();
                    st.kyotaku = *kyotaku;
                    push(
                        [ev::REACH_ACCEPTED, rel(*actor), NO_TILE, 0, 0],
                        &mut tokens,
                    );
                }
                ViewEvent::Call {
                    kind,
                    actor,
                    target,
                    tile,
                    consumed,
                } => {
                    // 鳴かれた牌は河から副露へ（見えている枚数は変わらない）
                    if let Some(x) = st.discards[usize::from(*target)].last()
                        && x == tile
                    {
                        st.called_from[usize::from(*target)] += 1;
                    }
                    if *actor == seat {
                        for t in consumed {
                            st.remove_own(*t);
                        }
                    }
                    for t in consumed {
                        st.visible[t.kind().index()] += 1;
                    }
                    st.melds[usize::from(*actor)] += 1;
                    push(
                        [*kind, rel(*actor), ti(*tile), rel(*target), 0],
                        &mut tokens,
                    );
                }
                ViewEvent::Ankan { actor, consumed } => {
                    st.melds[usize::from(*actor)] += 1;
                    let mut tile = UNKNOWN_TILE;
                    if let Some(c) = consumed {
                        tile = ti(Tile::plain(c[0].kind()));
                        for t in c {
                            if *actor == seat {
                                st.remove_own(*t);
                            }
                            st.visible[t.kind().index()] += 1;
                        }
                    }
                    if *actor == seat {
                        st.drawn = None;
                    }
                    push([ev::ANKAN, rel(*actor), tile, 0, 0], &mut tokens);
                }
                ViewEvent::Kakan { actor, tile } => {
                    if *actor == seat {
                        st.remove_own(*tile);
                        st.drawn = None;
                    }
                    st.visible[tile.kind().index()] += 1;
                    push([ev::KAKAN, rel(*actor), ti(*tile), 0, 0], &mut tokens);
                }
                ViewEvent::Kita { actor, tile } => {
                    if *actor == seat {
                        st.remove_own(*tile);
                        st.drawn = None;
                    }
                    st.visible[tile.kind().index()] += 1;
                    st.kita[usize::from(*actor)] += 1;
                    push([ev::KITA, rel(*actor), ti(*tile), 0, 0], &mut tokens);
                }
                ViewEvent::Dora { indicator } => {
                    st.indicators.push(*indicator);
                    st.visible[indicator.kind().index()] += 1;
                    push([ev::DORA, 0, ti(*indicator), 0, 0], &mut tokens);
                }
            }
        }
        (self.policy_vector(&st, seat, choices, reward), tokens)
    }

    fn policy_vector(
        &self,
        st: &State,
        seat: u8,
        choices: &[Action],
        reward: &RewardInput,
    ) -> Vec<f32> {
        let rules = &self.rules;
        let n = rules.table.num_players;
        let s = usize::from(seat);
        let mut f: Vec<f32> = Vec::with_capacity(self.layout().policy());
        // ---- hand
        let own = st.own;
        for &c in own.iter() {
            f.push(f32::from(c) / 4.0);
        }
        for k in [4usize, 13, 22] {
            f.push(f32::from(st.own_red[k]));
        }
        let mut drawn = [0f32; 37];
        if let Some(t) = st.drawn {
            drawn[tile_index(t)] = 1.0;
        }
        f.extend(drawn);
        // ---- self：シャンテン数・受け入れ（自分の手牌と見えている牌だけから）・立直・フリテン
        let melds = st.melds[s];
        let visible = st.visible_with_own();
        let remaining = |k: usize| 4u8.saturating_sub(visible[k]);
        let total: u32 = own.iter().map(|&x| u32::from(x)).sum();
        let (sh, uk) = if total % 3 == 1 {
            self.shanten_ukeire(&own, melds, &remaining)
        } else {
            // 14枚形（ツモ後）：今のシャンテン数、受け入れは打牌の候補ごとに見る
            (self.calc.shanten(&own, melds), 0)
        };
        f.push((f32::from(sh) + 1.0) / 8.0);
        f.push(uk as f32 / 60.0);
        f.push(f32::from(u8::from(st.riichi[s])));
        let waits = if total % 3 == 1 {
            agari::waits(&own, melds == 0)
        } else {
            0
        };
        let furiten = st.discards[s]
            .iter()
            .any(|t| waits & (1 << t.kind().index()) != 0);
        f.push(f32::from(u8::from(furiten)));
        // ---- discard_candidates：打牌の候補ごとに「切った後のシャンテン数と受け入れ」
        let mut cand = [0f32; 37 * 3];
        for a in choices {
            if let Action::Dahai { tile, .. } = a {
                let i = tile_index(*tile);
                let mut c = own;
                c[tile.kind().index()] -= 1;
                let (sh, uk) = self.shanten_ukeire(&c, melds, &remaining);
                cand[i] = 1.0;
                cand[37 + i] = (f32::from(sh) + 1.0) / 8.0;
                cand[74 + i] = uk as f32 / 60.0;
            }
        }
        f.extend(cand);
        // ---- table
        let onehot = |i: usize, len: usize, f: &mut Vec<f32>| {
            for j in 0..len {
                f.push(f32::from(u8::from(i == j)));
            }
        };
        onehot(st.bakaze - 27, 4, &mut f);
        onehot(usize::from(st.kyoku.saturating_sub(1)), 4, &mut f);
        onehot(usize::from((st.oya + n - seat) % n), 4, &mut f);
        f.push(st.honba as f32 / 10.0);
        f.push(st.kyotaku as f32 / 10.0);
        let live = self.live0.saturating_sub(st.draws as usize);
        f.push(live as f32 / 70.0);
        let regular_last_wind = if rules.table.length == GameLength::Tonpuu {
            27
        } else {
            28
        };
        let in_extension =
            rules.table.length != GameLength::Single && st.bakaze > regular_last_wind;
        let regular_last = rules.table.length == GameLength::Single
            || (st.bakaze == regular_last_wind && st.kyoku == n);
        f.push(f32::from(u8::from(in_extension)));
        f.push(f32::from(u8::from(regular_last)));
        // ---- seats（自分から見た相対の席 0〜3。三麻の4つ目は 0）
        let ranks = ranks_of(&st.scores, st.oya);
        for r in 0..4u8 {
            if r >= n {
                f.extend([0.0; 7]);
                continue;
            }
            let a = usize::from((seat + r) % n);
            f.push(st.scores[a] as f32 / 10000.0);
            f.push((st.scores[a] - st.scores[s]) as f32 / 10000.0);
            f.push(f32::from(ranks[a]) / 4.0);
            f.push(f32::from(u8::from(st.riichi[a])));
            f.push(f32::from(st.kita[a]) / 4.0);
            f.push(f32::from(st.melds[a]) / 4.0);
            f.push(st.discards[a].len() as f32 / 24.0);
        }
        onehot(usize::from(ranks[s] - 1), 4, &mut f);
        // ---- tiles：ドラになる牌の種類ごとの枚数（抜いた北のドラを含む）・見えている枚数
        let mut dora = [0f32; NUM_KINDS];
        for t in &st.indicators {
            dora[dora_from_indicator(rules.tiles.manzu, t.kind()).index()] += 1.0;
        }
        if rules.kita.mode == mj_core::rules::KitaMode::Nukidora {
            dora[NORTH] += f32::from(rules.kita.dora_per_kita);
        }
        f.extend(dora.iter().map(|x| x / 4.0));
        f.extend(visible.iter().map(|&x| f32::from(x) / 4.0));
        // ---- rules・reward
        f.extend(&self.rule_features);
        f.extend(reward.rank_points);
        f.push(reward.score_coef * 10000.0);
        f.push(if reward.score_coef == 0.0 {
            0.0
        } else {
            (st.scores[s] - reward.score_base) as f32 / 10000.0
        });
        debug_assert_eq!(f.len(), self.layout().policy());
        f
    }

    fn shanten_ukeire(
        &self,
        c: &KindCounts,
        melds: u8,
        remaining: &dyn Fn(usize) -> u8,
    ) -> (i8, u32) {
        let sh = self.calc.shanten(c, melds);
        let uk = self
            .calc
            .improving_kinds(c, melds)
            .iter()
            .map(|k| u32::from(remaining(k.index())))
            .sum();
        (sh, uk)
    }

    /// 価値関数だけの入力：他家の手牌（相対の席 1〜3）、ツモ山の残り（めくっていない表示牌を含む）、裏ドラ表示牌、他家のシャンテン数
    fn critic(&self, events: &[Event], seat: u8) -> Vec<f32> {
        let n = self.rules.table.num_players;
        // 全情報のイベント列が食い違っていても（漏れの検査で隠れた欄を書き換えたものなど）落ちないように 0 で止める
        let dec = |h: &mut KindCounts, k: usize| h[k] = h[k].saturating_sub(1);
        let mut hands: Vec<KindCounts> = vec![[0; NUM_KINDS]; usize::from(n)];
        let mut melds = vec![0u8; usize::from(n)];
        let mut wall = [0u8; NUM_KINDS];
        let mut ura = [0f32; NUM_KINDS];
        let mut taken = 0usize;
        let mut rinshan = 0usize;
        let mut revealed = 1usize;
        let mut pile: Vec<Tile> = Vec::new();
        let mut inds: Vec<Tile> = Vec::new();
        let mut uras: Vec<Tile> = Vec::new();
        for e in events {
            match e {
                Event::KyokuStart(s) => {
                    for (p, h) in s.tehais.iter().enumerate() {
                        for t in h {
                            hands[p][t.kind().index()] += 1;
                        }
                    }
                    pile = s.pile.clone();
                    inds = s.dora_indicators.clone();
                    uras = s.ura_indicators.clone();
                }
                Event::Tsumo {
                    actor,
                    pai,
                    rinshan: r,
                } => {
                    hands[usize::from(*actor)][pai.kind().index()] += 1;
                    if *r {
                        rinshan += 1;
                    } else {
                        taken += 1;
                    }
                }
                Event::Dahai { actor, pai, .. } => {
                    dec(&mut hands[usize::from(*actor)], pai.kind().index())
                }
                Event::Chi {
                    actor, consumed, ..
                }
                | Event::Pon {
                    actor, consumed, ..
                }
                | Event::Daiminkan {
                    actor, consumed, ..
                }
                | Event::Ankan { actor, consumed } => {
                    for t in consumed {
                        dec(&mut hands[usize::from(*actor)], t.kind().index());
                    }
                    melds[usize::from(*actor)] += 1;
                }
                Event::Kakan { actor, pai, .. } => {
                    dec(&mut hands[usize::from(*actor)], pai.kind().index())
                }
                Event::Kita { actor, pai } => {
                    dec(&mut hands[usize::from(*actor)], pai.kind().index())
                }
                Event::Dora { .. } => revealed += 1,
                _ => {}
            }
        }
        // ツモ山の残り：通常のツモで引いていない先頭側と、嶺上で引いていない末尾側
        let end = pile.len().saturating_sub(rinshan);
        for t in pile.iter().take(end).skip(taken) {
            wall[t.kind().index()] += 1;
        }
        for t in inds.iter().skip(revealed) {
            wall[t.kind().index()] += 1;
        }
        for t in uras.iter().take(revealed) {
            ura[t.kind().index()] += 1.0;
        }
        let mut f = Vec::with_capacity(N_CRITIC);
        for r in 1..4u8 {
            if r >= n {
                f.extend([0.0; NUM_KINDS]);
                continue;
            }
            let p = usize::from((seat + r) % n);
            f.extend(hands[p].iter().map(|&x| f32::from(x) / 4.0));
        }
        f.extend(wall.iter().map(|&x| f32::from(x) / 4.0));
        f.extend(ura.iter().map(|x| x / 4.0));
        for r in 1..4u8 {
            if r >= n {
                f.push(0.0);
                continue;
            }
            let p = usize::from((seat + r) % n);
            let c = hands[p];
            f.push((f32::from(self.calc.shanten(&c, melds[p])) + 1.0) / 8.0);
        }
        debug_assert_eq!(f.len(), N_CRITIC);
        f
    }
}

/// 席視点のイベント列をなぞった状態
struct State {
    bakaze: usize,
    kyoku: u8,
    honba: u32,
    kyotaku: u32,
    oya: u8,
    scores: Vec<i32>,
    own: KindCounts,
    own_red: [u8; NUM_KINDS],
    drawn: Option<Tile>,
    draws: u32,
    discards: Vec<Vec<Tile>>,
    called_from: Vec<u8>,
    riichi: Vec<bool>,
    pending_reach: Option<u8>,
    melds: Vec<u8>,
    kita: Vec<u8>,
    indicators: Vec<Tile>,
    /// 自分の手牌以外で見えている枚数（河・副露・北抜き・表示牌）
    visible: [u8; NUM_KINDS],
}

impl State {
    fn new(n: u8) -> Self {
        let n = usize::from(n);
        Self {
            bakaze: 27,
            kyoku: 1,
            honba: 0,
            kyotaku: 0,
            oya: 0,
            scores: vec![0; n],
            own: [0; NUM_KINDS],
            own_red: [0; NUM_KINDS],
            drawn: None,
            draws: 0,
            discards: vec![Vec::new(); n],
            called_from: vec![0; n],
            riichi: vec![false; n],
            pending_reach: None,
            melds: vec![0; n],
            kita: vec![0; n],
            indicators: Vec::new(),
            visible: [0; NUM_KINDS],
        }
    }

    fn add_own(&mut self, t: Tile) {
        self.own[t.kind().index()] += 1;
        if t.is_red() {
            self.own_red[t.kind().index()] += 1;
        }
    }

    fn remove_own(&mut self, t: Tile) {
        self.own[t.kind().index()] -= 1;
        if t.is_red() {
            self.own_red[t.kind().index()] -= 1;
        }
    }

    fn visible_with_own(&self) -> [u8; NUM_KINDS] {
        let mut v = self.visible;
        for k in 0..NUM_KINDS {
            v[k] = (v[k] + self.own[k]).min(4);
        }
        v
    }
}

/// 順位（1 始まり、同点は起家に近い方が上。`table.tie_break = seat_order` と同じ。
/// 局の途中の順位なので、起家は「東1局の親」＝席 0 とする）
fn ranks_of(scores: &[i32], _oya: u8) -> Vec<u8> {
    let n = scores.len();
    (0..n)
        .map(|i| {
            1 + (0..n)
                .filter(|&j| scores[j] > scores[i] || (scores[j] == scores[i] && j < i))
                .count() as u8
        })
        .collect()
}

/// ルール設定の特徴：カタログの「打ち方に効く」項目（`Item::play`）を、カタログの順に数値にする
/// （真偽値は 0/1、整数は 10000 で割る（点の項目）か 10 で割る、順位点は人数分、列挙は one-hot、集合は multi-hot）
pub fn rule_features(rules: &Rules) -> Vec<f32> {
    let t = rules_to_table(rules);
    let mut f = Vec::new();
    for it in CATALOG.iter().filter(|i| i.play) {
        let v = lookup(&t, it.key).unwrap_or_else(|| panic!("{} missing", it.key));
        match it.kind {
            Kind::Bool { .. } => f.push(f32::from(u8::from(v.as_bool().unwrap_or(false)))),
            Kind::Int { unit, .. } => {
                let x = v.as_integer().unwrap_or(0) as f32;
                f.push(if unit == "点" { x / 10000.0 } else { x / 10.0 });
            }
            Kind::IntPerRank { .. } => {
                let a = v.as_array().cloned().unwrap_or_default();
                for i in 0..4 {
                    f.push(a.get(i).and_then(|x| x.as_integer()).unwrap_or(0) as f32 / 10000.0);
                }
            }
            Kind::Enum(vals) => {
                let s = v.as_str().unwrap_or("");
                for (name, _) in vals {
                    f.push(f32::from(u8::from(*name == s)));
                }
            }
            Kind::EnumSet(vals) => {
                let a = v.as_array().cloned().unwrap_or_default();
                for (name, _) in vals {
                    f.push(f32::from(u8::from(
                        a.iter().any(|x| x.as_str() == Some(name)),
                    )));
                }
            }
        }
    }
    f
}
