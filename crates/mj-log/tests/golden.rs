//! ゴールデンテスト：固定シードから生成した牌山の sha256 を fixtures/golden/walls.json と照合する。
//! RNG（rand_chacha）や生成手順が変わると落ちる。意図した変更なら rng 名の版を上げ、
//! `MJ_UPDATE_GOLDEN=1 cargo test -p mj-log --test golden` で書き直す。
//! 局進行が入ったら「固定の行動列から作った牌譜のハッシュ」もここに足す。

use mj_core::rules::PresetFile;
use mj_core::wall::{RNG_NAME, generate_wall};
use mj_log::{wall_canonical, wall_to_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const CASES: &[(&str, u64, u64)] = &[
    ("rules/tenhou-sanma.toml", 0, 0),
    ("rules/tenhou-sanma.toml", 1, 0),
    ("rules/tenhou-sanma.toml", 1, 1),
    ("rules/tenhou-sanma.toml", 20261122, 7),
    ("rules/tenhou-sanma.toml", u64::MAX, 3),
    ("fixtures/rules/test-yonma-tiles.toml", 0, 0),
    ("fixtures/rules/test-yonma-tiles.toml", 1, 0),
    ("fixtures/rules/test-yonma-tiles.toml", 20261122, 7),
];

fn compute() -> Value {
    let rows: Vec<Value> = CASES
        .iter()
        .map(|&(rules, seed, kyoku)| {
            let text = std::fs::read_to_string(root().join(rules)).unwrap();
            let spec = PresetFile::parse(&text).unwrap().wall_spec().unwrap();
            let w = generate_wall(&spec, seed, kyoku);
            let (tehais, wall) = wall_to_json(&w);
            let digest = hex::encode(Sha256::digest(wall_canonical(&tehais, &wall).as_bytes()));
            json!({"rules": rules, "game_seed": format!("{seed:#x}"), "kyoku_idx": kyoku,
                   "first_pile_tiles": wall["pile"].as_array().unwrap()[..5],
                   "sha256": digest})
        })
        .collect();
    json!({"rng": RNG_NAME, "walls": rows})
}

#[test]
fn walls_match_golden() {
    let path = root().join("fixtures/golden/walls.json");
    let got = compute();
    if std::env::var("MJ_UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        return;
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        got, want,
        "wall golden mismatch (see crates/mj-log/tests/golden.rs)"
    );
}
