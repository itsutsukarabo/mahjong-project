//! 四麻の局単位の差分テスト（majiang-core の `Majiang.Dev.Game`）。
//!
//! 1. mj-core でボット（`mj-bots`）同士の四麻の半荘を回し、牌譜（イベント列）を得る
//! 2. 牌譜を majiang-core の牌譜形式（`qipai`・`zimo`・`dapai`・`fulou`・`gang`・`gangzimo`・`kaigang`・
//!    `hule`・`pingju`）に変換し、`Majiang.Dev.Game`（牌譜の打牌・副露を再生して対局をやり直すクラス）に渡す
//! 3. majiang-core が作り直した牌譜と、こちらの牌譜を突き合わせる：
//!    - 進行：各局の `qipai`（場・局・本場・供託・各家の持ち点）と打牌・副露・ツモの並び。majiang-core が
//!      こちらの行動を不正として受け付けなければ、ここで食い違う
//!    - カンドラ：めくった表示牌の並び（めくる位置は両者で違うので、並びだけ比べる）
//!    - 局の結果：和了者ごとの点数の移動（`fenpei`）と得点（`defen`）、流局の種類と点数の移動
//!    - 終局：最終の持ち点（供託の精算後）、順位、精算（ポイント）
//!
//! ルールは `fixtures/rules/majiang-yonma-game.toml`（`test-yonma.toml` を majiang-core に合わせた上書き）。
//! 延長戦の親の連荘は2通り（`end_if_reached` ↔ majiang-core 延長戦方式 1、`continue` ↔ 延長戦方式 2）を半分ずつ回す。

use mj_bots::RuleBot;
use mj_core::event::{Event, Ryukyoku};
use mj_core::game::{Game, WallSource};
use mj_core::rules::{Rules, presets};
use mj_core::tile::Tile;
use serde_json::{Value, json};

pub const OVERRIDES: &str = include_str!("../../../fixtures/rules/majiang-yonma-game.toml");

/// 比べる組み合わせ：（名前, こちらのルールの上書き, majiang-core の rule の上書き）
pub fn variants() -> Vec<(&'static str, Rules, Value)> {
    let base = presets::with(&presets::test_yonma(), OVERRIDES);
    vec![
        ("end_if_reached", base.clone(), json!({"延長戦方式": 1})),
        (
            "continue",
            presets::with(&base, "[table]\nextension_renchan = 'continue'"),
            json!({"延長戦方式": 2}),
        ),
    ]
}

/// MJAI 表記 → majiang-core の牌（`m1`…、赤は `m0`、字牌は `z1`〜`z7`）
fn mj(t: Tile) -> String {
    let s = t.to_string();
    let honors = ["E", "S", "W", "N", "P", "F", "C"];
    if let Some(i) = honors.iter().position(|h| *h == s) {
        return format!("z{}", i + 1);
    }
    let b = s.as_bytes();
    let suit = b[1] as char;
    let n = if t.is_red() { '0' } else { b[0] as char };
    format!("{suit}{n}")
}

fn digit(t: Tile) -> char {
    mj(t).chars().nth(1).expect("digit")
}

fn suit(t: Tile) -> char {
    mj(t).chars().next().expect("suit")
}

fn num(t: Tile) -> u8 {
    t.kind().num().unwrap_or(0)
}

/// 手牌の文字列（`m123p406z11` の形。並びは問わない。Dev.Game は数字を読むだけ）
fn hand_string(tiles: &[Tile]) -> String {
    let mut out = String::new();
    for s in ['m', 'p', 's', 'z'] {
        let ds: String = tiles
            .iter()
            .filter(|t| suit(**t) == s)
            .map(|t| digit(*t))
            .collect();
        if !ds.is_empty() {
            out.push(s);
            out.push_str(&ds);
        }
    }
    out
}

/// 副露の向き（majiang-core：`+` 下家・`=` 対面・`-` 上家から）
fn dir(discarder_l: usize, caller_l: usize) -> char {
    ['_', '+', '=', '-'][(4 + discarder_l - caller_l) % 4]
}

/// 刻子・槓子の手から出す牌の数字（5 は赤を後に：`550`、それ以外は同じ数字）
fn same_digits(consumed: &[Tile]) -> String {
    let mut v: Vec<char> = consumed.iter().map(|t| digit(*t)).collect();
    v.sort_by_key(|c| if *c == '0' { 1 } else { 0 });
    v.into_iter().collect()
}

/// こちらの1局のイベント列 → majiang-core の牌譜の1局分（`l` は局の親から数えた席）
pub fn kyoku_log(events: &[Event]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut oya = 0usize;
    let l = |seat: u8, oya: usize| (usize::from(seat) + 4 - oya) % 4;
    let mut pending_reach: Option<u8> = None;
    // 席ごとのポンの文字列（加槓で使う）
    let mut pons: Vec<Vec<(Tile, String)>> = vec![Vec::new(); 4];
    let mut last_dahai_actor = 0u8;
    for e in events {
        match e {
            Event::KyokuStart(s) => {
                oya = usize::from(s.oya);
                let wind = ["E", "S", "W", "N"]
                    .iter()
                    .position(|w| *w == s.bakaze.to_string())
                    .expect("bakaze");
                let by_l = |i: usize| (oya + i) % 4;
                out.push(json!({"qipai": {
                    "zhuangfeng": wind,
                    "jushu": s.kyoku - 1,
                    "changbang": s.honba,
                    "lizhibang": s.kyotaku,
                    "defen": (0..4).map(|i| s.scores[by_l(i)]).collect::<Vec<_>>(),
                    "baopai": mj(s.dora_indicator),
                    "shoupai": (0..4).map(|i| hand_string(&s.tehais[by_l(i)])).collect::<Vec<_>>(),
                }}));
            }
            Event::Tsumo {
                actor,
                pai,
                rinshan,
            } => {
                let key = if *rinshan { "gangzimo" } else { "zimo" };
                out.push(json!({key: {"l": l(*actor, oya), "p": mj(*pai)}}));
            }
            Event::Reach { actor } => pending_reach = Some(*actor),
            Event::Dahai {
                actor,
                pai,
                tsumogiri,
            } => {
                let mut p = mj(*pai);
                if *tsumogiri {
                    p.push('_');
                }
                if pending_reach.take() == Some(*actor) {
                    p.push('*');
                }
                last_dahai_actor = *actor;
                out.push(json!({"dapai": {"l": l(*actor, oya), "p": p}}));
            }
            Event::Pon {
                actor,
                target,
                pai,
                consumed,
            } => {
                let d = dir(l(*target, oya), l(*actor, oya));
                let m = format!("{}{}{}{d}", suit(*pai), same_digits(consumed), digit(*pai));
                pons[usize::from(*actor)].push((*pai, m.clone()));
                out.push(json!({"fulou": {"l": l(*actor, oya), "m": m}}));
            }
            Event::Daiminkan {
                actor,
                target,
                pai,
                consumed,
            } => {
                let d = dir(l(*target, oya), l(*actor, oya));
                let m = format!("{}{}{}{d}", suit(*pai), same_digits(consumed), digit(*pai));
                out.push(json!({"fulou": {"l": l(*actor, oya), "m": m}}));
            }
            Event::Chi {
                actor,
                target,
                pai,
                consumed,
            } => {
                let d = dir(l(*target, oya), l(*actor, oya));
                let mut c = consumed.clone();
                c.sort_by_key(|t| num(*t));
                let (a, b) = (c[0], c[1]);
                let n = num(*pai);
                let p = format!("{}{d}", digit(*pai));
                let body = if n < num(a) {
                    format!("{p}{}{}", digit(a), digit(b))
                } else if n < num(b) {
                    format!("{}{p}{}", digit(a), digit(b))
                } else {
                    format!("{}{}{p}", digit(a), digit(b))
                };
                out.push(
                    json!({"fulou": {"l": l(*actor, oya), "m": format!("{}{body}", suit(*pai))}}),
                );
            }
            Event::Ankan { actor, consumed } => {
                let m = format!("{}{}", suit(consumed[0]), same_digits(consumed));
                out.push(json!({"gang": {"l": l(*actor, oya), "m": m}}));
            }
            Event::Kakan { actor, pai, .. } => {
                let pon = pons[usize::from(*actor)]
                    .iter()
                    .find(|(t, _)| t.kind() == pai.kind())
                    .map(|(_, m)| m.clone())
                    .expect("kakan needs a pon");
                out.push(
                    json!({"gang": {"l": l(*actor, oya), "m": format!("{pon}{}", digit(*pai))}}),
                );
            }
            Event::Dora { dora_indicator } => {
                out.push(json!({"kaigang": {"baopai": mj(*dora_indicator)}}));
            }
            Event::Hora(h) => {
                let ura: Option<Vec<String>> = if h.ura_indicators.is_empty() {
                    None
                } else {
                    Some(h.ura_indicators.iter().map(|t| mj(*t)).collect())
                };
                let fenpei: Vec<i32> = (0..4).map(|i| h.deltas[(oya + i) % 4]).collect();
                out.push(json!({"hule": {"l": l(h.actor, oya), "fubaopai": ura,
                                         "defen": h.points, "fenpei": fenpei}}));
            }
            Event::Ryukyoku(r) => out.push(pingju(r, oya, last_dahai_actor)),
            _ => {}
        }
    }
    out
}

fn pingju_name(reason: &str) -> &'static str {
    match reason {
        "exhaustive" => "荒牌平局",
        "nagashi_mangan" => "流し満貫",
        "kyushu_kyuhai" => "九種九牌",
        "all_riichi" => "四家立直",
        "same_wind_discards" => "四風連打",
        "suukaikan" => "四開槓",
        "all_ron" => "三家和",
        other => panic!("unknown ryukyoku reason {other}"),
    }
}

fn pingju(r: &Ryukyoku, oya: usize, last_dahai_actor: u8) -> Value {
    let name = pingju_name(r.reason);
    let by_l = |i: usize| (oya + i) % 4;
    // Dev.Game が返事を作るのに使う手牌：九種九牌は宣言者、三家和は放銃者以外の3人
    let shoupai: Vec<String> = (0..4)
        .map(|i| {
            let s = by_l(i) as u8;
            let show = match r.reason {
                "kyushu_kyuhai" => r.actor == Some(s),
                "all_ron" => s != last_dahai_actor,
                _ => false,
            };
            if show {
                hand_string(&r.hands[usize::from(s)])
            } else {
                String::new()
            }
        })
        .collect();
    let fenpei: Vec<i32> = (0..4).map(|i| r.deltas[by_l(i)]).collect();
    // 荒牌平局・流し満貫は majiang-core が自分で判定するので名前を空で渡す（返事は作らない）
    let script_name = match r.reason {
        "exhaustive" | "nagashi_mangan" => "",
        _ => name,
    };
    json!({"pingju": {"name": script_name, "shoupai": shoupai, "fenpei": fenpei, "_name": name}})
}

/// ボット同士の1半荘。（majiang-core の Dev.Game に渡す台本, こちらの結果）
pub fn play(rules: &Rules, seed: u64) -> (Value, Value) {
    let bot = RuleBot::new(rules);
    let mut g = Game::new(rules.clone(), WallSource::Seed(seed));
    while !g.is_over() {
        let k = g.current_kyoku();
        let acts: Vec<_> = g
            .decisions()
            .iter()
            .map(|d| (d.seat, bot.choose(k, d)))
            .collect();
        g.step(&acts).expect("legal");
    }
    let logs: Vec<Vec<Value>> = g.kyokus().map(kyoku_log).collect();
    let end = g.end().expect("game over");
    let script = json!({
        "title": format!("mj-core seed {seed}"),
        "player": ["0", "1", "2", "3"],
        "qijia": 0,
        "log": logs,
    });
    let ours = json!({
        "log": logs,
        "defen": end.scores,
        "rank": end.ranks,
        // majiang-core の point は千点単位・小数1桁の文字列（順位点 '20.0' など小数の表記のとき）
        "point": end.settlement.iter().map(|x| format!("{:.1}", f64::from(*x) / 1000.0)).collect::<Vec<_>>(),
    });
    (script, ours)
}

/// 1局分の比較用の形：進行（kaigang を除く）、カンドラの並び、結果
fn split(log: &[Value]) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut flow = Vec::new();
    let mut dora = Vec::new();
    let mut result = Vec::new();
    for e in log {
        let (k, v) = e.as_object().and_then(|o| o.iter().next()).expect("entry");
        match k.as_str() {
            "kaigang" => dora.push(v["baopai"].clone()),
            "hule" => result
                .push(json!({"hule": {"l": v["l"], "defen": v["defen"], "fenpei": v["fenpei"]}})),
            "pingju" => {
                let name = v.get("_name").unwrap_or(&v["name"]).clone();
                result.push(json!({"pingju": {"name": name, "fenpei": v["fenpei"]}}));
            }
            // 配牌は牌山から作り直すので同じになる。文字列の並び（majiang-core は整列する）だけが違うので比べない
            "qipai" => {
                let mut q = v.clone();
                q.as_object_mut().expect("qipai").remove("shoupai");
                flow.push(json!({"qipai": q}));
            }
            _ => flow.push(e.clone()),
        }
    }
    (flow, dora, result)
}

/// 比べた結果。最初に食い違った局と項目。
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct GameVerdict {
    pub kyokus_ours: usize,
    pub kyokus_theirs: usize,
    pub kyokus_matched: usize,
    pub first_mismatch: Option<Value>,
}

pub fn judge(ours: &Value, theirs: &Value) -> GameVerdict {
    // 検出の確認用（--inject-fault）：こちらの最初の和了の点数の移動を 100 点ずらす
    let mut ours = ours.clone();
    if crate::INJECT_FAULT.load(std::sync::atomic::Ordering::Relaxed)
        && let Some(f) = ours["log"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .flat_map(|k| k.as_array_mut().into_iter().flatten())
            .find_map(|e| e.get_mut("hule"))
    {
        f["fenpei"][0] = json!(f["fenpei"][0].as_i64().unwrap_or(0) + 100);
    }
    let ours = &ours;
    let a = ours["log"].as_array().expect("log");
    let b = theirs["log"].as_array().expect("log");
    let mut v = GameVerdict {
        kyokus_ours: a.len(),
        kyokus_theirs: b.len(),
        ..Default::default()
    };
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let (fa, da, ra) = split(x.as_array().expect("kyoku"));
        let (fb, db, rb) = split(y.as_array().expect("kyoku"));
        let what = if fa != fb {
            let j = fa
                .iter()
                .zip(&fb)
                .position(|(p, q)| p != q)
                .unwrap_or(fa.len().min(fb.len()));
            Some(json!({"kyoku_idx": i, "item": "flow", "at": j,
                        "ours": fa.get(j), "majiang": fb.get(j)}))
        } else if da != db {
            Some(json!({"kyoku_idx": i, "item": "kan_dora", "ours": da, "majiang": db}))
        } else if ra != rb {
            Some(json!({"kyoku_idx": i, "item": "result", "ours": ra, "majiang": rb}))
        } else {
            None
        };
        if let Some(w) = what {
            v.first_mismatch = Some(w);
            return v;
        }
        v.kyokus_matched += 1;
    }
    if theirs["wanted_more_kyoku"] == json!(true) {
        v.first_mismatch = Some(json!({"item": "kyoku_count", "ours": a.len(),
                                       "majiang": "対局を続ける（次の局を始めようとした）",
                                       "last_qipai_majiang": b.last().and_then(|k| k.get(0))}));
        return v;
    }
    if a.len() != b.len() {
        v.first_mismatch =
            Some(json!({"item": "kyoku_count", "ours": a.len(), "majiang": b.len()}));
        return v;
    }
    for key in ["defen", "rank", "point"] {
        if ours[key] != theirs[key] {
            v.first_mismatch =
                Some(json!({"item": key, "ours": ours[key], "majiang": theirs[key]}));
            return v;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use mj_core::tile::Tile;

    fn t(s: &str) -> Tile {
        s.parse().unwrap()
    }

    #[test]
    fn tile_and_meld_strings() {
        assert_eq!(mj(t("5mr")), "m0");
        assert_eq!(mj(t("C")), "z7");
        assert_eq!(same_digits(&[t("5pr"), t("5p")]), "50");
        // 席0 が席3（上家）の 5p をポン（赤を1枚使う）
        assert_eq!(dir(3, 0), '-');
        assert_eq!(dir(1, 0), '+');
    }
}
