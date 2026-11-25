//! 牌譜をエンジンで再生する：牌譜のイベントから各判断ポイントの行動を読み取り、
//! エンジンが同じイベント列を出すことと、注釈が判断ポイントを指していることを確かめる（LOG_FORMAT §6）。
//!
//! 読み取りの規則（判断ポイントの直前のイベントの、次のイベントで決まる）：
//! - 手番：`dahai` → 打牌、`reach`→`dahai` → 立直、`ankan`/`kakan`/`kita` → それぞれ、
//!   `hora`（ツモ）→ ツモ和了、`ryukyoku`（九種九牌）→ 九種九牌
//! - 応答：その席の `hora` → ロン、その席の `pon`/`chi`/`daiminkan` → 鳴き、それ以外は見送り
//!   （ノーテン宣言ありで、流局の `tenpai` がその席で false なのに聴牌なら「見送ってノーテン」）

use crate::write::event_body;
use crate::{LogError, event_key};
use mj_core::action::Action;
use mj_core::kyoku::{Decision, Kyoku, KyokuSetup};
use mj_core::rules::Rules;
use mj_core::tile::Tile;
use mj_core::wall::Wall;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

fn err<T>(m: impl Into<String>) -> Result<T, LogError> {
    Err(LogError(m.into()))
}

fn tile(v: &Value) -> Result<Tile, LogError> {
    v.as_str()
        .ok_or_else(|| LogError(format!("not a tile: {v}")))?
        .parse()
        .map_err(|e| LogError(format!("{e}")))
}

fn tiles(v: &Value) -> Result<Vec<Tile>, LogError> {
    v.as_array()
        .ok_or_else(|| LogError(format!("not a tile list: {v}")))?
        .iter()
        .map(tile)
        .collect()
}

/// 判断ポイント1つ（局、直前のイベント、席、選択肢、選んだ行動）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayedDecision {
    pub kyoku_idx: i64,
    pub event_idx: u64,
    pub seat: u8,
    pub choices: Vec<Action>,
    pub chosen: Action,
}

/// 全情報版の牌譜を局ごとに再生する（各局は `kyoku_start` の状態と牌山から始める。途中で打ち切った牌譜も扱える）。
/// エンジンのイベントが牌譜と食い違えばエラー。
pub fn replay(events: &[Value]) -> Result<Vec<ReplayedDecision>, LogError> {
    let gs = events.first().ok_or_else(|| LogError("empty log".into()))?;
    let rules: Rules =
        serde_json::from_value(gs["rules"].clone()).map_err(|e| LogError(format!("rules: {e}")))?;
    let rules = Arc::new(rules);
    // 局ごとに分ける（kyoku_idx で突き合わせる）
    let mut kyokus: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for e in events {
        let (_, k, _) = event_key(e)?;
        if k >= 0 {
            kyokus.entry(k).or_default().push(e);
        }
    }
    let mut out = Vec::new();
    for (&ki, log) in &kyokus {
        let ks = log[0];
        if ks["type"] != "kyoku_start" {
            return err(format!("kyoku {ki} does not start with kyoku_start"));
        }
        let wind = tile(&ks["bakaze"])?.kind().index() - 27;
        let setup = KyokuSetup {
            bakaze: wind as u8,
            kyoku: ks["kyoku"].as_u64().unwrap_or(1) as u8 - 1,
            honba: ks["honba"].as_u64().unwrap_or(0) as u32,
            kyotaku: ks["kyotaku"].as_u64().unwrap_or(0) as u32,
            oya: ks["oya"].as_u64().unwrap_or(0) as u8,
            scores: serde_json::from_value(ks["scores"].clone())
                .map_err(|e| LogError(format!("scores: {e}")))?,
            wall: Wall {
                haipai: ks["tehais"]
                    .as_array()
                    .ok_or_else(|| LogError("tehais".into()))?
                    .iter()
                    .map(tiles)
                    .collect::<Result<_, _>>()?,
                pile: tiles(&ks["wall"]["pile"])?,
                dora_indicators: tiles(&ks["wall"]["dora_indicators"])?,
                ura_indicators: tiles(&ks["wall"]["ura_indicators"])?,
            },
        };
        let mut k = Kyoku::new(rules.clone(), setup);
        while !k.is_done() {
            let ds: Vec<Decision> = k.decisions().to_vec();
            let produced = k.events().len();
            // 判断ポイントの直前のイベント＝エンジンが出した最後のイベント
            let last = produced - 1;
            let mut acts = Vec::new();
            for d in &ds {
                let a = read_action(d, log.get(produced), log.get(produced + 1))?;
                if !d.choices.contains(&a) {
                    return err(format!(
                        "kyoku {ki} event {last}: seat {} chose {a}, not among engine choices",
                        d.seat
                    ));
                }
                out.push(ReplayedDecision {
                    kyoku_idx: ki,
                    event_idx: last as u64,
                    seat: d.seat,
                    choices: d.choices.clone(),
                    chosen: a,
                });
                acts.push((d.seat, a));
            }
            k.step(&acts).map_err(|e| LogError(format!("{e}")))?;
        }
        // エンジンのイベントと牌譜のイベントを比べる（牌山の塩・コミットと共通フィールドを除く）
        let eng = k.events();
        if eng.len() != log.len() {
            return err(format!(
                "kyoku {ki}: engine has {} events, log has {}",
                eng.len(),
                log.len()
            ));
        }
        for (ei, (e, l)) in eng.iter().zip(log).enumerate() {
            let mine = event_body(e);
            let mut theirs = (*l).clone();
            if let Some(o) = theirs.as_object_mut() {
                for key in [
                    "game_id",
                    "kyoku_idx",
                    "event_idx",
                    "wall_salt",
                    "wall_commit",
                ] {
                    o.remove(key);
                }
            }
            if mine != theirs {
                return err(format!(
                    "kyoku {ki} event {ei}: engine {mine} != log {theirs}"
                ));
            }
        }
    }
    Ok(out)
}

fn read_action(
    d: &Decision,
    next: Option<&&Value>,
    next2: Option<&&Value>,
) -> Result<Action, LogError> {
    let seat = u64::from(d.seat);
    let Some(e) = next else {
        return err("log ended before the engine");
    };
    let actor = e["actor"].as_u64();
    let typ = e["type"].as_str().unwrap_or("");
    let is_turn = d
        .choices
        .iter()
        .any(|a| matches!(a, Action::Dahai { .. } | Action::Reach { .. }));
    if is_turn {
        if actor != Some(seat) && typ != "ryukyoku" {
            return err(format!("turn of seat {seat} but next event is {e}"));
        }
        return Ok(match typ {
            "dahai" => Action::Dahai {
                tile: tile(&e["pai"])?,
                tsumogiri: e["tsumogiri"].as_bool().unwrap_or(false),
            },
            "reach" => {
                let n = next2.ok_or_else(|| LogError("reach without dahai".into()))?;
                Action::Reach {
                    tile: tile(&n["pai"])?,
                    tsumogiri: n["tsumogiri"].as_bool().unwrap_or(false),
                }
            }
            "ankan" => Action::Ankan(tiles(&e["consumed"])?[0].kind()),
            "kakan" => Action::Kakan(tile(&e["pai"])?),
            "kita" => Action::Kita,
            "hora" => Action::Tsumo,
            "ryukyoku" if e["reason"] == "kyushu_kyuhai" => Action::Kyushu,
            _ => return err(format!("cannot read a turn action from {e}")),
        });
    }
    // 応答
    let call = |t: &str| -> Result<Option<Action>, LogError> {
        if typ == t && actor == Some(seat) {
            let c = tiles(&e["consumed"])?;
            return Ok(Some(match t {
                "pon" => Action::Pon(c[0], c[1]),
                "chi" => Action::Chi(c[0], c[1]),
                _ => Action::Daiminkan,
            }));
        }
        Ok(None)
    };
    if typ == "hora" && actor == Some(seat) {
        return Ok(Action::Ron);
    }
    // 同時のロン：2人目以降の hora は次の次以降にある
    if typ == "hora"
        && next2.is_some_and(|n| n["type"] == "hora" && n["actor"].as_u64() == Some(seat))
    {
        return Ok(Action::Ron);
    }
    for t in ["pon", "chi", "daiminkan"] {
        if let Some(a) = call(t)? {
            return Ok(a);
        }
    }
    if d.choices.contains(&Action::PassNoten) || d.choices.contains(&Action::DeclareNoten) {
        // 河底：流局の tenpai で宣言を読む
        let tenpai = e["tenpai"][d.seat as usize].as_bool().unwrap_or(true);
        if d.choices.contains(&Action::DeclareNoten) {
            return Ok(if tenpai {
                Action::DeclareTenpai
            } else {
                Action::DeclareNoten
            });
        }
        return Ok(if tenpai {
            Action::Pass
        } else {
            Action::PassNoten
        });
    }
    Ok(Action::Pass)
}

/// エンジンで再生した判断ポイントと注釈を突き合わせる：注釈のキーが判断ポイントで、
/// `decision` がそこで選ばれた行動、`choices` があればエンジンの選択肢と同じであること。
pub fn check_annotations(
    annotations: &[Value],
    decisions: &[ReplayedDecision],
) -> Result<usize, LogError> {
    for a in annotations {
        let (_, k, e) = event_key(a)?;
        let seat = a["seat"].as_u64().unwrap_or(u64::MAX);
        let d = decisions
            .iter()
            .find(|d| d.kyoku_idx == k && d.event_idx == e && u64::from(d.seat) == seat)
            .ok_or_else(|| {
                LogError(format!(
                    "annotation {k}/{e}/seat {seat} is not a decision point"
                ))
            })?;
        if a["decision"].as_str() != Some(d.chosen.to_string().as_str()) {
            return err(format!(
                "annotation {k}/{e}/seat {seat}: decision {} != replay {}",
                a["decision"], d.chosen
            ));
        }
        if let Some(c) = a.get("choices") {
            let want: Vec<String> = d.choices.iter().map(|x| x.to_string()).collect();
            let got: Vec<String> =
                serde_json::from_value(c.clone()).map_err(|e| LogError(format!("{e}")))?;
            if got != want {
                return err(format!(
                    "annotation {k}/{e}/seat {seat}: choices {got:?} != engine {want:?}"
                ));
            }
        }
    }
    Ok(annotations.len())
}
