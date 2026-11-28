//! LOG_FORMAT v0.2 のサンプル（docs/spec/samples/）を Rust 側でも検証する。
//! Python の生成スクリプトと Rust の redact が同じ規則であることを、1行ずつの一致で確かめる。

use mj_core::rules::Rules;
use mj_log::{
    Viewer, canonical_json, event_key, parse_jsonl, redact, validate_annotations, validate_full,
};
use serde_json::Value;

const FULL: &str = include_str!("../../../docs/spec/samples/sample_full.jsonl");
const PUBLIC: &str = include_str!("../../../docs/spec/samples/sample_public.jsonl");
const ANNOT: &str = include_str!("../../../docs/spec/samples/sample_annot.jsonl");

#[test]
fn full_sample_is_valid() {
    let ev = parse_jsonl(FULL).unwrap();
    let r = validate_full(&ev).unwrap();
    assert_eq!((r.events, r.kyoku, r.draws, r.rinshan_draws), (25, 1, 7, 1));
}

#[test]
fn redact_public_matches_committed_public_sample() {
    let ev = parse_jsonl(FULL).unwrap();
    let ours = redact(&ev, Viewer::Public).unwrap();
    let want: Vec<&str> = PUBLIC.lines().collect();
    assert_eq!(ours.len(), want.len());
    for (o, w) in ours.iter().zip(&want) {
        assert_eq!(canonical_json(o), *w);
    }
}

#[test]
fn redaction_keeps_keys_and_hides_private_values() {
    let ev = parse_jsonl(FULL).unwrap();
    for viewer in [
        Viewer::Public,
        Viewer::Seat(0),
        Viewer::Seat(1),
        Viewer::Seat(2),
    ] {
        let out = redact(&ev, viewer).unwrap();
        let k1: Vec<_> = ev.iter().map(|e| event_key(e).unwrap()).collect();
        let k2: Vec<_> = out.iter().map(|e| event_key(e).unwrap()).collect();
        assert_eq!(k1, k2, "{viewer:?}");
        for e in &out {
            if e["type"] == "kyoku_start" {
                assert!(e.get("wall").is_none() && e.get("wall_salt").is_none());
                assert!(e.get("wall_commit").is_some());
            }
            if e["type"] == "game_start" {
                assert!(e["seed"].get("game_seed").is_none());
            }
        }
        for (full, red) in ev.iter().zip(&out) {
            if full["type"] == "tsumo" {
                let mine = matches!(viewer, Viewer::Seat(n) if full["actor"] == u64::from(n));
                assert_eq!(red["pai"] == full["pai"], mine, "{viewer:?} {full}");
            }
            if full["type"] == "kyoku_start" {
                for i in 0..3u8 {
                    let mine = viewer == Viewer::Seat(i);
                    assert_eq!(
                        red["tehais"][usize::from(i)] == full["tehais"][usize::from(i)],
                        mine
                    );
                }
            }
        }
    }
}

#[test]
fn ryukyoku_reveal_and_closed_ankan_follow_rules() {
    let mut ev = parse_jsonl(FULL).unwrap();
    ev[0]["rules"]["log_visibility"]["ankan"] = Value::from("closed");
    let ryu: Value = serde_json::json!({
        "game_id": "g-20261122-000001", "kyoku_idx": 0, "event_idx": 99, "type": "ryukyoku",
        "reason": "exhaustive", "tenpai": [true, false, true],
        "hands": [["1p"], ["2p"], ["3p"]], "deltas": [1000, -2000, 1000], "scores": [0, 0, 0]});
    let ankan: Value = serde_json::json!({
        "game_id": "g-20261122-000001", "kyoku_idx": 0, "event_idx": 98, "type": "ankan",
        "actor": 1, "consumed": ["E", "E", "E", "E"]});
    ev.push(ankan);
    ev.push(ryu);
    let public = redact(&ev, Viewer::Public).unwrap();
    assert_eq!(
        public[25]["consumed"],
        serde_json::json!(["?", "?", "?", "?"])
    );
    assert_eq!(
        public[26]["hands"],
        serde_json::json!([["1p"], null, ["3p"]])
    );
    let seat1 = redact(&ev, Viewer::Seat(1)).unwrap();
    assert_eq!(
        seat1[25]["consumed"],
        serde_json::json!(["E", "E", "E", "E"])
    );
    assert_eq!(
        seat1[26]["hands"],
        serde_json::json!([["1p"], ["2p"], ["3p"]])
    );
}

/// 暗槓と流局を足したサンプルを、公開範囲の設定を変えて redact する。
fn redact_with_visibility(ankan: &str, reveal: &str, viewer: Viewer) -> (Value, Value) {
    let mut ev = parse_jsonl(FULL).unwrap();
    ev[0]["rules"]["log_visibility"]["ankan"] = Value::from(ankan);
    ev[0]["rules"]["log_visibility"]["ryukyoku_reveal"] = Value::from(reveal);
    ev.push(serde_json::json!({
        "game_id": "g-20261122-000001", "kyoku_idx": 0, "event_idx": 98, "type": "ankan",
        "actor": 1, "consumed": ["E", "E", "E", "E"]}));
    ev.push(serde_json::json!({
        "game_id": "g-20261122-000001", "kyoku_idx": 0, "event_idx": 99, "type": "ryukyoku",
        "reason": "exhaustive", "tenpai": [true, false, true],
        "hands": [["1p"], ["2p"], ["3p"]], "deltas": [1000, -2000, 1000], "scores": [0, 0, 0]}));
    let n = ev.len();
    let red = redact(&ev, viewer).unwrap();
    (red[n - 2]["consumed"].clone(), red[n - 1]["hands"].clone())
}

#[test]
fn log_visibility_ankan_open_and_closed() {
    use serde_json::json;
    // open：誰の版でも暗槓の牌が見える。closed：本人以外は "?"
    for v in [Viewer::Public, Viewer::Seat(0), Viewer::Seat(1)] {
        assert_eq!(
            redact_with_visibility("open", "all", v).0,
            json!(["E", "E", "E", "E"])
        );
    }
    assert_eq!(
        redact_with_visibility("closed", "all", Viewer::Public).0,
        json!(["?", "?", "?", "?"])
    );
    assert_eq!(
        redact_with_visibility("closed", "all", Viewer::Seat(0)).0,
        json!(["?", "?", "?", "?"])
    );
    assert_eq!(
        redact_with_visibility("closed", "all", Viewer::Seat(1)).0,
        json!(["E", "E", "E", "E"])
    );
}

#[test]
fn log_visibility_ryukyoku_reveal_each_value() {
    use serde_json::json;
    let hands = |reveal, v| redact_with_visibility("open", reveal, v).1;
    // 聴牌は席0・席2
    assert_eq!(
        hands("tenpai_only", Viewer::Public),
        json!([["1p"], null, ["3p"]])
    );
    assert_eq!(
        hands("tenpai_only", Viewer::Seat(1)),
        json!([["1p"], ["2p"], ["3p"]])
    );
    assert_eq!(
        hands("all", Viewer::Public),
        json!([["1p"], ["2p"], ["3p"]])
    );
    assert_eq!(hands("none", Viewer::Public), json!([null, null, null]));
    assert_eq!(hands("none", Viewer::Seat(2)), json!([null, null, ["3p"]]));
}

#[test]
fn sample_rules_equal_main_preset() {
    let ev = parse_jsonl(FULL).unwrap();
    let in_log: Rules = serde_json::from_value(ev[0]["rules"].clone()).unwrap();
    assert_eq!(in_log, mj_core::rules::presets::tenhou_sanma());
}

#[test]
fn annotations_point_at_existing_events() {
    let ev = parse_jsonl(FULL).unwrap();
    let ann = parse_jsonl(ANNOT).unwrap();
    assert_eq!(validate_annotations(&ann, &ev).unwrap(), 4);
    let mut bad = ann.clone();
    bad[0]["event_idx"] = Value::from(500);
    assert!(validate_annotations(&bad, &ev).is_err());
    let mut dup = ann.clone();
    dup.push(ann[0].clone());
    assert!(validate_annotations(&dup, &ev).is_err());
    // 判断ポイントは選択肢が2つ以上で、decision はその中にある
    let mut one = ann.clone();
    one[0]["choices"] = serde_json::json!(["pon:C,C"]);
    assert!(validate_annotations(&one, &ev).is_err());
    let mut outside = ann.clone();
    outside[0]["decision"] = Value::from("daiminkan");
    assert!(validate_annotations(&outside, &ev).is_err());
    // selection は任意（サンプルには書いていない）。書くなら "sample" か "greedy"
    for (v, ok) in [("sample", true), ("greedy", true), ("argmax", false)] {
        let mut sel = ann.clone();
        sel[0]["selection"] = Value::from(v);
        assert_eq!(validate_annotations(&sel, &ev).is_ok(), ok, "{v}");
        let typed: mj_log::schema::Annotation = serde_json::from_value(sel[0].clone()).unwrap();
        assert_eq!(typed.selection.as_deref(), Some(v));
    }
    let typed: mj_log::schema::Annotation = serde_json::from_value(ann[0].clone()).unwrap();
    assert_eq!(typed.selection, None);
}

#[test]
fn tampered_wall_is_rejected() {
    let mut ev = parse_jsonl(FULL).unwrap();
    ev[1]["wall"]["pile"][10] = Value::from("C");
    assert!(validate_full(&ev).is_err());
    let mut ev = parse_jsonl(FULL).unwrap();
    ev[1]["wall_salt"] = Value::from("00");
    assert!(validate_full(&ev).unwrap_err().0.contains("wall_commit"));
}

#[test]
fn sample_replays_on_the_engine_and_annotations_are_decision_points() {
    // サンプルはエンジンで作った牌譜ではない（手で組んだ）ので、エンジンで再生して食い違いがないことを確かめる
    let ev = parse_jsonl(FULL).unwrap();
    let decisions = mj_log::replay::replay(&ev).unwrap_or_else(|e| panic!("{e}"));
    let ann = parse_jsonl(ANNOT).unwrap();
    assert_eq!(
        mj_log::replay::check_annotations(&ann, &decisions).unwrap_or_else(|e| panic!("{e}")),
        4
    );
}
