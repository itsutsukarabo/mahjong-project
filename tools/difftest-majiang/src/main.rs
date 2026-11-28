//! mj-core と @kobalab/majiang-core の差分テスト（手牌レベル、四麻）。
//!
//! 層：シャンテン数・待ち（`Util.xiangting` / `Util.tingpai`）と、和了・点数（`Util.hule`、[`hora`]）。
//!
//! 1. 固定シード（または乱数シード）から手牌のケースを生成する
//! 2. Node のハーネス（`node/harness.js`）に流し、majiang-core の答えを受け取る
//! 3. `case_id` で突き合わせ、比較した件数／除外した件数／不一致件数を出す
//! 4. 不一致は最小化して `failures.jsonl` に書く
//!
//! 使い方：
//! `difftest-majiang --cases 20000 [--hora-cases N] --seed 20261122 [--random-seed] [--out DIR] [--node node] [--harness PATH]`
//! （`--hora-cases` の既定は `--cases` と同じ）
//! 不一致が1件でもあれば終了コード 1。

mod game;
mod hora;

use mj_core::rules::Manzu;
use mj_core::shanten::ShantenCalc;
use mj_core::tile::{KindCounts, NUM_KINDS, TileKind};
use mj_core::wall::uniform_below;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// 1ケース：門前の手牌（種類ごとの枚数）と副露（ポンとして表す。シャンテン数には数だけが効く）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Case {
    case_id: String,
    strategy: &'static str,
    #[serde(skip)]
    hand: KindCounts,
    #[serde(skip)]
    pons: Vec<TileKind>,
    majiang: String,
}

impl Case {
    fn new(case_id: String, strategy: &'static str, hand: KindCounts, pons: Vec<TileKind>) -> Self {
        let majiang = majiang_string(&hand, &pons);
        Self {
            case_id,
            strategy,
            hand,
            pons,
            majiang,
        }
    }

    fn concealed(&self) -> usize {
        self.hand.iter().map(|&n| usize::from(n)).sum()
    }

    /// 13枚形（3n+1）なら待ちも比較する。
    fn is_waiting_form(&self) -> bool {
        self.concealed() % 3 == 1
    }
}

/// majiang-core の手牌表記（例 `m123p55z11,z222=`）。
fn majiang_string(hand: &KindCounts, pons: &[TileKind]) -> String {
    let mut s = String::new();
    for (letter, range) in [('m', 0..9), ('p', 9..18), ('s', 18..27), ('z', 27..34)] {
        let mut part = String::new();
        for k in range.clone() {
            for _ in 0..hand[k] {
                part.push(char::from(b'1' + (k - range.start) as u8));
            }
        }
        if !part.is_empty() {
            s.push(letter);
            s.push_str(&part);
        }
    }
    for p in pons {
        let (letter, n) = match p.index() {
            k @ 0..=8 => ('m', k + 1),
            k @ 9..=17 => ('p', k - 8),
            k @ 18..=26 => ('s', k - 17),
            k => ('z', k - 26),
        };
        s.push_str(&format!(",{letter}{n}{n}{n}="));
    }
    s
}

#[derive(Debug, Deserialize)]
struct Answer {
    case_id: String,
    shanten: Option<i32>,
    tingpai: Option<Vec<String>>,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Verdict {
    shanten_mismatch: bool,
    /// None = 比較対象外（14枚形）
    waits_mismatch: Option<bool>,
    ours_shanten: i8,
    ours_waits: Option<Vec<String>>,
}

/// 最小化の仕組みを確かめるための故障注入（`--inject-fault`）：1m を2枚以上持つ手のシャンテン数を1ずらす。
/// 和了・点数の層では混一色の符を 10 ずらす（[`hora`]）。
static INJECT_FAULT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn judge(calc: &ShantenCalc, c: &Case, a: &Answer) -> Result<Verdict, String> {
    if let Some(e) = &a.error {
        return Err(format!("harness error for {}: {e}", c.case_id));
    }
    let melds = c.pons.len() as u8;
    let mut ours = calc.shanten(&c.hand, melds);
    if INJECT_FAULT.load(std::sync::atomic::Ordering::Relaxed) && c.hand[0] >= 2 {
        ours += 1;
    }
    let theirs = a.shanten.ok_or("missing shanten")?;
    let (ours_waits, waits_mismatch) = if c.is_waiting_form() {
        let mut w: Vec<String> = calc
            .improving_kinds(&c.hand, melds)
            .iter()
            .map(|k| k.to_string())
            .collect();
        let mut t = a
            .tingpai
            .clone()
            .ok_or("missing tingpai for 13-tile form")?;
        w.sort();
        t.sort();
        let m = w != t;
        (Some(w), Some(m))
    } else {
        if a.tingpai.is_some() {
            return Err(format!("{}: tingpai for 14-tile form", c.case_id));
        }
        (None, None)
    };
    Ok(Verdict {
        shanten_mismatch: i32::from(ours) != theirs,
        waits_mismatch,
        ours_shanten: ours,
        ours_waits,
    })
}

struct Harness {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Harness {
    fn spawn(node: &str, script: &PathBuf) -> Self {
        let mut child = Command::new(node)
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to start {node} {}: {e}", script.display()));
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    /// 1回に送る件数は少なめにする（パイプのバッファが詰まらないように）。
    fn ask(&mut self, cases: &[Case]) -> Vec<Answer> {
        let reqs: Vec<serde_json::Value> = cases
            .iter()
            .map(|c| serde_json::json!({"case_id": c.case_id, "majiang": c.majiang}))
            .collect();
        self.request(&reqs)
    }

    fn ask_hora(&mut self, cases: &[hora::HoraCase]) -> Vec<hora::HoraAnswer> {
        let reqs: Vec<serde_json::Value> = cases
            .iter()
            .map(|c| {
                serde_json::json!({"case_id": c.case_id, "op": "hule", "shoupai": c.shoupai,
                                   "rongpai": c.rongpai, "param": c.param})
            })
            .collect();
        self.request(&reqs)
    }

    fn ask_game(
        &mut self,
        case_id: &str,
        script: &serde_json::Value,
        rule: &serde_json::Value,
    ) -> serde_json::Value {
        let req =
            serde_json::json!({"case_id": case_id, "op": "game", "script": script, "rule": rule});
        let mut out: Vec<serde_json::Value> = self.request(std::slice::from_ref(&req));
        let a = out.pop().expect("one answer");
        assert_eq!(a["case_id"], case_id, "case_id of the game answer");
        if let Some(e) = a.get("error") {
            panic!("harness error for {case_id}: {e}");
        }
        a["game"].clone()
    }

    fn request<T: serde::de::DeserializeOwned>(&mut self, reqs: &[serde_json::Value]) -> Vec<T> {
        for r in reqs {
            writeln!(self.stdin, "{r}").unwrap();
        }
        self.stdin.flush().unwrap();
        let mut out = Vec::with_capacity(reqs.len());
        for _ in reqs {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            assert!(n > 0, "harness closed its output");
            out.push(serde_json::from_str(&line).expect("harness output is JSON"));
        }
        out
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

// ---------------------------------------------------------------- 生成

struct Gen {
    rng: ChaCha8Rng,
    left: KindCounts,
}

impl Gen {
    fn below(&mut self, n: usize) -> usize {
        uniform_below(&mut self.rng, n as u32) as usize
    }

    fn reset(&mut self) {
        self.left = [4; NUM_KINDS];
    }

    /// 残り枚数に比例して1枚引く。
    fn draw(&mut self) -> usize {
        let total: usize = self.left.iter().map(|&n| usize::from(n)).sum();
        let mut r = self.below(total);
        for (k, left) in self.left.iter_mut().enumerate() {
            let n = usize::from(*left);
            if r < n {
                *left -= 1;
                return k;
            }
            r -= n;
        }
        unreachable!()
    }

    fn take(&mut self, k: usize, n: u8) -> bool {
        if self.left[k] >= n {
            self.left[k] -= n;
            true
        } else {
            false
        }
    }

    fn random_pons(&mut self, n: usize) -> Vec<TileKind> {
        let mut pons = Vec::new();
        while pons.len() < n {
            let k = self.below(NUM_KINDS);
            if self.take(k, 3) {
                pons.push(TileKind::new(k as u8).unwrap());
            }
        }
        pons
    }

    /// 和了形を組み、いくつかの牌を入れ替える（低シャンテンの手を多く作るため）。
    fn near_complete(&mut self, melds: usize) -> KindCounts {
        let mut h = [0u8; NUM_KINDS];
        loop {
            let k = self.below(NUM_KINDS);
            if self.take(k, 2) {
                h[k] += 2;
                break;
            }
        }
        let mut sets = 0;
        while sets < 4 - melds {
            let k = self.below(NUM_KINDS);
            let shuntsu = k < 27 && k % 9 <= 6 && self.below(3) > 0;
            if shuntsu {
                if self.left[k] > 0 && self.left[k + 1] > 0 && self.left[k + 2] > 0 {
                    for j in k..k + 3 {
                        self.left[j] -= 1;
                        h[j] += 1;
                    }
                    sets += 1;
                }
            } else if self.take(k, 3) {
                h[k] += 3;
                sets += 1;
            }
        }
        for _ in 0..self.below(3) {
            self.swap_one(&mut h);
        }
        h
    }

    fn swap_one(&mut self, h: &mut KindCounts) {
        let held: Vec<usize> = (0..NUM_KINDS).filter(|&k| h[k] > 0).collect();
        let out = held[self.below(held.len())];
        h[out] -= 1;
        self.left[out] += 1;
        let k = self.draw();
        h[k] += 1;
    }

    fn drop_one(&mut self, h: &mut KindCounts) {
        let held: Vec<usize> = (0..NUM_KINDS).filter(|&k| h[k] > 0).collect();
        let out = held[self.below(held.len())];
        h[out] -= 1;
        self.left[out] += 1;
    }

    fn case(&mut self, i: usize, seed: u64) -> Case {
        self.reset();
        let id = format!("s{seed}-{i:07}");
        let fourteen = self.below(2) == 0;
        match i % 4 {
            0 => {
                let melds = self.below(5);
                let pons = self.random_pons(melds);
                let n = 13 - 3 * melds + usize::from(fourteen);
                let mut h = [0u8; NUM_KINDS];
                for _ in 0..n {
                    h[self.draw()] += 1;
                }
                Case::new(id, "uniform", h, pons)
            }
            1 => {
                let melds = self.below(5);
                let pons = self.random_pons(melds);
                let mut h = self.near_complete(melds);
                if !fourteen {
                    self.drop_one(&mut h);
                }
                Case::new(id, "near_complete", h, pons)
            }
            2 => {
                let mut h = [0u8; NUM_KINDS];
                let mut pairs = 0;
                while pairs < 7 {
                    let k = self.below(NUM_KINDS);
                    if self.take(k, 2) {
                        h[k] += 2;
                        pairs += 1;
                    }
                }
                for _ in 0..self.below(4) {
                    self.swap_one(&mut h);
                }
                if !fourteen {
                    self.drop_one(&mut h);
                }
                Case::new(id, "chiitoi_like", h, vec![])
            }
            _ => {
                let mut h = [0u8; NUM_KINDS];
                for k in TileKind::all().filter(|k| k.is_terminal_or_honor()) {
                    self.take(k.index(), 1);
                    h[k.index()] += 1;
                }
                let k = self.draw();
                h[k] += 1;
                for _ in 0..self.below(4) {
                    self.swap_one(&mut h);
                }
                if !fourteen {
                    self.drop_one(&mut h);
                }
                Case::new(id, "kokushi_like", h, vec![])
            }
        }
    }
}

// ---------------------------------------------------------------- 最小化

fn mismatched(v: &Verdict) -> bool {
    v.shanten_mismatch || v.waits_mismatch == Some(true)
}

/// 不一致を保ったまま手を単純にする：
/// (a) 門前の刻子を副露に移す（門前の枚数を減らす）、(b) 1枚を小さい種類に置き換える。
fn shrink(calc: &ShantenCalc, h: &mut Harness, c: &Case, budget: &mut usize) -> Case {
    let mut cur = c.clone();
    let still = |h: &mut Harness, cand: &Case, budget: &mut usize| -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let a = h.ask(std::slice::from_ref(cand));
        judge(calc, cand, &a[0])
            .map(|v| mismatched(&v))
            .unwrap_or(false)
    };
    let valid = |hand: &KindCounts, pons: &[TileKind]| {
        (0..NUM_KINDS)
            .all(|k| hand[k] + 3 * pons.iter().filter(|p| p.index() == k).count() as u8 <= 4)
    };
    let mut progress = true;
    while progress {
        progress = false;
        for k in 0..NUM_KINDS {
            if cur.hand[k] >= 3 && cur.pons.len() < 4 {
                let mut hand = cur.hand;
                hand[k] -= 3;
                let mut pons = cur.pons.clone();
                pons.push(TileKind::new(k as u8).unwrap());
                let cand = Case::new(format!("{}-m", c.case_id), c.strategy, hand, pons);
                if still(h, &cand, budget) {
                    cur = cand;
                    progress = true;
                }
            }
        }
        for from in 0..NUM_KINDS {
            if cur.hand[from] == 0 {
                continue;
            }
            for to in 0..from {
                let mut hand = cur.hand;
                hand[from] -= 1;
                hand[to] += 1;
                if !valid(&hand, &cur.pons) {
                    continue;
                }
                let cand = Case::new(
                    format!("{}-m", c.case_id),
                    c.strategy,
                    hand,
                    cur.pons.clone(),
                );
                if still(h, &cand, budget) {
                    cur = cand;
                    progress = true;
                    break;
                }
            }
        }
    }
    cur
}

// ---------------------------------------------------------------- main

#[derive(Debug, Default, Serialize)]
struct Counts {
    compared: u64,
    excluded: u64,
    mismatched: u64,
}

fn main() {
    let mut cases_n: usize = 20_000;
    let mut hora_n: Option<usize> = None;
    let mut game_n: Option<usize> = None;
    let mut seed: u64 = 20_261_122;
    let mut random_seed = false;
    let mut out_dir: Option<PathBuf> = None;
    let mut node = "node".to_string();
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut harness_path = here.join("node").join("harness.js");
    let mut max_shrink_queries: usize = 5_000;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--cases" => cases_n = val().parse().unwrap(),
            "--hora-cases" => hora_n = Some(val().parse().unwrap()),
            "--game-cases" => game_n = Some(val().parse().unwrap()),
            "--seed" => seed = val().parse().unwrap(),
            "--random-seed" => random_seed = true,
            "--out" => out_dir = Some(PathBuf::from(val())),
            "--node" => node = val(),
            "--harness" => harness_path = PathBuf::from(val()),
            "--max-shrink-queries" => max_shrink_queries = val().parse().unwrap(),
            "--inject-fault" => INJECT_FAULT.store(true, std::sync::atomic::Ordering::Relaxed),
            _ => panic!("unknown argument {a}"),
        }
    }
    if random_seed {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        seed = t.as_nanos() as u64;
    }
    eprintln!("difftest-majiang: seed={seed} cases={cases_n}");

    let calc = ShantenCalc::new(Manzu::All);
    let mut gen_ = Gen {
        rng: ChaCha8Rng::seed_from_u64(seed),
        left: [4; NUM_KINDS],
    };
    let mut harness = Harness::spawn(&node, &harness_path);

    let mut shanten = Counts::default();
    let mut waits = Counts::default();
    let mut by_strategy: BTreeMap<&str, u64> = BTreeMap::new();
    let mut failures: Vec<(Case, Verdict)> = Vec::new();
    let chunk = 200;
    let mut i = 0;
    while i < cases_n {
        let n = chunk.min(cases_n - i);
        let batch: Vec<Case> = (i..i + n).map(|j| gen_.case(j, seed)).collect();
        let answers = harness.ask(&batch);
        // 行の並びに頼らず case_id で突き合わせる（CLAUDE.md）
        let by_id: BTreeMap<&str, &Answer> =
            answers.iter().map(|a| (a.case_id.as_str(), a)).collect();
        assert_eq!(
            by_id.len(),
            batch.len(),
            "duplicate or missing case_id in harness output"
        );
        for c in &batch {
            let a = by_id
                .get(c.case_id.as_str())
                .unwrap_or_else(|| panic!("no answer for {}", c.case_id));
            let v = judge(&calc, c, a).unwrap_or_else(|e| panic!("{e}"));
            *by_strategy.entry(c.strategy).or_default() += 1;
            shanten.compared += 1;
            if v.shanten_mismatch {
                shanten.mismatched += 1;
            }
            match v.waits_mismatch {
                None => waits.excluded += 1,
                Some(m) => {
                    waits.compared += 1;
                    if m {
                        waits.mismatched += 1;
                    }
                }
            }
            if mismatched(&v) {
                failures.push((c.clone(), v));
            }
        }
        i += n;
    }

    // ---- 和了・点数
    let hora_n = hora_n.unwrap_or(cases_n);
    let hora_rules = hora::rules();
    let mut hgen = hora::HoraGen::new(ChaCha8Rng::seed_from_u64(seed ^ 0x686f7261));
    let mut hora_counts = Counts::default();
    let mut hora_by_palette: BTreeMap<&str, u64> = BTreeMap::new();
    // 比べたケースに出た役（majiang-core の答えで数える。どの役が実際に突き合わされたかの記録）
    let mut hora_yaku: BTreeMap<String, u64> = BTreeMap::new();
    let mut hora_failures: Vec<serde_json::Value> = Vec::new();
    let mut hora_budget = max_shrink_queries;
    let mut i = 0;
    while i < hora_n {
        let n = chunk.min(hora_n - i);
        let batch: Vec<hora::HoraCase> = (i..i + n)
            .map(|j| hgen.case(j, seed, &hora_rules))
            .collect();
        let answers = harness.ask_hora(&batch);
        let by_id: BTreeMap<&str, &hora::HoraAnswer> =
            answers.iter().map(|a| (a.case_id.as_str(), a)).collect();
        assert_eq!(by_id.len(), batch.len(), "duplicate or missing case_id");
        for c in &batch {
            let a = by_id
                .get(c.case_id.as_str())
                .unwrap_or_else(|| panic!("no answer for {}", c.case_id));
            *hora_by_palette.entry(c.palette).or_default() += 1;
            let theirs = hora::theirs_score(a).unwrap_or_else(|e| panic!("{e}"));
            let ours = hora::ours_score(&hora_rules, c);
            if theirs.is_none() && ours.is_none() {
                // 役なし（どちらも和了にならない）は比べる値がないので除外
                hora_counts.excluded += 1;
                continue;
            }
            hora_counts.compared += 1;
            for (id, _, _) in theirs
                .iter()
                .chain(ours.iter())
                .take(1)
                .flat_map(|x| x.yaku.iter())
            {
                *hora_yaku.entry(id.clone()).or_default() += 1;
            }
            if theirs != ours {
                hora_counts.mismatched += 1;
                // 最小化：状況役・ドラを外しても食い違うものへ縮める
                let mut cur = c.clone();
                let mut progress = true;
                while progress && hora_budget > 0 {
                    progress = false;
                    for cand in hora::simpler(&cur) {
                        if hora_budget == 0 {
                            break;
                        }
                        hora_budget -= 1;
                        let a = harness.ask_hora(std::slice::from_ref(&cand));
                        let t = hora::theirs_score(&a[0]).ok().flatten();
                        if t != hora::ours_score(&hora_rules, &cand) {
                            cur = cand;
                            progress = true;
                            break;
                        }
                    }
                }
                let a = harness.ask_hora(std::slice::from_ref(&cur));
                hora_failures.push(serde_json::json!({
                    "seed": seed,
                    "original": {"case_id": c.case_id, "palette": c.palette, "shoupai": c.shoupai,
                                 "rongpai": c.rongpai, "param": c.param,
                                 "ours": ours, "majiang": theirs},
                    "minimized": {"shoupai": cur.shoupai, "rongpai": cur.rongpai, "param": cur.param,
                                  "ours": hora::ours_score(&hora_rules, &cur),
                                  "majiang": hora::theirs_score(&a[0]).ok().flatten()},
                }));
            }
        }
        i += n;
    }

    // ---- 四麻の局単位（ボット同士の半荘を majiang-core の Dev.Game で再生して突き合わせる）
    let game_n = game_n.unwrap_or(cases_n / 500);
    let variants = game::variants();
    let mut games = Counts::default();
    let mut game_kyokus = Counts::default();
    let mut game_by_variant: BTreeMap<&str, u64> = BTreeMap::new();
    let mut game_ends: BTreeMap<String, u64> = BTreeMap::new();
    // 延長戦（西入以降）の局の数（組み合わせごと）。延長戦の親の連荘の分岐が試されたかの目安
    let mut game_extension: BTreeMap<&str, u64> = BTreeMap::new();
    let mut game_rare: BTreeMap<&str, u64> = mj_bots::stats::RARE.iter().map(|k| (*k, 0)).collect();
    let mut game_failures: Vec<serde_json::Value> = Vec::new();
    let mut by_player: BTreeMap<&str, Counts> = BTreeMap::new();
    for i in 0..game_n {
        let (vname, rules, mrule) = &variants[i % variants.len()];
        let gseed = (seed ^ 0x67616d65).wrapping_add(i as u64);
        let case_id = format!("game-{i}");
        // 奇数番は探索用ボットを席ごとに混ぜる（D-047）
        let mixed = i % 2 == 1;
        let (script, ours, rare) = game::play(rules, gseed, mixed);
        let pc = by_player
            .entry(if mixed { "mixed" } else { "rule" })
            .or_default();
        pc.compared += 1;
        let theirs = harness.ask_game(&case_id, &script, mrule);
        let v = game::judge(&ours, &theirs);
        games.compared += 1;
        *game_by_variant.entry(vname).or_default() += 1;
        for r in rare {
            *game_rare.entry(r).or_default() += 1;
        }
        game_kyokus.compared += v.kyokus_matched as u64;
        for k in ours["log"].as_array().unwrap() {
            for e in k.as_array().unwrap() {
                if e.get("qipai")
                    .is_some_and(|q| q["zhuangfeng"].as_u64() >= Some(2))
                {
                    *game_extension.entry(vname).or_default() += 1;
                }
                if let Some(h) = e.get("pingju") {
                    *game_ends
                        .entry(h["_name"].as_str().unwrap().to_string())
                        .or_default() += 1;
                } else if e.get("hule").is_some() {
                    *game_ends.entry("和了".into()).or_default() += 1;
                }
            }
        }
        if let Some(m) = &v.first_mismatch {
            games.mismatched += 1;
            pc.mismatched += 1;
            game_kyokus.mismatched += 1;
            game_failures.push(serde_json::json!({
                "seed": seed, "case_id": case_id, "variant": vname, "game_seed": gseed,
                "verdict": v, "mismatch": m, "script": script, "majiang": theirs,
            }));
        }
    }

    // ---- 狙った局面（D-047。fixtures/scenarios の difftest に majiang があるもの）
    let mut scen = Counts::default();
    let mut scen_rows: Vec<serde_json::Value> = Vec::new();
    let mut scen_rare: BTreeMap<&str, u64> = mj_bots::stats::RARE.iter().map(|k| (*k, 0)).collect();
    let defs =
        mj_scenarios::load_dir(&mj_scenarios::fixtures_dir()).unwrap_or_else(|e| panic!("{e}"));
    let (_, srules, smrule) = &variants[0];
    for d in defs.iter().filter(|d| {
        d.status == mj_scenarios::Status::Ready && d.difftest.iter().any(|x| x == "majiang")
    }) {
        let (script, ours, rare) = match game::play_scenario(srules, d) {
            Ok(x) => x,
            Err(e) => {
                scen.excluded += 1;
                scen_rows.push(serde_json::json!({"id": d.id, "result": "excluded", "reason": e}));
                continue;
            }
        };
        let theirs = harness.ask_game(&format!("scenario-{}", d.id), &script, smrule);
        let v = game::judge_scenario(&ours, &theirs);
        let expected: Vec<&str> = d
            .expect
            .as_ref()
            .map(|e| e.rare.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let reached = expected.iter().all(|r| rare.contains(r));
        for r in &rare {
            *scen_rare.entry(r).or_default() += 1;
        }
        scen.compared += 1;
        let result = if v.first_mismatch.is_some() {
            "mismatch"
        } else if !reached {
            "not_reached"
        } else {
            "match"
        };
        if result != "match" {
            scen.mismatched += 1;
        }
        scen_rows.push(serde_json::json!({"id": d.id, "result": result, "rare": rare, "mismatch": v.first_mismatch,
                                          "script": if result == "match" { serde_json::Value::Null } else { script }}));
    }
    for r in scen_rows.iter().filter(|r| r["result"] != "match") {
        eprintln!("SCENARIO {r}");
    }

    let mut budget = max_shrink_queries;
    let mut records = Vec::new();
    for (c, v) in &failures {
        let small = shrink(&calc, &mut harness, c, &mut budget);
        let a = harness.ask(std::slice::from_ref(&small));
        let sv = judge(&calc, &small, &a[0]).unwrap();
        records.push(serde_json::json!({
            "seed": seed,
            "original": {"case_id": c.case_id, "strategy": c.strategy, "majiang": c.majiang,
                         "ours_shanten": v.ours_shanten, "ours_waits": v.ours_waits},
            "minimized": {"majiang": small.majiang, "ours_shanten": sv.ours_shanten,
                          "ours_waits": sv.ours_waits, "majiang_shanten": a[0].shanten,
                          "majiang_tingpai": a[0].tingpai},
        }));
    }

    let summary = serde_json::json!({
        "seed": seed,
        "cases": cases_n,
        "by_strategy": by_strategy,
        "shanten": shanten,
        "waits": waits,
        "waits_excluded_reason": "14枚形（ツモ後）は majiang-core の tingpai が null を返すため比較しない",
        "hora": hora_counts,
        "hora_cases": hora_n,
        "hora_by_palette": hora_by_palette,
        "hora_yaku": hora_yaku,
        "hora_excluded_reason": "役がなく、どちらも和了にならないケース（比べる値がない）",
        "games": games,
        "game_cases": game_n,
        "game_kyokus": game_kyokus,
        "game_by_variant": game_by_variant,
        "game_kyoku_ends": game_ends,
        "game_extension_kyokus": game_extension,
        "game_rare_events": game_rare,
        "games_by_player": by_player,
        "scenarios": scen,
        "scenario_rare_events": scen_rare,
        "scenario_results": scen_rows,
        "game_note": "games：半荘ごと（compared＝比べた半荘、mismatched＝どこかで食い違った半荘）。game_kyokus：最初の食い違いまでに一致した局（compared）と、食い違った局（mismatched）。ルールは fixtures/rules/majiang-yonma-game.toml、打ち手は偶数番が mj-bots の RuleBot、奇数番が RuleBot と探索用ボット（mj_bots::explore）を席ごとに混ぜたもの（games_by_player）",
        "failures": failures.len(),
        "hora_failures": hora_failures.len(),
        "game_failures": game_failures.len(),
        "majiang_core": "1.4.1",
    });
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("summary.json"),
            serde_json::to_string_pretty(&summary).unwrap(),
        )
        .unwrap();
        let mut f = std::fs::File::create(dir.join("failures.jsonl")).unwrap();
        for r in &records {
            writeln!(f, "{r}").unwrap();
        }
        let mut f = std::fs::File::create(dir.join("hora_failures.jsonl")).unwrap();
        for r in &hora_failures {
            writeln!(f, "{r}").unwrap();
        }
        let mut f = std::fs::File::create(dir.join("game_failures.jsonl")).unwrap();
        for r in &game_failures {
            writeln!(f, "{r}").unwrap();
        }
    }
    for r in records.iter().take(20) {
        eprintln!("MISMATCH {r}");
    }
    for r in hora_failures.iter().take(20) {
        eprintln!("HORA MISMATCH {r}");
    }
    for r in game_failures.iter().take(5) {
        eprintln!("GAME MISMATCH {} {}", r["case_id"], r["mismatch"]);
    }
    if !failures.is_empty()
        || !hora_failures.is_empty()
        || !game_failures.is_empty()
        || scen.mismatched > 0
    {
        std::process::exit(1);
    }
}
