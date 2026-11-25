//! 牌譜（`docs/spec/LOG_FORMAT.md` v0.2）の読み書き・検証・情報の秘匿（redact）。
//!
//! 現時点では型付きのイベントは持たず、`serde_json::Value` の上で検証と変換を行う
//! （局進行が mj-core に入った段階で型付きのイベントに置き換える）。

use mj_core::rules::{AnkanVisibility, Rules, RyukyokuReveal};
use mj_core::tile::Tile;
use mj_core::wall::{Wall, tile_multiset};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

pub const FORMAT_VERSION: &str = "0.2";
pub const HIDDEN: &str = "?";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogError(pub String);

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LogError {}

fn err<T>(msg: impl Into<String>) -> Result<T, LogError> {
    Err(LogError(msg.into()))
}

/// 区切りに空白を入れない JSON（キー順は入力のまま、非 ASCII はエスケープしない）。
pub fn canonical_json(v: &Value) -> String {
    serde_json::to_string(v).expect("Value always serializes")
}

pub fn parse_jsonl(text: &str) -> Result<Vec<Value>, LogError> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| LogError(format!("line {}: {e}", i + 1))))
        .collect()
}

pub fn to_jsonl(events: &[Value]) -> String {
    events.iter().map(|e| canonical_json(e) + "\n").collect()
}

fn tiles_json(t: &[Tile]) -> Value {
    Value::Array(t.iter().map(|t| Value::String(t.to_string())).collect())
}

/// 牌山を牌譜の形にする：(`tehais`, `wall`)。
pub fn wall_to_json(w: &Wall) -> (Value, Value) {
    let tehais = Value::Array(w.haipai.iter().map(|h| tiles_json(h)).collect());
    let wall = json!({
        "pile": tiles_json(&w.pile),
        "dora_indicators": tiles_json(&w.dora_indicators),
        "ura_indicators": tiles_json(&w.ura_indicators),
    });
    (tehais, wall)
}

/// 牌山の正規化した JSON（コミットとゴールデンテストの対象）。
pub fn wall_canonical(tehais: &Value, wall: &Value) -> String {
    canonical_json(&json!({"haipai": tehais, "wall": wall}))
}

/// `wall_commit`（LOG_FORMAT §4.6）。
pub fn wall_commit(salt_hex: &str, tehais: &Value, wall: &Value) -> String {
    let mut h = Sha256::new();
    h.update(salt_hex.as_bytes());
    h.update(wall_canonical(tehais, wall).as_bytes());
    format!("sha256:{}", hex::encode(h.finalize()))
}

/// `wall_salt` を OS の暗号論的乱数から作る（16 バイト → 32 文字の16進）。シードからは導出しない。
pub fn generate_salt() -> Result<String, LogError> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| LogError(format!("OS RNG unavailable: {e}")))?;
    Ok(hex::encode(b))
}

/// 誰に見せる牌譜か。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer {
    Public,
    Seat(u8),
}

impl Viewer {
    fn label(self) -> String {
        match self {
            Viewer::Public => "public".into(),
            Viewer::Seat(n) => format!("seat:{n}"),
        }
    }

    fn sees_seat(self, seat: u64) -> bool {
        matches!(self, Viewer::Seat(n) if u64::from(n) == seat)
    }
}

fn hidden(n: usize) -> Value {
    Value::Array(vec![Value::String(HIDDEN.into()); n])
}

fn obj_mut<'a>(v: &'a mut Value, what: &str) -> Result<&'a mut Map<String, Value>, LogError> {
    v.as_object_mut()
        .ok_or_else(|| LogError(format!("{what}: event is not an object")))
}

fn rules_of(events: &[Value]) -> Result<Rules, LogError> {
    let gs = events
        .iter()
        .find(|e| e["type"] == "game_start")
        .ok_or_else(|| LogError("no game_start".into()))?;
    let rules: Rules = serde_json::from_value(gs["rules"].clone())
        .map_err(|e| LogError(format!("game_start.rules: {e}")))?;
    rules
        .validate()
        .map_err(|e| LogError(format!("game_start.rules: {e}")))?;
    Ok(rules)
}

/// 全情報版から席視点版・公開情報版を作る（LOG_FORMAT §5）。イベントの増減・並べ替えはしない。
pub fn redact(events: &[Value], viewer: Viewer) -> Result<Vec<Value>, LogError> {
    let rules = rules_of(events)?;
    let mut out = Vec::with_capacity(events.len());
    for e in events {
        let mut p = e.clone();
        let ty = e["type"].as_str().unwrap_or_default().to_string();
        let actor = e["actor"].as_u64();
        let o = obj_mut(&mut p, &ty)?;
        match ty.as_str() {
            "game_start" => {
                o.insert("visibility".into(), Value::String(viewer.label()));
                if let Some(seed) = o.get_mut("seed").and_then(Value::as_object_mut) {
                    seed.remove("game_seed");
                }
            }
            "kyoku_start" => {
                if let Some(Value::Array(hands)) = o.get_mut("tehais") {
                    for (i, h) in hands.iter_mut().enumerate() {
                        if !viewer.sees_seat(i as u64) {
                            let n = h.as_array().map_or(0, Vec::len);
                            *h = hidden(n);
                        }
                    }
                }
                o.remove("wall");
                o.remove("wall_salt");
            }
            "tsumo" => {
                if !actor.is_some_and(|a| viewer.sees_seat(a)) {
                    o.insert("pai".into(), Value::String(HIDDEN.into()));
                }
            }
            "ankan" => {
                if rules.log_visibility.ankan == AnkanVisibility::Closed
                    && !actor.is_some_and(|a| viewer.sees_seat(a))
                {
                    o.insert("consumed".into(), hidden(4));
                }
            }
            "ryukyoku" => {
                let tenpai: Vec<bool> = o
                    .get("tenpai")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|b| b.as_bool().unwrap_or(false)).collect())
                    .unwrap_or_default();
                if let Some(Value::Array(hands)) = o.get_mut("hands") {
                    for (i, h) in hands.iter_mut().enumerate() {
                        let shown = viewer.sees_seat(i as u64)
                            || match rules.log_visibility.ryukyoku_reveal {
                                RyukyokuReveal::All => true,
                                RyukyokuReveal::TenpaiOnly => {
                                    tenpai.get(i).copied().unwrap_or(false)
                                }
                                RyukyokuReveal::None => false,
                            };
                        if !shown {
                            *h = Value::Null;
                        }
                    }
                }
            }
            _ => {}
        }
        out.push(p);
    }
    Ok(out)
}

/// 3つ組 (game_id, kyoku_idx, event_idx)。
pub type EventKey = (String, i64, u64);

pub fn event_key(e: &Value) -> Result<EventKey, LogError> {
    let g = e["game_id"]
        .as_str()
        .ok_or_else(|| LogError(format!("no game_id: {e}")))?;
    let k = e["kyoku_idx"]
        .as_i64()
        .ok_or_else(|| LogError(format!("no kyoku_idx: {e}")))?;
    let i = e["event_idx"]
        .as_u64()
        .ok_or_else(|| LogError(format!("no event_idx: {e}")))?;
    Ok((g.to_string(), k, i))
}

/// 全情報版を検証したときの件数。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub events: usize,
    pub kyoku: usize,
    pub draws: usize,
    pub rinshan_draws: usize,
}

fn tile_list(v: &Value, what: &str) -> Result<Vec<Tile>, LogError> {
    v.as_array()
        .ok_or_else(|| LogError(format!("{what}: not an array")))?
        .iter()
        .map(|t| {
            t.as_str()
                .ok_or_else(|| LogError(format!("{what}: not a string")))?
                .parse::<Tile>()
                .map_err(|e| LogError(format!("{what}: {e}")))
        })
        .collect()
}

/// 全情報版の牌譜を検証する：ID の一意・連番、ルール、牌山の多重集合とコミット、
/// ツモ・嶺上ツモが `pile` の先頭・末尾と一致すること。
pub fn validate_full(events: &[Value]) -> Result<Report, LogError> {
    let rules = rules_of(events)?;
    if events.first().map(|e| &e["type"]) != Some(&json!("game_start")) {
        return err("first event must be game_start");
    }
    if events.first().map(|e| &e["visibility"]) != Some(&json!("full")) {
        return err("game_start.visibility must be full");
    }
    let game_id = event_key(&events[0])?.0;
    let mut seen = BTreeSet::new();
    let mut next_idx: BTreeMap<i64, u64> = BTreeMap::new();
    for e in events {
        let key = event_key(e)?;
        if key.0 != game_id {
            return err(format!("mixed game_id: {}", key.0));
        }
        let want = next_idx.entry(key.1).or_insert(0);
        if key.2 != *want {
            return err(format!(
                "kyoku {} event_idx {} (expected {want})",
                key.1, key.2
            ));
        }
        *want += 1;
        if !seen.insert(key.clone()) {
            return err(format!("duplicate key {key:?}"));
        }
    }
    let mut expected: Vec<Tile> = tile_multiset(&rules.tiles);
    expected.sort();
    let mut report = Report {
        events: events.len(),
        ..Report::default()
    };
    let mut pile: Vec<Tile> = Vec::new();
    let (mut front, mut back) = (0usize, 0usize);
    for e in events {
        match e["type"].as_str() {
            Some("kyoku_start") => {
                report.kyoku += 1;
                if e["event_idx"] != json!(0) {
                    return err("kyoku_start must be event_idx 0");
                }
                let tehais = &e["tehais"];
                let wall = &e["wall"];
                let mut all = Vec::new();
                for (i, h) in tehais.as_array().into_iter().flatten().enumerate() {
                    all.extend(tile_list(h, &format!("tehais[{i}]"))?);
                }
                pile = tile_list(&wall["pile"], "wall.pile")?;
                let dora = tile_list(&wall["dora_indicators"], "wall.dora_indicators")?;
                let ura = tile_list(&wall["ura_indicators"], "wall.ura_indicators")?;
                let slots = usize::from(rules.wall.dora_indicator_slots);
                if dora.len() != slots || ura.len() != slots {
                    return err("indicator count differs from rules.wall.dora_indicator_slots");
                }
                if e["dora_indicator"].as_str() != Some(dora[0].to_string().as_str()) {
                    return err("kyoku_start.dora_indicator != wall.dora_indicators[0]");
                }
                all.extend(pile.iter().copied());
                all.extend(dora);
                all.extend(ura);
                all.sort();
                if all != expected {
                    return err(format!(
                        "kyoku {}: wall is not the rules' tile multiset",
                        e["kyoku_idx"]
                    ));
                }
                let salt = e["wall_salt"]
                    .as_str()
                    .ok_or_else(|| LogError("no wall_salt".into()))?;
                if e["wall_commit"].as_str() != Some(wall_commit(salt, tehais, wall).as_str()) {
                    return err("wall_commit mismatch");
                }
                front = 0;
                back = pile.len();
            }
            Some("tsumo") => {
                let pai: Tile = e["pai"]
                    .as_str()
                    .ok_or_else(|| LogError("tsumo.pai".into()))?
                    .parse()
                    .map_err(|x| LogError(format!("tsumo.pai: {x}")))?;
                let rinshan = e["rinshan"].as_bool().unwrap_or(false);
                let got = if rinshan {
                    report.rinshan_draws += 1;
                    back = back
                        .checked_sub(1)
                        .ok_or_else(|| LogError("pile exhausted".into()))?;
                    pile.get(back)
                } else {
                    report.draws += 1;
                    front += 1;
                    pile.get(front - 1)
                };
                if front > back || got != Some(&pai) {
                    return err(format!(
                        "{:?}: draw does not match wall.pile",
                        event_key(e)?
                    ));
                }
                if !rinshan && pile.len() - front < rules.wall.reserve() {
                    return err("drew past haitei (rules.wall.dead_wall)");
                }
            }
            _ => {}
        }
    }
    Ok(report)
}

/// 注釈（`*.annot.jsonl`）のキーを検証する：一意で、対応するイベントがあり、seat が人数の範囲内。
pub fn validate_annotations(annotations: &[Value], events: &[Value]) -> Result<usize, LogError> {
    let rules = rules_of(events)?;
    let ids: BTreeSet<EventKey> = events.iter().map(event_key).collect::<Result<_, _>>()?;
    let mut keys: HashMap<(EventKey, u64), ()> = HashMap::new();
    for a in annotations {
        let k = event_key(a)?;
        let seat = a["seat"]
            .as_u64()
            .ok_or_else(|| LogError(format!("annotation without seat: {a}")))?;
        if seat >= u64::from(rules.table.num_players) {
            return err(format!("annotation seat out of range: {a}"));
        }
        if !ids.contains(&k) {
            return err(format!("annotation for missing event {k:?}"));
        }
        if keys.insert((k.clone(), seat), ()).is_some() {
            return err(format!("duplicate annotation key {k:?} seat {seat}"));
        }
    }
    Ok(keys.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salt_is_32_hex_chars_and_not_repeated() {
        let a = generate_salt().unwrap();
        let b = generate_salt().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
