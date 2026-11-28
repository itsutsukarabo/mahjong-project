//! 観測の実測：イベント列の長さの分布（1局・1半荘）と、観測1つを作る時間。
//!
//! ```text
//! cargo run --release -p mj-features --example obs_bench -- [--rules tenhou-sanma|test-yonma] [--games 1000] [--seed 1]
//! ```
//! - 打ち手は mj-bots の RuleBot（同じ引数なら同じ結果）
//! - 長さは席視点のイベント列のトークン数（局の終わりの時点。判断ポイントの観測はその途中までの長さ）
//! - 時間は判断ポイントごとの `Observer::observe` だけを測る（1スレッド）

use mj_bots::RuleBot;
use mj_core::event::Event;
use mj_core::game::{Game, WallSource};
use mj_core::rules::presets;
use mj_features::obs::{Observer, RewardInput, seat_view};
use std::time::{Duration, Instant};

fn pct(v: &mut [usize], p: f64) -> usize {
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str, d: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    };
    let rules_name = get("--rules", "tenhou-sanma");
    let games: u64 = get("--games", "1000").parse().unwrap();
    let seed: u64 = get("--seed", "1").parse().unwrap();
    let rules = match rules_name.as_str() {
        "tenhou-sanma" => presets::tenhou_sanma(),
        "mjsoul-sanma" => presets::mjsoul_sanma(),
        _ => presets::test_yonma(),
    };
    let bot = RuleBot::new(&rules);
    let obs = Observer::new(&rules);
    let reward = RewardInput::default();
    let mut per_kyoku: Vec<usize> = Vec::new();
    let mut per_game: Vec<usize> = Vec::new();
    let mut obs_time = Duration::ZERO;
    let mut n_obs = 0u64;
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    let fnv = |h: u64, x: u64| (h ^ x).wrapping_mul(0x0100_0000_01b3);
    for g_i in 0..games {
        let mut g = Game::new(rules.clone(), WallSource::Seed((seed << 32) + g_i));
        while !g.is_over() {
            let k = g.current_kyoku();
            let mut acts = Vec::new();
            for d in g.decisions() {
                let t0 = Instant::now();
                let o = obs.observe(k.events(), d.seat, &d.choices, &reward);
                obs_time += t0.elapsed();
                n_obs += 1;
                // 観測すべての指紋（実装を変えても結果が同じかを、前後の実行で比べる）
                for x in o.policy.iter().chain(&o.critic) {
                    digest = fnv(digest, u64::from(x.to_bits()));
                }
                for t in &o.events {
                    for v in t {
                        digest = fnv(digest, *v as u64);
                    }
                }
                for m in &o.mask {
                    digest = fnv(digest, u64::from(*m));
                }
                acts.push((d.seat, bot.choose(k, d)));
            }
            g.step(&acts).expect("legal");
        }
        let mut game_len = 0;
        for events in g.kyokus() {
            // 席0 の視点（トークンの数は席によらない：見える・見えないは欄の中身だけ）
            let view: Vec<Event> = events.to_vec();
            let n = obs
                .observe(&view[..view.len()], 0, &[], &reward)
                .events
                .len();
            let _ = seat_view(&rules, &view, 0);
            per_kyoku.push(n);
            game_len += n;
        }
        per_game.push(game_len);
    }
    let avg = |v: &[usize]| v.iter().sum::<usize>() as f64 / v.len() as f64;
    let (ak, ag) = (avg(&per_kyoku), avg(&per_game));
    let (mk, mg) = (
        *per_kyoku.iter().max().unwrap(),
        *per_game.iter().max().unwrap(),
    );
    let (pk, pg) = (pct(&mut per_kyoku, 0.99), pct(&mut per_game, 0.99));
    let (pk999, pg999) = (pct(&mut per_kyoku, 0.999), pct(&mut per_game, 0.999));
    println!(
        "rules={rules_name} games={games} kyokus={} seed={seed}",
        per_kyoku.len()
    );
    println!("tokens per kyoku: avg={ak:.1} p99={pk} p99.9={pk999} max={mk}");
    println!("tokens per game:  avg={ag:.1} p99={pg} p99.9={pg999} max={mg}");
    println!(
        "observe(): {n_obs} observations, {:.2} us each (1 thread), digest {digest:016x}",
        obs_time.as_secs_f64() * 1e6 / n_obs as f64
    );
    let l = obs.layout();
    println!(
        "layout: {:?} policy={} critic={}",
        l.sections,
        l.policy(),
        l.critic
    );
}
