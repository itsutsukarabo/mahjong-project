//! 局のイベント列から「珍しい局面」を数える（差分テスト・ボット対局で、どの局面が実際に比べられたかの記録）。

use mj_core::event::Event;
use mj_core::yaku::Yaku;

/// 数える局面の名前（表の行の順）
pub const RARE: &[&str] = &[
    "suukaikan",
    "kyushu_kyuhai",
    "pao",
    "pao_ron",
    "pao_compound",
    "double_ron",
    "ron_on_kita",
    "ron_on_kita_ippatsu",
    "chankan",
    "rinshan_kaihou",
    "rinshan_kaihou_after_kita",
    "kan_dora_on_kita",
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
    let mut prev_non_dora: Option<&Event> = None;
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
            // 北抜きの後、補充のツモの前にカンドラがめくられた（D-040）
            Event::Dora { .. } if matches!(prev, Some(Event::Kita { .. })) => {
                out.push("kan_dora_on_kita");
            }
            Event::Tsumo { rinshan, .. } => {
                // 北抜きと補充のツモの間にカンドラがめくられることがある（D-040）ので、ドラ表示を除いた1つ前を見る
                prev_tsumo_after_kita =
                    *rinshan && matches!(prev_non_dora, Some(Event::Kita { .. }));
            }
            Event::Hora(h) => {
                let has = |y: Yaku| h.yaku.iter().any(|i| i.yaku == y);
                if h.pao.is_some() {
                    out.push("pao");
                    if h.actor != h.target {
                        out.push("pao_ron");
                    }
                    // 複合役満（役満の役が2つ以上、または2倍役満）
                    if h.yakuman >= 2 {
                        out.push("pao_compound");
                    }
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
        if !matches!(e, Event::Dora { .. }) {
            prev_non_dora = Some(e);
        }
        if !matches!(e, Event::Hora(_)) {
            last_non_hora = Some(e);
        }
    }
    out
}

/// 1局の席ごとの打ち方の統計（02_RL_DESIGN「記録する統計」。数え方は RL_TRAINING §5.1）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeatStats {
    /// 局の数
    pub kyokus: u32,
    /// 和了した局（ダブロンの両者はそれぞれ1）
    pub hora: u32,
    /// そのうちツモ和了
    pub tsumo: u32,
    /// 和了の点数（`hora.points`：本場・供託を除いた手の値）の合計
    pub hora_points: i64,
    /// 放銃した局（ロン・槍槓・北抜きへのロンの放銃者。ダブロンでも1局に1）
    pub deal_in: u32,
    /// 立直を宣言した局
    pub riichi: u32,
    /// 副露（チー・ポン・大明槓）した局。暗槓・北抜きは数えない
    pub call: u32,
    /// 荒牌流局（`exhaustive` と `nagashi_mangan`）の局
    pub exhaustive: u32,
    /// 荒牌流局のうち聴牌だった局
    pub exhaustive_tenpai: u32,
    /// 局収支の合計（局の終わりの持ち点 − 局の始めの持ち点。立直棒・本場・供託を含む）
    pub score_delta: i64,
    /// 和了の精算（`hora.deltas`）の合計：和了者は＋（本場・供託を含む）、放銃者・ツモの支払いは −。
    /// 立直棒の供託・ノーテン罰符・流し満貫は含まない（①-b の報酬、RL_TRAINING §2.3）
    pub hora_delta: i64,
}

impl SeatStats {
    pub fn add(&mut self, o: &SeatStats) {
        self.kyokus += o.kyokus;
        self.hora += o.hora;
        self.tsumo += o.tsumo;
        self.hora_points += o.hora_points;
        self.deal_in += o.deal_in;
        self.riichi += o.riichi;
        self.call += o.call;
        self.exhaustive += o.exhaustive;
        self.exhaustive_tenpai += o.exhaustive_tenpai;
        self.score_delta += o.score_delta;
        self.hora_delta += o.hora_delta;
    }
}

/// 1局のイベント列から席ごとの統計を数える（`n` は人数）
pub fn kyoku_seat_stats(events: &[Event], n: u8) -> Vec<SeatStats> {
    let n = usize::from(n);
    let mut s = vec![
        SeatStats {
            kyokus: 1,
            ..Default::default()
        };
        n
    ];
    let mut dealt_in = vec![false; n];
    let mut riichi = vec![false; n];
    let mut call = vec![false; n];
    let mut start: Option<&[i32]> = None;
    for e in events {
        match e {
            Event::KyokuStart(k) => start = Some(&k.scores),
            Event::KyokuEnd(k) => {
                if let Some(st) = start {
                    for (p, x) in s.iter_mut().enumerate() {
                        x.score_delta += i64::from(k.scores[p] - st[p]);
                    }
                }
            }
            Event::Reach { actor } => riichi[usize::from(*actor)] = true,
            Event::Chi { actor, .. }
            | Event::Pon { actor, .. }
            | Event::Daiminkan { actor, .. } => call[usize::from(*actor)] = true,
            Event::Hora(h) => {
                let a = usize::from(h.actor);
                s[a].hora += 1;
                s[a].hora_points += h.points;
                for (p, d) in h.deltas.iter().enumerate() {
                    s[p].hora_delta += i64::from(*d);
                }
                if h.actor == h.target {
                    s[a].tsumo += 1;
                } else {
                    dealt_in[usize::from(h.target)] = true;
                }
            }
            Event::Ryukyoku(r) if matches!(r.reason, "exhaustive" | "nagashi_mangan") => {
                for (p, st) in s.iter_mut().enumerate() {
                    st.exhaustive += 1;
                    if r.tenpai.get(p).copied().unwrap_or(false) {
                        st.exhaustive_tenpai += 1;
                    }
                }
            }
            _ => {}
        }
    }
    for p in 0..n {
        s[p].deal_in = u32::from(dealt_in[p]);
        s[p].riichi = u32::from(riichi[p]);
        s[p].call = u32::from(call[p]);
    }
    s
}

/// 対局のすべての局の席ごとの統計の和
pub fn game_seat_stats<'a>(kyokus: impl IntoIterator<Item = &'a [Event]>, n: u8) -> Vec<SeatStats> {
    let mut total = vec![SeatStats::default(); usize::from(n)];
    for k in kyokus {
        for (t, s) in total.iter_mut().zip(kyoku_seat_stats(k, n)) {
            t.add(&s);
        }
    }
    total
}
