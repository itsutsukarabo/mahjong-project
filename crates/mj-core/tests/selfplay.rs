//! 無作為な合法手での自己対戦：止まらずに終局し、不変条件が保たれること。

use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::policy::{greedy_choice, uniform_choice};
use mj_core::rules::{Rules, presets};
use mj_core::shanten::ShantenCalc;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

/// 対局を最後まで打つ。戻り値は局数。
fn play(rules: &Rules, seed: u64) -> (Game, usize) {
    play_with(rules, seed, false)
}

/// `biased`：シャンテン数を下げる打ち手（和了・立直・鳴きの分岐を通す）
fn play_with(rules: &Rules, seed: u64, biased: bool) -> (Game, usize) {
    let calc = ShantenCalc::new(rules.tiles.manzu);
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xabcdef);
    let mut steps = 0;
    while !g.is_over() {
        let acts: Vec<_> = g
            .decisions()
            .iter()
            .map(|d| {
                assert!(d.choices.len() >= 2, "decision point has >= 2 choices");
                let mut sorted = d.choices.clone();
                sorted.sort();
                assert_eq!(sorted, d.choices, "choices are in spec order");
                let a = if biased {
                    let k = g.current_kyoku();
                    let hand = k.concealed_counts(d.seat);
                    let melds = k.melds(d.seat).len() as u8;
                    greedy_choice(&mut rng, &d.choices, &hand, melds, &calc, 30)
                } else {
                    uniform_choice(&mut rng, &d.choices)
                };
                (d.seat, a)
            })
            .collect();
        g.step(&acts).expect("legal step");
        steps += 1;
        assert!(steps < 100_000, "game does not terminate");
    }
    let n = g.kyokus().count();
    (g, n)
}

fn check_invariants(rules: &Rules, g: &Game) {
    let n = usize::from(rules.table.num_players);
    let total = rules.points.starting * n as i32;
    for ev in g.kyokus() {
        for e in ev {
            if let Event::KyokuEnd(k) = e {
                let sum: i32 =
                    k.scores.iter().sum::<i32>() + k.kyotaku as i32 * rules.points.riichi_deposit;
                assert_eq!(sum, total, "points are conserved");
                assert_eq!(k.chip_deltas.iter().sum::<i32>(), 0);
            }
        }
    }
    let end = g.end().unwrap();
    // 残った供託はトップが受け取る（天鳳三麻・テスト用四麻）ので、精算の合計は 0
    assert_eq!(end.settlement.iter().sum::<i32>(), 0);
    assert_eq!(end.scores.iter().sum::<i32>(), total);
    let mut ranks = end.ranks.clone();
    ranks.sort();
    assert_eq!(ranks, (1..=n as u8).collect::<Vec<_>>());
}

#[test]
fn random_selfplay_sanma() {
    let rules = presets::tenhou_sanma();
    let mut kyokus = 0;
    for seed in 0..150 {
        let (g, n) = play(&rules, seed);
        check_invariants(&rules, &g);
        kyokus += n;
    }
    assert!(kyokus > 150);
}

#[test]
fn random_selfplay_yonma() {
    let rules = presets::test_yonma();
    for seed in 0..100 {
        let (g, _) = play(&rules, seed);
        check_invariants(&rules, &g);
    }
}

#[test]
fn selfplay_is_deterministic() {
    let rules = presets::tenhou_sanma();
    let (a, _) = play(&rules, 42);
    let (b, _) = play(&rules, 42);
    let ea: Vec<_> = a.kyokus().map(|k| k.to_vec()).collect();
    let eb: Vec<_> = b.kyokus().map(|k| k.to_vec()).collect();
    assert_eq!(ea, eb);
}

#[test]
fn biased_selfplay_sanma_and_yonma() {
    let mut horas = 0;
    // デバッグビルド（CI の cargo test）ではシャンテン数の計算が遅いので局数を減らす
    let scale = if cfg!(debug_assertions) { 3 } else { 1 };
    for (rules, games) in [
        (presets::tenhou_sanma(), 150 / scale),
        (presets::test_yonma(), 80 / scale),
    ] {
        for seed in 0..games {
            let (g, _) = play_with(&rules, seed, true);
            check_invariants(&rules, &g);
            horas += g
                .kyokus()
                .flatten()
                .filter(|e| matches!(e, Event::Hora(_)))
                .count();
        }
    }
    assert!(
        horas > 600 / scale,
        "greedy play reaches hora often: {horas}"
    );
}
