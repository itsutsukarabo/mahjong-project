//! 変換表（D-018）に当たった手の例の回帰テスト（Python なしで回る）。
//! `fixtures/difftest-riichienv/conversions.jsonl` は差分テストの `--out` の `conversions.jsonl` を写したもの。
//! mj-core の答え（シャンテン数・待ち）が記録と同じで、RiichiEnv の答えとの違いが変換表の形のままであることを確かめる。

use mj_core::agari;
use mj_core::rules::Manzu;
use mj_core::shanten::ShantenCalc;
use mj_core::tile::{Tile, kind_counts};
use serde_json::Value;

#[test]
fn recorded_conversions_still_hold() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/difftest-riichienv/conversions.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("fixture");
    let calc = ShantenCalc::new(Manzu::TerminalsOnly);
    let mut n = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).unwrap();
        let tiles: Vec<Tile> = v["tiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().parse().unwrap())
            .collect();
        let c = kind_counts(&tiles);
        assert_eq!(
            i64::from(calc.shanten(&c, 0)),
            v["ours"]["shanten"].as_i64().unwrap(),
            "{line}"
        );
        if v["rule"].as_str().unwrap().contains("ours 0 / riichienv 1") {
            assert_eq!(agari::waits(&c, true), 0, "{line}");
            assert_eq!(v["riichienv"]["shanten"].as_i64(), Some(1), "{line}");
        }
        n += 1;
    }
    assert!(n > 0);
}
