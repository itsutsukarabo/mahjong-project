//! 牌山を手で組んで狙った局面を起こす（D-047）。
//!
//! - 局面の定義は `fixtures/scenarios/*.toml`（1局面1ファイル）。一覧は `docs/spec/SCENARIOS.md`（[`render_md`] で生成）
//! - 定義：配牌・ツモの順・嶺上牌・ドラ表示牌（[`mj_core::scenario::Scenario`] と同じ組み方）と、行動の台本（[`ScriptPlayer`]）、
//!   期待される結果（[`Expect`]）
//! - 仕様テスト（`tests/scenarios.rs`）は、定義のルールで回して期待を確かめる。差分テスト（difftest-riichienv・difftest-majiang）は
//!   同じ牌山・同じ台本を比較相手と並べて回す
//! - オーナーが局面を足すときは、文章の欄（`title`・`description`・`expect_text`・`source`）だけを書いて `status = "requested"` にすればよい
//!   （配牌などは実装側が埋める）

use mj_bots::Player;
use mj_core::action::Action;
use mj_core::event::Event;
use mj_core::kyoku::{Decision, Kyoku};
use mj_core::rules::{Rules, presets};
use mj_core::scenario::{Scenario, default_action};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// 配牌・台本・期待がそろい、テストで回る
    #[default]
    Ready,
    /// 文章だけの依頼（まだ回さない）
    Requested,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Def {
    pub id: String,
    /// 珍しい局面の表の行（例：「槍槓」）
    pub category: String,
    pub title: String,
    /// 局面の説明（麻雀の用語で）
    pub description: String,
    /// 期待される結果（文章）
    pub expect_text: String,
    /// 出典（ルールの文書・オーナー確認など）
    pub source: String,
    #[serde(default)]
    pub status: Status,
    /// `tenhou-sanma` / `mjsoul-sanma` / `test-yonma`
    #[serde(default)]
    pub rules: String,
    /// ルールの上書き（TOML）
    #[serde(default)]
    pub overrides: String,
    /// 比べる相手（`riichienv` は三麻、`majiang` は四麻）
    #[serde(default)]
    pub difftest: Vec<String>,
    /// 比べない・比べられない理由（比較相手のルールの違いなど）
    #[serde(default)]
    pub difftest_note: String,
    pub setup: Option<Setup>,
    /// 行動の台本。1要素がその場の1回の判断で、`席=行動` を空白で並べる（同じ場で複数の席が行動するとき。例 `1=ron 2=ron`）
    #[serde(default)]
    pub script: Vec<String>,
    pub expect: Option<Expect>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    #[serde(default)]
    pub oya: u8,
    #[serde(default)]
    pub bakaze: u8,
    #[serde(default)]
    pub honba: u32,
    #[serde(default)]
    pub kyotaku: u32,
    pub scores: Option<Vec<i32>>,
    /// 席 0, 1, … の配牌（13枚ずつ）
    pub hands: Vec<String>,
    /// 通常のツモの順（親の第1ツモから）
    #[serde(default)]
    pub draws: String,
    /// 嶺上牌の順
    #[serde(default)]
    pub rinshan: String,
    #[serde(default)]
    pub dora: String,
    #[serde(default)]
    pub ura: String,
    /// 最後の通常のツモ（ハイテイ）
    pub haitei: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// 必ず出る珍しい局面（`mj_bots::stats::RARE` の名前）
    #[serde(default)]
    pub rare: Vec<String>,
    /// 和了（出る順）
    #[serde(default)]
    pub hora: Vec<HoraExpect>,
    /// 流局の種類（`exhaustive` / `suukaikan` / `nagashi_mangan` …）
    pub ryukyoku: Option<String>,
    /// 局の点数の移動の合計（席ごと。和了・流局・立直の供託を含む、局の始めと終わりの持ち点の差）
    pub deltas: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoraExpect {
    pub seat: u8,
    /// 放銃者（ツモなら和了者と同じ）
    pub from: Option<u8>,
    /// 必ず付く役（牌譜の `yaku[].id`）
    #[serde(default)]
    pub yaku: Vec<String>,
    /// 付かない役
    #[serde(default)]
    pub not_yaku: Vec<String>,
    /// 包の人
    pub pao: Option<u8>,
    /// 役満の倍数
    pub yakuman: Option<u8>,
    /// この和了の点数の移動
    pub deltas: Option<Vec<i32>>,
}

/// `fixtures/scenarios/*.toml` を名前の順に読む
pub fn load_dir(dir: &Path) -> Result<Vec<Def>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let s = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let d: Def = toml::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))?;
        let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or_default();
        if d.id != stem {
            return Err(format!(
                "{}: id {} differs from the file name",
                p.display(),
                d.id
            ));
        }
        out.push(d);
    }
    let mut ids: Vec<&str> = out.iter().map(|d| d.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    if ids.len() != out.len() {
        return Err("duplicate scenario id".into());
    }
    Ok(out)
}

/// リポジトリの `fixtures/scenarios`
pub fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios")
}

pub fn preset(name: &str) -> Result<Rules, String> {
    Ok(match name {
        "tenhou-sanma" => presets::tenhou_sanma(),
        "mjsoul-sanma" => presets::mjsoul_sanma(),
        "test-yonma" => presets::test_yonma(),
        other => return Err(format!("unknown rules {other}")),
    })
}

impl Def {
    /// 定義のルール（プリセット ＋ 上書き）
    pub fn rules(&self) -> Result<Rules, String> {
        let base = preset(&self.rules)?;
        Ok(if self.overrides.is_empty() {
            base
        } else {
            presets::with(&base, &self.overrides)
        })
    }

    pub fn is_sanma(&self) -> bool {
        self.rules != "test-yonma"
    }

    /// 牌山の組み方（`rules` の牌の集合で組む）
    pub fn scenario(&self, rules: &Rules) -> Result<Scenario, String> {
        let st = self.setup.as_ref().ok_or("no setup")?;
        let hands: Vec<&str> = st.hands.iter().map(String::as_str).collect();
        if hands.len() != usize::from(rules.table.num_players) {
            return Err(format!(
                "{} hands for {} players",
                hands.len(),
                rules.table.num_players
            ));
        }
        let mut s = Scenario::new(rules.clone(), &hands)
            .draws(&st.draws)
            .rinshan(&st.rinshan)
            .dora(&st.dora)
            .ura(&st.ura)
            .oya(st.oya)
            .honba(st.honba, st.kyotaku);
        if let Some(h) = &st.haitei {
            s = s.haitei(h);
        }
        if let Some(sc) = &st.scores {
            s = s.scores(sc);
        }
        s.bakaze = st.bakaze;
        Ok(s)
    }

    pub fn script(&self) -> Result<ScriptPlayer, String> {
        ScriptPlayer::parse(&self.script)
    }
}

/// 台本どおりに打つ打ち手。台本の次の1歩が「その席たちに判断ポイントがあり、どの行動も選択肢にある」場でだけ使い、
/// それ以外の場は既定の行動（見送り → ツモ切り → 最初の選択肢）で進める。
/// 打牌・立直の牌は、手出し・ツモ切りを書かなければどちらにも合わせる
#[derive(Debug, Clone)]
pub struct ScriptPlayer {
    steps: Vec<Vec<(u8, Action)>>,
    next: usize,
}

fn flip(a: Action) -> Option<Action> {
    match a {
        Action::Dahai { tile, tsumogiri } => Some(Action::Dahai {
            tile,
            tsumogiri: !tsumogiri,
        }),
        Action::Reach { tile, tsumogiri } => Some(Action::Reach {
            tile,
            tsumogiri: !tsumogiri,
        }),
        _ => None,
    }
}

impl ScriptPlayer {
    pub fn parse(lines: &[String]) -> Result<Self, String> {
        let mut steps = Vec::new();
        for l in lines {
            let mut st = Vec::new();
            for part in l.split_whitespace() {
                let (s, a) = part
                    .split_once('=')
                    .ok_or_else(|| format!("{part}: expected seat=action"))?;
                let s: u8 = s.parse().map_err(|_| format!("{part}: bad seat"))?;
                let a: Action = a.parse().map_err(|e| format!("{part}: {e:?}"))?;
                st.push((s, a));
            }
            if st.is_empty() {
                return Err("empty step".into());
            }
            steps.push(st);
        }
        Ok(Self { steps, next: 0 })
    }

    /// 台本を使い切ったか
    pub fn done(&self) -> bool {
        self.next == self.steps.len()
    }

    /// 使った歩数と全体
    pub fn progress(&self) -> (usize, usize) {
        (self.next, self.steps.len())
    }

    fn resolve(d: &Decision, a: Action, explicit: bool) -> Option<Action> {
        if d.choices.contains(&a) {
            return Some(a);
        }
        if !explicit {
            return flip(a).filter(|b| d.choices.contains(b));
        }
        None
    }
}

impl Player for ScriptPlayer {
    fn act(&mut self, _k: &Kyoku, ds: &[Decision]) -> Vec<(u8, Action)> {
        if let Some(st) = self.steps.get(self.next) {
            let mut chosen = Vec::new();
            for (s, a) in st {
                // `:tsumogiri` を書いたときだけ手出し・ツモ切りを区別する
                let explicit = matches!(
                    a,
                    Action::Dahai {
                        tsumogiri: true,
                        ..
                    } | Action::Reach {
                        tsumogiri: true,
                        ..
                    }
                );
                match ds.iter().find(|d| d.seat == *s) {
                    Some(d) => match Self::resolve(d, *a, explicit) {
                        Some(x) => chosen.push((*s, x)),
                        None => break,
                    },
                    None => break,
                }
            }
            if chosen.len() == st.len() {
                self.next += 1;
                return ds
                    .iter()
                    .map(|d| {
                        let a = chosen
                            .iter()
                            .find(|(s, _)| *s == d.seat)
                            .map(|(_, a)| *a)
                            .unwrap_or_else(|| default_action(&d.choices));
                        (d.seat, a)
                    })
                    .collect();
            }
        }
        ds.iter()
            .map(|d| (d.seat, default_action(&d.choices)))
            .collect()
    }
}

/// 局を台本で終わりまで回す（台本を使い切らなければ誤り）
pub fn play_kyoku(k: &mut Kyoku, p: &mut ScriptPlayer) -> Result<(), String> {
    for _ in 0..10_000 {
        if k.is_done() {
            if !p.done() {
                let (a, b) = p.progress();
                return Err(format!("kyoku ended after {a} of {b} script steps"));
            }
            return Ok(());
        }
        let ds = k.decisions().to_vec();
        let acts = p.act(k, &ds);
        k.step(&acts).map_err(|e| format!("engine rejected: {e}"))?;
    }
    Err("no progress".into())
}

/// 局のイベント列が期待どおりか
pub fn check(
    e: &Expect,
    events: &[Event],
    start_scores: &[i32],
    end_scores: &[i32],
) -> Result<(), String> {
    let rare = mj_bots::stats::rare_events(events);
    for r in &e.rare {
        if !mj_bots::stats::RARE.contains(&r.as_str()) {
            return Err(format!("unknown rare event {r}"));
        }
        if !rare.contains(&r.as_str()) {
            return Err(format!("rare event {r} did not occur (got {rare:?})"));
        }
    }
    let horas: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Hora(h) => Some(&**h),
            _ => None,
        })
        .collect();
    if horas.len() != e.hora.len() {
        return Err(format!("{} hora, expected {}", horas.len(), e.hora.len()));
    }
    for (h, x) in horas.iter().zip(&e.hora) {
        let ids: Vec<&str> = h.yaku.iter().map(|y| y.yaku.id()).collect();
        if h.actor != x.seat {
            return Err(format!("hora by {} (expected {})", h.actor, x.seat));
        }
        if let Some(f) = x.from
            && h.target != f
        {
            return Err(format!("hora from {} (expected {f})", h.target));
        }
        for y in &x.yaku {
            if !ids.contains(&y.as_str()) {
                return Err(format!("seat {}: yaku {y} missing (got {ids:?})", h.actor));
            }
        }
        for y in &x.not_yaku {
            if ids.contains(&y.as_str()) {
                return Err(format!("seat {}: yaku {y} should not be there", h.actor));
            }
        }
        if x.pao.is_some() && h.pao != x.pao {
            return Err(format!("pao {:?} (expected {:?})", h.pao, x.pao));
        }
        if let Some(m) = x.yakuman
            && h.yakuman != m
        {
            return Err(format!("yakuman {} (expected {m})", h.yakuman));
        }
        if let Some(d) = &x.deltas
            && &h.deltas != d
        {
            return Err(format!("hora deltas {:?} (expected {d:?})", h.deltas));
        }
    }
    let ryu = events.iter().find_map(|e| match e {
        Event::Ryukyoku(r) => Some(r.reason),
        _ => None,
    });
    match (&e.ryukyoku, ryu) {
        (Some(a), Some(b)) if a == b => {}
        (None, None) => {}
        (a, b) => return Err(format!("ryukyoku {b:?} (expected {a:?})")),
    }
    if let Some(d) = &e.deltas {
        let got: Vec<i32> = end_scores
            .iter()
            .zip(start_scores)
            .map(|(a, b)| a - b)
            .collect();
        if &got != d {
            return Err(format!("kyoku deltas {got:?} (expected {d:?})"));
        }
    }
    Ok(())
}

/// 定義のルールで回して期待を確かめる（仕様テスト）。回した局を返す
pub fn run_spec(d: &Def) -> Result<Kyoku, String> {
    let rules = d.rules()?;
    let sc = d.scenario(&rules)?;
    let start = sc.setup().scores;
    let mut k = sc.kyoku();
    let mut p = d.script()?;
    play_kyoku(&mut k, &mut p)?;
    check(
        d.expect.as_ref().ok_or("no expect")?,
        k.events(),
        &start,
        k.scores(),
    )?;
    Ok(k)
}

const BEGIN: &str = "<!-- BEGIN GENERATED: cargo run -p mj-scenarios -- gen -->";
const END: &str = "<!-- END GENERATED -->";

/// `docs/spec/SCENARIOS.md` の生成部分（印の間だけを書き換える。印の外はオーナーが自由に書ける）
pub fn render_md(current: &str, defs: &[Def]) -> Result<String, String> {
    let (head, rest) = current.split_once(BEGIN).ok_or("no BEGIN marker")?;
    let (_, tail) = rest.split_once(END).ok_or("no END marker")?;
    let mut s = String::new();
    s.push_str(BEGIN);
    s.push_str("\n\n| 局面 | 表の行 | ルール | 比べる相手 | 状態 |\n|---|---|---|---|---|\n");
    for d in defs {
        let st = match d.status {
            Status::Ready => "テストあり",
            Status::Requested => "**依頼（未実装）**",
        };
        let dt = if d.difftest.is_empty() {
            "仕様テストのみ".to_string()
        } else {
            d.difftest.join("・")
        };
        s.push_str(&format!(
            "| [{}](#{}) | {} | {} | {} | {} |\n",
            d.title,
            d.id,
            d.category,
            if d.rules.is_empty() { "—" } else { &d.rules },
            dt,
            st
        ));
    }
    for d in defs {
        s.push_str(&format!(
            "\n### {}\n<a id=\"{}\"></a>`fixtures/scenarios/{}.toml`\n\n",
            d.title, d.id, d.id
        ));
        s.push_str(&format!("- **局面**：{}\n", d.description.trim()));
        s.push_str(&format!("- **期待される結果**：{}\n", d.expect_text.trim()));
        s.push_str(&format!("- **出典**：{}\n", d.source.trim()));
        if !d.difftest_note.trim().is_empty() {
            s.push_str(&format!("- **差分テスト**：{}\n", d.difftest_note.trim()));
        }
    }
    s.push('\n');
    s.push_str(END);
    Ok(format!("{head}{s}{tail}"))
}
