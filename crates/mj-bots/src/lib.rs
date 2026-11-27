//! ルールベースのボット（Q-007）。評価の基準点と、差分テスト・性能計測の対局の生成に使う。
//!
//! 判断の材料は**その席が見てよい情報だけ**（自分の手牌、全員の河・副露・北抜き・立直、ドラ表示牌）。
//! 局のイベント列から公開の情報を取り出し、他家の手牌・ツモ・牌山は読まない（[`PublicView`]）。
//! 乱数を使わないので、同じ局面なら同じ行動を選ぶ（決定論）。
//!
//! 打ち方（[`RuleBot::choose`]）：
//! 1. 和了（ツモ・ロン）は必ず選ぶ
//! 2. 押し引き：自分以外に立直者がいて、自分が聴牌でなければ「降り」：立直者に最も安全な牌を切る
//!    （現物・立直後の通過牌 → 3枚見えた字牌 → 2枚見えた字牌・筋の么九牌 → 筋 → 么九牌 → その他）
//! 3. 打牌：切った後のシャンテン数が最小、次に受け入れ（見えていない枚数で数える）が最大、
//!    次に孤立した字牌・么九牌を先に切る
//! 4. 立直：聴牌で、待ちの牌が1枚以上見えずに残っていて、門前なら立直する
//! 5. 鳴き：役牌（三元牌・場風・自風）の対子だけポンする。大明槓・チーはしない。
//!    暗槓・加槓はシャンテン数が悪くならなければする（降りているときはしない）。北抜きは必ずする
//! 6. 九種九牌：シャンテン数が3以上なら流局にする
//! 7. ノーテン宣言はしない（聴牌のまま）

use mj_core::action::Action;
use mj_core::agari;
use mj_core::event::Event;
use mj_core::kyoku::{Decision, Kyoku};
use mj_core::rules::Rules;
use mj_core::shanten::ShantenCalc;
use mj_core::tile::{KindCounts, NUM_KINDS, TileKind};

const EAST: usize = 27;
const NORTH: usize = 30;
const HAKU: usize = 31;

/// その席から見える公開の情報。
#[derive(Debug, Clone)]
pub struct PublicView {
    pub seat: u8,
    pub n: u8,
    pub oya: u8,
    pub bakaze: usize,
    /// 席ごとの河（種類）
    pub discards: Vec<Vec<usize>>,
    /// 立直した席と、立直の宣言牌の河での位置
    pub riichi_at: Vec<Option<usize>>,
    /// 立直者ごとの、立直の後に誰かが切った牌（通過牌）
    pub passed_after_riichi: Vec<[bool; NUM_KINDS]>,
    /// 見えている枚数（自分の手牌・全員の河・副露・北抜き・ドラ表示牌）
    pub visible: [u8; NUM_KINDS],
}

impl PublicView {
    /// 局のイベント列から作る。自分以外の `tsumo` の牌と、`kyoku_start` の他家の配牌・牌山は読まない。
    pub fn new(k: &Kyoku, seat: u8) -> Self {
        let n = k.rules().table.num_players;
        let mut v = PublicView {
            seat,
            n,
            oya: 0,
            bakaze: EAST,
            discards: vec![Vec::new(); usize::from(n)],
            riichi_at: vec![None; usize::from(n)],
            passed_after_riichi: vec![[false; NUM_KINDS]; usize::from(n)],
            visible: k.concealed_counts(seat),
        };
        let mut pending_reach: Option<u8> = None;
        for e in k.events() {
            match e {
                Event::KyokuStart(s) => {
                    v.oya = s.oya;
                    v.bakaze = s.bakaze.kind().index();
                    v.visible[s.dora_indicator.kind().index()] += 1;
                }
                Event::Reach { actor } => pending_reach = Some(*actor),
                Event::Dahai { actor, pai, .. } => {
                    let k = pai.kind().index();
                    let a = usize::from(*actor);
                    if pending_reach == Some(*actor) {
                        v.riichi_at[a] = Some(v.discards[a].len());
                        pending_reach = None;
                    }
                    v.discards[a].push(k);
                    for (r, passed) in v.riichi_at.iter().zip(v.passed_after_riichi.iter_mut()) {
                        if r.is_some() {
                            passed[k] = true;
                        }
                    }
                    v.visible[k] += 1;
                }
                // 鳴いた牌は河から消えるが、見えている枚数には副露として数える（下の副露の加算で
                // 鳴かれた牌を数えるので、ここでは河の分を1枚戻す）
                Event::Pon { pai, .. } | Event::Chi { pai, .. } | Event::Daiminkan { pai, .. } => {
                    v.visible[pai.kind().index()] -= 1;
                }
                // 抜いた北は手牌から消えるので、自分の分も数える
                Event::Kita { pai, .. } => v.visible[pai.kind().index()] += 1,
                Event::Dora { dora_indicator } => v.visible[dora_indicator.kind().index()] += 1,
                _ => {}
            }
        }
        // 副露（暗槓を含む）は全員分。自分の副露も手牌（concealed）には含まれない
        for s in 0..n {
            for m in k.melds(s) {
                for t in &m.tiles {
                    v.visible[t.kind().index()] += 1;
                }
            }
        }
        for x in v.visible.iter_mut() {
            *x = (*x).min(4);
        }
        v
    }

    /// 見えていない枚数
    pub fn remaining(&self, k: usize) -> u8 {
        4 - self.visible[k]
    }

    fn seat_wind(&self) -> usize {
        EAST + usize::from((self.seat + self.n - self.oya) % self.n)
    }

    fn is_yakuhai(&self, k: usize) -> bool {
        k >= HAKU || k == self.bakaze || k == self.seat_wind()
    }

    /// 自分以外の立直者
    fn riichi_opponents(&self) -> Vec<usize> {
        (0..usize::from(self.n))
            .filter(|&s| s != usize::from(self.seat) && self.riichi_at[s].is_some())
            .collect()
    }

    /// 牌の種類 `k` の、立直者 `r` に対する危険度（0 が安全）。
    fn danger(&self, r: usize, k: usize) -> u8 {
        if self.discards[r].contains(&k) || self.passed_after_riichi[r][k] {
            return 0;
        }
        let kind = TileKind::new(k as u8).expect("kind");
        if kind.is_honor() {
            return match self.visible[k] {
                3.. => 1,
                2 => 2,
                _ => 4,
            };
        }
        let num = kind.num().expect("number");
        let suji = |d: i8| {
            let m = num as i8 + d;
            (1..=9).contains(&m) && self.discards[r].contains(&((k as i8 + d) as usize))
        };
        // 筋：1・9 は 4・6、2〜8 は両側（片方だけなら半分の筋）
        let both = match num {
            1..=3 => suji(3),
            7..=9 => suji(-3),
            _ => suji(3) && suji(-3),
        };
        let terminal = num == 1 || num == 9;
        match (both, terminal) {
            (true, true) => 2,
            (true, false) => 3,
            (false, true) => 4,
            (false, false) => 5,
        }
    }
}

/// ルールベースのボット。
pub struct RuleBot {
    calc: ShantenCalc,
}

impl RuleBot {
    pub fn new(rules: &Rules) -> Self {
        Self {
            calc: ShantenCalc::new(rules.tiles.manzu),
        }
    }

    /// 判断ポイント `d` での行動。
    pub fn choose(&self, k: &Kyoku, d: &Decision) -> Action {
        let ch = &d.choices;
        if let Some(a) = ch.iter().find(|a| matches!(a, Action::Tsumo | Action::Ron)) {
            return *a;
        }
        let view = PublicView::new(k, d.seat);
        let hand = k.concealed_counts(d.seat);
        let melds = k.melds(d.seat).len() as u8;
        let has_discard = ch
            .iter()
            .any(|a| matches!(a, Action::Dahai { .. } | Action::Reach { .. }));
        if !has_discard {
            return self.respond(&view, &hand, melds, ch);
        }
        // ツモ後・鳴いた後の判断
        let cur = self.calc.shanten(&hand, melds);
        let opponents = view.riichi_opponents();
        let folding = !opponents.is_empty() && cur >= 1;
        if ch.contains(&Action::Kyushu) && cur >= 3 {
            return Action::Kyushu;
        }
        if ch.contains(&Action::Kita) {
            return Action::Kita;
        }
        if !folding {
            for a in ch {
                if let Action::Ankan(t) = a {
                    let mut c = hand;
                    c[t.index()] -= 4;
                    if self.calc.shanten(&c, melds + 1) <= cur {
                        return *a;
                    }
                }
                if let Action::Kakan(t) = a
                    && opponents.is_empty()
                {
                    let mut c = hand;
                    c[t.kind().index()] -= 1;
                    if self.calc.shanten(&c, melds) <= cur {
                        return *a;
                    }
                }
            }
        }
        self.discard(&view, &hand, melds, ch, folding, &opponents)
    }

    fn respond(&self, view: &PublicView, hand: &KindCounts, melds: u8, ch: &[Action]) -> Action {
        let pass = ch
            .iter()
            .copied()
            .find(|a| *a == Action::Pass)
            .unwrap_or(ch[0]);
        if !view.riichi_opponents().is_empty() {
            return pass;
        }
        for a in ch {
            if let Action::Pon(x, _) = a {
                let k = x.kind().index();
                if view.is_yakuhai(k) && k != NORTH {
                    let mut c = *hand;
                    c[k] -= 2;
                    // ポンして1枚切った形が今より悪くならないとき
                    let before = self.calc.shanten(hand, melds);
                    let after = (0..NUM_KINDS)
                        .filter(|&i| c[i] > 0)
                        .map(|i| {
                            let mut d = c;
                            d[i] -= 1;
                            self.calc.shanten(&d, melds + 1)
                        })
                        .min()
                        .unwrap_or(i8::MAX);
                    if after <= before {
                        return *a;
                    }
                }
            }
        }
        pass
    }

    fn discard(
        &self,
        view: &PublicView,
        hand: &KindCounts,
        melds: u8,
        ch: &[Action],
        folding: bool,
        opponents: &[usize],
    ) -> Action {
        // (選択肢, 切る種類, 切った後のシャンテン数)
        let cands: Vec<(Action, usize, i8)> = ch
            .iter()
            .filter_map(|a| match a {
                Action::Dahai { tile, .. } | Action::Reach { tile, .. } => {
                    let k = tile.kind().index();
                    let mut c = *hand;
                    c[k] -= 1;
                    Some((*a, k, self.calc.shanten(&c, melds)))
                }
                _ => None,
            })
            .collect();
        if folding {
            let danger = |k: usize| -> u32 {
                opponents
                    .iter()
                    .map(|&r| u32::from(view.danger(r, k)))
                    .sum()
            };
            return cands
                .iter()
                .filter(|(a, ..)| matches!(a, Action::Dahai { .. }))
                .min_by_key(|(_, k, s)| (danger(*k), *s))
                .map(|(a, ..)| *a)
                .unwrap_or(cands[0].0);
        }
        let best_s = cands.iter().map(|c| c.2).min().expect("a discard");
        let mut best: Option<(Action, (u32, u8))> = None;
        for &(a, k, s) in &cands {
            if s != best_s || matches!(a, Action::Reach { .. }) {
                continue;
            }
            let mut c = *hand;
            c[k] -= 1;
            let ukeire: u32 = self
                .calc
                .improving_kinds(&c, melds)
                .iter()
                .map(|t| u32::from(view.remaining(t.index())))
                .sum();
            let score = (ukeire, isolation(hand, k));
            if best.is_none_or(|(_, b)| score > b) {
                best = Some((a, score));
            }
        }
        let (chosen, _) = best.expect("a discard at the best shanten");
        // 立直：同じ牌の立直があり、待ちが残っているなら立直する
        if best_s == 0
            && let Action::Dahai { tile, tsumogiri } = chosen
        {
            let reach = Action::Reach { tile, tsumogiri };
            if ch.contains(&reach) {
                let mut c = *hand;
                c[tile.kind().index()] -= 1;
                let waits = agari::waits(&c, melds == 0);
                if agari::mask_kinds(waits).any(|w| view.remaining(w.index()) > 0) {
                    return reach;
                }
            }
        }
        chosen
    }
}

/// 孤立した牌ほど大きい（同じ受け入れなら先に切る）：字牌・么九牌で、近くに牌がないもの。
fn isolation(hand: &KindCounts, k: usize) -> u8 {
    let kind = TileKind::new(k as u8).expect("kind");
    let alone = hand[k] == 1;
    if kind.is_honor() {
        return if alone { 4 } else { 1 };
    }
    let num = kind.num().expect("number");
    let near = (-2i8..=2)
        .filter(|&d| d != 0)
        .filter(|&d| {
            let m = num as i8 + d;
            (1..=9).contains(&m) && hand[(k as i8 + d) as usize] > 0
        })
        .count();
    let terminal = num == 1 || num == 9;
    match (alone && near == 0, terminal) {
        (true, true) => 3,
        (true, false) => 2,
        _ => 0,
    }
}
