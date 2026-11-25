//! 無作為な合法手での自己対戦：止まらずに終局し、不変条件が保たれること。

use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::rules::{Rules, presets};
use mj_core::wall::uniform_below;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

/// 対局を最後まで打つ。戻り値は局数。
fn play(rules: &Rules, seed: u64) -> (Game, usize) {
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
                let i = uniform_below(&mut rng, d.choices.len() as u32) as usize;
                (d.seat, d.choices[i])
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
