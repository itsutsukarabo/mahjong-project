//! 牌譜の型（`mj_log::schema`）と再生（`mj_log::view`）の検査。
//!
//! 1. サンプルとエンジンの対局の牌譜（全情報版・席視点版・公開情報版）を型で読んで書き戻すと、元と同じ JSON になる
//!    （型が牌譜のフィールドを落とさない・足さない）
//! 2. 再生の各時点で、どの席も「手牌 ＋ ツモった牌 ＋ 3 × 副露の数」が 13 か 14
//! 3. 公開情報版の再生は、手牌以外（河・副露・北抜き・点数・ドラ表示・残り枚数）が全情報版と同じ

use mj_core::action::Action;
use mj_core::game::{Game, WallSource};
use mj_core::policy::greedy_choice;
use mj_core::rules::{Rules, presets};
use mj_core::shanten::ShantenCalc;
use mj_log::schema::{LogEvent, parse_annotations, parse_events};
use mj_log::view::{Replay, TableView};
use mj_log::write::{GameMeta, game_log};
use mj_log::{Viewer, redact, to_jsonl};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn play(rules: &Rules, seed: u64) -> Vec<Value> {
    let calc = ShantenCalc::new(rules.tiles.manzu);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
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
        g.step(&acts).unwrap();
    }
    let n = rules.table.num_players;
    let meta = GameMeta {
        game_id: format!("schema-{seed}"),
        players: (0..n).map(|i| format!("p{i}")).collect(),
        engine_version: "0.1.0".into(),
        engine_git: "test".into(),
    };
    game_log(&g, &meta, &mut |k| format!("{k:032x}"))
}

fn logs() -> Vec<(String, Vec<Value>)> {
    let mut out = Vec::new();
    let sample =
        std::fs::read_to_string(root().join("docs/spec/samples/sample_full.jsonl")).unwrap();
    out.push(("sample".to_string(), mj_log::parse_jsonl(&sample).unwrap()));
    let cases = [
        (presets::tenhou_sanma(), 1u64),
        (presets::tenhou_sanma(), 2),
        (presets::mjsoul_sanma(), 3),
        (presets::test_yonma(), 4),
    ];
    for (r, s) in cases {
        out.push((format!("game-{s}"), play(&r, s)));
    }
    out
}

fn roundtrip(events: &[Value]) -> Vec<LogEvent> {
    let typed = parse_events(&to_jsonl(events)).unwrap();
    for (v, t) in events.iter().zip(&typed) {
        let back = serde_json::to_value(t).unwrap();
        assert_eq!(&back, v, "round trip of {}", v["type"]);
    }
    typed
}

fn check_counts(name: &str, states: &[TableView]) {
    for st in states {
        for s in &st.seats {
            let n = s.hand.len() + usize::from(s.drawn.is_some()) + 3 * s.melds.len();
            assert!(
                n == 13 || n == 14,
                "{name} kyoku {} event {} seat {}: {n} tiles",
                st.kyoku_idx,
                st.event_idx,
                s.seat
            );
        }
    }
}

#[test]
fn schema_roundtrip_and_replay_invariants() {
    for (name, full) in logs() {
        let n = full[0]["rules"]["table"]["num_players"].as_u64().unwrap() as u8;
        let mut versions = vec![
            ("full", full.clone()),
            ("public", redact(&full, Viewer::Public).unwrap()),
        ];
        versions.push(("seat:1", redact(&full, Viewer::Seat(1)).unwrap()));
        let mut by_version = Vec::new();
        for (v, events) in &versions {
            let typed = roundtrip(events);
            let r = Replay::from_events(typed).unwrap();
            let meta = r.meta();
            assert_eq!(meta.num_players, n);
            let mut all = Vec::new();
            for k in &meta.kyokus {
                let states = r.kyoku_states(k.kyoku_idx).unwrap();
                assert_eq!(states.len() as u32, k.events);
                check_counts(&format!("{name} {v}"), &states);
                all.extend(states);
            }
            by_version.push(all);
        }
        // 公開情報版は手牌以外が全情報版と同じ
        let strip = |v: &[TableView]| -> Vec<TableView> {
            v.iter()
                .cloned()
                .map(|mut t| {
                    for s in &mut t.seats {
                        s.hand.clear();
                        s.drawn = None;
                    }
                    // 流局の手牌の公開範囲は版で違う（LOG_FORMAT §4.4）
                    if let Some(r) = t.result.as_mut().and_then(|r| r.ryukyoku.as_mut()) {
                        r.hands.clear();
                    }
                    t
                })
                .collect()
        };
        let (a, b, c) = (
            strip(&by_version[0]),
            strip(&by_version[1]),
            strip(&by_version[2]),
        );
        assert_eq!(a.len(), b.len());
        for (i, ((x, y), z)) in a.iter().zip(&b).zip(&c).enumerate() {
            assert_eq!(x, y, "{name}: public differs at state {i}");
            assert_eq!(x, z, "{name}: seat view differs at state {i}");
        }
        // 全情報版の手牌に `?` はない。公開情報版の手牌は和了・流局の公開まで `?` だけ
        for st in &by_version[0] {
            assert!(st.seats.iter().all(|s| s.hand.iter().all(|t| t != "?")));
        }
    }
}

#[test]
fn sample_annotations_parse() {
    let text =
        std::fs::read_to_string(root().join("docs/spec/samples/sample_annot.jsonl")).unwrap();
    let a = parse_annotations(&text).unwrap();
    assert!(!a.is_empty());
    assert!(a.iter().any(|x| x.policy.is_some()));
}

#[test]
fn replay_screen_fixtures_open() {
    // 牌譜再生画面のテスト用の牌譜（mj-py の自己対戦・RuleBot 同士・サンプル）がすべて読めて、最後まで再生できる
    let dir = root().join("web/apps/replay/public/fixtures");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&p).unwrap();
        if name.ends_with(".annot.jsonl") {
            assert!(!parse_annotations(&text).unwrap().is_empty(), "{name}");
            continue;
        }
        let r = Replay::from_jsonl(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        for k in r.meta().kyokus {
            check_counts(&name, &r.kyoku_states(k.kyoku_idx).unwrap());
        }
        n += 1;
    }
    assert!(n >= 5, "{n} logs");
}
