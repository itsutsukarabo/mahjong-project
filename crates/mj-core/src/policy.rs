//! テスト・計測用の簡単な打ち手（学習の対象ではない）。
//!
//! [`biased_choice`]：和了は選びやすく、立直・鳴きもときどき選ぶ乱数の打ち手。
//! [`greedy_choice`]：シャンテン数が最も小さくなる牌を切り、和了・立直は必ず選ぶ打ち手。
//! 一様な乱数では和了がほとんど起きない（三麻1局戦で約 0.6%）ので、和了・点数の分岐を通すために使う。

use crate::action::Action;
use crate::shanten::ShantenCalc;
use crate::tile::KindCounts;
use crate::wall::uniform_below;
use rand_core::RngCore;

pub fn uniform_choice(rng: &mut impl RngCore, choices: &[Action]) -> Action {
    choices[uniform_below(rng, choices.len() as u32) as usize]
}

pub fn biased_choice(rng: &mut impl RngCore, choices: &[Action]) -> Action {
    let pct = |rng: &mut dyn RngCore, p: u32| uniform_below(rng, 100) < p;
    let find =
        |f: &dyn Fn(&Action) -> bool| choices.iter().copied().filter(|a| f(a)).collect::<Vec<_>>();
    let wins = find(&|a| matches!(a, Action::Tsumo | Action::Ron));
    if !wins.is_empty() && pct(rng, 90) {
        return wins[0];
    }
    let reach = find(&|a| matches!(a, Action::Reach { .. }));
    if !reach.is_empty() && pct(rng, 60) {
        return reach[uniform_below(rng, reach.len() as u32) as usize];
    }
    let calls = find(&|a| {
        matches!(
            a,
            Action::Pon(..)
                | Action::Chi(..)
                | Action::Daiminkan
                | Action::Ankan(_)
                | Action::Kakan(_)
                | Action::Kita
        )
    });
    if !calls.is_empty() && pct(rng, 40) {
        return calls[uniform_below(rng, calls.len() as u32) as usize];
    }
    let rest = find(&|a| {
        !matches!(
            a,
            Action::Tsumo | Action::Ron | Action::Reach { .. } | Action::Kyushu
        )
    });
    if rest.is_empty() {
        return uniform_choice(rng, choices);
    }
    // 打牌は手出しの孤立した字牌を少し優先する（手が進みやすく、和了が増える）
    let honors: Vec<Action> = rest
        .iter()
        .copied()
        .filter(|a| matches!(a, Action::Dahai { tile, .. } if tile.kind().is_honor()))
        .collect();
    if !honors.is_empty() && pct(rng, 40) {
        return honors[uniform_below(rng, honors.len() as u32) as usize];
    }
    rest[uniform_below(rng, rest.len() as u32) as usize]
}

/// シャンテン数が最も小さくなる打牌を選ぶ（同点は乱数）。和了・立直は必ず、鳴きは `call_pct` % で選ぶ。
/// `hand` は判断する席の門前の牌（ツモ後なら 14 枚形）、`melds` は副露の数。
pub fn greedy_choice(
    rng: &mut impl RngCore,
    choices: &[Action],
    hand: &KindCounts,
    melds: u8,
    calc: &ShantenCalc,
    call_pct: u32,
) -> Action {
    if let Some(a) = choices
        .iter()
        .find(|a| matches!(a, Action::Tsumo | Action::Ron))
    {
        return *a;
    }
    let pick = |rng: &mut dyn RngCore, v: &[Action]| v[uniform_below(rng, v.len() as u32) as usize];
    let calls: Vec<Action> = choices
        .iter()
        .copied()
        .filter(|a| {
            matches!(
                a,
                Action::Pon(..)
                    | Action::Chi(..)
                    | Action::Daiminkan
                    | Action::Ankan(_)
                    | Action::Kakan(_)
                    | Action::Kita
            )
        })
        .collect();
    if !calls.is_empty() && uniform_below(rng, 100) < call_pct {
        return pick(rng, &calls);
    }
    // 打牌・立直：切った後のシャンテン数が最小のもの
    let mut best: Vec<Action> = Vec::new();
    let mut best_s = i8::MAX;
    for a in choices {
        let tile = match a {
            Action::Dahai { tile, .. } | Action::Reach { tile, .. } => *tile,
            _ => continue,
        };
        let mut c = *hand;
        c[tile.kind().index()] -= 1;
        let s = calc.shanten(&c, melds);
        if s < best_s {
            best_s = s;
            best.clear();
        }
        if s == best_s {
            best.push(*a);
        }
    }
    let reach: Vec<Action> = best
        .iter()
        .copied()
        .filter(|a| matches!(a, Action::Reach { .. }))
        .collect();
    if !reach.is_empty() {
        return pick(rng, &reach);
    }
    if best.is_empty() {
        // 打牌がない（応答の場）：見送り
        return choices
            .iter()
            .copied()
            .find(|a| *a == Action::Pass)
            .unwrap_or_else(|| pick(rng, choices));
    }
    pick(rng, &best)
}
