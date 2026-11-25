//! 無作為な合法手での自己対戦の速さ（局/秒）と、局の終わり方の内訳。
//!
//! ```text
//! cargo run --release -p mj-core --example selfplay_bench -- [--rules tenhou-sanma|test-yonma]
//!     [--mode single|game] [--seconds 10] [--threads 1]
//! ```
//! `single` は1局戦（`table.length = single` に上書き）、`game` はプリセットの長さ（天鳳三麻は東南戦）。
//! 観測（特徴量）は作らない。RiichiEnv の Rust コア単体の計測（`docs/research/ENGINE_SURVEY.md` §3.2）と同じ条件。

use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::rules::{Rules, presets};
use mj_core::wall::uniform_below;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Stats {
    kyokus: u64,
    games: u64,
    steps: u64,
    events: u64,
    ends: BTreeMap<String, u64>,
}

fn run(rules: &Rules, seed0: u64, until: Instant) -> Stats {
    let mut st = Stats::default();
    let mut rng = ChaCha8Rng::seed_from_u64(seed0);
    let mut seed = seed0 << 32;
    while Instant::now() < until {
        let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
        seed += 1;
        while !g.is_over() {
            let acts: Vec<_> = g
                .decisions()
                .iter()
                .map(|d| {
                    let i = uniform_below(&mut rng, d.choices.len() as u32) as usize;
                    (d.seat, d.choices[i])
                })
                .collect();
            g.step(&acts).expect("legal");
            st.steps += 1;
        }
        st.games += 1;
        for k in g.kyokus() {
            st.kyokus += 1;
            st.events += k.len() as u64;
            for e in k {
                let key = match e {
                    Event::Hora(h) => Some(
                        if h.actor == h.target {
                            "hora:tsumo"
                        } else {
                            "hora:ron"
                        }
                        .to_string(),
                    ),
                    Event::Ryukyoku(r) => Some(format!("ryukyoku:{}", r.reason)),
                    _ => None,
                };
                if let Some(k) = key {
                    *st.ends.entry(k).or_default() += 1;
                }
            }
        }
    }
    st
}

fn main() {
    let mut rules_name = "tenhou-sanma".to_string();
    let mut mode = "single".to_string();
    let mut seconds = 10u64;
    let mut threads = 1usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().expect("value");
        match a.as_str() {
            "--rules" => rules_name = v,
            "--mode" => mode = v,
            "--seconds" => seconds = v.parse().unwrap(),
            "--threads" => threads = v.parse().unwrap(),
            _ => panic!("unknown {a}"),
        }
    }
    let base = match rules_name.as_str() {
        "tenhou-sanma" => presets::tenhou_sanma(),
        "test-yonma" => presets::test_yonma(),
        _ => panic!("unknown rules"),
    };
    let rules = if mode == "single" {
        presets::with(&base, "[table]\nlength = 'single'")
    } else {
        base
    };
    let start = Instant::now();
    let until = start + Duration::from_secs(seconds);
    let results: Vec<Stats> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                let r = &rules;
                s.spawn(move || run(r, t as u64 + 1, until))
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let el = start.elapsed().as_secs_f64();
    let mut total = Stats::default();
    for r in results {
        total.kyokus += r.kyokus;
        total.games += r.games;
        total.steps += r.steps;
        total.events += r.events;
        for (k, v) in r.ends {
            *total.ends.entry(k).or_default() += v;
        }
    }
    println!(
        "rules={rules_name} mode={mode} threads={threads} seconds={el:.2} games={} kyokus={} kyoku/s={:.0} steps/kyoku={:.1} events/kyoku={:.1} us/step={:.2}",
        total.games,
        total.kyokus,
        total.kyokus as f64 / el,
        total.steps as f64 / total.kyokus as f64,
        total.events as f64 / total.kyokus as f64,
        el * 1e6 * threads as f64 / total.steps as f64,
    );
    for (k, v) in &total.ends {
        println!(
            "  {k}: {v} ({:.1}%)",
            *v as f64 * 100.0 / total.kyokus as f64
        );
    }
}
