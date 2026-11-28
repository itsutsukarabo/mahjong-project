//! 牌譜の再生：イベントを1つずつ当てて、各時点の卓の状態（[`TableView`]）を作る。牌譜再生画面（`web/`）は
//! これを WASM（`crates/mj-wasm`）経由で受け取って描くだけにする（画面側にルールの処理を書かない。原則1）。
//!
//! - 和了判定・点数計算・合法手の判定はしない。牌譜に書かれたこと（ツモ・打牌・鳴き・点数）をそのまま積み上げる
//! - 全情報版なら全員の手牌が分かる。公開情報版・席視点版では見えない牌は `?` のまま数だけ合わせる
//! - ツモ山の残りは、ルールの牌の枚数・王牌の枚数とツモの回数から数える（LOG_FORMAT §4.2）
//! - 局の区切りは `kyoku_idx` で行い、`event_idx` の連番を確かめる（行の並びに頼らない。CLAUDE.md）

use crate::schema::{self, Body, LogEvent, Meld};
use mj_core::rules::Rules;
use mj_core::tile::Tile;
use serde::Serialize;
use std::collections::BTreeMap;

#[cfg(feature = "ts")]
use ts_rs::TS;

/// 河の1枚
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct RiverTile {
    pub pai: String,
    pub tsumogiri: bool,
    /// 立直の宣言牌
    pub riichi: bool,
    /// 鳴かれた牌（チー・ポン・大明槓、ロンされた牌は `ron`）
    pub called: bool,
    /// ロンされた牌
    pub ron: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct SeatView {
    pub seat: u8,
    pub name: String,
    /// 自風（`E` `S` `W` `N`）
    pub wind: String,
    pub score: i32,
    /// 門前の手牌（整列済み。ツモった牌は `drawn` に分けて持つ。見えない牌は `?`）
    pub hand: Vec<String>,
    pub drawn: Option<String>,
    pub river: Vec<RiverTile>,
    pub melds: Vec<Meld>,
    pub kita: Vec<String>,
    pub riichi: bool,
}

/// 局の結果（和了・流局）
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct ResultView {
    pub horas: Vec<schema::Hora>,
    pub ryukyoku: Option<schema::Ryukyoku>,
}

/// ある時点（`event_idx` のイベントを当てた直後）の卓
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct TableView {
    pub kyoku_idx: i32,
    pub event_idx: u32,
    /// このイベントの種類と行動した席
    pub event_type: String,
    pub actor: Option<u8>,
    pub bakaze: String,
    pub kyoku: u8,
    pub honba: u32,
    pub kyotaku: u32,
    pub oya: u8,
    pub dora_indicators: Vec<String>,
    /// 通常のツモの残り
    pub live_remaining: u32,
    /// 巡目（親から数えた、通常のツモの周回。1 始まり）
    pub junme: u32,
    pub seats: Vec<SeatView>,
    pub result: Option<ResultView>,
}

/// 局の見出し（局の一覧）
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct KyokuSummary {
    pub kyoku_idx: i32,
    pub bakaze: String,
    pub kyoku: u8,
    pub honba: u32,
    /// `hora` / 流局の種類 / 途中までの牌譜なら `unfinished`
    pub end: String,
    pub events: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct ReplayMeta {
    pub game_id: String,
    pub visibility: String,
    pub num_players: u8,
    pub players: Vec<String>,
    pub kyokus: Vec<KyokuSummary>,
    pub game_end: Option<schema::GameEnd>,
}

/// 型で読んだ牌譜（局ごとに分けたもの）
pub struct Replay {
    start: schema::GameStart,
    game_id: String,
    rules: Rules,
    kyokus: BTreeMap<i32, Vec<LogEvent>>,
    end: Option<schema::GameEnd>,
}

const WINDS: [&str; 4] = ["E", "S", "W", "N"];

fn sort_tiles(v: &mut [String]) {
    v.sort_by_key(|s| match s.parse::<Tile>() {
        Ok(t) => (0, Some(t)),
        Err(_) => (1, None),
    });
}

/// 手牌から1枚除く（なければ `?` を1枚除く。見えない手）
fn take(hand: &mut Vec<String>, pai: &str) -> Result<(), String> {
    if let Some(i) = hand.iter().position(|x| x == pai) {
        hand.remove(i);
        return Ok(());
    }
    if let Some(i) = hand.iter().position(|x| x == "?") {
        hand.remove(i);
        return Ok(());
    }
    Err(format!("{pai} is not in the hand {hand:?}"))
}

impl Replay {
    pub fn from_jsonl(text: &str) -> Result<Self, String> {
        Self::from_events(schema::parse_events(text)?)
    }

    pub fn from_events(events: Vec<LogEvent>) -> Result<Self, String> {
        let first = events.first().ok_or("empty log")?;
        let Body::GameStart(start) = &first.body else {
            return Err("the first event is not game_start".into());
        };
        let start = start.clone();
        let rules: Rules =
            serde_json::from_value(start.rules.clone()).map_err(|e| format!("rules: {e}"))?;
        let game_id = first.game_id.clone();
        let mut kyokus: BTreeMap<i32, Vec<LogEvent>> = BTreeMap::new();
        let mut end = None;
        for e in events {
            if e.game_id != game_id {
                return Err(format!("game_id {} differs from {game_id}", e.game_id));
            }
            if e.kyoku_idx < 0 {
                if let Body::GameEnd(g) = &e.body {
                    end = Some(g.clone());
                }
                continue;
            }
            kyokus.entry(e.kyoku_idx).or_default().push(e);
        }
        for (k, v) in &kyokus {
            for (i, e) in v.iter().enumerate() {
                if e.event_idx as usize != i {
                    return Err(format!(
                        "kyoku {k}: event_idx {} at position {i}",
                        e.event_idx
                    ));
                }
            }
            if !matches!(v[0].body, Body::KyokuStart(_)) {
                return Err(format!("kyoku {k} does not start with kyoku_start"));
            }
        }
        Ok(Self {
            start,
            game_id,
            rules,
            kyokus,
            end,
        })
    }

    pub fn num_players(&self) -> u8 {
        self.rules.table.num_players
    }

    pub fn meta(&self) -> ReplayMeta {
        let kyokus = self
            .kyokus
            .iter()
            .map(|(&k, v)| {
                let Body::KyokuStart(ks) = &v[0].body else {
                    unreachable!("checked")
                };
                let end = v
                    .iter()
                    .find_map(|e| match &e.body {
                        Body::Hora(_) => Some("hora".to_string()),
                        Body::Ryukyoku(r) => Some(r.reason.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "unfinished".into());
                KyokuSummary {
                    kyoku_idx: k,
                    bakaze: ks.bakaze.clone(),
                    kyoku: ks.kyoku,
                    honba: ks.honba,
                    end,
                    events: v.len() as u32,
                }
            })
            .collect();
        ReplayMeta {
            game_id: self.game_id.clone(),
            visibility: self.start.visibility.clone(),
            num_players: self.num_players(),
            players: self.start.players.iter().map(|p| p.name.clone()).collect(),
            kyokus,
            game_end: self.end.clone(),
        }
    }

    /// 1局の、各イベントを当てた直後の卓（`event_idx` の順）
    pub fn kyoku_states(&self, kyoku_idx: i32) -> Result<Vec<TableView>, String> {
        let events = self
            .kyokus
            .get(&kyoku_idx)
            .ok_or_else(|| format!("no kyoku {kyoku_idx}"))?;
        let n = usize::from(self.num_players());
        let Body::KyokuStart(ks) = &events[0].body else {
            unreachable!("checked")
        };
        let total = mj_core::wall::tile_multiset(&self.rules.tiles).len();
        let slots = usize::from(self.rules.wall.dora_indicator_slots);
        let pile = total - 13 * n - 2 * slots;
        let reserve = usize::from(self.rules.wall.dead_wall) - 2 * slots;
        let live0 = (pile - reserve) as u32;
        let wind_of = |s: usize| WINDS[(s + n - usize::from(ks.oya)) % n].to_string();
        let mut seats: Vec<SeatView> = (0..n)
            .map(|s| {
                let mut hand = ks.tehais.get(s).cloned().unwrap_or_default();
                sort_tiles(&mut hand);
                SeatView {
                    seat: s as u8,
                    name: self
                        .start
                        .players
                        .iter()
                        .find(|p| usize::from(p.seat) == s)
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                    wind: wind_of(s),
                    score: ks.scores.get(s).copied().unwrap_or(0),
                    hand,
                    drawn: None,
                    river: Vec::new(),
                    melds: Vec::new(),
                    kita: Vec::new(),
                    riichi: false,
                }
            })
            .collect();
        let mut dora = vec![ks.dora_indicator.clone()];
        let (mut honba, mut kyotaku) = (ks.honba, ks.kyotaku);
        let mut draws = 0u32;
        let mut normal_draws = 0u32;
        let mut pending_riichi: Option<u8> = None;
        let mut last_discard: Option<(usize, usize)> = None;
        let mut result: Option<ResultView> = None;
        let mut out = Vec::with_capacity(events.len());
        // ツモった牌を手に入れる（打牌・槓・北抜きの前）
        fn merge_drawn(s: &mut SeatView) {
            if let Some(d) = s.drawn.take() {
                s.hand.push(d);
                sort_tiles(&mut s.hand);
            }
        }
        for e in events {
            let ctx = |m: String| format!("kyoku {kyoku_idx} event {}: {m}", e.event_idx);
            let mut actor = None;
            match &e.body {
                Body::KyokuStart(_) => {}
                Body::Tsumo(t) => {
                    let s = &mut seats[usize::from(t.actor)];
                    merge_drawn(s);
                    s.drawn = Some(t.pai.clone());
                    draws += 1;
                    if t.rinshan != Some(true) {
                        normal_draws += 1;
                    }
                    actor = Some(t.actor);
                }
                Body::Dahai(d) => {
                    let a = usize::from(d.actor);
                    let s = &mut seats[a];
                    if d.tsumogiri && s.drawn.is_some() {
                        s.drawn = None;
                    } else {
                        merge_drawn(s);
                        take(&mut s.hand, &d.pai).map_err(ctx)?;
                    }
                    let riichi = pending_riichi == Some(d.actor);
                    if riichi {
                        pending_riichi = None;
                    }
                    s.river.push(RiverTile {
                        pai: d.pai.clone(),
                        tsumogiri: d.tsumogiri,
                        riichi,
                        called: false,
                        ron: false,
                    });
                    last_discard = Some((a, s.river.len() - 1));
                    actor = Some(d.actor);
                }
                Body::Reach(r) => {
                    pending_riichi = Some(r.actor);
                    actor = Some(r.actor);
                }
                Body::ReachAccepted(r) => {
                    seats[usize::from(r.actor)].riichi = true;
                    for (s, v) in seats.iter_mut().zip(&r.scores) {
                        s.score = *v;
                    }
                    kyotaku = r.kyotaku;
                    actor = Some(r.actor);
                }
                Body::Chi(c) | Body::Pon(c) | Body::Daiminkan(c) => {
                    let kind = match &e.body {
                        Body::Chi(_) => "chi",
                        Body::Pon(_) => "pon",
                        _ => "daiminkan",
                    };
                    if let Some(r) = seats[usize::from(c.target)].river.last_mut() {
                        r.called = true;
                    }
                    let s = &mut seats[usize::from(c.actor)];
                    for t in &c.consumed {
                        take(&mut s.hand, t).map_err(ctx)?;
                    }
                    s.melds.push(Meld {
                        kind: kind.into(),
                        pai: Some(c.pai.clone()),
                        consumed: c.consumed.clone(),
                        target: Some(c.target),
                    });
                    actor = Some(c.actor);
                }
                Body::Ankan(k) => {
                    let s = &mut seats[usize::from(k.actor)];
                    merge_drawn(s);
                    for t in &k.consumed {
                        take(&mut s.hand, t).map_err(ctx)?;
                    }
                    s.melds.push(Meld {
                        kind: "ankan".into(),
                        pai: None,
                        consumed: k.consumed.clone(),
                        target: None,
                    });
                    actor = Some(k.actor);
                }
                Body::Kakan(k) => {
                    let s = &mut seats[usize::from(k.actor)];
                    merge_drawn(s);
                    take(&mut s.hand, &k.pai).map_err(ctx)?;
                    // 元のポン（consumed は元のポンの3枚）を加槓に変える
                    let same = |m: &Meld| {
                        let mut a: Vec<String> =
                            m.consumed.iter().cloned().chain(m.pai.clone()).collect();
                        let mut b = k.consumed.clone();
                        sort_tiles(&mut a);
                        sort_tiles(&mut b);
                        m.kind == "pon" && a == b
                    };
                    let i = s
                        .melds
                        .iter()
                        .position(same)
                        .ok_or_else(|| ctx("kakan without the pon".into()))?;
                    s.melds[i].kind = "kakan".into();
                    s.melds[i].consumed.push(k.pai.clone());
                    actor = Some(k.actor);
                }
                Body::Kita(k) => {
                    let s = &mut seats[usize::from(k.actor)];
                    merge_drawn(s);
                    take(&mut s.hand, &k.pai).map_err(ctx)?;
                    s.kita.push(k.pai.clone());
                    actor = Some(k.actor);
                }
                Body::Dora(d) => dora.push(d.dora_indicator.clone()),
                Body::Hora(h) => {
                    if h.actor != h.target
                        && let Some((a, i)) = last_discard
                        && a == usize::from(h.target)
                    {
                        seats[a].river[i].ron = true;
                    }
                    // 和了の手は牌譜の hand に書かれている（見えない手もここで分かる）
                    let s = &mut seats[usize::from(h.actor)];
                    s.hand = h.hand.tehai.clone();
                    sort_tiles(&mut s.hand);
                    for (s, v) in seats.iter_mut().zip(&h.scores) {
                        s.score = *v;
                    }
                    result
                        .get_or_insert_with(|| ResultView {
                            horas: Vec::new(),
                            ryukyoku: None,
                        })
                        .horas
                        .push(h.clone());
                    actor = Some(h.actor);
                }
                Body::Ryukyoku(r) => {
                    for (s, h) in seats.iter_mut().zip(&r.hands) {
                        if let Some(h) = h {
                            merge_drawn(s);
                            s.hand = h.clone();
                            sort_tiles(&mut s.hand);
                        }
                    }
                    for (s, v) in seats.iter_mut().zip(&r.scores) {
                        s.score = *v;
                    }
                    result = Some(ResultView {
                        horas: Vec::new(),
                        ryukyoku: Some(r.clone()),
                    });
                    actor = r.actor;
                }
                Body::KyokuEnd(k) => {
                    for (s, v) in seats.iter_mut().zip(&k.scores) {
                        s.score = *v;
                    }
                    kyotaku = k.kyotaku;
                    honba = ks.honba;
                }
                Body::GameStart(_) | Body::GameEnd(_) => {
                    return Err(ctx("game event inside a kyoku".into()));
                }
            }
            out.push(TableView {
                kyoku_idx,
                event_idx: e.event_idx,
                event_type: event_type(&e.body).into(),
                actor,
                bakaze: ks.bakaze.clone(),
                kyoku: ks.kyoku,
                honba,
                kyotaku,
                oya: ks.oya,
                dora_indicators: dora.clone(),
                live_remaining: live0.saturating_sub(draws),
                junme: normal_draws.saturating_sub(1) / n as u32 + 1,
                seats: seats.clone(),
                result: result.clone(),
            });
        }
        Ok(out)
    }
}

pub fn event_type(b: &Body) -> &'static str {
    match b {
        Body::GameStart(_) => "game_start",
        Body::KyokuStart(_) => "kyoku_start",
        Body::Tsumo(_) => "tsumo",
        Body::Dahai(_) => "dahai",
        Body::Reach(_) => "reach",
        Body::ReachAccepted(_) => "reach_accepted",
        Body::Chi(_) => "chi",
        Body::Pon(_) => "pon",
        Body::Daiminkan(_) => "daiminkan",
        Body::Ankan(_) => "ankan",
        Body::Kakan(_) => "kakan",
        Body::Kita(_) => "kita",
        Body::Dora(_) => "dora",
        Body::Hora(_) => "hora",
        Body::Ryukyoku(_) => "ryukyoku",
        Body::KyokuEnd(_) => "kyoku_end",
        Body::GameEnd(_) => "game_end",
    }
}
