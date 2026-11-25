//! ゴールデンテスト：固定の行動列から作った牌譜。
//!
//! `fixtures/golden/games/<name>.actions.json` の行動列をエンジンで再生し、
//! (1) 各判断ポイントで行動がエンジンの選択肢にあること、(2) 全情報版が `validate_full` を通ること、
//! (3) 席視点版・公開情報版に変換できること、(4) 牌譜の sha256 が `fixtures/golden/games.json` と一致すること、を確かめる。
//!
//! 行動列は「シャンテン数を下げる打ち手」（`mj_core::policy::greedy_choice`）を固定シードで動かして作った。
//! エンジンの挙動を意図して変えたら `MJ_UPDATE_GOLDEN=1 cargo test -p mj-log --test golden_games` で作り直す。
//! `wall_salt` はテスト用の固定値（本番は OS の乱数、LOG_FORMAT §4.6）。

use mj_core::action::Action;
use mj_core::game::{Game, WallSource};
use mj_core::policy::greedy_choice;
use mj_core::rules::{Rules, presets};
use mj_core::shanten::ShantenCalc;
use mj_log::write::{GameMeta, game_log};
use mj_log::{Viewer, redact, to_jsonl, validate_full};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// (名前, ルール, 対局の長さの上書き, 牌山のシード)
const CASES: &[(&str, &str, Option<&str>, u64)] = &[
    ("sanma-hanchan-1", "tenhou-sanma", None, 1),
    ("sanma-hanchan-2", "tenhou-sanma", None, 2),
    ("sanma-tonpuu-3", "tenhou-sanma", Some("tonpuu"), 3),
    ("sanma-single-4", "tenhou-sanma", Some("single"), 4),
    ("yonma-hanchan-1", "test-yonma", None, 1),
];

fn rules_for(name: &str, length: Option<&str>) -> Rules {
    let base = match name {
        "tenhou-sanma" => presets::tenhou_sanma(),
        "test-yonma" => presets::test_yonma(),
        _ => panic!("unknown rules {name}"),
    };
    match length {
        Some(l) => presets::with(&base, &format!("[table]\nlength = '{l}'")),
        None => base,
    }
}

fn meta(name: &str, n: u8) -> GameMeta {
    GameMeta {
        game_id: format!("golden-{name}"),
        players: (0..n).map(|i| format!("greedy-{i}")).collect(),
        engine_version: "0.1.0".into(),
        engine_git: "golden".into(),
    }
}

fn test_salt(kyoku_idx: u64) -> String {
    format!("{:032x}", 0x60_1d_e1 + kyoku_idx)
}

/// 打ち手を動かして行動列を作る（作り直すときだけ使う）
fn generate(rules: &Rules, seed: u64) -> Vec<Vec<(u8, String)>> {
    let calc = ShantenCalc::new(rules.tiles.manzu);
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x9a1d);
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
    let mut steps = Vec::new();
    while !g.is_over() {
        let acts: Vec<(u8, Action)> = g
            .decisions()
            .iter()
            .map(|d| {
                let k = g.current_kyoku();
                let hand = k.concealed_counts(d.seat);
                let melds = k.melds(d.seat).len() as u8;
                (
                    d.seat,
                    greedy_choice(&mut rng, &d.choices, &hand, melds, &calc, 30),
                )
            })
            .collect();
        steps.push(acts.iter().map(|(s, a)| (*s, a.to_string())).collect());
        g.step(&acts).unwrap();
    }
    steps
}

fn replay(rules: &Rules, seed: u64, steps: &[Vec<(u8, String)>]) -> Game {
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
    for (i, step) in steps.iter().enumerate() {
        let acts: Vec<(u8, Action)> = step
            .iter()
            .map(|(s, a)| (*s, a.parse().unwrap_or_else(|e| panic!("step {i}: {e}"))))
            .collect();
        let seats: Vec<u8> = g.decisions().iter().map(|d| d.seat).collect();
        assert_eq!(
            seats,
            acts.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            "step {i}: decision seats"
        );
        g.step(&acts).unwrap_or_else(|e| panic!("step {i}: {e}"));
    }
    assert!(g.is_over(), "action list ends the game");
    g
}

#[test]
fn golden_games_replay_and_match() {
    let update = std::env::var("MJ_UPDATE_GOLDEN").as_deref() == Ok("1");
    let dir = root().join("fixtures/golden/games");
    let mut rows = Vec::new();
    for &(name, rules_name, length, seed) in CASES {
        let rules = rules_for(rules_name, length);
        let path = dir.join(format!("{name}.actions.json"));
        let steps: Vec<Vec<(u8, String)>> = if update {
            let s = generate(&rules, seed);
            let doc = json!({"name": name, "rules": rules_name, "length": length, "game_seed": seed,
                             "policy": "greedy_choice(call_pct=30), rng seed = game_seed ^ 0x9a1d",
                             "steps": s});
            std::fs::write(&path, serde_json::to_string(&doc).unwrap() + "\n").unwrap();
            s
        } else {
            let doc: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            serde_json::from_value(doc["steps"].clone()).unwrap()
        };
        let g = replay(&rules, seed, &steps);
        let mut salt = test_salt;
        let log = game_log(&g, &meta(name, rules.table.num_players), &mut salt);
        let report = validate_full(&log).unwrap_or_else(|e| panic!("{name}: {e}"));
        for v in
            std::iter::once(Viewer::Public).chain((0..rules.table.num_players).map(Viewer::Seat))
        {
            let r = redact(&log, v).unwrap();
            assert_eq!(r.len(), log.len());
        }
        let text = to_jsonl(&log);
        let count = |t: &str| log.iter().filter(|e| e["type"] == t).count();
        rows.push(json!({
            "name": name, "events": log.len(), "kyoku": report.kyoku, "steps": steps.len(),
            "hora": count("hora"), "ryukyoku": count("ryukyoku"),
            "bytes": text.len(), "sha256": hex::encode(Sha256::digest(text.as_bytes())),
        }));
    }
    let path = root().join("fixtures/golden/games.json");
    let got = json!({"format_version": mj_log::FORMAT_VERSION, "games": rows});
    if update {
        std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        return;
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        got, want,
        "game log golden mismatch (see crates/mj-log/tests/golden_games.rs)"
    );
}
