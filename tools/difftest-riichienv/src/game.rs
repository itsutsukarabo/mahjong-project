//! 局進行の層（北抜きを含む）：RiichiEnv の局と mj-core の局を**同じ牌山で並べて**進める。
//!
//! 1. RiichiEnv（`3p-red-single`）を `seed` で始め、牌山（`env.wall`）と配牌を受け取る
//! 2. 同じ牌山を mj-core の `Wall`（手で組んだ牌山）に写して局を始める（写し方は [`our_wall`]）
//! 3. 判断ポイントごとに、判断する席の集合と、各席の選択肢（正規形）を比べる。RiichiEnv で選択肢が1つの席は
//!    自動で進める（mj-core は選択肢が1つの場面を判断ポイントにしない。D-027）
//! 4. mj-core のボット（`mj-bots`）が選んだ行動を両方に与える。立直は RiichiEnv では「立直」→「打牌」の2手なので、
//!    立直の後に RiichiEnv が出す打牌の選択肢と、mj-core の `reach:<牌>` の牌の集合も比べる
//! 5. 局の終わりに、イベントの流れ（ツモ・打牌・鳴き・北抜き・ドラ・立直）と、和了・流局の点数の移動、最終の持ち点を比べる
//!
//! 除外：RiichiEnv の牌山の作り（5つ目以降の嶺上牌と、4つ目・5つ目のドラ表示牌が通常のツモの位置と重なる。
//! `state_3p/wall.rs`）は mj-core（王牌は常に別の 14 枚）と写せないので、嶺上ツモが5回以上か、
//! カンドラ（局の最初の表示牌を除く）が3枚以上めくられた局は比べない。

use crate::harness::Harness;
use mj_bots::RuleBot;
use mj_core::action::Action;
use mj_core::event::Event;
use mj_core::kyoku::{Kyoku, KyokuSetup};
use mj_core::rules::Rules;
use mj_core::tile::{Tile, TileKind};
use mj_core::wall::Wall;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn tile_of_id(id: u64) -> Tile {
    let red = matches!(id, 16 | 52 | 88);
    Tile::new(TileKind::new((id / 4) as u8).expect("kind"), red).expect("tile")
}

/// RiichiEnv の牌山（`env.wall` ＋ 親の第1ツモ ＝ 配牌後の 69 枚、`T[0]` が嶺上側）→ mj-core の牌山
///
/// - 通常のツモ：RiichiEnv は末尾から取る → `pile[i] = T[68 − i]`（55 枚）
/// - 嶺上ツモ：RiichiEnv は先頭から取る → `pile[58 − j] = T[j]`（4 枚まで）
/// - ドラ表示牌：RiichiEnv の D1〜D3 は `T[8], T[10], T[12]`（裏は `T[9], T[11], T[13]`）。
///   D4・D5 は RiichiEnv では通常のツモの位置（`T[14..18]`）と重なるので、mj-core には残りの `T[4..8]` を入れる（比べない）
pub fn our_wall(t: &[Tile], hands: Vec<Vec<Tile>>) -> Wall {
    assert_eq!(t.len(), 69, "wall after the deal");
    let mut pile: Vec<Tile> = (0..55).map(|i| t[68 - i]).collect();
    for j in (0..4).rev() {
        pile.push(t[j]);
    }
    Wall {
        haipai: hands,
        pile,
        dora_indicators: vec![t[8], t[10], t[12], t[4], t[6]],
        ura_indicators: vec![t[9], t[11], t[13], t[5], t[7]],
    }
}

/// mj-core の選択肢の正規形（ハーネスの正規形と同じ。手出し・ツモ切りと、立直の牌は区別しない）
fn canon(a: &Action) -> String {
    match a {
        Action::Dahai { tile, .. } => format!("dahai:{tile}"),
        Action::Reach { .. } => "reach".into(),
        Action::Pon(x, y) | Action::Chi(x, y) => {
            let mut v = [x.to_string(), y.to_string()];
            v.sort();
            let ty = if matches!(a, Action::Pon(..)) {
                "pon"
            } else {
                "chi"
            };
            format!("{ty}:{},{}", v[0], v[1])
        }
        Action::Daiminkan => "daiminkan".into(),
        Action::Ankan(k) => format!("ankan:{}", Tile::plain(*k)),
        Action::Kakan(t) => format!("kakan:{t}"),
        Action::Kita => "kita".into(),
        Action::Tsumo => "tsumo".into(),
        Action::Ron => "ron".into(),
        Action::Kyushu => "kyushu".into(),
        Action::Pass => "pass".into(),
        other => format!("other:{other}"),
    }
}

/// mj-core の行動 → ハーネスへの行動（MJAI に近い形）
fn request(a: &Action) -> Value {
    match a {
        Action::Dahai { tile, tsumogiri } | Action::Reach { tile, tsumogiri } => {
            json!({"type": "dahai", "pai": tile.to_string(), "tsumogiri": tsumogiri})
        }
        Action::Pon(x, y) => json!({"type": "pon", "consumed": [x.to_string(), y.to_string()]}),
        Action::Chi(x, y) => json!({"type": "chi", "consumed": [x.to_string(), y.to_string()]}),
        Action::Daiminkan => json!({"type": "daiminkan"}),
        Action::Ankan(k) => json!({"type": "ankan", "kind": Tile::plain(*k).to_string()}),
        Action::Kakan(t) => json!({"type": "kakan", "pai": t.to_string()}),
        Action::Kita => json!({"type": "kita"}),
        Action::Tsumo => json!({"type": "tsumo"}),
        Action::Ron => json!({"type": "ron"}),
        Action::Kyushu => json!({"type": "kyushu"}),
        _ => json!({"type": "pass"}),
    }
}

/// ハーネスの正規形1つ → ハーネスへの行動（RiichiEnv で選択肢が1つの席を自動で進めるとき）
fn request_of_canon(c: &str, drawn: Option<&str>) -> Value {
    let (ty, arg) = c.split_once(':').unwrap_or((c, ""));
    match ty {
        "dahai" => json!({"type": "dahai", "pai": arg, "tsumogiri": drawn == Some(arg)}),
        "ankan" => json!({"type": "ankan", "kind": arg}),
        "kakan" => json!({"type": "kakan", "pai": arg}),
        "pon" | "chi" => json!({"type": ty, "consumed": arg.split(',').collect::<Vec<_>>()}),
        _ => json!({"type": ty}),
    }
}

/// 局の流れの比較用：（種類, 席, 牌）
fn our_flow(events: &[Event]) -> Vec<Value> {
    let mut out = Vec::new();
    for e in events {
        let v = match e {
            Event::Tsumo { actor, pai, .. } => json!(["tsumo", actor, pai.to_string()]),
            Event::Dahai { actor, pai, .. } => json!(["dahai", actor, pai.to_string()]),
            Event::Reach { actor } => json!(["reach", actor]),
            Event::ReachAccepted { actor, .. } => json!(["reach_accepted", actor]),
            Event::Pon { actor, pai, .. } => json!(["pon", actor, pai.to_string()]),
            Event::Chi { actor, pai, .. } => json!(["chi", actor, pai.to_string()]),
            Event::Daiminkan { actor, pai, .. } => json!(["daiminkan", actor, pai.to_string()]),
            Event::Ankan { actor, consumed } => {
                json!(["ankan", actor, Tile::plain(consumed[0].kind()).to_string()])
            }
            Event::Kakan { actor, pai, .. } => json!(["kakan", actor, pai.to_string()]),
            Event::Kita { actor, .. } => json!(["kita", actor]),
            Event::Dora { dora_indicator } => json!(["dora", dora_indicator.to_string()]),
            Event::Hora(h) => json!(["hora", h.actor, h.target, h.deltas]),
            Event::Ryukyoku(r) => json!(["ryukyoku", r.deltas]),
            _ => continue,
        };
        out.push(v);
    }
    out
}

fn their_flow(events: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for e in events {
        let ty = e["type"].as_str().unwrap_or("");
        let v = match ty {
            "tsumo" | "dahai" | "pon" | "chi" | "kakan" => json!([ty, e["actor"], e["pai"]]),
            "daiminkan" | "kan" => json!(["daiminkan", e["actor"], e["pai"]]),
            "ankan" => {
                let c = e["consumed"][0].as_str().unwrap_or("");
                json!(["ankan", e["actor"], c.trim_end_matches('r')])
            }
            "reach" | "reach_accepted" => json!([ty, e["actor"]]),
            "kita" | "nukidora" => json!(["kita", e["actor"]]),
            "dora" => json!(["dora", e["dora_marker"]]),
            "hora" => json!([
                "hora",
                e["actor"],
                e["target"],
                e.get("deltas").or(e.get("delta"))
            ]),
            "ryukyoku" => json!(["ryukyoku", e.get("deltas").or(e.get("delta"))]),
            _ => continue,
        };
        out.push(v);
    }
    out
}

/// ドラ（めくる位置が両者で違う）を除いた流れ
fn flow_no_dora(f: &[Value]) -> Vec<Value> {
    f.iter().filter(|v| v[0] != "dora").cloned().collect()
}

fn doras(f: &[Value]) -> Vec<Value> {
    f.iter().filter(|v| v[0] == "dora").cloned().collect()
}

fn tail_of(f: &[Value]) -> Vec<Value> {
    f.iter().rev().take(8).rev().cloned().collect()
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct GameOutcome {
    pub excluded: Option<String>,
    pub mismatch: Option<Value>,
    /// 比べた判断ポイント（席ごと）
    pub decisions: u64,
    pub ended_by: Option<String>,
}

pub fn run(h: &mut Harness, rules: &Rules, seed: u64, gid: &str) -> GameOutcome {
    let bot = RuleBot::new(rules);
    let mut out = GameOutcome::default();
    let st = h.ask(json!({"op": "game_new", "game": gid, "seed": seed}));
    if let Some(e) = st.get("error") {
        panic!("harness error: {e}");
    }
    let mut wall: Vec<Tile> = st["wall"]
        .as_array()
        .expect("wall")
        .iter()
        .map(|v| tile_of_id(v.as_u64().expect("id")))
        .collect();
    let drawn_id = st["drawn_id"].as_u64().expect("oya drew");
    wall.push(tile_of_id(drawn_id));
    let oya = st["oya"].as_u64().expect("oya") as u8;
    let hands: Vec<Vec<Tile>> = st["hands"]
        .as_array()
        .expect("hands")
        .iter()
        .enumerate()
        .map(|(s, hv)| {
            let mut ids: Vec<u64> = hv
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            if s == usize::from(oya)
                && let Some(i) = ids.iter().position(|&x| x == drawn_id)
            {
                ids.remove(i);
            }
            let mut t: Vec<Tile> = ids.into_iter().map(tile_of_id).collect();
            t.sort();
            t
        })
        .collect();
    let setup = KyokuSetup {
        bakaze: 0,
        kyoku: 0,
        honba: 0,
        kyotaku: 0,
        oya,
        scores: vec![rules.points.starting; 3],
        wall: our_wall(&wall, hands),
    };
    let mut k = Kyoku::new(Arc::new(rules.clone()), setup);
    let mut their_events: Vec<Value> = st["events"].as_array().cloned().unwrap_or_default();
    let mut state = st;
    let mut guard = 0;
    let fail = |out: &mut GameOutcome, v: Value| {
        if out.mismatch.is_none() {
            out.mismatch = Some(v);
        }
    };
    while out.mismatch.is_none() {
        guard += 1;
        assert!(guard < 2000, "too many steps");
        let seats: BTreeMap<u8, Vec<String>> = state["seats"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(s, v)| {
                        let l: Vec<String> = v
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|x| x.as_str().unwrap().to_string())
                            .collect();
                        (s.parse::<u8>().unwrap(), l)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let their_done = state["done"].as_bool().unwrap_or(false);
        if k.is_done() && their_done {
            break;
        }
        if their_done && !k.is_done() {
            fail(
                &mut out,
                json!({"item": "kyoku_end", "ours_done": false, "riichienv_done": true,
                                  "ours_tail": tail_of(&our_flow(k.events())), "riichienv_tail": tail_of(&their_flow(&their_events))}),
            );
            break;
        }
        let ours = k.decisions().to_vec();
        if ours.is_empty() && k.is_done() && !seats.values().all(|l| l.len() == 1) {
            fail(
                &mut out,
                json!({"item": "kyoku_end", "ours_done": true, "riichienv_done": false, "riichienv_seats": seats,
                                  "ours_tail": tail_of(&our_flow(k.events())), "riichienv_tail": tail_of(&their_flow(&their_events))}),
            );
            break;
        }
        let drawn = state["drawn"].as_str().map(String::from);
        // mj-core は選択肢が1つの場面（立直後のツモ切りなど）を自動で進めるので、RiichiEnv が選択肢1つの席だけを
        // 待っていて、RiichiEnv の流れが mj-core より遅れていれば、RiichiEnv だけを先に進める
        let behind = flow_no_dora(&their_flow(&their_events)).len()
            < flow_no_dora(&our_flow(k.events())).len();
        if !seats.is_empty() && seats.values().all(|l| l.len() == 1) && behind {
            let mut m = serde_json::Map::new();
            for (s, l) in &seats {
                m.insert(s.to_string(), request_of_canon(&l[0], drawn.as_deref()));
            }
            state = h.ask(json!({"op": "game_step", "game": gid, "actions": m}));
            if let Some(e) = state.get("error") {
                fail(
                    &mut out,
                    json!({"item": "riichienv_rejected", "error": e, "actions": m}),
                );
                break;
            }
            their_events.extend(state["events"].as_array().cloned().unwrap_or_default());
            continue;
        }
        let mut acts_theirs = serde_json::Map::new();
        // RiichiEnv で選択肢が1つで、mj-core に判断ポイントがない席は自動で進める
        for (s, l) in &seats {
            if l.len() == 1 && !ours.iter().any(|d| d.seat == *s) {
                acts_theirs.insert(s.to_string(), request_of_canon(&l[0], drawn.as_deref()));
            }
        }
        let their_multi: BTreeSet<u8> = seats
            .iter()
            .filter(|(_, l)| l.len() > 1)
            .map(|(s, _)| *s)
            .collect();
        let our_seats: BTreeSet<u8> = ours.iter().map(|d| d.seat).collect();
        if their_multi.difference(&our_seats).next().is_some() {
            fail(
                &mut out,
                json!({"item": "decision_seats", "ours": our_seats, "riichienv": seats, "flow": our_flow(k.events())}),
            );
            break;
        }
        let mut acts_ours = Vec::new();
        let mut reach_seat: Option<(u8, Action, BTreeSet<String>)> = None;
        for d in &ours {
            out.decisions += 1;
            let mine: BTreeSet<String> = d.choices.iter().map(canon).collect();
            let theirs: BTreeSet<String> = seats
                .get(&d.seat)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();
            // RiichiEnv で選択肢が1つ（自動）でも、mj-core が2つ以上なら食い違い
            if mine != theirs && !(theirs.is_empty() && mine.len() == 1) {
                let tail: Vec<Value> = their_events.iter().rev().take(8).rev().cloned().collect();
                fail(
                    &mut out,
                    json!({"item": "choices", "seat": d.seat, "ours": mine, "riichienv": theirs,
                                      "riichienv_seats": seats, "riichienv_events_tail": tail,
                                      "flow": our_flow(k.events())}),
                );
                break;
            }
            let a = bot.choose(&k, d);
            if let Action::Reach { .. } = a {
                let tiles: BTreeSet<String> = d
                    .choices
                    .iter()
                    .filter_map(|c| match c {
                        Action::Reach { tile, .. } => Some(format!("dahai:{tile}")),
                        _ => None,
                    })
                    .collect();
                reach_seat = Some((d.seat, a, tiles));
                acts_theirs.insert(d.seat.to_string(), json!({"type": "reach"}));
            } else {
                acts_theirs.insert(d.seat.to_string(), request(&a));
            }
            acts_ours.push((d.seat, a));
        }
        if out.mismatch.is_some() {
            break;
        }
        state = h.ask(json!({"op": "game_step", "game": gid, "actions": acts_theirs}));
        if let Some(e) = state.get("error") {
            fail(
                &mut out,
                json!({"item": "riichienv_rejected", "error": e, "actions": acts_theirs,
                                  "flow": our_flow(k.events())}),
            );
            break;
        }
        their_events.extend(state["events"].as_array().cloned().unwrap_or_default());
        if let Some((s, a, tiles)) = reach_seat {
            // 立直の後の打牌の選択肢（立直できる牌）を比べてから、打牌を送る
            let theirs: BTreeSet<String> = state["seats"][s.to_string()]
                .as_array()
                .map(|v| v.iter().map(|x| x.as_str().unwrap().to_string()).collect())
                .unwrap_or_default();
            if theirs != tiles {
                fail(
                    &mut out,
                    json!({"item": "riichi_discards", "seat": s, "ours": tiles, "riichienv": theirs}),
                );
                break;
            }
            let mut m = serde_json::Map::new();
            m.insert(s.to_string(), request(&a));
            state = h.ask(json!({"op": "game_step", "game": gid, "actions": m}));
            if let Some(e) = state.get("error") {
                fail(&mut out, json!({"item": "riichienv_rejected", "error": e}));
                break;
            }
            their_events.extend(state["events"].as_array().cloned().unwrap_or_default());
        }
        if let Err(e) = k.step(&acts_ours) {
            panic!("our engine rejected the bot's action: {e}");
        }
    }
    h.ask(json!({"op": "game_end", "game": gid}));
    // 除外：牌山の作りの違い（上の説明）
    let rinshan = k
        .events()
        .iter()
        .filter(|e| matches!(e, Event::Tsumo { rinshan: true, .. }))
        .count();
    let kan_dora = k
        .events()
        .iter()
        .filter(|e| matches!(e, Event::Dora { .. }))
        .count();
    if rinshan >= 5 || kan_dora >= 3 {
        out.excluded = Some(format!(
            "wall_model (rinshan {rinshan}, kan_dora {kan_dora})"
        ));
        out.mismatch = None;
        return out;
    }
    if out.mismatch.is_none() {
        let mut a = flow_no_dora(&our_flow(k.events()));
        // 検出の確認用（--inject-fault）：こちらの和了の点数の移動を 100 点ずらす
        if crate::INJECT_FAULT.load(std::sync::atomic::Ordering::Relaxed)
            && let Some(h) = a.iter_mut().find(|v| v[0] == "hora")
        {
            h[3][0] = json!(h[3][0].as_i64().unwrap_or(0) + 100);
        }
        let b = flow_no_dora(&their_flow(&their_events));
        if doras(&our_flow(k.events())) != doras(&their_flow(&their_events)) {
            out.mismatch = Some(
                json!({"item": "kan_dora", "ours": doras(&our_flow(k.events())),
                                       "riichienv": doras(&their_flow(&their_events))}),
            );
        } else if a != b {
            let i = a
                .iter()
                .zip(&b)
                .position(|(x, y)| x != y)
                .unwrap_or(a.len().min(b.len()));
            out.mismatch = Some(
                json!({"item": "flow", "at": i, "ours": a.get(i), "riichienv": b.get(i),
                                       "ours_tail": a.get(i.saturating_sub(3)..(i + 3).min(a.len())),
                                       "riichienv_tail": b.get(i.saturating_sub(3)..(i + 3).min(b.len()))}),
            );
        } else if k.scores()
            != state["scores"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .map(|x| x.as_i64().unwrap() as i32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
                .as_slice()
        {
            out.mismatch =
                Some(json!({"item": "scores", "ours": k.scores(), "riichienv": state["scores"]}));
        }
    }
    out.ended_by = k.events().iter().rev().find_map(|e| match e {
        Event::Hora(_) => Some("hora".to_string()),
        Event::Ryukyoku(r) => Some(r.reason.to_string()),
        _ => None,
    });
    out
}
