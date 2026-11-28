//! 牌譜（LOG_FORMAT v0.4）と注釈の**型**。TS の型（`web/packages/log-types`）はここから生成する（手書きしない。REPO_LAYOUT §2）。
//!
//! - 読み書きの本体（`write.rs`・`redact`・検証）は `serde_json::Value` で扱う。この型は、牌譜を型として読む側
//!   （牌譜の再生 [`crate::view`]、WASM、FE）のためのもの
//! - 牌は表記の文字列（`5pr`・`?` など）のまま持つ
//! - 型と実際の牌譜が食い違わないことは、サンプルとエンジンの対局の牌譜を「型で読んで書き戻すと同じ JSON になる」ことで確かめる（`tests/schema.rs`）
//! - TS の生成：`cargo test -p mj-log --features ts --test ts_types`（`MJ_UPDATE_TS=1` で書き出し、なしなら一致を確かめる）

use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

/// null と「キーがない」を区別する（`seed.game_seed` は全情報版で null、公開版ではキーがない）
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// 牌譜の1行（共通フィールド ＋ イベントの本体）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct LogEvent {
    pub game_id: String,
    /// 局番号（0 始まり）。対局全体のイベントは -1
    pub kyoku_idx: i32,
    pub event_idx: u32,
    #[serde(flatten)]
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    GameStart(GameStart),
    KyokuStart(KyokuStart),
    Tsumo(Tsumo),
    Dahai(Dahai),
    Reach(Actor),
    ReachAccepted(ReachAccepted),
    Chi(Call),
    Pon(Call),
    Daiminkan(Call),
    Ankan(Ankan),
    Kakan(Kakan),
    Kita(Kita),
    Dora(Dora),
    Hora(Hora),
    Ryukyoku(Ryukyoku),
    KyokuEnd(KyokuEnd),
    GameEnd(GameEnd),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Engine {
    pub name: String,
    pub version: String,
    pub git: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Seed {
    /// 全情報版：`"0x…"` か null（手で組んだ牌山）。席視点版・公開版：キーがない
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "ts", ts(optional, type = "string | null"))]
    pub game_seed: Option<Option<String>>,
    pub rng: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Player {
    pub seat: u8,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct GameStart {
    pub format_version: String,
    /// `full` / `seat:N` / `public`
    pub visibility: String,
    pub engine: Engine,
    /// `mj-core::rules::Rules` の JSON（キーは rules/*.toml と同じ）
    #[cfg_attr(feature = "ts", ts(type = "Record<string, Record<string, unknown>>"))]
    pub rules: serde_json::Value,
    pub seed: Seed,
    pub players: Vec<Player>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Wall {
    pub pile: Vec<String>,
    pub dora_indicators: Vec<String>,
    pub ura_indicators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct KyokuStart {
    pub bakaze: String,
    pub kyoku: u8,
    pub honba: u32,
    pub kyotaku: u32,
    pub oya: u8,
    pub scores: Vec<i32>,
    pub dora_indicator: String,
    /// 席ごとの配牌（見えない牌は `?`）
    pub tehais: Vec<Vec<String>>,
    /// 全情報版だけ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub wall: Option<Wall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub wall_salt: Option<String>,
    pub wall_commit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Tsumo {
    pub actor: u8,
    pub pai: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rinshan: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Dahai {
    pub actor: u8,
    pub pai: String,
    pub tsumogiri: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Actor {
    pub actor: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct ReachAccepted {
    pub actor: u8,
    pub scores: Vec<i32>,
    pub kyotaku: u32,
}

/// チー・ポン・大明槓
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Call {
    pub actor: u8,
    pub target: u8,
    pub pai: String,
    pub consumed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Ankan {
    pub actor: u8,
    pub consumed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Kakan {
    pub actor: u8,
    pub pai: String,
    pub consumed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Kita {
    pub actor: u8,
    pub pai: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Dora {
    pub dora_indicator: String,
}

/// 副露・暗槓（`hora.hand.melds` と再生の卓の状態）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Meld {
    /// `chi` / `pon` / `daiminkan` / `kakan` / `ankan`
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pai: Option<String>,
    pub consumed: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub target: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct HoraHand {
    pub tehai: Vec<String>,
    pub melds: Vec<Meld>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct YakuEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub han: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub yakuman: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Hora {
    pub actor: u8,
    pub target: u8,
    pub pai: String,
    pub hand: HoraHand,
    pub yaku: Vec<YakuEntry>,
    pub han: u8,
    pub fu: u8,
    pub yakuman: u8,
    pub points: i32,
    pub ura_indicators: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pao: Option<u8>,
    pub deltas: Vec<i32>,
    pub scores: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Ryukyoku {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub actor: Option<u8>,
    pub tenpai: Vec<bool>,
    /// 公開しない席は null
    pub hands: Vec<Option<Vec<String>>>,
    pub deltas: Vec<i32>,
    pub scores: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct ChipItem {
    pub seat: u8,
    pub kind: String,
    pub count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Chips {
    pub deltas: Vec<i32>,
    pub items: Vec<ChipItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct KyokuEnd {
    pub reason: String,
    pub scores: Vec<i32>,
    pub kyotaku: u32,
    pub honba_next: u32,
    pub renchan: bool,
    pub chips: Chips,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct GameEnd {
    pub scores: Vec<i32>,
    pub ranks: Vec<u8>,
    pub settlement: Vec<i32>,
    pub tobi: Vec<bool>,
    pub chips_total: Vec<i32>,
}

/// 注釈（`*.annot.jsonl` の1行、LOG_FORMAT §6）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct Annotation {
    pub game_id: String,
    pub kyoku_idx: i32,
    pub event_idx: u32,
    pub seat: u8,
    pub decision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub choices: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub model: Option<String>,
    /// `[[行動, 確率], ...]`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub policy: Option<Vec<(String, f64)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// 行動の選び方：`"sample"`（確率どおりに抽選）／`"greedy"`（一番確率の高い行動）。省略は記録なし（LOG_FORMAT §6）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub selection: Option<String>,
}

/// JSONL を型で読む（1行目からの行番号付きのエラー）
pub fn parse_events(text: &str) -> Result<Vec<LogEvent>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1)))
        .collect()
}

pub fn parse_annotations(text: &str) -> Result<Vec<Annotation>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1)))
        .collect()
}
