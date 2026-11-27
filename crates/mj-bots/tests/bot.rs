//! ルールベースのボットのテスト：合法手だけを選ぶ、決定論、押し引き。

use mj_bots::RuleBot;
use mj_core::action::Action;
use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::rules::{Rules, presets};
use mj_core::scenario::{Scenario, act, auto_until_seat};

fn single(r: Rules) -> Rules {
    presets::with(&r, "[table]\nlength = 'single'")
}

/// ボット同士で1局戦を回し、イベント列を返す（ボットの行動が合法でなければ `step` が失敗する）。
fn play(rules: &Rules, seed: u64) -> Vec<Event> {
    let bot = RuleBot::new(rules);
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
    while !g.is_over() {
        let k = g.current_kyoku();
        let acts: Vec<_> = g
            .decisions()
            .iter()
            .map(|d| {
                let a = bot.choose(k, d);
                assert!(d.choices.contains(&a), "{a} not in choices");
                (d.seat, a)
            })
            .collect();
        g.step(&acts).expect("legal");
    }
    g.kyokus().flatten().cloned().collect()
}

#[test]
fn bot_plays_legal_and_deterministic_games() {
    for rules in [
        single(presets::tenhou_sanma()),
        single(presets::mjsoul_sanma()),
        single(presets::test_yonma()),
    ] {
        let mut hora = 0;
        for seed in 0..12 {
            let a = play(&rules, seed);
            assert_eq!(a, play(&rules, seed), "deterministic");
            hora += a.iter().any(|e| matches!(e, Event::Hora(_))) as u32;
        }
        // 乱数の打ち手（和了 0.6%）と違い、多くの局が和了で終わる
        assert!(hora >= 6, "hora in {hora} / 12 kyoku");
    }
}

#[test]
fn bot_folds_against_riichi_with_genbutsu() {
    // 席1 が F（發）をツモ切りで立直。席0 は形の悪い手（2シャンテン以上）で、F の対子を持っている
    let s0 = "1m 1m 9m 2p 6p 9p 3s 7s S W F F C";
    let s1 = "1p 2p 3p 4p 5p 6p 7p 8p 9p 2s 3s 5s 5s";
    let s2 = "9m 9m 1p 4p 8p 2s 6s 9s S W P P C";
    let run = |second: &str| {
        let rules = presets::tenhou_sanma();
        let bot = RuleBot::new(&rules);
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("E F 1s E")
            .kyoku();
        act(&mut k, 0, "dahai:E:tsumogiri");
        act(&mut k, 1, second);
        // 席0 は F をポンできる：見送る
        act(&mut k, 0, "pass");
        assert!(auto_until_seat(&mut k, 0));
        let d = k.decisions()[0].clone();
        assert_eq!(d.seat, 0);
        bot.choose(&k, &d)
    };
    // 立直者の現物（F）を切って降りる
    assert_eq!(run("reach:F:tsumogiri").to_string(), "dahai:F");
    // 立直者がいなければ F の対子は残す
    let a = run("dahai:F:tsumogiri");
    assert!(
        !matches!(a, Action::Dahai { tile, .. } if tile.to_string() == "F"),
        "{a}"
    );
}
