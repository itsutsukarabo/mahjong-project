//! ACTION_SPACE §5 の検査。
//! 1. 番号 → 表記 → 番号が恒等、表記 → Action → 表記が恒等（325 通り）。初版の対応を fixtures に固定（D-038）
//! 2. 自己対戦のすべての判断ポイントで、選択肢を番号にすると重複がない（並びの一致は求めない。D-038）
//! 3. 合法手マスクの 1 の数が選択肢の数と一致する

use mj_bots::RuleBot;
use mj_core::action::Action;
use mj_core::game::{Game, WallSource};
use mj_core::policy::uniform_choice;
use mj_core::rules::{Rules, presets};
use mj_features::{ActionId, NUM_ACTIONS, action_to_id, id_to_action, legal_mask};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use std::collections::{BTreeMap, HashSet};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/features/action_ids.txt"
);

#[test]
fn ids_and_strings_roundtrip_and_are_frozen() {
    let mut lines = Vec::new();
    for i in 0..NUM_ACTIONS as u16 {
        let a = id_to_action(ActionId(i));
        assert_eq!(action_to_id(&a), Some(ActionId(i)));
        let s = a.to_string();
        let b: Action = s.parse().unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(b, a, "{s}");
        assert_eq!(b.to_string(), s);
        lines.push(format!("{i}\t{s}"));
    }
    let text = lines.join("\n") + "\n";
    if std::env::var("MJ_UPDATE_ACTION_IDS").as_deref() == Ok("1") {
        std::fs::create_dir_all(std::path::Path::new(FIXTURE).parent().unwrap()).unwrap();
        std::fs::write(FIXTURE, &text).unwrap();
    }
    let want = std::fs::read_to_string(FIXTURE).expect("fixtures/features/action_ids.txt");
    // 背番号（D-038）：初版の 325 個の対応は変えない。足した行動は末尾に足す
    assert!(
        text.starts_with(&want.replace("\r\n", "\n")),
        "action ids changed (existing ids must not move; append new actions at the end)"
    );
}

/// 席ごとに、ボットか一様な乱数の打ち手で回し、すべての判断ポイントの選択肢を検査する
fn check_games(rules: &Rules, games: u64, random: bool) -> (u64, BTreeMap<String, u64>) {
    let bot = RuleBot::new(rules);
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let mut points = 0;
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    for seed in 0..games {
        let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
        while !g.is_over() {
            let k = g.current_kyoku();
            let mut acts = Vec::new();
            for d in g.decisions() {
                points += 1;
                let ids: Vec<ActionId> = d
                    .choices
                    .iter()
                    .map(|a| action_to_id(a).unwrap_or_else(|| panic!("{a} has no id")))
                    .collect();
                let set: HashSet<ActionId> = ids.iter().copied().collect();
                assert_eq!(set.len(), ids.len(), "duplicate ids in {:?}", d.choices);
                let m = legal_mask(&d.choices);
                assert_eq!(m.iter().filter(|&&x| x).count(), d.choices.len());
                for a in &d.choices {
                    let s = a.to_string();
                    let kind = s.split(':').next().unwrap().to_string();
                    *kinds.entry(kind).or_default() += 1;
                }
                let a = if random {
                    uniform_choice(&mut rng, &d.choices)
                } else {
                    bot.choose(k, d)
                };
                acts.push((d.seat, a));
            }
            g.step(&acts).expect("legal");
        }
    }
    (points, kinds)
}

#[test]
fn every_decision_point_maps_to_distinct_ids() {
    let single = |r: Rules| presets::with(&r, "[table]\nlength = 'single'");
    let mut total = 0;
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    for (rules, random) in [
        (single(presets::tenhou_sanma()), false),
        (single(presets::tenhou_sanma()), true),
        (single(presets::mjsoul_sanma()), false),
        (single(presets::test_yonma()), false),
        (single(presets::test_yonma()), true),
        // ノーテン宣言ありで declare / pass:noten も出す
        (
            presets::with(
                &single(presets::test_yonma()),
                "[draws]\nnoten_declaration = true",
            ),
            true,
        ),
    ] {
        let (n, kinds) = check_games(&rules, 20, random);
        total += n;
        for (k, v) in kinds {
            *seen.entry(k).or_default() += v;
        }
    }
    eprintln!("decision points checked: {total}; choices by kind: {seen:?}");
    assert!(total > 1000, "decision points: {total}");
    // 行動の種類が一通り出ていること（番号表のどの区間も試されている）
    for k in [
        "tsumo", "ron", "ankan", "kita", "pon", "chi", "reach", "dahai", "pass",
    ] {
        assert!(seen.contains_key(k), "{k} never appeared: {seen:?}");
    }
}
