//! 観測の検査（OBSERVATION.md §5）。
//! 1. 漏れの検査：見えている情報が同じで、他家の手牌・他家のツモ・ツモ山の中身・めくっていない表示牌・裏ドラ表示牌だけが
//!    違う局面（全情報のイベント列の隠れた欄を書き換えたもの）で、方策の入力（policy・events・mask）が完全に一致する
//! 2. 決定論：同じ局面からは常に同じ観測になる（別の Observer でも）
//! 3. 区画の次元が Layout と一致する

use mj_bots::RuleBot;
use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::policy::uniform_choice;
use mj_core::rules::{Rules, presets};
use mj_core::tile::{Tile, TileKind};
use mj_core::wall::uniform_below;
use mj_features::obs::{MAX_EVENTS, Observer, RewardInput};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

fn random_tile(rng: &mut ChaCha8Rng, rules: &Rules) -> Tile {
    let all = mj_core::wall::tile_multiset(&rules.tiles);
    all[uniform_below(rng, all.len() as u32) as usize]
}

/// 席 `seat` から見えない欄だけを書き換えたイベント列
fn hide_shuffled(events: &[Event], seat: u8, rules: &Rules, rng: &mut ChaCha8Rng) -> Vec<Event> {
    events
        .iter()
        .map(|e| match e {
            Event::KyokuStart(s) => {
                let mut s = s.clone();
                for (p, h) in s.tehais.iter_mut().enumerate() {
                    if p != usize::from(seat) {
                        for t in h.iter_mut() {
                            *t = random_tile(rng, rules);
                        }
                    }
                }
                for t in s.pile.iter_mut() {
                    *t = random_tile(rng, rules);
                }
                // めくっていない表示牌（2枚目以降は Dora イベントで出る）と裏ドラ
                for t in s.dora_indicators.iter_mut().skip(1) {
                    *t = random_tile(rng, rules);
                }
                for t in s.ura_indicators.iter_mut() {
                    *t = random_tile(rng, rules);
                }
                Event::KyokuStart(s)
            }
            Event::Tsumo {
                actor,
                pai: _,
                rinshan,
            } if *actor != seat => Event::Tsumo {
                actor: *actor,
                pai: random_tile(rng, rules),
                rinshan: *rinshan,
            },
            other => other.clone(),
        })
        .collect()
}

fn check(rules: Rules, games: u64, random: bool) -> (u64, u64) {
    let bot = RuleBot::new(&rules);
    let obs = Observer::new(&rules);
    let obs2 = Observer::new(&rules);
    let layout = obs.layout();
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    let mut points = 0;
    let mut critic_changed = 0;
    let reward = RewardInput {
        rank_points: [1.0, 0.0, -1.0, 0.0],
        score_coef: 0.0,
        score_base: 0,
    };
    for seed in 0..games {
        let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
        while !g.is_over() {
            let k = g.current_kyoku();
            let mut acts = Vec::new();
            for d in g.decisions() {
                points += 1;
                let events = k.events();
                let a = obs.observe(events, d.seat, &d.choices, &reward);
                assert_eq!(a.policy.len(), layout.policy());
                assert_eq!(a.critic.len(), layout.critic);
                assert!(a.policy.iter().all(|x| x.is_finite()));
                // D-045：イベント列の上限（1局 192）を超えたらテストは失敗
                assert!(
                    a.events.len() <= MAX_EVENTS,
                    "events {} > {MAX_EVENTS}",
                    a.events.len()
                );
                // 決定論
                assert_eq!(a, obs.observe(events, d.seat, &d.choices, &reward));
                assert_eq!(a, obs2.observe(events, d.seat, &d.choices, &reward));
                // 漏れの検査
                let hidden = hide_shuffled(events, d.seat, &rules, &mut rng);
                let b = obs.observe(&hidden, d.seat, &d.choices, &reward);
                assert_eq!(a.policy, b.policy, "policy leaked hidden information");
                assert_eq!(a.events, b.events, "event tokens leaked hidden information");
                assert_eq!(a.mask, b.mask);
                if a.critic != b.critic {
                    critic_changed += 1;
                }
                let act = if random {
                    uniform_choice(&mut rng, &d.choices)
                } else {
                    bot.choose(k, d)
                };
                acts.push((d.seat, act));
            }
            g.step(&acts).expect("legal");
        }
    }
    (points, critic_changed)
}

#[test]
fn policy_input_does_not_depend_on_hidden_information() {
    let single = |r: Rules| presets::with(&r, "[table]\nlength = 'single'");
    let mut total = 0;
    let mut changed = 0;
    for (rules, random) in [
        (single(presets::tenhou_sanma()), false),
        (single(presets::tenhou_sanma()), true),
        (single(presets::mjsoul_sanma()), false),
        (single(presets::test_yonma()), false),
    ] {
        let (n, c) = check(rules, 10, random);
        total += n;
        changed += c;
    }
    eprintln!("decision points checked: {total}, critic changed by hidden info: {changed}");
    assert!(total > 1000);
    // 価値関数の入力は隠れた情報で変わる（区画が分かれていることの確認）
    assert!(changed * 2 > total, "critic should see hidden information");
}

#[test]
fn layout_sections() {
    let o = Observer::new(&presets::tenhou_sanma());
    let l = o.layout();
    eprintln!("{l:?} policy={}", l.policy());
    let _ = TileKind::new(0);
}
