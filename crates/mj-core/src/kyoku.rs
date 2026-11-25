//! 局進行：局の状態・合法手・状態遷移（判断ポイント単位、D-022）。
//!
//! API の基本単位は判断ポイント（`docs/spec/DECISION_POINTS.md`）：
//! [`Kyoku::decisions`] が「今どの席に、どの選択肢を提示しているか」を返し、
//! [`Kyoku::step`] で選ばれた行動を受け取って次の判断ポイントまで進める。
//! 選択肢が1つしかない席は判断ポイントにせず、エンジンが自動で選ぶ。
//!
//! 選択肢の並びは [`Action`] の順序（DECISION_POINTS §5）で固定する。乱数は使わない（決定論）。
//! 人数は `rules.table.num_players`（3 か 4）。

use crate::action::Action;
use crate::agari;
use crate::event::*;
use crate::rules::*;
use crate::tile::{KindCounts, NUM_KINDS, Tile, TileKind};
use crate::wall::Wall;
use crate::yaku::{self, OpenKind, OpenSet, RiichiKind, WinInput, WinResult, Yaku, round_up_100};
use std::sync::Arc;

const NORTH: usize = 30;

/// 局の始まりの状態。
#[derive(Debug, Clone)]
pub struct KyokuSetup {
    /// 場風（0 = 東）
    pub bakaze: u8,
    /// その場の何局目か（0 始まり）
    pub kyoku: u8,
    pub honba: u32,
    pub kyotaku: u32,
    pub oya: u8,
    pub scores: Vec<i32>,
    pub wall: Wall,
}

/// 判断ポイント：ある席に提示した選択肢（2つ以上、DECISION_POINTS §5 の並び）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub seat: u8,
    pub choices: Vec<Action>,
}

/// 局の結果（次の局を決めるのに使う）。
#[derive(Debug, Clone, PartialEq)]
pub struct KyokuOutcome {
    pub hora: bool,
    pub renchan: bool,
    pub honba_next: u32,
    pub kyotaku_next: u32,
    pub scores: Vec<i32>,
    pub chip_deltas: Vec<i32>,
    /// 荒牌流局・途中流局のときの親の聴牌（和了なら false）
    pub oya_tenpai: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError(pub String);

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "engine error: {}", self.0)
    }
}

impl std::error::Error for EngineError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeldKind {
    Chi,
    Pon,
    Daiminkan,
    Kakan,
    Ankan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meld {
    pub kind: MeldKind,
    /// 面子のすべての牌（鳴いた牌を含む）
    pub tiles: Vec<Tile>,
    pub called: Option<Tile>,
    pub from: Option<u8>,
}

impl Meld {
    fn open_set(&self) -> OpenSet {
        let kind = match self.kind {
            MeldKind::Chi => OpenKind::Chi,
            MeldKind::Pon => OpenKind::Pon,
            MeldKind::Daiminkan | MeldKind::Kakan => OpenKind::Minkan,
            MeldKind::Ankan => OpenKind::Ankan,
        };
        let tile = self
            .tiles
            .iter()
            .map(|t| t.kind())
            .min()
            .expect("non-empty meld");
        OpenSet { kind, tile }
    }

    fn is_open(&self) -> bool {
        self.kind != MeldKind::Ankan
    }

    fn is_kan(&self) -> bool {
        matches!(
            self.kind,
            MeldKind::Daiminkan | MeldKind::Kakan | MeldKind::Ankan
        )
    }

    fn view(&self) -> MeldView {
        let kind = match self.kind {
            MeldKind::Chi => "chi",
            MeldKind::Pon => "pon",
            MeldKind::Daiminkan => "daiminkan",
            MeldKind::Kakan => "kakan",
            MeldKind::Ankan => "ankan",
        };
        let mut consumed = self.tiles.clone();
        if let Some(c) = self.called
            && let Some(i) = consumed.iter().position(|t| *t == c)
        {
            consumed.remove(i);
        }
        MeldView {
            kind,
            pai: self.called,
            consumed,
            target: self.from,
        }
    }
}

fn red_slot(k: TileKind) -> Option<usize> {
    match k.index() {
        4 => Some(0),
        13 => Some(1),
        22 => Some(2),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct Player {
    c: KindCounts,
    red: [u8; 3],
    melds: Vec<Meld>,
    kita: u8,
    discards: Vec<Tile>,
    discard_mask: u64,
    /// 河が么九牌だけで、鳴かれていない（流し満貫の条件）
    nagashi: bool,
    riichi: Option<RiichiKind>,
    riichi_accepted: bool,
    ippatsu: bool,
    furiten_temp: bool,
    furiten_riichi: bool,
    /// 門前の牌が 3n+1 枚のときの待ち
    waits: u64,
    first_discard_done: bool,
    pao: Vec<(PaoYaku, u8)>,
    declared_noten: bool,
}

impl Player {
    fn new(hand: &[Tile]) -> Self {
        let mut p = Player {
            c: [0; NUM_KINDS],
            red: [0; 3],
            melds: Vec::new(),
            kita: 0,
            discards: Vec::new(),
            discard_mask: 0,
            nagashi: true,
            riichi: None,
            riichi_accepted: false,
            ippatsu: false,
            furiten_temp: false,
            furiten_riichi: false,
            waits: 0,
            first_discard_done: false,
            pao: Vec::new(),
            declared_noten: false,
        };
        for t in hand {
            p.add(*t);
        }
        p
    }

    fn add(&mut self, t: Tile) {
        self.c[t.kind().index()] += 1;
        if t.is_red() {
            self.red[red_slot(t.kind()).expect("red five")] += 1;
        }
    }

    fn red_of(&self, k: TileKind) -> u8 {
        red_slot(k).map_or(0, |i| self.red[i])
    }

    fn has(&self, t: Tile) -> bool {
        let k = t.kind();
        if t.is_red() {
            self.red_of(k) > 0
        } else {
            self.c[k.index()] > self.red_of(k)
        }
    }

    fn remove(&mut self, t: Tile) {
        assert!(self.has(t), "tile {t} not in hand");
        self.c[t.kind().index()] -= 1;
        if t.is_red() {
            self.red[red_slot(t.kind()).expect("red five")] -= 1;
        }
    }

    /// 門前の牌（整列済み）
    fn tiles(&self) -> Vec<Tile> {
        let mut out = Vec::with_capacity(14);
        for k in TileKind::all() {
            let n = self.c[k.index()];
            let r = self.red_of(k);
            for i in 0..n {
                out.push(Tile::new(k, i < r).expect("red only on fives"));
            }
        }
        out
    }

    /// 門前の牌の種類（赤は別）ごとに1つずつ
    fn distinct_tiles(&self) -> Vec<Tile> {
        let mut out = Vec::new();
        for k in TileKind::all() {
            let n = self.c[k.index()];
            if n == 0 {
                continue;
            }
            let r = self.red_of(k);
            if r > 0 {
                out.push(Tile::new(k, true).expect("five"));
            }
            if n > r {
                out.push(Tile::plain(k));
            }
        }
        out
    }

    fn closed(&self) -> bool {
        self.melds.iter().all(|m| !m.is_open())
    }

    fn sets(&self) -> Vec<OpenSet> {
        self.melds.iter().map(Meld::open_set).collect()
    }

    fn update_waits(&mut self) {
        self.waits = agari::waits(&self.c, self.melds.is_empty());
    }

    fn furiten(&self) -> bool {
        self.furiten_temp || self.furiten_riichi || self.waits & self.discard_mask != 0
    }

    fn aka_total(&self) -> u8 {
        let in_melds = self
            .melds
            .iter()
            .flat_map(|m| m.tiles.iter())
            .filter(|t| t.is_red())
            .count() as u8;
        self.red.iter().sum::<u8>() + in_melds
    }

    /// 手牌・副露にある種類 k の枚数
    fn held(&self, k: usize) -> u8 {
        self.c[k]
            + self
                .melds
                .iter()
                .flat_map(|m| m.tiles.iter())
                .filter(|t| t.kind().index() == k)
                .count() as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RespondKind {
    Discard,
    Kakan,
    Kita,
    Ankan,
}

#[derive(Debug, Clone)]
enum Phase {
    /// ツモ・嶺上ツモ・鳴いた後の、手番の人の判断
    Turn {
        actor: u8,
        rinshan: bool,
        /// 鳴いた直後なら、喰い替えで切れない種類（ビット）。`None` ならツモの後
        after_call: Option<(u64, MeldKind)>,
    },
    /// 打牌・加槓・北抜き・暗槓への応答
    Respond {
        kind: RespondKind,
        actor: u8,
        tile: Tile,
        riichi_pending: bool,
        houtei: bool,
    },
    Done,
}

/// 局の状態。
#[derive(Debug, Clone)]
pub struct Kyoku {
    rules: Arc<Rules>,
    n: u8,
    bakaze: u8,
    honba: u32,
    kyotaku: u32,
    oya: u8,
    scores: Vec<i32>,
    pile: Vec<Tile>,
    front: usize,
    back: usize,
    reserve: usize,
    dora_indicators: Vec<Tile>,
    ura_indicators: Vec<Tile>,
    revealed: usize,
    pending_dora: usize,
    rinshan_draws: u8,
    kans: Vec<u8>,
    players: Vec<Player>,
    drawn: Option<Tile>,
    /// 第1巡に鳴き・暗槓・（ルールにより）北抜きがまだない
    uninterrupted: bool,
    discard_total: usize,
    first_winds: Vec<TileKind>,
    suukaikan_pending: bool,
    events: Vec<Event>,
    phase: Phase,
    /// 今の場の各席の選択肢（1つだけの席も含む）
    options: Vec<(u8, Vec<Action>)>,
    decisions: Vec<Decision>,
    outcome: Option<KyokuOutcome>,
    chip_deltas: Vec<i32>,
    chip_items: Vec<ChipItem>,
}

impl Kyoku {
    pub fn new(rules: Arc<Rules>, setup: KyokuSetup) -> Self {
        let n = rules.table.num_players;
        assert_eq!(setup.wall.haipai.len(), usize::from(n));
        assert_eq!(setup.scores.len(), usize::from(n));
        let reserve = rules.wall.reserve();
        let players: Vec<Player> = setup
            .wall
            .haipai
            .iter()
            .map(|h| {
                let mut p = Player::new(h);
                p.update_waits();
                p
            })
            .collect();
        let pile = setup.wall.pile.clone();
        let back = pile.len();
        let mut k = Kyoku {
            n,
            bakaze: setup.bakaze,
            honba: setup.honba,
            kyotaku: setup.kyotaku,
            oya: setup.oya,
            scores: setup.scores.clone(),
            pile,
            front: 0,
            back,
            reserve,
            dora_indicators: setup.wall.dora_indicators.clone(),
            ura_indicators: setup.wall.ura_indicators.clone(),
            revealed: 1,
            pending_dora: 0,
            rinshan_draws: 0,
            kans: vec![0; usize::from(n)],
            players,
            drawn: None,
            uninterrupted: true,
            discard_total: 0,
            first_winds: Vec::new(),
            suukaikan_pending: false,
            events: Vec::new(),
            phase: Phase::Done,
            options: Vec::new(),
            decisions: Vec::new(),
            outcome: None,
            chip_deltas: vec![0; usize::from(n)],
            chip_items: Vec::new(),
            rules,
        };
        k.events.push(Event::KyokuStart(Box::new(KyokuStart {
            bakaze: Tile::plain(TileKind::new(27 + setup.bakaze).expect("wind")),
            kyoku: setup.kyoku + 1,
            honba: setup.honba,
            kyotaku: setup.kyotaku,
            oya: setup.oya,
            scores: setup.scores,
            dora_indicator: k.dora_indicators[0],
            tehais: setup.wall.haipai.clone(),
            pile: setup.wall.pile,
            dora_indicators: setup.wall.dora_indicators,
            ura_indicators: setup.wall.ura_indicators,
        })));
        k.draw(k.oya, false);
        k.advance();
        k
    }

    // ------------------------------------------------------------ 公開 API

    /// 今の判断ポイント（席の昇順）。空なら局は終わっている。
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn outcome(&self) -> Option<&KyokuOutcome> {
        self.outcome.as_ref()
    }

    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// 判断ポイントのすべての席の行動を受け取って進める。
    pub fn step(&mut self, actions: &[(u8, Action)]) -> Result<(), EngineError> {
        if self.decisions.is_empty() {
            return Err(EngineError("no decision is pending".into()));
        }
        for d in &self.decisions {
            let mine: Vec<&Action> = actions
                .iter()
                .filter(|(s, _)| *s == d.seat)
                .map(|(_, a)| a)
                .collect();
            match mine.as_slice() {
                [a] if d.choices.contains(a) => {}
                [a] => return Err(EngineError(format!("seat {}: {a} is not a choice", d.seat))),
                _ => {
                    return Err(EngineError(format!(
                        "seat {} needs exactly one action",
                        d.seat
                    )));
                }
            }
        }
        if let Some((s, _)) = actions
            .iter()
            .find(|(s, _)| !self.decisions.iter().any(|d| d.seat == *s))
        {
            return Err(EngineError(format!("seat {s} has no decision")));
        }
        let answers: Vec<(u8, Action)> = self
            .options
            .iter()
            .map(|(s, c)| {
                let a = actions
                    .iter()
                    .find(|(t, _)| t == s)
                    .map(|(_, a)| *a)
                    .unwrap_or(c[0]);
                (*s, a)
            })
            .collect();
        self.decisions.clear();
        self.apply(answers);
        self.advance();
        Ok(())
    }

    // ------------------------------------------------------------ 進行の骨組み

    fn advance(&mut self) {
        loop {
            if matches!(self.phase, Phase::Done) {
                self.options.clear();
                self.decisions.clear();
                return;
            }
            self.options = self.compute_options();
            self.decisions = self
                .options
                .iter()
                .filter(|(_, c)| c.len() >= 2)
                .map(|(s, c)| Decision {
                    seat: *s,
                    choices: c.clone(),
                })
                .collect();
            if !self.decisions.is_empty() {
                return;
            }
            let answers: Vec<(u8, Action)> = self.options.iter().map(|(s, c)| (*s, c[0])).collect();
            self.apply(answers);
        }
    }

    fn apply(&mut self, answers: Vec<(u8, Action)>) {
        match self.phase.clone() {
            Phase::Turn { actor, rinshan, .. } => {
                let a = answers
                    .iter()
                    .find(|(s, _)| *s == actor)
                    .map(|(_, a)| *a)
                    .expect("actor answers");
                self.apply_turn(actor, a, rinshan);
            }
            Phase::Respond {
                kind,
                actor,
                tile,
                riichi_pending,
                houtei,
            } => self.resolve_respond(kind, actor, tile, riichi_pending, houtei, &answers),
            Phase::Done => {}
        }
    }

    fn emit(&mut self, e: Event) {
        self.events.push(e);
    }

    fn next_seat(&self, s: u8) -> u8 {
        (s + 1) % self.n
    }

    /// 通常のツモで引ける残り枚数
    fn live(&self) -> usize {
        (self.back - self.front).saturating_sub(self.reserve)
    }

    fn can_rinshan(&self) -> bool {
        self.live() > 0 && self.rinshan_draws < self.rules.wall.rinshan_max
    }

    fn kan_total(&self) -> u8 {
        self.kans.iter().sum()
    }

    fn draw(&mut self, seat: u8, rinshan: bool) {
        let t = if rinshan {
            self.back -= 1;
            self.rinshan_draws += 1;
            self.pile[self.back]
        } else {
            let t = self.pile[self.front];
            self.front += 1;
            t
        };
        self.players[usize::from(seat)].add(t);
        self.drawn = Some(t);
        self.emit(Event::Tsumo {
            actor: seat,
            pai: t,
            rinshan,
        });
        self.phase = Phase::Turn {
            actor: seat,
            rinshan,
            after_call: None,
        };
    }

    fn reveal_dora(&mut self) {
        if self.rules.dora.kan_dora && self.revealed < self.dora_indicators.len() {
            let t = self.dora_indicators[self.revealed];
            self.revealed += 1;
            self.emit(Event::Dora { dora_indicator: t });
        }
    }

    fn flush_pending_dora(&mut self) {
        while self.pending_dora > 0 {
            self.pending_dora -= 1;
            self.reveal_dora();
        }
    }

    fn jikaze(&self, s: u8) -> TileKind {
        TileKind::new(27 + (s + self.n - self.oya) % self.n).expect("wind")
    }

    fn bakaze_kind(&self) -> TileKind {
        TileKind::new(27 + self.bakaze).expect("wind")
    }

    fn dora_kinds(&self) -> Vec<TileKind> {
        self.dora_indicators[..self.revealed]
            .iter()
            .map(|t| t.kind())
            .collect()
    }

    fn ura_tiles(&self) -> Vec<Tile> {
        let n = if self.rules.dora.kan_ura {
            self.revealed
        } else {
            1
        };
        self.ura_indicators[..n].to_vec()
    }

    fn break_ippatsu_all(&mut self) {
        for p in &mut self.players {
            p.ippatsu = false;
        }
    }

    // ------------------------------------------------------------ 和了の判定

    /// 和了の評価。`hand` は和了牌を含む門前の牌。`atozuke` の条件も確かめる。
    #[allow(clippy::too_many_arguments)]
    fn evaluate_win(
        &self,
        seat: u8,
        hand: &KindCounts,
        win: Tile,
        tsumo: bool,
        rinshan: bool,
        chankan: bool,
        houtei: bool,
    ) -> Option<WinResult> {
        let p = &self.players[usize::from(seat)];
        let sets = p.sets();
        let haitei = tsumo && !rinshan && self.live() == 0;
        let first_draw = tsumo && !rinshan && self.uninterrupted && !p.first_discard_done;
        let ura: Vec<TileKind> = if p.riichi.is_some() {
            self.ura_tiles().iter().map(|t| t.kind()).collect()
        } else {
            Vec::new()
        };
        let dora = self.dora_kinds();
        let aka = p.aka_total() + u8::from(!tsumo && win.is_red());
        let w = WinInput {
            rules: &self.rules,
            hand: *hand,
            win_tile: win.kind(),
            sets: &sets,
            tsumo,
            riichi: p.riichi.unwrap_or(RiichiKind::None),
            ippatsu: p.ippatsu,
            haitei,
            houtei,
            rinshan,
            chankan,
            first_draw,
            oya: seat == self.oya,
            bakaze: self.bakaze_kind(),
            jikaze: self.jikaze(seat),
            dora_indicators: &dora,
            ura_indicators: &ura,
            aka,
            kita: p.kita,
        };
        let r = yaku::evaluate(&w)?;
        if !self.rules.calls.atozuke && !p.closed() && !self.all_waits_have_yaku(seat, hand, win) {
            return None;
        }
        Some(r)
    }

    /// 後付けなし：13枚の形のすべての待ちで、手の形・場風・自風だけで役が付くか
    fn all_waits_have_yaku(&self, seat: u8, hand: &KindCounts, win: Tile) -> bool {
        let p = &self.players[usize::from(seat)];
        let mut before = *hand;
        before[win.kind().index()] -= 1;
        let waits = agari::waits(&before, false);
        let sets = p.sets();
        agari::mask_kinds(waits).all(|w| {
            let mut h = before;
            h[w.index()] += 1;
            let input = WinInput {
                rules: &self.rules,
                hand: h,
                win_tile: w,
                sets: &sets,
                tsumo: false,
                riichi: RiichiKind::None,
                ippatsu: false,
                haitei: false,
                houtei: false,
                rinshan: false,
                chankan: false,
                first_draw: false,
                oya: seat == self.oya,
                bakaze: self.bakaze_kind(),
                jikaze: self.jikaze(seat),
                dora_indicators: &[],
                ura_indicators: &[],
                aka: 0,
                kita: 0,
            };
            yaku::evaluate(&input).is_some()
        })
    }

    /// 他家の牌でロンできるか（`chankan`：槍槓・北抜きへのロンで槍槓が付く場合）
    fn ron_result(&self, seat: u8, tile: Tile, chankan: bool, houtei: bool) -> Option<WinResult> {
        let p = &self.players[usize::from(seat)];
        if p.waits & (1 << tile.kind().index()) == 0 || p.furiten() {
            return None;
        }
        let mut h = p.c;
        h[tile.kind().index()] += 1;
        self.evaluate_win(seat, &h, tile, false, false, chankan, houtei)
    }

    // ------------------------------------------------------------ 合法手

    fn compute_options(&self) -> Vec<(u8, Vec<Action>)> {
        match &self.phase {
            Phase::Turn {
                actor,
                rinshan,
                after_call,
            } => vec![(*actor, self.turn_options(*actor, *rinshan, *after_call))],
            Phase::Respond {
                kind,
                actor,
                tile,
                houtei,
                ..
            } => self.respond_options(*kind, *actor, *tile, *houtei),
            Phase::Done => Vec::new(),
        }
    }

    fn kuikae_ok(&self, forbidden: u64, t: Tile) -> bool {
        forbidden & (1 << t.kind().index()) == 0
    }

    fn turn_options(
        &self,
        actor: u8,
        rinshan: bool,
        after_call: Option<(u64, MeldKind)>,
    ) -> Vec<Action> {
        let rules = &self.rules;
        let p = &self.players[usize::from(actor)];
        let mut out = Vec::new();
        let n_kind = |k: usize| p.c[k];
        let kita_mode = rules.kita.mode == KitaMode::Nukidora;
        if let Some((forbidden, kind)) = after_call {
            // 鳴いた後：打牌（喰い替えの牌を除く）、北抜き（ポンの後、ルールで許すとき）
            let kita_ok = kita_mode
                && kind == MeldKind::Pon
                && rules.kita.after_pon
                && n_kind(NORTH) > 0
                && self.can_rinshan();
            let must_kita = kita_ok && rules.kita.mandatory;
            if kita_ok {
                out.push(Action::Kita);
            }
            if !must_kita {
                for t in p.distinct_tiles() {
                    if self.kuikae_ok(forbidden, t) {
                        out.push(Action::Dahai {
                            tile: t,
                            tsumogiri: false,
                        });
                    }
                }
            }
            out.sort();
            return out;
        }
        let drawn = self.drawn.expect("drawn tile after tsumo");
        let live = self.live();
        // ツモ和了
        if self
            .evaluate_win(actor, &p.c, drawn, true, rinshan, false, false)
            .is_some()
        {
            out.push(Action::Tsumo);
        }
        // 九種九牌
        if rules.draws.kyushu_kyuhai && self.uninterrupted && !p.first_discard_done && !rinshan {
            let kinds = TileKind::all()
                .filter(|k| k.is_terminal_or_honor() && p.c[k.index()] > 0)
                .count();
            if kinds >= 9 {
                out.push(Action::Kyushu);
            }
        }
        let kan_ok = self.kan_total() < 4 && self.can_rinshan();
        let riichi = p.riichi.is_some();
        // 暗槓
        if kan_ok {
            for k in TileKind::all().filter(|k| p.c[k.index()] == 4) {
                if riichi && !self.riichi_ankan_ok(actor, k, drawn) {
                    continue;
                }
                out.push(Action::Ankan(k));
            }
            // 加槓
            if !riichi {
                for m in p.melds.iter().filter(|m| m.kind == MeldKind::Pon) {
                    let k = m.tiles[0].kind();
                    if p.c[k.index()] > 0 {
                        let t = if p.red_of(k) > 0 {
                            Tile::new(k, true).expect("five")
                        } else {
                            Tile::plain(k)
                        };
                        out.push(Action::Kakan(t));
                    }
                }
            }
        }
        // 北抜き
        let north = Tile::plain(TileKind::new(NORTH as u8).expect("N"));
        let kita_ok = kita_mode
            && n_kind(NORTH) > 0
            && self.can_rinshan()
            && (!riichi
                || (rules.kita.after_riichi == KitaAfterRiichi::DrawnOnly && drawn == north));
        if kita_ok {
            out.push(Action::Kita);
        }
        let must_kita = kita_ok && rules.kita.mandatory;
        // 打牌（立直中はツモ切りだけ）
        let mut discards: Vec<(Tile, bool)> = Vec::new();
        if riichi {
            discards.push((drawn, true));
        } else {
            let mut rest = p.clone();
            rest.remove(drawn);
            for t in rest.distinct_tiles() {
                discards.push((t, false));
            }
            discards.push((drawn, true));
        }
        if must_kita {
            discards.retain(|(t, _)| t.kind().index() != NORTH);
        }
        // 立直
        let can_riichi = !riichi
            && p.closed()
            && self.scores[usize::from(actor)] >= rules.riichi.min_points
            && (!rules.riichi.requires_draw || live >= usize::from(self.n));
        for &(t, tsumogiri) in &discards {
            if can_riichi {
                let mut c = p.c;
                c[t.kind().index()] -= 1;
                let tenpai = agari::is_tenpai(&c, p.melds.is_empty());
                let furiten_ok = rules.riichi.furiten_riichi
                    || agari::waits(&c, p.melds.is_empty())
                        & (p.discard_mask | (1 << t.kind().index()))
                        == 0;
                if tenpai && furiten_ok {
                    out.push(Action::Reach { tile: t, tsumogiri });
                }
            }
            out.push(Action::Dahai { tile: t, tsumogiri });
        }
        out.sort();
        out.dedup();
        out
    }

    /// 立直後の暗槓：ツモった牌での暗槓で、待ちが変わらない（`form_unchanged` なら面子の取り方も）
    fn riichi_ankan_ok(&self, actor: u8, k: TileKind, drawn: Tile) -> bool {
        let rule = self.rules.riichi.ankan_after_riichi;
        if rule == RiichiAnkan::Forbidden || drawn.kind() != k {
            return false;
        }
        let p = &self.players[usize::from(actor)];
        let mut before = p.c;
        before[k.index()] -= 1;
        let w_before = agari::waits(&before, p.melds.is_empty());
        let mut after = before;
        after[k.index()] -= 3;
        let w_after = agari::standard_waits(&after);
        if w_before != w_after || w_before == 0 {
            return false;
        }
        if rule == RiichiAnkan::FormUnchanged {
            for w in agari::mask_kinds(w_before) {
                let mut h = before;
                h[w.index()] += 1;
                if !agari::is_standard_agari(&h) {
                    return false; // 七対子などの形
                }
                let all_koutsu = agari::decompose(&h)
                    .iter()
                    .all(|d| d.sets.contains(&agari::Set::Koutsu(k)));
                if !all_koutsu {
                    return false;
                }
            }
        }
        true
    }

    fn respond_options(
        &self,
        kind: RespondKind,
        actor: u8,
        tile: Tile,
        houtei: bool,
    ) -> Vec<(u8, Vec<Action>)> {
        let rules = &self.rules;
        let mut out = Vec::new();
        let k = tile.kind();
        let ki = k.index();
        for off in 1..self.n {
            let s = (actor + off) % self.n;
            let p = &self.players[usize::from(s)];
            let mut opts = Vec::new();
            // ロン
            let can_ron = match kind {
                RespondKind::Discard => {
                    (ki != NORTH || rules.kita.ron_on_discard)
                        && self.ron_result(s, tile, false, houtei).is_some()
                }
                RespondKind::Kakan => self.ron_result(s, tile, true, false).is_some(),
                RespondKind::Kita => {
                    rules.kita.ron_on_kita
                        && self
                            .ron_result(s, tile, rules.kita.ron_counts_as_chankan, false)
                            .is_some()
                }
                RespondKind::Ankan => {
                    rules.yakuman.kokushi_ron_on_ankan
                        && p.melds.is_empty()
                        && self.ron_result(s, tile, true, false).is_some_and(|r| {
                            r.yaku
                                .iter()
                                .any(|y| matches!(y.yaku, Yaku::Kokushi | Yaku::Kokushi13))
                        })
                }
            };
            if can_ron {
                opts.push(Action::Ron);
            }
            // 鳴き（打牌だけ。立直中・河底（ルールで許さない場合）は不可）
            if kind == RespondKind::Discard
                && p.riichi.is_none()
                && (!houtei || rules.calls.haitei_call)
                && (self.live() > 0 || rules.calls.haitei_call)
            {
                let n = p.c[ki];
                if n >= 3 && self.kan_total() < 4 && self.can_rinshan() {
                    opts.push(Action::Daiminkan);
                }
                if n >= 2 {
                    for (a, b) in self.pon_pairs(p, k) {
                        // 喰い替えで切る牌がなくなるポンはできない
                        let mut rest = p.clone();
                        rest.remove(a);
                        rest.remove(b);
                        let forbidden = self.kuikae_mask(k, None);
                        if rest
                            .distinct_tiles()
                            .iter()
                            .any(|t| self.kuikae_ok(forbidden, *t))
                        {
                            opts.push(Action::Pon(a, b));
                        }
                    }
                }
                if rules.calls.chi && s == self.next_seat(actor) && ki < 27 {
                    for (a, b) in self.chi_pairs(p, k) {
                        let mut rest = p.clone();
                        rest.remove(a);
                        rest.remove(b);
                        let forbidden = self.kuikae_mask(k, Some((a.kind(), b.kind())));
                        if rest
                            .distinct_tiles()
                            .iter()
                            .any(|t| self.kuikae_ok(forbidden, *t))
                        {
                            opts.push(Action::Chi(a, b));
                        }
                    }
                }
            }
            let decl = kind == RespondKind::Discard
                && houtei
                && rules.draws.noten_declaration
                && agari::is_tenpai(&p.c, p.melds.is_empty());
            if !opts.is_empty() || decl {
                opts.push(Action::Pass);
                if decl {
                    opts.push(Action::PassNoten);
                }
                opts.sort();
                out.push((s, opts));
            }
        }
        // 河底の打牌者のノーテン宣言
        if kind == RespondKind::Discard && houtei && rules.draws.noten_declaration {
            let p = &self.players[usize::from(actor)];
            if agari::is_tenpai(&p.c, p.melds.is_empty()) {
                out.push((actor, vec![Action::DeclareTenpai, Action::DeclareNoten]));
            }
        }
        out.sort_by_key(|(s, _)| *s);
        out
    }

    fn pon_pairs(&self, p: &Player, k: TileKind) -> Vec<(Tile, Tile)> {
        let r = p.red_of(k);
        let plain = p.c[k.index()] - r;
        let red = Tile::new(k, true);
        let pl = Tile::plain(k);
        let mut v = Vec::new();
        if plain >= 2 {
            v.push((pl, pl));
        }
        if r >= 1 && plain >= 1 {
            v.push((red.expect("five"), pl));
        }
        if r >= 2 {
            let rt = red.expect("five");
            v.push((rt, rt));
        }
        v.sort();
        v
    }

    fn chi_pairs(&self, p: &Player, k: TileKind) -> Vec<(Tile, Tile)> {
        let i = k.index();
        let num = i % 9;
        let mut v = Vec::new();
        let mut variants = |a: usize, b: usize| {
            let ka = TileKind::new(a as u8).expect("kind");
            let kb = TileKind::new(b as u8).expect("kind");
            let ta: Vec<Tile> = p
                .distinct_tiles()
                .into_iter()
                .filter(|t| t.kind() == ka)
                .collect();
            let tb: Vec<Tile> = p
                .distinct_tiles()
                .into_iter()
                .filter(|t| t.kind() == kb)
                .collect();
            for x in &ta {
                for y in &tb {
                    v.push((*x, *y));
                }
            }
        };
        if num >= 2 {
            variants(i - 2, i - 1);
        }
        if (1..=7).contains(&num) {
            variants(i - 1, i + 1);
        }
        if num <= 6 {
            variants(i + 1, i + 2);
        }
        v.sort();
        v
    }

    /// 鳴いた直後に切れない種類（ビット）
    fn kuikae_mask(&self, called: TileKind, chi: Option<(TileKind, TileKind)>) -> u64 {
        let rule = self.rules.calls.kuikae;
        if rule == Kuikae::Allowed {
            return 0;
        }
        let mut m = 1u64 << called.index();
        if rule == Kuikae::Forbidden
            && let Some((a, b)) = chi
        {
            let (lo, hi) = if a < b {
                (a.index(), b.index())
            } else {
                (b.index(), a.index())
            };
            let c = called.index();
            let same_suit = |x: usize| x / 9 == c / 9;
            if c < lo && hi + 1 < 27 && same_suit(hi + 1) && (hi + 1) % 9 != 0 {
                // 鳴いた牌が下端：反対側の筋（例：3 を 45 で鳴いて 6）
                m |= 1 << (hi + 1);
            }
            if c > hi && lo >= 1 && same_suit(lo - 1) && lo % 9 != 0 {
                m |= 1 << (lo - 1);
            }
        }
        m
    }

    // ------------------------------------------------------------ 遷移：手番

    fn apply_turn(&mut self, actor: u8, a: Action, rinshan: bool) {
        match a {
            Action::Tsumo => {
                let t = self.drawn.expect("drawn");
                self.hora(&[actor], None, t, rinshan, false, false);
            }
            Action::Kyushu => self.abortive("kyushu_kyuhai", Some(actor)),
            Action::Dahai { tile, tsumogiri } => self.discard(actor, tile, tsumogiri, false),
            Action::Reach { tile, tsumogiri } => self.discard(actor, tile, tsumogiri, true),
            Action::Ankan(k) => self.ankan(actor, k),
            Action::Kakan(t) => self.kakan(actor, t),
            Action::Kita => self.kita(actor),
            other => panic!("{other} is not a turn action"),
        }
    }

    fn discard(&mut self, actor: u8, tile: Tile, tsumogiri: bool, riichi: bool) {
        let first = {
            let p = &self.players[usize::from(actor)];
            !p.first_discard_done
        };
        if riichi {
            let double = self.uninterrupted && first;
            let p = &mut self.players[usize::from(actor)];
            p.riichi = Some(if double {
                RiichiKind::Double
            } else {
                RiichiKind::Single
            });
            self.emit(Event::Reach { actor });
        }
        let houtei = self.live() == 0;
        {
            let p = &mut self.players[usize::from(actor)];
            // 一発は立直の宣言から、次の自分の打牌まで
            p.ippatsu = riichi;
            p.remove(tile);
            p.discards.push(tile);
            p.discard_mask |= 1 << tile.kind().index();
            if !tile.kind().is_terminal_or_honor() {
                p.nagashi = false;
            }
            p.furiten_temp = false;
            p.first_discard_done = true;
            p.update_waits();
        }
        self.drawn = None;
        if self.uninterrupted && self.discard_total < usize::from(self.n) {
            self.first_winds.push(tile.kind());
        }
        self.discard_total += 1;
        self.emit(Event::Dahai {
            actor,
            pai: tile,
            tsumogiri,
        });
        self.flush_pending_dora();
        self.phase = Phase::Respond {
            kind: RespondKind::Discard,
            actor,
            tile,
            riichi_pending: riichi,
            houtei,
        };
    }

    fn after_kan_dora(&mut self, kind: MeldKind) {
        // 前の槓の後めくりのドラは、次の槓の前にめくる
        self.flush_pending_dora();
        let immediate = match self.rules.dora.kan_dora_timing {
            KanDoraTiming::AllImmediate => true,
            KanDoraTiming::AllDelayed => false,
            KanDoraTiming::AnkanImmediate => kind == MeldKind::Ankan,
        };
        if immediate {
            self.reveal_dora();
        } else {
            self.pending_dora += 1;
        }
    }

    fn interrupt(&mut self) {
        self.uninterrupted = false;
        self.break_ippatsu_all();
    }

    fn ankan(&mut self, actor: u8, k: TileKind) {
        let tiles: Vec<Tile> = {
            let p = &self.players[usize::from(actor)];
            let r = p.red_of(k);
            (0..4).map(|i| Tile::new(k, i < r).expect("five")).collect()
        };
        {
            let p = &mut self.players[usize::from(actor)];
            for t in &tiles {
                p.remove(*t);
            }
            p.melds.push(Meld {
                kind: MeldKind::Ankan,
                tiles: tiles.clone(),
                called: None,
                from: None,
            });
        }
        self.kans[usize::from(actor)] += 1;
        self.note_kan();
        self.drawn = None;
        self.interrupt();
        self.emit(Event::Ankan {
            actor,
            consumed: tiles.clone(),
        });
        self.after_kan_dora(MeldKind::Ankan);
        self.phase = Phase::Respond {
            kind: RespondKind::Ankan,
            actor,
            tile: tiles[3],
            riichi_pending: false,
            houtei: false,
        };
    }

    fn kakan(&mut self, actor: u8, t: Tile) {
        let consumed = {
            let p = &mut self.players[usize::from(actor)];
            p.remove(t);
            let m = p
                .melds
                .iter_mut()
                .find(|m| m.kind == MeldKind::Pon && m.tiles[0].kind() == t.kind())
                .expect("pon to extend");
            let consumed = m.tiles.clone();
            m.kind = MeldKind::Kakan;
            m.tiles.push(t);
            consumed
        };
        self.kans[usize::from(actor)] += 1;
        self.note_kan();
        self.drawn = None;
        self.interrupt();
        self.emit(Event::Kakan {
            actor,
            pai: t,
            consumed,
        });
        self.after_kan_dora(MeldKind::Kakan);
        self.phase = Phase::Respond {
            kind: RespondKind::Kakan,
            actor,
            tile: t,
            riichi_pending: false,
            houtei: false,
        };
    }

    fn note_kan(&mut self) {
        if self.kan_total() == 4 && self.kans.iter().filter(|&&n| n > 0).count() >= 2 {
            self.suukaikan_pending = true;
        }
    }

    fn kita(&mut self, actor: u8) {
        let north = Tile::plain(TileKind::new(NORTH as u8).expect("N"));
        {
            let p = &mut self.players[usize::from(actor)];
            p.remove(north);
            p.kita += 1;
        }
        self.drawn = None;
        if self.rules.kita.breaks_first_turn {
            self.uninterrupted = false;
        }
        if self.rules.kita.breaks_ippatsu {
            self.break_ippatsu_all();
        }
        self.emit(Event::Kita { actor, pai: north });
        self.phase = Phase::Respond {
            kind: RespondKind::Kita,
            actor,
            tile: north,
            riichi_pending: false,
            houtei: false,
        };
    }

    // ------------------------------------------------------------ 遷移：応答

    #[allow(clippy::too_many_arguments)]
    fn resolve_respond(
        &mut self,
        kind: RespondKind,
        actor: u8,
        tile: Tile,
        riichi_pending: bool,
        houtei: bool,
        answers: &[(u8, Action)],
    ) {
        let n = self.n;
        // ロン（放銃者の下家から順）
        let mut rons: Vec<u8> = Vec::new();
        for off in 1..n {
            let s = (actor + off) % n;
            if answers.iter().any(|&(t, a)| t == s && a == Action::Ron) {
                rons.push(s);
            }
        }
        // ロンできたのに見送った席は振聴
        for &(s, a) in answers {
            let could = self
                .options
                .iter()
                .any(|(t, c)| *t == s && c.contains(&Action::Ron));
            if could && a != Action::Ron {
                let p = &mut self.players[usize::from(s)];
                if p.riichi.is_some() {
                    p.furiten_riichi = true;
                } else {
                    p.furiten_temp = true;
                }
            }
            if a == Action::PassNoten || a == Action::DeclareNoten {
                self.players[usize::from(s)].declared_noten = true;
            }
        }
        if !rons.is_empty() {
            if self.rules.draws.all_ron && rons.len() == usize::from(n - 1) {
                self.abortive("all_ron", None);
                return;
            }
            rons.truncate(usize::from(self.rules.calls.max_ron));
            if riichi_pending && !self.rules.riichi.no_deposit_on_deal_in {
                self.accept_riichi(actor);
            }
            let chankan = match kind {
                RespondKind::Kakan | RespondKind::Ankan => true,
                RespondKind::Kita => self.rules.kita.ron_counts_as_chankan,
                RespondKind::Discard => false,
            };
            self.hora(&rons, Some(actor), tile, false, chankan, houtei);
            return;
        }
        match kind {
            RespondKind::Discard => {
                if riichi_pending {
                    self.accept_riichi(actor);
                }
                // 途中流局
                if self.rules.draws.all_riichi && self.players.iter().all(|p| p.riichi_accepted) {
                    self.abortive("all_riichi", None);
                    return;
                }
                if self.rules.draws.same_wind_discards
                    && self.uninterrupted
                    && self.discard_total == usize::from(n)
                    && self.first_winds.len() == usize::from(n)
                    && (27..31).contains(&self.first_winds[0].index())
                    && self.first_winds.iter().all(|k| *k == self.first_winds[0])
                {
                    self.abortive("same_wind_discards", None);
                    return;
                }
                if self.suukaikan_pending && self.rules.draws.suukaikan {
                    self.abortive("suukaikan", None);
                    return;
                }
                // 鳴き：大明槓・ポン → チー
                let mut call: Option<(u8, Action)> = None;
                for pri in [0u8, 1] {
                    for off in 1..n {
                        let s = (actor + off) % n;
                        if let Some(&(_, a)) = answers.iter().find(|(t, _)| *t == s) {
                            let hit = match a {
                                Action::Daiminkan | Action::Pon(..) => pri == 0,
                                Action::Chi(..) => pri == 1,
                                _ => false,
                            };
                            if hit && call.is_none() {
                                call = Some((s, a));
                            }
                        }
                    }
                }
                if let Some((s, a)) = call {
                    self.apply_call(s, actor, tile, a);
                    return;
                }
                if self.live() == 0 {
                    self.exhaustive();
                    return;
                }
                let next = self.next_seat(actor);
                self.draw(next, false);
            }
            RespondKind::Kakan | RespondKind::Kita | RespondKind::Ankan => {
                self.draw(actor, true);
            }
        }
    }

    fn accept_riichi(&mut self, actor: u8) {
        let dep = self.rules.points.riichi_deposit;
        self.scores[usize::from(actor)] -= dep;
        self.kyotaku += 1;
        self.players[usize::from(actor)].riichi_accepted = true;
        self.emit(Event::ReachAccepted {
            actor,
            scores: self.scores.clone(),
            kyotaku: self.kyotaku,
        });
    }

    fn apply_call(&mut self, s: u8, target: u8, tile: Tile, a: Action) {
        self.interrupt();
        self.players[usize::from(target)].nagashi = false;
        let (kind, consumed): (MeldKind, Vec<Tile>) = match a {
            Action::Pon(x, y) => (MeldKind::Pon, vec![x, y]),
            Action::Chi(x, y) => (MeldKind::Chi, vec![x, y]),
            Action::Daiminkan => {
                let p = &self.players[usize::from(s)];
                let k = tile.kind();
                let r = p.red_of(k);
                (
                    MeldKind::Daiminkan,
                    (0..3).map(|i| Tile::new(k, i < r).expect("five")).collect(),
                )
            }
            other => panic!("{other} is not a call"),
        };
        {
            let p = &mut self.players[usize::from(s)];
            for t in &consumed {
                p.remove(*t);
            }
            let mut tiles = consumed.clone();
            tiles.push(tile);
            tiles.sort();
            p.melds.push(Meld {
                kind,
                tiles,
                called: Some(tile),
                from: Some(target),
            });
        }
        self.note_pao(s, target, kind, tile.kind());
        let ev = match kind {
            MeldKind::Pon => Event::Pon {
                actor: s,
                target,
                pai: tile,
                consumed: consumed.clone(),
            },
            MeldKind::Chi => Event::Chi {
                actor: s,
                target,
                pai: tile,
                consumed: consumed.clone(),
            },
            _ => Event::Daiminkan {
                actor: s,
                target,
                pai: tile,
                consumed: consumed.clone(),
            },
        };
        self.emit(ev);
        if kind == MeldKind::Daiminkan {
            self.kans[usize::from(s)] += 1;
            self.note_kan();
            self.after_kan_dora(MeldKind::Daiminkan);
            self.draw(s, true);
            return;
        }
        let forbidden = match kind {
            MeldKind::Chi => {
                self.kuikae_mask(tile.kind(), Some((consumed[0].kind(), consumed[1].kind())))
            }
            _ => self.kuikae_mask(tile.kind(), None),
        };
        self.phase = Phase::Turn {
            actor: s,
            rinshan: false,
            after_call: Some((forbidden, kind)),
        };
    }

    fn note_pao(&mut self, s: u8, target: u8, kind: MeldKind, k: TileKind) {
        let pao_rules = self.rules.yakuman.pao.clone();
        let p = &mut self.players[usize::from(s)];
        let triplet_kinds: Vec<usize> = p
            .melds
            .iter()
            .filter(|m| m.kind != MeldKind::Chi)
            .map(|m| m.tiles[0].kind().index())
            .collect();
        let ki = k.index();
        if (31..34).contains(&ki)
            && pao_rules.contains(&PaoYaku::Daisangen)
            && (31..34).all(|d| triplet_kinds.contains(&d))
        {
            p.pao.push((PaoYaku::Daisangen, target));
        }
        if (27..31).contains(&ki)
            && pao_rules.contains(&PaoYaku::Daisuushii)
            && (27..31).all(|d| triplet_kinds.contains(&d))
        {
            p.pao.push((PaoYaku::Daisuushii, target));
        }
        if kind == MeldKind::Daiminkan
            && pao_rules.contains(&PaoYaku::Suukantsu)
            && p.melds.iter().filter(|m| m.is_kan()).count() == 4
        {
            p.pao.push((PaoYaku::Suukantsu, target));
        }
    }

    // ------------------------------------------------------------ 和了・流局

    fn hora(
        &mut self,
        winners: &[u8],
        loser: Option<u8>,
        tile: Tile,
        rinshan: bool,
        chankan: bool,
        houtei: bool,
    ) {
        let rules = self.rules.clone();
        let sticks = self.kyotaku;
        let m = winners.len() as u32;
        for (i, &w) in winners.iter().enumerate() {
            let tsumo = loser.is_none();
            let p = &self.players[usize::from(w)];
            let mut hand = p.c;
            if !tsumo {
                hand[tile.kind().index()] += 1;
            }
            let r = self
                .evaluate_win(w, &hand, tile, tsumo, rinshan, chankan, houtei)
                .expect("hora was a legal choice");
            // 積み棒と供託
            let split = rules.calls.multi_ron_deposit == MultiRonDeposit::Split;
            let honba_on = i == 0 || split;
            let stick_share = if split {
                sticks / m + u32::from((i as u32) < sticks % m)
            } else if i == 0 {
                sticks
            } else {
                0
            };
            // 包
            let pao = self.pao_for(w, &r);
            let (mut deltas, points) = self.hora_payment(w, loser, &r, pao, honba_on);
            deltas[usize::from(w)] += stick_share as i32 * rules.points.riichi_deposit;
            for (s, d) in self.scores.iter_mut().zip(&deltas) {
                *s += d;
            }
            if rules.chips.enabled {
                self.hora_chips(w, loser, &r);
            }
            let p = &self.players[usize::from(w)];
            let mut tehai = p.tiles();
            if tsumo && let Some(i) = tehai.iter().position(|t| *t == tile) {
                tehai.remove(i);
            }
            let ura = if p.riichi.is_some() {
                self.ura_tiles()
            } else {
                Vec::new()
            };
            let ev = Hora {
                actor: w,
                target: loser.unwrap_or(w),
                pai: tile,
                tehai,
                melds: p.melds.iter().map(Meld::view).collect(),
                yaku: r.yaku.clone(),
                han: r.han,
                fu: r.fu,
                yakuman: r.yakuman,
                points,
                ura_indicators: ura,
                pao: pao.map(|(s, _)| s),
                deltas,
                scores: self.scores.clone(),
            };
            self.emit(Event::Hora(Box::new(ev)));
        }
        self.kyotaku = 0;
        let renchan = winners.contains(&self.oya);
        self.finish(
            "hora",
            renchan,
            if renchan { self.honba + 1 } else { 0 },
            true,
            false,
        );
    }

    /// 包の人と、包の対象の役満の基本点
    fn pao_for(&self, w: u8, r: &WinResult) -> Option<(u8, i64)> {
        if r.yakuman == 0 || r.kazoe {
            return None;
        }
        let p = &self.players[usize::from(w)];
        for &(y, seat) in &p.pao {
            let yk = match y {
                PaoYaku::Daisangen => Yaku::Daisangen,
                PaoYaku::Daisuushii => Yaku::Daisuushii,
                PaoYaku::Suukantsu => Yaku::Suukantsu,
            };
            if let Some(item) = r.yaku.iter().find(|i| i.yaku == yk) {
                return Some((seat, 8000 * i64::from(item.yakuman.min(r.yakuman))));
            }
        }
        None
    }

    /// 和了の点数の移動（供託を除く）と、手の値。
    fn hora_payment(
        &self,
        w: u8,
        loser: Option<u8>,
        r: &WinResult,
        pao: Option<(u8, i64)>,
        honba_on: bool,
    ) -> (Vec<i32>, i64) {
        let rules = &self.rules;
        let n = usize::from(self.n);
        let mut d = vec![0i64; n];
        let wi = usize::from(w);
        let oya_w = w == self.oya;
        let honba = if honba_on { i64::from(self.honba) } else { 0 };
        let mul = if oya_w { 6 } else { 4 };
        let mut base = r.base;
        let mut points = 0i64;
        let mut honba_paid = false;
        if let Some((ps, full)) = pao {
            let base2 = match loser {
                Some(_) if rules.yakuman.pao_ron_split => full / 2,
                Some(_) => 0,
                None => full,
            };
            if base2 > 0 {
                base -= base2;
                let amt = base2 * mul;
                d[wi] += amt;
                d[usize::from(ps)] -= amt;
                points += amt;
                if base == 0 {
                    // ツモの全額が包：本場も包の人が払う
                    let h = i64::from(rules.points.honba_ron) * honba;
                    d[wi] += h;
                    d[usize::from(ps)] -= h;
                    honba_paid = true;
                }
            }
        }
        if base > 0 {
            match loser {
                Some(l) => {
                    let amt = round_up_100(base * mul);
                    let h = i64::from(rules.points.honba_ron) * honba;
                    d[wi] += amt + h;
                    d[usize::from(l)] -= amt + h;
                    points += amt;
                }
                None => {
                    let payers: Vec<usize> = (0..n).filter(|&s| s != wi).collect();
                    let missing = 4 - n; // 三麻でいない北家の人数
                    let missing_share =
                        round_up_100(base * if oya_w { 2 } else { 1 }) * missing as i64;
                    let extra = match rules.points.tsumo_payment {
                        TsumoPayment::Split if missing > 0 => {
                            round_up_100((missing_share as f64 / payers.len() as f64).ceil() as i64)
                        }
                        _ => 0,
                    };
                    let he = i64::from(rules.points.honba_tsumo_each) * honba;
                    for &ps in &payers {
                        let share = round_up_100(
                            base * if oya_w || ps == usize::from(self.oya) {
                                2
                            } else {
                                1
                            },
                        ) + extra;
                        d[ps] -= share + if honba_paid { 0 } else { he };
                        d[wi] += share + if honba_paid { 0 } else { he };
                        points += share;
                    }
                }
            }
        } else if !honba_paid && let Some(l) = loser {
            let h = i64::from(rules.points.honba_ron) * honba;
            d[wi] += h;
            d[usize::from(l)] -= h;
        }
        (d.into_iter().map(|x| x as i32).collect(), points)
    }

    fn hora_chips(&mut self, w: u8, loser: Option<u8>, r: &WinResult) {
        let v = self.rules.chips.values.clone();
        let p = &self.players[usize::from(w)];
        let closed = p.closed();
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
            None => (0..self.n).filter(|&s| s != w).collect(),
        };
        for (kind, count) in per {
            if count == 0 {
                continue;
            }
            for &ps in &payers {
                self.chip_deltas[usize::from(ps)] -= count as i32;
                self.chip_deltas[usize::from(w)] += count as i32;
            }
            self.chip_items.push(ChipItem {
                seat: w,
                kind,
                count,
            });
        }
    }

    /// 荒牌流局時の聴牌（形式聴牌・ノーテン宣言のルールに従う）
    fn tenpai_for_draw(&self, s: u8) -> bool {
        let p = &self.players[usize::from(s)];
        if p.declared_noten {
            return false;
        }
        let closed = p.melds.is_empty();
        if !agari::is_tenpai(&p.c, closed) {
            return false;
        }
        if self.rules.draws.keishiki_tenpai {
            return true;
        }
        // 役のある聴牌で、待ちの牌が残っている（自分で4枚使っていない）
        agari::mask_kinds(p.waits).any(|w| {
            if p.held(w.index()) >= 4 {
                return false;
            }
            let mut h = p.c;
            h[w.index()] += 1;
            let sets = p.sets();
            let input = WinInput {
                rules: &self.rules,
                hand: h,
                win_tile: w,
                sets: &sets,
                tsumo: false,
                riichi: p.riichi.unwrap_or(RiichiKind::None),
                ippatsu: false,
                haitei: false,
                houtei: false,
                rinshan: false,
                chankan: false,
                first_draw: false,
                oya: s == self.oya,
                bakaze: self.bakaze_kind(),
                jikaze: self.jikaze(s),
                dora_indicators: &[],
                ura_indicators: &[],
                aka: 0,
                kita: 0,
            };
            yaku::evaluate(&input).is_some()
        })
    }

    fn exhaustive(&mut self) {
        let rules = self.rules.clone();
        let n = usize::from(self.n);
        let tenpai: Vec<bool> = (0..self.n).map(|s| self.tenpai_for_draw(s)).collect();
        let mut deltas = vec![0i32; n];
        let nagashi: Vec<u8> = (0..self.n)
            .filter(|&s| rules.draws.nagashi_mangan && self.players[usize::from(s)].nagashi)
            .collect();
        let reason;
        if !nagashi.is_empty() {
            reason = "nagashi_mangan";
            for &s in &nagashi {
                let r = WinResult {
                    yaku: Vec::new(),
                    han: 5,
                    fu: 0,
                    yakuman: 0,
                    kazoe: false,
                    base: 2000,
                };
                let (d, _) = self.hora_payment(s, None, &r, None, false);
                for (x, y) in deltas.iter_mut().zip(d) {
                    *x += y;
                }
            }
        } else {
            reason = "exhaustive";
            let t = tenpai.iter().filter(|&&b| b).count() as i32;
            if t > 0 && t < n as i32 {
                let total = rules.points.noten_penalty_total;
                for (s, &tp) in tenpai.iter().enumerate() {
                    deltas[s] = if tp {
                        total / t
                    } else {
                        -total / (n as i32 - t)
                    };
                }
            }
        }
        for (s, d) in self.scores.iter_mut().zip(&deltas) {
            *s += d;
        }
        let oya_tenpai = tenpai[usize::from(self.oya)];
        self.emit(Event::Ryukyoku(Box::new(Ryukyoku {
            reason,
            tenpai: tenpai.clone(),
            hands: self.players.iter().map(Player::tiles).collect(),
            deltas,
            scores: self.scores.clone(),
        })));
        let renchan = match rules.draws.renchan {
            Renchan::Agari => false,
            Renchan::Tenpai => oya_tenpai,
            Renchan::Always => true,
        };
        self.finish("ryukyoku", renchan, self.honba + 1, false, oya_tenpai);
    }

    fn abortive(&mut self, reason: &'static str, declarer: Option<u8>) {
        let n = usize::from(self.n);
        let tenpai: Vec<bool> = (0..self.n)
            .map(|s| {
                let p = &self.players[usize::from(s)];
                agari::is_tenpai(&p.c, p.melds.is_empty())
            })
            .collect();
        let _ = declarer;
        self.emit(Event::Ryukyoku(Box::new(Ryukyoku {
            reason,
            tenpai: tenpai.clone(),
            hands: self.players.iter().map(Player::tiles).collect(),
            deltas: vec![0; n],
            scores: self.scores.clone(),
        })));
        let renchan = self.rules.draws.abortive_renchan;
        self.finish(
            "ryukyoku",
            renchan,
            self.honba + 1,
            false,
            tenpai[usize::from(self.oya)],
        );
    }

    fn finish(
        &mut self,
        reason: &'static str,
        renchan: bool,
        honba_next: u32,
        hora: bool,
        oya_tenpai: bool,
    ) {
        self.emit(Event::KyokuEnd(Box::new(KyokuEnd {
            reason,
            scores: self.scores.clone(),
            kyotaku: self.kyotaku,
            honba_next,
            renchan,
            chip_deltas: self.chip_deltas.clone(),
            chip_items: self.chip_items.clone(),
        })));
        self.outcome = Some(KyokuOutcome {
            hora,
            renchan,
            honba_next,
            kyotaku_next: self.kyotaku,
            scores: self.scores.clone(),
            chip_deltas: self.chip_deltas.clone(),
            oya_tenpai,
        });
        self.phase = Phase::Done;
    }
}
