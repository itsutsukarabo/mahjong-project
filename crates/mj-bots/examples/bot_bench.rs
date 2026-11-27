//! ボット同士の対局の統計（和了率・放銃率・平均打点・流局率など）と速さ（局/秒）。
//!
//! ```text
//! cargo run --release -p mj-bots --example bot_bench -- [--rules tenhou-sanma|mjsoul-sanma|test-yonma]
//!     [--mode single|game] [--games 2000] [--threads 1] [--seed 1]
//! ```
//! - `single` は1局戦（`table.length = single` に上書き）、`game` はプリセットの長さ（天鳳三麻は東南戦）
//! - 牌山は `chacha8-v1`（`game_seed = seed × 2^32 + 対局の番号`）。ボットは乱数を使わないので、同じ引数なら同じ結果
//! - 速さは対局の生成（判断ポイントごとのボットの計算を含む）全体の局/秒。観測（特徴量）は作らない

use mj_bots::RuleBot;
use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::rules::{Rules, presets};
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Default, Clone)]
struct Stats {
    games: u64,
    kyokus: u64,
    steps: u64,
    player_kyokus: u64,
    hora: u64,
    tsumo: u64,
    deal_in: u64,
    hora_points: i64,
    hora_kyokus: u64,
    riichi: u64,
    called: u64,
    ends: BTreeMap<String, u64>,
    rare: BTreeMap<&'static str, u64>,
}

impl Stats {
    fn add(&mut self, o: &Stats) {
        self.games += o.games;
        self.kyokus += o.kyokus;
        self.steps += o.steps;
        self.player_kyokus += o.player_kyokus;
        self.hora += o.hora;
        self.tsumo += o.tsumo;
        self.deal_in += o.deal_in;
        self.hora_points += o.hora_points;
        self.hora_kyokus += o.hora_kyokus;
        self.riichi += o.riichi;
        self.called += o.called;
        for (k, v) in &o.ends {
            *self.ends.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.rare {
            *self.rare.entry(k).or_default() += v;
        }
    }
}

fn run(rules: &Rules, seeds: impl Iterator<Item = u64>) -> Stats {
    let bot = RuleBot::new(rules);
    let n = u64::from(rules.table.num_players);
    let mut st = Stats::default();
    for seed in seeds {
        let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
        while !g.is_over() {
            let k = g.current_kyoku();
            let acts: Vec<_> = g
                .decisions()
                .iter()
                .map(|d| (d.seat, bot.choose(k, d)))
                .collect();
            g.step(&acts).expect("legal");
            st.steps += 1;
        }
        st.games += 1;
        for events in g.kyokus() {
            st.kyokus += 1;
            st.player_kyokus += n;
            let mut called = vec![false; n as usize];
            let mut any_hora = false;
            for e in events {
                match e {
                    Event::Hora(h) => {
                        any_hora = true;
                        st.hora += 1;
                        st.hora_points += h.points;
                        if h.actor == h.target {
                            st.tsumo += 1;
                        } else {
                            st.deal_in += 1;
                        }
                    }
                    Event::ReachAccepted { .. } => st.riichi += 1,
                    Event::Pon { actor, .. }
                    | Event::Chi { actor, .. }
                    | Event::Daiminkan { actor, .. } => called[usize::from(*actor)] = true,
                    Event::Ryukyoku(r) => *st.ends.entry(r.reason.to_string()).or_default() += 1,
                    _ => {}
                }
            }
            if any_hora {
                st.hora_kyokus += 1;
                *st.ends.entry("hora".into()).or_default() += 1;
            }
            st.called += called.iter().filter(|&&c| c).count() as u64;
            for r in mj_bots::stats::rare_events(events) {
                *st.rare.entry(r).or_default() += 1;
            }
        }
    }
    st
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str, default: &str| -> String {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };
    let rules_name = get("--rules", "tenhou-sanma");
    let mode = get("--mode", "single");
    let games: u64 = get("--games", "2000").parse().expect("--games");
    let threads: u64 = get("--threads", "1").parse().expect("--threads");
    let seed: u64 = get("--seed", "1").parse().expect("--seed");
    let base = match rules_name.as_str() {
        "tenhou-sanma" => presets::tenhou_sanma(),
        "mjsoul-sanma" => presets::mjsoul_sanma(),
        "test-yonma" => presets::test_yonma(),
        other => panic!("unknown rules {other}"),
    };
    let rules = if mode == "single" {
        presets::with(&base, "[table]\nlength = 'single'")
    } else {
        base
    };
    let t0 = Instant::now();
    let per = games.div_ceil(threads);
    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let rules = rules.clone();
            std::thread::spawn(move || {
                let lo = t * per;
                let hi = ((t + 1) * per).min(games);
                run(&rules, (lo..hi).map(|i| (seed << 32) + i))
            })
        })
        .collect();
    let mut st = Stats::default();
    for h in handles {
        st.add(&h.join().expect("thread"));
    }
    let secs = t0.elapsed().as_secs_f64();
    let pk = st.player_kyokus as f64;
    let pct = |x: u64, d: f64| 100.0 * x as f64 / d;
    println!(
        "rules={rules_name} mode={mode} games={} kyokus={} threads={threads} seed={seed}",
        st.games, st.kyokus
    );
    println!(
        "elapsed={secs:.2}s kyoku/s={:.0} steps/s={:.0}",
        st.kyokus as f64 / secs,
        st.steps as f64 / secs
    );
    println!(
        "per player-kyoku: hora={:.2}% (tsumo {:.2}%, ron {:.2}%) deal_in={:.2}% riichi={:.2}% called={:.2}%",
        pct(st.hora, pk),
        pct(st.tsumo, pk),
        pct(st.deal_in, pk),
        pct(st.deal_in, pk),
        pct(st.riichi, pk),
        pct(st.called, pk)
    );
    println!(
        "per kyoku: ended_by_hora={:.2}% avg_hora_points={:.0}",
        pct(st.hora_kyokus, st.kyokus as f64),
        st.hora_points as f64 / st.hora.max(1) as f64
    );
    let ends: Vec<String> = st
        .ends
        .iter()
        .map(|(k, v)| format!("{k}={v} ({:.2}%)", pct(*v, st.kyokus as f64)))
        .collect();
    println!("kyoku ends: {}", ends.join(" "));
    let rare: Vec<String> = mj_bots::stats::RARE
        .iter()
        .map(|k| format!("{k}={}", st.rare.get(k).copied().unwrap_or(0)))
        .collect();
    println!("rare: {}", rare.join(" "));
}
