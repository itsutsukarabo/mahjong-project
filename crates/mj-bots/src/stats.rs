//! 局のイベント列から「珍しい局面」を数える（差分テスト・ボット対局で、どの局面が実際に比べられたかの記録）。

use mj_core::event::Event;
use mj_core::yaku::Yaku;

/// 数える局面の名前（表の行の順）
pub const RARE: &[&str] = &[
    "suukaikan",
    "kyushu_kyuhai",
    "pao",
    "double_ron",
    "ron_on_kita",
    "ron_on_kita_ippatsu",
    "chankan",
    "rinshan_kaihou",
    "rinshan_kaihou_after_kita",
    "nagashi_mangan",
    "double_riichi",
    "tenhou",
    "chiihou",
    "kazoe_yakuman",
];

/// 1局のイベント列に出た珍しい局面（同じ局面が1局に2回あれば2つ）
pub fn rare_events(events: &[Event]) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut prev: Option<&Event> = None;
    let mut prev_tsumo_after_kita = false;
    let mut ron_targets: Vec<u8> = Vec::new();
    let mut last_non_hora: Option<&Event> = None;
    for e in events {
        match e {
            Event::Ryukyoku(r) => match r.reason {
                "suukaikan" => out.push("suukaikan"),
                "kyushu_kyuhai" => out.push("kyushu_kyuhai"),
                "nagashi_mangan" => out.push("nagashi_mangan"),
                _ => {}
            },
            Event::Tsumo { rinshan, .. } => {
                prev_tsumo_after_kita = *rinshan && matches!(prev, Some(Event::Kita { .. }));
            }
            Event::Hora(h) => {
                let has = |y: Yaku| h.yaku.iter().any(|i| i.yaku == y);
                if h.pao.is_some() {
                    out.push("pao");
                }
                if h.actor != h.target {
                    if ron_targets.contains(&h.target) {
                        out.push("double_ron");
                    }
                    ron_targets.push(h.target);
                    // 北抜きへのロン：和了の前の最後のイベント（同時の和了を除く）が北抜き
                    let after_kita = matches!(last_non_hora, Some(Event::Kita { .. }));
                    if after_kita {
                        out.push("ron_on_kita");
                        if has(Yaku::Ippatsu) {
                            out.push("ron_on_kita_ippatsu");
                        }
                    }
                }
                if has(Yaku::Chankan) {
                    out.push("chankan");
                }
                if has(Yaku::Rinshan) {
                    out.push("rinshan_kaihou");
                    if prev_tsumo_after_kita {
                        out.push("rinshan_kaihou_after_kita");
                    }
                }
                if has(Yaku::DoubleRiichi) {
                    out.push("double_riichi");
                }
                if has(Yaku::Tenhou) {
                    out.push("tenhou");
                }
                if has(Yaku::Chiihou) {
                    out.push("chiihou");
                }
                if h.yakuman > 0 && h.yaku.iter().all(|i| i.yakuman == 0) {
                    out.push("kazoe_yakuman");
                }
            }
            _ => {}
        }
        prev = Some(e);
        if !matches!(e, Event::Hora(_)) {
            last_non_hora = Some(e);
        }
    }
    out
}
