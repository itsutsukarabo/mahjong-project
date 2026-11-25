//! ゴールデンの行動列から牌譜を作り、標準出力に JSONL で書く（確認用）。
//! `cargo run -p mj-log --example dump_golden -- sanma-hanchan-1`

use mj_core::action::Action;
use mj_core::game::{Game, WallSource};
use mj_core::rules::presets;
use mj_log::to_jsonl;
use mj_log::write::{GameMeta, game_log};
use serde_json::Value;

fn main() {
    let name = std::env::args().nth(1).expect("name");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("fixtures/golden/games/{name}.actions.json")))
            .unwrap(),
    )
    .unwrap();
    let base = match doc["rules"].as_str().unwrap() {
        "tenhou-sanma" => presets::tenhou_sanma(),
        _ => presets::test_yonma(),
    };
    let rules = match doc["length"].as_str() {
        Some(l) => presets::with(&base, &format!("[table]\nlength = '{l}'")),
        None => base,
    };
    let steps: Vec<Vec<(u8, String)>> = serde_json::from_value(doc["steps"].clone()).unwrap();
    let mut g = Game::new(
        rules.clone(),
        WallSource::Seed(doc["game_seed"].as_u64().unwrap()),
    );
    for s in &steps {
        let acts: Vec<(u8, Action)> = s.iter().map(|(x, a)| (*x, a.parse().unwrap())).collect();
        g.step(&acts).unwrap();
    }
    let meta = GameMeta {
        game_id: format!("golden-{name}"),
        players: (0..rules.table.num_players)
            .map(|i| format!("greedy-{i}"))
            .collect(),
        engine_version: "0.1.0".into(),
        engine_git: "golden".into(),
    };
    let mut salt = |k: u64| format!("{:032x}", 0x60_1d_e1 + k);
    print!("{}", to_jsonl(&game_log(&g, &meta, &mut salt)));
}
