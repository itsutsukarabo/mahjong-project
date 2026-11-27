//! mj-core と RiichiEnv 0.4.11 の三麻の差分テスト。
//!
//! ```text
//! difftest-riichienv [--cases 20000] [--hora-cases N] [--game-cases N] [--seed S | --random-seed]
//!     [--python PATH] [--harness PATH] [--out DIR] [--inject-fault]
//! ```
//! - 層：手牌（シャンテン数・待ち）、和了・点数、局進行（北抜きを含む、`game.rs`）
//! - RiichiEnv は隔離した venv の Python で動かす（D-010。`--python` はその venv の python）
//! - 結果は層ごとに「比較した件数／除外した件数／不一致件数」。不一致は最小化して `--out` に書く
//! - ルールは `fixtures/rules/riichienv-sanma.toml`（`tenhou-sanma.toml` を RiichiEnv の三麻の固定値に合わせた上書き）
//! - シャンテン数の定義の違い（D-018）は、下の [`convert_shanten`] の変換表で吸収する

mod game;
mod hand;
mod harness;
mod hora;

use mj_core::rules::{Rules, presets};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

pub static INJECT_FAULT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub const OVERRIDES: &str = include_str!("../../../fixtures/rules/riichienv-sanma.toml");

pub fn rules() -> Rules {
    presets::with(&presets::tenhou_sanma(), OVERRIDES)
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
struct Counts {
    compared: u64,
    excluded: u64,
    mismatched: u64,
}

/// シャンテン数の定義の違い（D-018）の変換表：mj-core の値と RiichiEnv の値が、この表の形のときは一致とみなす。
/// （表の行と件数は summary.json の `shanten_converted` に、例は `conversions.jsonl` に出す）
///
/// mj-core（majiang-core と同じ）は残り枚数を考えない。RiichiEnv は手の中にある4枚目を待ち・面子の材料に数えない。
/// 変換：RiichiEnv の値が「1枚ずつ交換して、生きた待ち（手の中に4枚ない種類）のある聴牌に届く最小の回数」
/// （[`count_aware_shanten`]）と一致すれば、定義の違いとみなす。
fn convert_shanten(ours: i8, theirs: i64, c: &mj_core::tile::KindCounts) -> Option<&'static str> {
    let n: u32 = c.iter().map(|&x| u32::from(x)).sum();
    if theirs <= i64::from(ours) || !c.contains(&4) {
        return None;
    }
    let t = theirs as i8;
    match n % 3 {
        1 => {
            if count_aware_shanten(c, t) != Some(t) {
                return None;
            }
            Some(if ours == 0 {
                "count_aware_13: ours 0 / riichienv 1（4枚使いの単騎待ちだけの聴牌）"
            } else {
                "count_aware_13: ours n / riichienv n+1 以上（手の中の4枚目を数えない）"
            })
        }
        2 => {
            // 14枚形：1枚切った13枚形の最小
            let best = (0..mj_core::tile::NUM_KINDS)
                .filter(|&d| c[d] > 0)
                .filter_map(|d| {
                    let mut x = *c;
                    x[d] -= 1;
                    count_aware_shanten(&x, t)
                })
                .min();
            (best == Some(t))
                .then_some("count_aware_14: 14枚形（1枚切った13枚形の最小）で同じ定義の違い")
        }
        _ => None,
    }
}

/// 13枚形で、1回に「1枚切って1枚引く」を何回すれば、生きた待ち（`agari::waits` が空でない）の聴牌になるか。
/// `limit` 回まで探す（引く牌は三麻で使う種類のうち、手の中に4枚ないもの）
fn count_aware_shanten(c: &mj_core::tile::KindCounts, limit: i8) -> Option<i8> {
    use mj_core::agari::waits;
    let kinds = hand::sanma_kinds();
    fn go(c: &mut mj_core::tile::KindCounts, depth: i8, kinds: &[usize]) -> bool {
        if mj_core::agari::waits(c, true) != 0 {
            return true;
        }
        if depth == 0 {
            return false;
        }
        for d in 0..mj_core::tile::NUM_KINDS {
            if c[d] == 0 {
                continue;
            }
            c[d] -= 1;
            for &t in kinds {
                if t == d || c[t] >= 4 {
                    continue;
                }
                c[t] += 1;
                let ok = go(c, depth - 1, kinds);
                c[t] -= 1;
                if ok {
                    c[d] += 1;
                    return true;
                }
            }
            c[d] += 1;
        }
        false
    }
    let mut x = *c;
    if waits(&x, true) != 0 {
        return Some(0);
    }
    (1..=limit.min(3)).find(|&k| go(&mut x, k, &kinds))
}

fn main() {
    let mut cases: usize = 20_000;
    let mut hora_n: Option<usize> = None;
    let mut game_n: Option<usize> = None;
    // 1局だけ回す（失敗した局の再現用。summary の game_seed を渡す）
    let mut only_game: Option<u64> = None;
    // 回帰用の局（fixtures/difftest-riichienv/regression_games.txt）を足す
    let mut seeds_file: Option<PathBuf> = None;
    let mut seed: u64 = 20_261_128;
    let mut random_seed = false;
    let mut out_dir: Option<PathBuf> = None;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut harness_path = here.join("harness").join("harness.py");
    let mut python = "python".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--cases" => cases = val().parse().unwrap(),
            "--hora-cases" => hora_n = Some(val().parse().unwrap()),
            "--game-cases" => game_n = Some(val().parse().unwrap()),
            "--game-seed" => only_game = Some(val().parse().unwrap()),
            "--game-seeds-file" => seeds_file = Some(PathBuf::from(val())),
            "--seed" => seed = val().parse().unwrap(),
            "--random-seed" => random_seed = true,
            "--out" => out_dir = Some(PathBuf::from(val())),
            "--python" => python = val(),
            "--harness" => harness_path = PathBuf::from(val()),
            "--inject-fault" => INJECT_FAULT.store(true, std::sync::atomic::Ordering::Relaxed),
            _ => panic!("unknown argument {a}"),
        }
    }
    if random_seed {
        seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
    }
    let hora_n = hora_n.unwrap_or(cases);
    let game_n = game_n.unwrap_or(cases / 100);
    eprintln!("difftest-riichienv: seed={seed} cases={cases} hora={hora_n} games={game_n}");
    let rules = rules();
    // 和了・点数の層だけ：RiichiEnv の HandEvaluator3P.calc は GameRule を受け取らず、四暗刻単騎・国士無双十三面を
    // 常に2倍役満にする（2026-11-28 の実測。局進行の層は GameRule.default_tenhou でシングル）
    let hora_rules = presets::with(
        &rules,
        "[yakuman]
double_special = true",
    );
    // 天和の和了牌：RiichiEnv は国士無双だけ十三面待ちとして数え（どの牌でも）、ほかの役満はツモ牌で数える。
    // mj-core は best（I-4）。ツモ牌（drawn）で数えた値と一致すれば、この違いとみなす
    let hora_rules_drawn = presets::with(
        &hora_rules,
        "[yakuman]
tenhou_win_tile = 'drawn'",
    );
    let mut h = harness::Harness::spawn(&python, &harness_path);

    // ---- 手牌：シャンテン数・待ち
    let calc = hand::calc();
    let mut g = hand::Gen {
        rng: ChaCha8Rng::seed_from_u64(seed),
    };
    let mut shanten = Counts::default();
    let mut waits = Counts::default();
    let mut converted: BTreeMap<&str, u64> = BTreeMap::new();
    // 変換表に当たった手の例（規則ごとに5件まで。fixtures/difftest-riichienv/conversions.jsonl の元）
    let mut converted_examples: Vec<Value> = Vec::new();
    let mut by_strategy: BTreeMap<&str, u64> = BTreeMap::new();
    let mut hand_failures: Vec<Value> = Vec::new();
    let chunk = 500;
    let mut i = 0;
    while i < cases {
        let n = chunk.min(cases - i);
        let batch: Vec<_> = (0..n)
            .map(|_| {
                let (c, s) = g.hand();
                let red = g.reds(&c);
                (c, red, s)
            })
            .collect();
        let reqs: Vec<Value> = batch.iter().map(|(c, r, _)| hand::request(c, r)).collect();
        let answers = h.ask_many(&reqs);
        for ((c, red, strategy), a) in batch.iter().zip(&answers) {
            if let Some(e) = a.get("error") {
                panic!("harness error: {e}");
            }
            *by_strategy.entry(strategy).or_default() += 1;
            let (ours_s, ours_w) = hand::ours(&calc, c);
            let theirs_s = a["shanten"].as_i64().expect("shanten");
            shanten.compared += 1;
            let mut bad = false;
            if i64::from(ours_s) != theirs_s {
                match convert_shanten(ours_s, theirs_s, c) {
                    Some(rule) => {
                        let e = converted.entry(rule).or_default();
                        *e += 1;
                        if *e <= 5 {
                            converted_examples.push(json!({"rule": rule, "tiles": hand::tiles_of(c, red),
                                "ours": {"shanten": ours_s, "waits": ours_w}, "riichienv": {"shanten": theirs_s, "waits": a["waits"]}}));
                        }
                    }
                    None => {
                        shanten.mismatched += 1;
                        bad = true;
                    }
                }
            }
            match (&ours_w, a["waits"].as_array()) {
                (Some(w), Some(t)) => {
                    waits.compared += 1;
                    let mut t: Vec<String> =
                        t.iter().map(|x| x.as_str().unwrap().to_string()).collect();
                    t.sort();
                    let mut w = w.clone();
                    w.sort();
                    if w != t {
                        waits.mismatched += 1;
                        bad = true;
                    }
                }
                _ => waits.excluded += 1,
            }
            if bad && hand_failures.len() < 200 {
                hand_failures.push(json!({"seed": seed, "strategy": strategy,
                    "tiles": hand::tiles_of(c, red), "ours": {"shanten": ours_s, "waits": ours_w},
                    "riichienv": {"shanten": theirs_s, "waits": a["waits"]}}));
            }
        }
        i += n;
    }

    // ---- 和了・点数
    let mut hg = hand::Gen {
        rng: ChaCha8Rng::seed_from_u64(seed ^ 0x686f7261),
    };
    let mut hora_counts = Counts::default();
    let mut hora_forms: BTreeMap<&str, u64> = BTreeMap::new();
    let mut hora_yaku: BTreeMap<String, u64> = BTreeMap::new();
    let mut hora_failures: Vec<Value> = Vec::new();
    let mut hora_converted: BTreeMap<&str, u64> = BTreeMap::new();
    let mut budget = 3000usize;
    let mut i = 0;
    while i < hora_n {
        let n = chunk.min(hora_n - i);
        let batch: Vec<hora::HoraCase> = (0..n).map(|_| hg.hora_case()).collect();
        let reqs: Vec<Value> = batch.iter().map(|c| c.request()).collect();
        let answers = h.ask_many(&reqs);
        for (c, a) in batch.iter().zip(&answers) {
            *hora_forms.entry(c.form).or_default() += 1;
            let theirs = hora::theirs(c, a).unwrap_or_else(|e| panic!("{e}"));
            let mut ours = c.ours(&hora_rules);
            if INJECT_FAULT.load(std::sync::atomic::Ordering::Relaxed)
                && let Some(o) = ours.as_mut()
                && o.yaku.iter().any(|y| y == "Honitsu")
            {
                o.fu += 10;
            }
            if theirs.is_none() && ours.is_none() {
                hora_counts.excluded += 1;
                continue;
            }
            hora_counts.compared += 1;
            for y in theirs
                .iter()
                .chain(ours.iter())
                .take(1)
                .flat_map(|s| s.yaku.iter())
            {
                *hora_yaku.entry(y.clone()).or_default() += 1;
            }
            if let (Some(t), Some(o)) = (&theirs, &ours)
                && t != o
                && hora::same_points_other_reading(t, o)
            {
                *hora_converted
                    .entry(
                        "same_points_other_reading: 役満と数え役満で支払いが同じ（取り方の違い）",
                    )
                    .or_default() += 1;
                continue;
            }
            if theirs != ours && c.first && c.seat_wind == 0 && theirs == c.ours(&hora_rules_drawn)
            {
                *hora_converted
                    .entry("tenhou_win_tile: RiichiEnv は天和の四暗刻などをツモ牌で数える（国士無双だけ十三面）。mj-core は best")
                    .or_default() += 1;
                continue;
            }
            if theirs != ours {
                hora_counts.mismatched += 1;
                // 最小化：状況役・ドラなどを外しても食い違うものへ縮める
                let mut cur = c.clone();
                let mut progress = true;
                while progress && budget > 0 {
                    progress = false;
                    for cand in hora::simpler(&cur) {
                        if budget == 0 {
                            break;
                        }
                        budget -= 1;
                        let a = h.ask(cand.request());
                        if hora::theirs(&cand, &a).ok().flatten() != cand.ours(&hora_rules) {
                            cur = cand;
                            progress = true;
                            break;
                        }
                    }
                }
                if hora_failures.len() < 200 {
                    let a = h.ask(cur.request());
                    hora_failures.push(json!({
                        "seed": seed,
                        "original": {"case": c, "request": c.request(), "ours": ours, "riichienv": theirs},
                        "minimized": {"case": cur, "request": cur.request(), "ours": cur.ours(&hora_rules),
                                      "riichienv": hora::theirs(&cur, &a).ok().flatten()},
                    }));
                }
            }
        }
        i += n;
    }

    // ---- 局進行
    let mut games = Counts::default();
    let mut game_decisions = 0u64;
    let mut game_ends: BTreeMap<String, u64> = BTreeMap::new();
    let mut game_excluded: BTreeMap<String, u64> = BTreeMap::new();
    let mut game_items: BTreeMap<String, u64> = BTreeMap::new();
    let mut game_failures: Vec<Value> = Vec::new();
    let mut game_seeds: Vec<u64> = match only_game {
        Some(g) => vec![g],
        None => (0..game_n)
            .map(|gi| (seed ^ 0x67616d65).wrapping_add(gi as u64) % (1 << 62))
            .collect(),
    };
    let mut regression = 0u64;
    if let Some(f) = &seeds_file {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for line in text.lines() {
            let v = line.split('#').next().unwrap_or("").trim();
            if !v.is_empty() {
                game_seeds.push(v.parse().unwrap_or_else(|e| panic!("{v}: {e}")));
                regression += 1;
            }
        }
    }
    for (gi, &gseed) in game_seeds.iter().enumerate() {
        let o = game::run(&mut h, &rules, gseed, &format!("g{gi}"));
        game_decisions += o.decisions;
        if let Some(r) = &o.excluded {
            games.excluded += 1;
            *game_excluded
                .entry(r.split(' ').next().unwrap_or("").to_string())
                .or_default() += 1;
            continue;
        }
        games.compared += 1;
        if let Some(e) = &o.ended_by {
            *game_ends.entry(e.clone()).or_default() += 1;
        }
        if let Some(m) = &o.mismatch {
            games.mismatched += 1;
            *game_items
                .entry(m["item"].as_str().unwrap_or("?").to_string())
                .or_default() += 1;
            if game_failures.len() < 200 {
                game_failures.push(json!({"seed": seed, "game_seed": gseed, "mismatch": m}));
            }
        }
    }

    let summary = json!({
        "seed": seed,
        "riichienv": "0.4.11",
        "rules": "fixtures/rules/riichienv-sanma.toml（tenhou-sanma への上書き）",
        "shanten": shanten,
        "shanten_converted": converted,
        "waits": waits,
        "waits_excluded_reason": "14枚形（3n+2）は待ちを比べない",
        "hand_by_strategy": by_strategy,
        "hora": hora_counts,
        "hora_excluded_reason": "どちらも和了にならないケース（役なし）",
        "hora_converted": hora_converted,
        "hora_by_form": hora_forms,
        "hora_yaku": hora_yaku,
        "games": games,
        "game_decisions_compared": game_decisions,
        "game_regression_seeds": regression,
        "game_excluded": game_excluded,
        "game_ends": game_ends,
        "game_mismatch_items": game_items,
        "hand_failures": hand_failures.len(),
        "hora_failures": hora_failures.len(),
        "game_failures": game_failures.len(),
    });
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("summary.json"),
            serde_json::to_string_pretty(&summary).unwrap(),
        )
        .unwrap();
        for (name, rows) in [
            ("hand_failures.jsonl", &hand_failures),
            ("hora_failures.jsonl", &hora_failures),
            ("game_failures.jsonl", &game_failures),
            ("conversions.jsonl", &converted_examples),
        ] {
            let mut f = std::fs::File::create(dir.join(name)).unwrap();
            for r in rows {
                writeln!(f, "{r}").unwrap();
            }
        }
    }
    for r in hand_failures.iter().take(5) {
        eprintln!("HAND MISMATCH {r}");
    }
    for r in hora_failures.iter().take(5) {
        eprintln!("HORA MISMATCH {}", r["minimized"]);
    }
    for r in game_failures.iter().take(5) {
        eprintln!("GAME MISMATCH {r}");
    }
    if shanten.mismatched + waits.mismatched + hora_counts.mismatched + games.mismatched > 0 {
        std::process::exit(1);
    }
}
