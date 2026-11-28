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

#[test]
fn seat_stats_match_the_events() {
    use mj_bots::stats::{SeatStats, game_seat_stats, kyoku_seat_stats};
    for rules in [
        single(presets::tenhou_sanma()),
        single(presets::test_yonma()),
    ] {
        let n = rules.table.num_players;
        let mut total = vec![SeatStats::default(); usize::from(n)];
        let mut horas = 0u32;
        let mut tsumos = 0u32;
        let mut ron_targets = 0u32;
        let mut reaches = 0u32;
        let mut exhaustive = 0u32;
        for seed in 0..40 {
            let ev = play(&rules, seed);
            let ks = kyoku_seat_stats(&ev, n);
            // 和了の精算：和了者は＋、放銃者は −、和了のない局は 0。席の和は和了の deltas の和（供託の分だけ正）
            let hora_sum: i64 = ev
                .iter()
                .filter_map(|e| match e {
                    Event::Hora(h) => Some(h.deltas.iter().map(|&d| i64::from(d)).sum::<i64>()),
                    _ => None,
                })
                .sum();
            assert_eq!(ks.iter().map(|s| s.hora_delta).sum::<i64>(), hora_sum);
            for s in &ks {
                if s.hora > 0 {
                    assert!(s.hora_delta > 0, "{s:?}");
                } else if s.deal_in > 0 {
                    assert!(s.hora_delta < 0, "{s:?}");
                }
                if !ev.iter().any(|e| matches!(e, Event::Hora(_))) {
                    assert_eq!(s.hora_delta, 0);
                }
            }
            for (t, s) in total.iter_mut().zip(ks) {
                assert!(s.deal_in <= 1 && s.riichi <= 1 && s.call <= 1);
                t.add(&s);
            }
            let mut targets = std::collections::BTreeSet::new();
            for e in &ev {
                match e {
                    Event::Hora(h) => {
                        horas += 1;
                        if h.actor == h.target {
                            tsumos += 1;
                        } else {
                            targets.insert(h.target);
                        }
                    }
                    Event::Reach { .. } => reaches += 1,
                    Event::Ryukyoku(r)
                        if r.reason == "exhaustive" || r.reason == "nagashi_mangan" =>
                    {
                        exhaustive += 1
                    }
                    _ => {}
                }
            }
            ron_targets += targets.len() as u32;
        }
        let sum = |f: fn(&SeatStats) -> u32| total.iter().map(f).sum::<u32>();
        assert_eq!(sum(|s| s.kyokus), 40 * u32::from(n));
        assert_eq!(sum(|s| s.hora), horas);
        assert_eq!(sum(|s| s.tsumo), tsumos);
        assert_eq!(sum(|s| s.deal_in), ron_targets);
        assert_eq!(sum(|s| s.riichi), reaches);
        assert_eq!(sum(|s| s.exhaustive), exhaustive * u32::from(n));
        assert!(horas > 10 && reaches > 0, "hora {horas} riichi {reaches}");
        assert!(total.iter().all(|s| s.exhaustive_tenpai <= s.exhaustive));
        // 対局の和は局の和と同じ
        let g = game_seat_stats([play(&rules, 3).as_slice()], n);
        assert_eq!(g, kyoku_seat_stats(&play(&rules, 3), n));
    }
}
