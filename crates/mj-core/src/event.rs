//! 局のイベント（牌譜の1行に当たる、整形前のデータ）。
//!
//! JSON への整形は `mj-log` が行う（CLAUDE.md：ログの整形はコアの外）。
//! フィールドの意味は `docs/spec/LOG_FORMAT.md` §4。

use crate::tile::Tile;
use crate::yaku::YakuItem;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    KyokuStart(Box<KyokuStart>),
    Tsumo {
        actor: u8,
        pai: Tile,
        rinshan: bool,
    },
    Dahai {
        actor: u8,
        pai: Tile,
        tsumogiri: bool,
    },
    Reach {
        actor: u8,
    },
    ReachAccepted {
        actor: u8,
        scores: Vec<i32>,
        kyotaku: u32,
    },
    Chi {
        actor: u8,
        target: u8,
        pai: Tile,
        consumed: Vec<Tile>,
    },
    Pon {
        actor: u8,
        target: u8,
        pai: Tile,
        consumed: Vec<Tile>,
    },
    Daiminkan {
        actor: u8,
        target: u8,
        pai: Tile,
        consumed: Vec<Tile>,
    },
    Ankan {
        actor: u8,
        consumed: Vec<Tile>,
    },
    Kakan {
        actor: u8,
        pai: Tile,
        consumed: Vec<Tile>,
    },
    Kita {
        actor: u8,
        pai: Tile,
    },
    Dora {
        dora_indicator: Tile,
    },
    Hora(Box<Hora>),
    Ryukyoku(Box<Ryukyoku>),
    KyokuEnd(Box<KyokuEnd>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct KyokuStart {
    /// 場風（`E`/`S`/`W`/`N`）
    pub bakaze: Tile,
    /// その場の何局目か（1始まり）
    pub kyoku: u8,
    pub honba: u32,
    pub kyotaku: u32,
    pub oya: u8,
    pub scores: Vec<i32>,
    pub dora_indicator: Tile,
    pub tehais: Vec<Vec<Tile>>,
    pub pile: Vec<Tile>,
    pub dora_indicators: Vec<Tile>,
    pub ura_indicators: Vec<Tile>,
}

/// 副露の表示（`hora.hand.melds`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeldView {
    /// `chi` / `pon` / `daiminkan` / `kakan` / `ankan`
    pub kind: &'static str,
    pub pai: Option<Tile>,
    pub consumed: Vec<Tile>,
    pub target: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hora {
    pub actor: u8,
    pub target: u8,
    pub pai: Tile,
    /// 和了牌を含まない門前の牌（整列済み）
    pub tehai: Vec<Tile>,
    pub melds: Vec<MeldView>,
    pub yaku: Vec<YakuItem>,
    pub han: u8,
    pub fu: u8,
    pub yakuman: u8,
    /// 和了の点数（本場・供託を除いた、和了者が受け取る手の値）
    pub points: i64,
    pub ura_indicators: Vec<Tile>,
    /// 包（責任払い）の人
    pub pao: Option<u8>,
    pub deltas: Vec<i32>,
    pub scores: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ryukyoku {
    /// `exhaustive` / `kyushu_kyuhai` / `suukaikan` / `all_riichi` / `same_wind_discards` / `all_ron` / `nagashi_mangan`
    pub reason: &'static str,
    /// 九種九牌の宣言者（九種九牌のときだけ）
    pub actor: Option<u8>,
    pub tenpai: Vec<bool>,
    /// 全員の門前の手牌（公開範囲は mj-log の redact が決める）
    pub hands: Vec<Vec<Tile>>,
    pub deltas: Vec<i32>,
    pub scores: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipItem {
    pub seat: u8,
    /// `ippatsu` / `aka` / `ura` / `yakuman`
    pub kind: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KyokuEnd {
    /// `hora` / `ryukyoku`
    pub reason: &'static str,
    pub scores: Vec<i32>,
    pub kyotaku: u32,
    pub honba_next: u32,
    pub renchan: bool,
    pub chip_deltas: Vec<i32>,
    pub chip_items: Vec<ChipItem>,
}
