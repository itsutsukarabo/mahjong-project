//! 牌譜再生画面（`web/apps/replay`）のテスト用の牌譜を作る：
//! - `mjpy-selfplay.mjlog.jsonl`：mj-py の環境（`VecEnv`、`record_logs`）で一様な乱数の行動の半荘（全情報版）
//! - `rulebot.mjlog.jsonl`：mj-bots の RuleBot 同士の半荘（全情報版）と、その公開情報版 `rulebot.public.mjlog.jsonl`
//! - `rulebot.annot.jsonl`：上の対局の判断ポイントごとの注釈（RuleBot の行動。確率・価値は「一様な乱数の打ち手ならこう」という形式の確認用の値）
//!
//! `cargo run --release -p mj-py --example replay_fixtures -- <出力先>`

use mj_bots::RuleBot;
use mj_core::game::{Game, WallSource};
use mj_core::rules::presets;
use mj_features::reward::RewardSpec;
use mj_features::{NUM_ACTIONS, action_to_id};
use mj_log::write::{GameMeta, game_log};
use mj_log::{Viewer, redact, to_jsonl};
use mj_py::env::{EnvConfig, VecEnv};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde_json::json;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("output dir"));
    std::fs::create_dir_all(&out).unwrap();
    let rules = presets::tenhou_sanma();

    // mj-py の自己対戦（一様な乱数の行動）
    let mut env = VecEnv::new(EnvConfig {
        rules: rules.clone(),
        num_tables: 1,
        seed: 20261129,
        threads: 1,
        reward: RewardSpec::parse("tenhou-sanma-7dan+").unwrap(),
        record_logs: true,
        max_events: mj_features::obs::MAX_EVENTS,
    });
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut b = env.observe();
    let log = loop {
        let acts: Vec<i64> = (0..b.decision_id.len())
            .map(|i| {
                let legal: Vec<usize> = (0..NUM_ACTIONS)
                    .filter(|&a| b.mask[i * NUM_ACTIONS + a])
                    .collect();
                legal[(rng.next_u32() as usize) % legal.len()] as i64
            })
            .collect();
        b = env.step(&b.decision_id.clone(), &acts).unwrap();
        if let Some(f) = b.finished.first() {
            break f.log_jsonl.clone().unwrap();
        }
    };
    std::fs::write(out.join("mjpy-selfplay.mjlog.jsonl"), log).unwrap();

    // RuleBot 同士の半荘と注釈
    let bot = RuleBot::new(&rules);
    let mut g = Game::new(rules.clone(), WallSource::Seed(7));
    let game_id = "rulebot-seed7";
    let mut annots = Vec::new();
    while !g.is_over() {
        let ki = g.kyokus().count() as i64 - 1;
        let k = g.current_kyoku();
        let ev = k.events().len() - 1;
        let acts: Vec<_> = g
            .decisions()
            .iter()
            .map(|d| {
                let a = bot.choose(k, d);
                let p = 1.0 / d.choices.len() as f64;
                debug_assert!(action_to_id(&a).is_some());
                annots.push(json!({
                    "game_id": game_id, "kyoku_idx": ki, "event_idx": ev, "seat": d.seat,
                    "decision": a.to_string(),
                    "choices": d.choices.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                    "model": "rulebot-v1",
                    "policy": d.choices.iter().map(|c| json!([c.to_string(), p])).collect::<Vec<_>>(),
                    "value": 0.0,
                }));
                (d.seat, a)
            })
            .collect();
        g.step(&acts).unwrap();
    }
    let meta = GameMeta {
        game_id: game_id.into(),
        players: vec!["rulebot-0".into(), "rulebot-1".into(), "rulebot-2".into()],
        engine_version: env!("CARGO_PKG_VERSION").into(),
        engine_git: "fixture".into(),
    };
    let full = game_log(&g, &meta, &mut |k| format!("{:032x}", 0xf1_7700 + k));
    mj_log::validate_annotations(&annots, &full).expect("annotations");
    std::fs::write(out.join("rulebot.mjlog.jsonl"), to_jsonl(&full)).unwrap();
    std::fs::write(
        out.join("rulebot.public.mjlog.jsonl"),
        to_jsonl(&redact(&full, Viewer::Public).unwrap()),
    )
    .unwrap();
    std::fs::write(out.join("rulebot.annot.jsonl"), to_jsonl(&annots)).unwrap();
    eprintln!("written to {}", out.display());
}
