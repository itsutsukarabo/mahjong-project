//! エンジンの対局（[`mj_core::game::Game`]）から全情報版の牌譜（JSONL の行）を作る。
//!
//! フィールドは `docs/spec/LOG_FORMAT.md` §4 のとおり。整形はここだけで行い、コアには置かない（CLAUDE.md 原則5）。

use crate::{FORMAT_VERSION, wall_commit};
use mj_core::event::{Event, Hora, MeldView};
use mj_core::game::{Game, WallSource};
use mj_core::tile::Tile;
use mj_core::wall::RNG_NAME;
use serde_json::{Value, json};

/// 牌譜の対局全体の情報。
#[derive(Debug, Clone)]
pub struct GameMeta {
    pub game_id: String,
    pub players: Vec<String>,
    pub engine_version: String,
    pub engine_git: String,
}

fn tiles(t: &[Tile]) -> Value {
    Value::Array(t.iter().map(|t| Value::String(t.to_string())).collect())
}

fn melds(m: &[MeldView]) -> Value {
    Value::Array(
        m.iter()
            .map(|m| {
                let mut o = json!({"type": m.kind});
                if let Some(p) = m.pai {
                    o["pai"] = json!(p.to_string());
                }
                o["consumed"] = tiles(&m.consumed);
                if let Some(t) = m.target {
                    o["target"] = json!(t);
                }
                o
            })
            .collect(),
    )
}

fn hora_json(h: &Hora) -> Value {
    let yaku: Vec<Value> = h
        .yaku
        .iter()
        .map(|y| {
            if y.yakuman > 0 {
                json!({"id": y.yaku.id(), "yakuman": y.yakuman})
            } else {
                json!({"id": y.yaku.id(), "han": y.han})
            }
        })
        .collect();
    let mut o = json!({
        "actor": h.actor, "target": h.target, "pai": h.pai.to_string(),
        "hand": {"tehai": tiles(&h.tehai), "melds": melds(&h.melds)},
        "yaku": yaku, "han": h.han, "fu": h.fu, "yakuman": h.yakuman, "points": h.points,
        "ura_indicators": tiles(&h.ura_indicators),
    });
    if let Some(p) = h.pao {
        o["pao"] = json!(p);
    }
    o["deltas"] = json!(h.deltas);
    o["scores"] = json!(h.scores);
    o
}

/// 局のイベント1つを、共通フィールドなしの JSON にする（`kyoku_start` の牌山のコミットは呼び出し側で足す）。
pub fn event_body(e: &Event) -> Value {
    match e {
        Event::KyokuStart(k) => json!({
            "type": "kyoku_start", "bakaze": k.bakaze.to_string(), "kyoku": k.kyoku,
            "honba": k.honba, "kyotaku": k.kyotaku, "oya": k.oya, "scores": k.scores,
            "dora_indicator": k.dora_indicator.to_string(),
            "tehais": Value::Array(k.tehais.iter().map(|h| tiles(h)).collect()),
            "wall": {"pile": tiles(&k.pile), "dora_indicators": tiles(&k.dora_indicators),
                     "ura_indicators": tiles(&k.ura_indicators)},
        }),
        Event::Tsumo {
            actor,
            pai,
            rinshan,
        } => {
            let mut o = json!({"type": "tsumo", "actor": actor, "pai": pai.to_string()});
            if *rinshan {
                o["rinshan"] = json!(true);
            }
            o
        }
        Event::Dahai {
            actor,
            pai,
            tsumogiri,
        } => {
            json!({"type": "dahai", "actor": actor, "pai": pai.to_string(), "tsumogiri": tsumogiri})
        }
        Event::Reach { actor } => json!({"type": "reach", "actor": actor}),
        Event::ReachAccepted {
            actor,
            scores,
            kyotaku,
        } => {
            json!({"type": "reach_accepted", "actor": actor, "scores": scores, "kyotaku": kyotaku})
        }
        Event::Chi {
            actor,
            target,
            pai,
            consumed,
        } => {
            json!({"type": "chi", "actor": actor, "target": target, "pai": pai.to_string(), "consumed": tiles(consumed)})
        }
        Event::Pon {
            actor,
            target,
            pai,
            consumed,
        } => {
            json!({"type": "pon", "actor": actor, "target": target, "pai": pai.to_string(), "consumed": tiles(consumed)})
        }
        Event::Daiminkan {
            actor,
            target,
            pai,
            consumed,
        } => {
            json!({"type": "daiminkan", "actor": actor, "target": target, "pai": pai.to_string(), "consumed": tiles(consumed)})
        }
        Event::Ankan { actor, consumed } => {
            json!({"type": "ankan", "actor": actor, "consumed": tiles(consumed)})
        }
        Event::Kakan {
            actor,
            pai,
            consumed,
        } => {
            json!({"type": "kakan", "actor": actor, "pai": pai.to_string(), "consumed": tiles(consumed)})
        }
        Event::Kita { actor, pai } => {
            json!({"type": "kita", "actor": actor, "pai": pai.to_string()})
        }
        Event::Dora { dora_indicator } => {
            json!({"type": "dora", "dora_indicator": dora_indicator.to_string()})
        }
        Event::Hora(h) => {
            let mut o = json!({"type": "hora"});
            for (k, v) in hora_json(h).as_object().expect("object") {
                o[k] = v.clone();
            }
            o
        }
        Event::Ryukyoku(r) => {
            let mut o = json!({"type": "ryukyoku", "reason": r.reason});
            if let Some(a) = r.actor {
                o["actor"] = json!(a);
            }
            o["tenpai"] = json!(r.tenpai);
            o["hands"] = Value::Array(r.hands.iter().map(|h| tiles(h)).collect());
            o["deltas"] = json!(r.deltas);
            o["scores"] = json!(r.scores);
            o
        }
        Event::KyokuEnd(k) => json!({
            "type": "kyoku_end", "reason": k.reason, "scores": k.scores, "kyotaku": k.kyotaku,
            "honba_next": k.honba_next, "renchan": k.renchan,
            "chips": {"deltas": k.chip_deltas,
                      "items": k.chip_items.iter().map(|c| json!({"seat": c.seat, "kind": c.kind, "count": c.count})).collect::<Vec<_>>()},
        }),
    }
}

fn with_key(game_id: &str, kyoku_idx: i64, event_idx: usize, body: Value) -> Value {
    let mut o = json!({"game_id": game_id, "kyoku_idx": kyoku_idx, "event_idx": event_idx});
    for (k, v) in body.as_object().expect("object") {
        o[k] = v.clone();
    }
    o
}

/// 全情報版の牌譜。`salt_for(kyoku_idx)` は各局の `wall_salt`（本番は [`crate::generate_salt`]、テストは固定値）。
pub fn game_log(
    game: &Game,
    meta: &GameMeta,
    salt_for: &mut dyn FnMut(u64) -> String,
) -> Vec<Value> {
    let mut out = Vec::new();
    let seed = match game.wall_source() {
        WallSource::Seed(s) => json!({"game_seed": format!("{s:#x}"), "rng": RNG_NAME}),
        WallSource::Manual(_) => json!({"game_seed": null, "rng": "manual"}),
    };
    let players: Vec<Value> = meta
        .players
        .iter()
        .enumerate()
        .map(|(i, n)| json!({"seat": i, "name": n}))
        .collect();
    out.push(with_key(
        &meta.game_id,
        -1,
        0,
        json!({
            "type": "game_start", "format_version": FORMAT_VERSION, "visibility": "full",
            "engine": {"name": "mj-core", "version": meta.engine_version, "git": meta.engine_git},
            "rules": serde_json::to_value(game.rules()).expect("rules serialize"),
            "seed": seed, "players": players,
        }),
    ));
    for (ki, events) in game.kyokus().enumerate() {
        for (ei, e) in events.iter().enumerate() {
            let mut body = event_body(e);
            if matches!(e, Event::KyokuStart(_)) {
                let salt = salt_for(ki as u64);
                let commit = wall_commit(&salt, &body["tehais"], &body["wall"]);
                body["wall_salt"] = json!(salt);
                body["wall_commit"] = json!(commit);
            }
            out.push(with_key(&meta.game_id, ki as i64, ei, body));
        }
    }
    if let Some(end) = game.end() {
        out.push(with_key(
            &meta.game_id,
            -1,
            1,
            json!({"type": "game_end", "scores": end.scores, "ranks": end.ranks,
                   "settlement": end.settlement, "tobi": end.tobi, "chips_total": end.chips_total}),
        ));
    }
    out
}
