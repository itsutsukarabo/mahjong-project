//! 役・符・点数。
//!
//! 和了した手（[`WinInput`]）から、点数が最も高くなる取り方（[`WinResult`]）を求める。
//! 比べる順：基本点 → 翻 → 符（同じなら先に見つけた方）。役満があれば役満だけを数える。
//!
//! ルールの項目で分岐するもの：`points.kiriage_mangan`・`kazoe_yakuman`・`min_han`（ドラを除いた翻）・
//! `double_wind_pair_fu`・`rinshan_tsumo_fu`、`calls.kuitan`、`kita.north_as_meld`・`dora_per_kita`、
//! `yakuman.composite`・`double_special`、`dora.*`。

use crate::agari::{self, Decomp, Set};
use crate::rules::{NorthAsMeld, Rules};
use crate::tile::{KindCounts, NUM_KINDS, TileKind};
use crate::wall::dora_from_indicator;

/// 副露（手の外の面子）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSet {
    pub kind: OpenKind,
    /// 刻子・槓子なら牌の種類、順子なら最小の牌
    pub tile: TileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    Chi,
    Pon,
    /// 大明槓・加槓
    Minkan,
    Ankan,
}

impl OpenSet {
    fn is_open(&self) -> bool {
        self.kind != OpenKind::Ankan
    }
    fn is_kan(&self) -> bool {
        matches!(self.kind, OpenKind::Minkan | OpenKind::Ankan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiichiKind {
    None,
    Single,
    Double,
}

/// 和了の情報（局進行から渡す）。
#[derive(Debug, Clone)]
pub struct WinInput<'a> {
    pub rules: &'a Rules,
    /// 手の中の牌（和了牌を含む）
    pub hand: KindCounts,
    pub win_tile: TileKind,
    pub sets: &'a [OpenSet],
    pub tsumo: bool,
    pub riichi: RiichiKind,
    pub ippatsu: bool,
    /// 海底摸月（通常のツモで最後の牌）
    pub haitei: bool,
    /// 河底撈魚
    pub houtei: bool,
    pub rinshan: bool,
    /// 槍槓（北抜きへのロンで `kita.ron_counts_as_chankan` のときも）
    pub chankan: bool,
    /// 第1ツモで鳴き・北抜きがない（天和・地和）
    pub first_draw: bool,
    pub oya: bool,
    pub bakaze: TileKind,
    pub jikaze: TileKind,
    /// めくられているドラ表示牌の種類
    pub dora_indicators: &'a [TileKind],
    /// 裏ドラ表示牌（立直していなければ空）
    pub ura_indicators: &'a [TileKind],
    /// 手・副露の赤牌の枚数
    pub aka: u8,
    /// 抜いた北の枚数
    pub kita: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Yaku {
    Riichi,
    DoubleRiichi,
    Ippatsu,
    MenzenTsumo,
    Pinfu,
    Tanyao,
    Iipeikou,
    YakuhaiHaku,
    YakuhaiHatsu,
    YakuhaiChun,
    YakuhaiBakaze,
    YakuhaiJikaze,
    YakuhaiNorth,
    Haitei,
    Houtei,
    Rinshan,
    Chankan,
    Chiitoitsu,
    Chanta,
    Ittsu,
    Sanshoku,
    SanshokuDoukou,
    Toitoi,
    Sanankou,
    Sankantsu,
    Shousangen,
    Honroutou,
    Honitsu,
    Junchan,
    Ryanpeikou,
    Chinitsu,
    // 役満
    Tenhou,
    Chiihou,
    Kokushi,
    Kokushi13,
    Suuankou,
    SuuankouTanki,
    Daisangen,
    Shousuushii,
    Daisuushii,
    Tsuuiisou,
    Ryuuiisou,
    Chinroutou,
    Chuuren,
    JunseiChuuren,
    Suukantsu,
    // ドラ
    Dora,
    Aka,
    Ura,
    Kita,
}

impl Yaku {
    /// 牌譜の `yaku[].id`。
    pub fn id(self) -> &'static str {
        use Yaku::*;
        match self {
            Riichi => "riichi",
            DoubleRiichi => "double_riichi",
            Ippatsu => "ippatsu",
            MenzenTsumo => "menzen_tsumo",
            Pinfu => "pinfu",
            Tanyao => "tanyao",
            Iipeikou => "iipeikou",
            YakuhaiHaku => "yakuhai_haku",
            YakuhaiHatsu => "yakuhai_hatsu",
            YakuhaiChun => "yakuhai_chun",
            YakuhaiBakaze => "yakuhai_bakaze",
            YakuhaiJikaze => "yakuhai_jikaze",
            YakuhaiNorth => "yakuhai_north",
            Haitei => "haitei",
            Houtei => "houtei",
            Rinshan => "rinshan",
            Chankan => "chankan",
            Chiitoitsu => "chiitoitsu",
            Chanta => "chanta",
            Ittsu => "ittsu",
            Sanshoku => "sanshoku",
            SanshokuDoukou => "sanshoku_doukou",
            Toitoi => "toitoi",
            Sanankou => "sanankou",
            Sankantsu => "sankantsu",
            Shousangen => "shousangen",
            Honroutou => "honroutou",
            Honitsu => "honitsu",
            Junchan => "junchan",
            Ryanpeikou => "ryanpeikou",
            Chinitsu => "chinitsu",
            Tenhou => "tenhou",
            Chiihou => "chiihou",
            Kokushi => "kokushi",
            Kokushi13 => "kokushi_13",
            Suuankou => "suuankou",
            SuuankouTanki => "suuankou_tanki",
            Daisangen => "daisangen",
            Shousuushii => "shousuushii",
            Daisuushii => "daisuushii",
            Tsuuiisou => "tsuuiisou",
            Ryuuiisou => "ryuuiisou",
            Chinroutou => "chinroutou",
            Chuuren => "chuuren",
            JunseiChuuren => "junsei_chuuren",
            Suukantsu => "suukantsu",
            Dora => "dora",
            Aka => "aka",
            Ura => "ura",
            Kita => "kita",
        }
    }

    pub fn is_dora(self) -> bool {
        matches!(self, Yaku::Dora | Yaku::Aka | Yaku::Ura | Yaku::Kita)
    }
}

/// 役1つ：翻（役満なら倍数を `yakuman` に）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YakuItem {
    pub yaku: Yaku,
    pub han: u8,
    /// 役満の倍数（役満でなければ 0）
    pub yakuman: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinResult {
    pub yaku: Vec<YakuItem>,
    /// 合計の翻（役満なら 0）
    pub han: u8,
    pub fu: u8,
    /// 役満の倍数の合計（数え役満は 1）
    pub yakuman: u8,
    /// 数え役満
    pub kazoe: bool,
    /// 基本点（子のロンは ×4、親のロンは ×6 を100点単位に切り上げ）
    pub base: i64,
    /// 点数の区分（満貫・跳満など）。`base` と同じ判定で決める
    pub limit: Limit,
}

/// 点数の区分（牌譜の `hora.limit`、LOG_FORMAT §4）。基本点を決めた判定（`base_and_limit`）がそのまま区分になる
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// 満貫に届かない（翻と符から計算した点数）。役がなく点数が 0 のときもこれ
    None,
    /// 5翻、または 4翻以下で基本点が 2000 以上（切り上げ満貫のルールなら 1920 以上：4翻30符・3翻60符）
    Mangan,
    /// 6〜7翻
    Haneman,
    /// 8〜10翻
    Baiman,
    /// 11〜12翻（数え役満のないルールでは 13翻以上も）
    Sanbaiman,
    /// 役満（倍数は `yakuman`）
    Yakuman,
    /// 13翻以上の数え役満（`points.kazoe_yakuman = true`）
    KazoeYakuman,
}

impl Limit {
    pub fn as_str(self) -> &'static str {
        match self {
            Limit::None => "none",
            Limit::Mangan => "mangan",
            Limit::Haneman => "haneman",
            Limit::Baiman => "baiman",
            Limit::Sanbaiman => "sanbaiman",
            Limit::Yakuman => "yakuman",
            Limit::KazoeYakuman => "kazoe_yakuman",
        }
    }
}

impl WinResult {
    pub fn yaku_han(&self) -> u8 {
        self.yaku
            .iter()
            .filter(|y| !y.yaku.is_dora())
            .map(|y| y.han)
            .sum()
    }
    pub fn count(&self, y: Yaku) -> u8 {
        self.yaku
            .iter()
            .filter(|i| i.yaku == y)
            .map(|i| i.han)
            .sum()
    }
}

pub fn round_up_100(x: i64) -> i64 {
    (x + 99) / 100 * 100
}

/// 翻・符から基本点。
pub fn base_points(rules: &Rules, han: u8, fu: u8, yakuman: u8) -> i64 {
    base_and_limit(rules, han, fu, yakuman).0
}

/// 翻・符から基本点と点数の区分（同じ判定で決める）
pub fn base_and_limit(rules: &Rules, han: u8, fu: u8, yakuman: u8) -> (i64, Limit) {
    if yakuman > 0 {
        return (8000 * i64::from(yakuman), Limit::Yakuman);
    }
    match han {
        13.. => {
            if rules.points.kazoe_yakuman {
                (8000, Limit::KazoeYakuman)
            } else {
                (6000, Limit::Sanbaiman)
            }
        }
        11..=12 => (6000, Limit::Sanbaiman),
        8..=10 => (4000, Limit::Baiman),
        6..=7 => (3000, Limit::Haneman),
        5 => (2000, Limit::Mangan),
        _ => {
            let b = i64::from(fu) << (2 + han);
            if b >= 2000 || (rules.points.kiriage_mangan && b >= 1920) {
                (2000, Limit::Mangan)
            } else {
                (b, Limit::None)
            }
        }
    }
}

const HAKU: usize = 31;
const HATSU: usize = 32;
const CHUN: usize = 33;
const NORTH: usize = 30;

fn is_yaochu(k: usize) -> bool {
    k >= 27 || k.is_multiple_of(9) || k % 9 == 8
}

/// 刻子・槓子・順子をまとめた面子（手の中の面子と副露）
#[derive(Debug, Clone, Copy)]
struct Block {
    /// 順子なら最小の牌
    tile: usize,
    shuntsu: bool,
    kan: bool,
    /// 副露（明刻・明槓・チー）。ロンで完成した刻子も明刻として扱う
    open: bool,
}

/// 待ちの形
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    Ryanmen,
    Kanchan,
    Penchan,
    Shanpon,
    Tanki,
}

struct Ctx<'a, 'b> {
    w: &'b WinInput<'a>,
    closed: bool,
    yakuhai_pair_fu: [u8; NUM_KINDS],
}

impl<'a, 'b> Ctx<'a, 'b> {
    fn new(w: &'b WinInput<'a>) -> Self {
        let closed = w.sets.iter().all(|s| !s.is_open());
        let mut yakuhai_pair_fu = [0u8; NUM_KINDS];
        for k in [HAKU, HATSU, CHUN] {
            yakuhai_pair_fu[k] = 2;
        }
        let b = w.bakaze.index();
        let j = w.jikaze.index();
        if b == j {
            yakuhai_pair_fu[b] = w.rules.points.double_wind_pair_fu;
        } else {
            yakuhai_pair_fu[b] = 2;
            yakuhai_pair_fu[j] = 2;
        }
        if w.rules.kita.north_as_meld == NorthAsMeld::Yakuhai && b != NORTH && j != NORTH {
            yakuhai_pair_fu[NORTH] = 2;
        }
        Self {
            w,
            closed,
            yakuhai_pair_fu,
        }
    }

    /// 手牌・副露・抜き北のすべての牌の種類ごとの枚数（ドラを数える用）
    fn all_counts(&self) -> KindCounts {
        let mut c = self.w.hand;
        for s in self.w.sets {
            let n = if s.is_kan() { 4 } else { 3 };
            if matches!(s.kind, OpenKind::Chi) {
                for d in 0..3 {
                    c[s.tile.index() + d] += 1;
                }
            } else {
                c[s.tile.index()] += n;
            }
        }
        c
    }

    fn dora_items(&self) -> Vec<YakuItem> {
        let rules = self.w.rules;
        let manzu = rules.tiles.manzu;
        let c = self.all_counts();
        let count_for = |inds: &[TileKind]| -> (u8, u8) {
            let mut n = 0u8;
            let mut north = 0u8;
            for ind in inds {
                let d = dora_from_indicator(manzu, *ind).index();
                n += c[d];
                if d == NORTH {
                    north += self.w.kita;
                }
            }
            (n, north)
        };
        let mut out = Vec::new();
        let (dora, dn) = count_for(self.w.dora_indicators);
        let kita = self.w.kita * rules.kita.dora_per_kita;
        if dora + dn > 0 {
            out.push(YakuItem {
                yaku: Yaku::Dora,
                han: dora + dn,
                yakuman: 0,
            });
        }
        if self.w.aka > 0 {
            out.push(YakuItem {
                yaku: Yaku::Aka,
                han: self.w.aka,
                yakuman: 0,
            });
        }
        if kita > 0 {
            out.push(YakuItem {
                yaku: Yaku::Kita,
                han: kita,
                yakuman: 0,
            });
        }
        if self.w.riichi != RiichiKind::None && rules.dora.ura {
            let (ura, un) = count_for(self.w.ura_indicators);
            if ura + un > 0 {
                out.push(YakuItem {
                    yaku: Yaku::Ura,
                    han: ura + un,
                    yakuman: 0,
                });
            }
        }
        out
    }

    /// 状況の役（手の形によらない）
    fn situational(&self) -> Vec<YakuItem> {
        let w = self.w;
        let mut out = Vec::new();
        let mut add = |y, han| {
            out.push(YakuItem {
                yaku: y,
                han,
                yakuman: 0,
            })
        };
        match w.riichi {
            RiichiKind::Single => add(Yaku::Riichi, 1),
            RiichiKind::Double => add(Yaku::DoubleRiichi, 2),
            RiichiKind::None => {}
        }
        if w.ippatsu && w.riichi != RiichiKind::None && w.rules.dora.ippatsu {
            add(Yaku::Ippatsu, 1);
        }
        if w.tsumo && self.closed {
            add(Yaku::MenzenTsumo, 1);
        }
        if w.haitei {
            add(Yaku::Haitei, 1);
        }
        if w.houtei {
            add(Yaku::Houtei, 1);
        }
        if w.rinshan {
            add(Yaku::Rinshan, 1);
        }
        if w.chankan {
            add(Yaku::Chankan, 1);
        }
        out
    }

    fn evaluate(&self) -> Option<WinResult> {
        let w = self.w;
        let mut best: Option<WinResult> = None;
        let min_han = w.rules.points.min_han;
        let mut consider = |r: WinResult| {
            if r.yaku_han() < min_han {
                return;
            }
            let better = match &best {
                None => true,
                Some(b) => (r.base, r.han, r.fu) > (b.base, b.han, b.fu),
            };
            if better {
                best = Some(r);
            }
        };
        // 役満
        let yakuman = self.yakuman_all();
        if !yakuman.is_empty() {
            let items = if w.rules.yakuman.composite {
                yakuman
            } else {
                let top = *yakuman.iter().max_by_key(|y| y.yakuman).expect("non-empty");
                vec![top]
            };
            let total: u8 = items.iter().map(|y| y.yakuman).sum();
            let (base, limit) = base_and_limit(w.rules, 0, 0, total);
            return Some(WinResult {
                base,
                limit,
                yaku: items,
                han: 0,
                fu: 0,
                yakuman: total,
                kazoe: false,
            });
        }
        let situ = self.situational();
        let dora = self.dora_items();
        // 一般形
        if agari::is_standard_agari(&w.hand) {
            for d in agari::decompose(&w.hand) {
                for (blocks, wait, pair) in self.interpretations(&d) {
                    let (mut items, pinfu) = self.form_yaku(&blocks, pair, wait);
                    items.extend(situ.iter().copied());
                    let fu = self.fu(&blocks, pair, wait, pinfu);
                    consider(self.finish(items, &dora, fu));
                }
            }
        }
        // 七対子
        if self.closed && w.sets.is_empty() && agari::is_chiitoi(&w.hand) {
            let mut items = vec![YakuItem {
                yaku: Yaku::Chiitoitsu,
                han: 2,
                yakuman: 0,
            }];
            items.extend(self.flush_terminal_yaku_chiitoi());
            items.extend(situ.iter().copied());
            consider(self.finish(items, &dora, 25));
        }
        best
    }

    fn finish(&self, mut items: Vec<YakuItem>, dora: &[YakuItem], fu: u8) -> WinResult {
        let yaku_han: u8 = items.iter().map(|y| y.han).sum();
        if yaku_han > 0 {
            items.extend(dora.iter().copied());
        }
        items.sort_by_key(|y| y.yaku);
        let han: u8 = items.iter().map(|y| y.han).sum();
        let kazoe = han >= 13 && self.w.rules.points.kazoe_yakuman;
        let (base, limit) = if yaku_han == 0 {
            (0, Limit::None)
        } else {
            base_and_limit(self.w.rules, han, fu, 0)
        };
        WinResult {
            base,
            limit,
            yaku: items,
            han,
            fu,
            yakuman: u8::from(kazoe),
            kazoe,
        }
    }

    /// 分解 × 和了牌の入り方（どの面子・雀頭に和了牌が入ったか）
    fn interpretations(&self, d: &Decomp) -> Vec<(Vec<Block>, Wait, usize)> {
        let w = self.w;
        let win = w.win_tile.index();
        let pair = d.pair.index();
        let mut base: Vec<Block> = d
            .sets
            .iter()
            .map(|s| match *s {
                Set::Shuntsu(k) => Block {
                    tile: k.index(),
                    shuntsu: true,
                    kan: false,
                    open: false,
                },
                Set::Koutsu(k) => Block {
                    tile: k.index(),
                    shuntsu: false,
                    kan: false,
                    open: false,
                },
            })
            .collect();
        let n_closed = base.len();
        for s in w.sets {
            base.push(Block {
                tile: s.tile.index(),
                shuntsu: s.kind == OpenKind::Chi,
                kan: s.is_kan(),
                open: s.is_open(),
            });
        }
        let mut out = Vec::new();
        if pair == win {
            out.push((base.clone(), Wait::Tanki, pair));
        }
        let mut seen = Vec::new();
        for i in 0..n_closed {
            let b = base[i];
            let key = (b.tile, b.shuntsu);
            if seen.contains(&key) {
                continue;
            }
            let wait = if b.shuntsu {
                if win < b.tile || win > b.tile + 2 {
                    continue;
                }
                let pos = win - b.tile;
                let n = b.tile % 9; // 0 始まりの数字
                if pos == 1 {
                    Wait::Kanchan
                } else if (pos == 2 && n == 0) || (pos == 0 && n == 6) {
                    Wait::Penchan
                } else {
                    Wait::Ryanmen
                }
            } else {
                if b.tile != win {
                    continue;
                }
                Wait::Shanpon
            };
            seen.push(key);
            let mut blocks = base.clone();
            if wait == Wait::Shanpon && !w.tsumo {
                blocks[i].open = true; // ロンで完成した刻子は明刻
            }
            out.push((blocks, wait, pair));
        }
        out
    }

    fn is_yakuhai_tile(&self, k: usize) -> bool {
        self.yakuhai_pair_fu[k] > 0
    }

    /// 手の形の役。戻り値の2つ目は平和か。
    fn form_yaku(&self, blocks: &[Block], pair: usize, wait: Wait) -> (Vec<YakuItem>, bool) {
        let w = self.w;
        let closed = self.closed;
        let mut out = Vec::new();
        let mut add = |y, closed_han: u8, open_han: u8| {
            let han = if closed { closed_han } else { open_han };
            if han > 0 {
                out.push(YakuItem {
                    yaku: y,
                    han,
                    yakuman: 0,
                });
            }
        };
        let tiles_of = |b: &Block| -> Vec<usize> {
            if b.shuntsu {
                vec![b.tile, b.tile + 1, b.tile + 2]
            } else {
                vec![b.tile]
            }
        };
        let all_tiles: Vec<usize> = blocks
            .iter()
            .flat_map(tiles_of)
            .chain(std::iter::once(pair))
            .collect();
        // 平和
        let pinfu = closed
            && blocks.iter().all(|b| b.shuntsu)
            && !self.is_yakuhai_tile(pair)
            && wait == Wait::Ryanmen;
        if pinfu {
            add(Yaku::Pinfu, 1, 0);
        }
        // 断么九
        if all_tiles.iter().all(|&k| !is_yaochu(k)) && (closed || w.rules.calls.kuitan) {
            add(Yaku::Tanyao, 1, 1);
        }
        // 一盃口・二盃口
        let mut seqs: Vec<usize> = blocks
            .iter()
            .filter(|b| b.shuntsu)
            .map(|b| b.tile)
            .collect();
        seqs.sort();
        let mut peikou = 0;
        let mut i = 0;
        while i + 1 < seqs.len() {
            if seqs[i] == seqs[i + 1] {
                peikou += 1;
                i += 2;
            } else {
                i += 1;
            }
        }
        if closed {
            if peikou == 2 {
                add(Yaku::Ryanpeikou, 3, 0);
            } else if peikou == 1 {
                add(Yaku::Iipeikou, 1, 0);
            }
        }
        // 役牌
        for b in blocks.iter().filter(|b| !b.shuntsu) {
            let k = b.tile;
            match k {
                HAKU => add(Yaku::YakuhaiHaku, 1, 1),
                HATSU => add(Yaku::YakuhaiHatsu, 1, 1),
                CHUN => add(Yaku::YakuhaiChun, 1, 1),
                _ => {}
            }
            if k == w.bakaze.index() {
                add(Yaku::YakuhaiBakaze, 1, 1);
            }
            if k == w.jikaze.index() {
                add(Yaku::YakuhaiJikaze, 1, 1);
            }
            if k == NORTH
                && w.rules.kita.north_as_meld == NorthAsMeld::Yakuhai
                && w.bakaze.index() != NORTH
                && w.jikaze.index() != NORTH
            {
                add(Yaku::YakuhaiNorth, 1, 1);
            }
        }
        // 混全帯么九・純全帯么九
        let block_has_yaochu = |b: &Block| tiles_of(b).iter().any(|&k| is_yaochu(k));
        let has_honor = all_tiles.iter().any(|&k| k >= 27);
        let all_have_yaochu = blocks.iter().all(block_has_yaochu) && is_yaochu(pair);
        let any_seq = blocks.iter().any(|b| b.shuntsu);
        if all_have_yaochu && any_seq {
            if has_honor {
                add(Yaku::Chanta, 2, 1);
            } else {
                add(Yaku::Junchan, 3, 2);
            }
        }
        // 一気通貫
        for s in 0..3 {
            let base = s * 9;
            if [0, 3, 6].iter().all(|o| seqs.contains(&(base + o))) {
                add(Yaku::Ittsu, 2, 1);
            }
        }
        // 三色同順・三色同刻
        for n in 0..9 {
            if n <= 6 && (0..3).all(|s| seqs.contains(&(s * 9 + n))) {
                add(Yaku::Sanshoku, 2, 1);
            }
            if (0..3).all(|s| blocks.iter().any(|b| !b.shuntsu && b.tile == s * 9 + n)) {
                add(Yaku::SanshokuDoukou, 2, 2);
            }
        }
        // 対々和・三暗刻・三槓子
        if blocks.iter().all(|b| !b.shuntsu) {
            add(Yaku::Toitoi, 2, 2);
        }
        let ankou = blocks.iter().filter(|b| !b.shuntsu && !b.open).count();
        if ankou == 3 {
            add(Yaku::Sanankou, 2, 2);
        }
        if blocks.iter().filter(|b| b.kan).count() == 3 {
            add(Yaku::Sankantsu, 2, 2);
        }
        // 小三元
        let dragons = blocks
            .iter()
            .filter(|b| !b.shuntsu && (HAKU..=CHUN).contains(&b.tile))
            .count();
        if dragons == 2 && (HAKU..=CHUN).contains(&pair) {
            add(Yaku::Shousangen, 2, 2);
        }
        // 混老頭
        if all_tiles.iter().all(|&k| is_yaochu(k)) && has_honor && !any_seq {
            add(Yaku::Honroutou, 2, 2);
        }
        // 混一色・清一色
        if single_suit(&all_tiles).is_some() {
            if has_honor {
                add(Yaku::Honitsu, 3, 2);
            } else {
                add(Yaku::Chinitsu, 6, 5);
            }
        }
        (out, pinfu)
    }

    /// 七対子と複合する役（断么九・混老頭・混一色・清一色）
    fn flush_terminal_yaku_chiitoi(&self) -> Vec<YakuItem> {
        let tiles: Vec<usize> = (0..NUM_KINDS).filter(|&k| self.w.hand[k] > 0).collect();
        let mut out = Vec::new();
        let mut add = |y, han| {
            out.push(YakuItem {
                yaku: y,
                han,
                yakuman: 0,
            })
        };
        if tiles.iter().all(|&k| !is_yaochu(k)) {
            add(Yaku::Tanyao, 1);
        }
        let has_honor = tiles.iter().any(|&k| k >= 27);
        if tiles.iter().all(|&k| is_yaochu(k)) && has_honor {
            add(Yaku::Honroutou, 2);
        }
        if single_suit(&tiles).is_some() {
            if has_honor {
                add(Yaku::Honitsu, 3);
            } else {
                add(Yaku::Chinitsu, 6);
            }
        }
        out
    }

    fn fu(&self, blocks: &[Block], pair: usize, wait: Wait, pinfu: bool) -> u8 {
        let w = self.w;
        if pinfu {
            return if w.tsumo { 20 } else { 30 };
        }
        let mut fu: u32 = 20;
        if self.closed && !w.tsumo {
            fu += 10;
        }
        if w.tsumo && (!w.rinshan || w.rules.points.rinshan_tsumo_fu) {
            fu += 2;
        }
        for b in blocks.iter().filter(|b| !b.shuntsu) {
            let mut f = if is_yaochu(b.tile) { 4 } else { 2 };
            if !b.open {
                f *= 2;
            }
            if b.kan {
                f *= 4;
            }
            fu += f;
        }
        fu += u32::from(self.yakuhai_pair_fu[pair]);
        if matches!(wait, Wait::Kanchan | Wait::Penchan | Wait::Tanki) {
            fu += 2;
        }
        if !self.closed && fu == 20 {
            fu = 30; // 喰い平和形のロン
        }
        (fu.div_ceil(10) * 10) as u8
    }

    /// すべての役満（取り方によらないものと、分解ごとに決まるものの最大）
    fn yakuman_all(&self) -> Vec<YakuItem> {
        let w = self.w;
        let dbl = w.rules.yakuman.double_special;
        let mut found: Vec<YakuItem> = Vec::new();
        let ym = |y, n: u8| YakuItem {
            yaku: y,
            han: 0,
            yakuman: n,
        };
        if w.first_draw && w.tsumo && w.sets.is_empty() {
            found.push(ym(if w.oya { Yaku::Tenhou } else { Yaku::Chiihou }, 1));
        }
        let c = self.all_counts();
        let kinds: Vec<usize> = (0..NUM_KINDS).filter(|&k| c[k] > 0).collect();
        // 国士無双
        if self.closed && w.sets.is_empty() && agari::is_kokushi(&w.hand) {
            let thirteen = w.hand[w.win_tile.index()] == 2;
            found.push(if thirteen {
                ym(Yaku::Kokushi13, if dbl { 2 } else { 1 })
            } else {
                ym(Yaku::Kokushi, 1)
            });
            return found;
        }
        if !agari::is_standard_agari(&w.hand) {
            // 七対子形（字一色だけがありうる）
            if kinds.iter().all(|&k| k >= 27) {
                found.push(ym(Yaku::Tsuuiisou, 1));
            }
            return found;
        }
        // 形によらない役満
        if kinds.iter().all(|&k| k >= 27) {
            found.push(ym(Yaku::Tsuuiisou, 1));
        }
        if kinds
            .iter()
            .all(|&k| k < 27 && (k.is_multiple_of(9) || k % 9 == 8))
        {
            found.push(ym(Yaku::Chinroutou, 1));
        }
        const GREEN: [usize; 6] = [19, 20, 21, 23, 25, HATSU]; // 2s 3s 4s 6s 8s 發
        if kinds.iter().all(|k| GREEN.contains(k)) {
            found.push(ym(Yaku::Ryuuiisou, 1));
        }
        if w.sets.iter().filter(|s| s.is_kan()).count() == 4 {
            found.push(ym(Yaku::Suukantsu, 1));
        }
        let trip = |k: usize| {
            w.sets
                .iter()
                .any(|s| s.kind != OpenKind::Chi && s.tile.index() == k)
                || w.hand[k] >= 3
        };
        if (HAKU..=CHUN).all(trip) {
            found.push(ym(Yaku::Daisangen, 1));
        }
        let winds = (27..31).filter(|&k| trip(k)).count();
        let wind_pair = (27..31).any(|k| !trip(k) && w.hand[k] == 2);
        if winds == 4 {
            found.push(ym(Yaku::Daisuushii, if dbl { 2 } else { 1 }));
        } else if winds == 3 && wind_pair {
            found.push(ym(Yaku::Shousuushii, 1));
        }
        // 九蓮宝燈（門前・副露なし・1色）
        if w.sets.is_empty()
            && self.closed
            && let Some(s) = single_suit(&kinds).filter(|_| kinds.iter().all(|&k| k < 27))
        {
            let base = s * 9;
            let need = [3, 1, 1, 1, 1, 1, 1, 1, 3];
            if (0..9).all(|n| w.hand[base + n] >= need[n]) {
                let mut before = w.hand;
                before[w.win_tile.index()] -= 1;
                let junsei = (0..9).all(|n| before[base + n] == need[n]);
                found.push(if junsei {
                    ym(Yaku::JunseiChuuren, if dbl { 2 } else { 1 })
                } else {
                    ym(Yaku::Chuuren, 1)
                });
            }
        }
        // 四暗刻（分解ごと）
        let mut suuankou: Option<YakuItem> = None;
        for d in agari::decompose(&w.hand) {
            for (blocks, wait, _) in self.interpretations(&d) {
                let ankou = blocks.iter().filter(|b| !b.shuntsu && !b.open).count();
                if ankou == 4 {
                    let y = if wait == Wait::Tanki {
                        ym(Yaku::SuuankouTanki, if dbl { 2 } else { 1 })
                    } else {
                        ym(Yaku::Suuankou, 1)
                    };
                    if suuankou.is_none_or(|s| y.yakuman > s.yakuman) {
                        suuankou = Some(y);
                    }
                }
            }
        }
        found.extend(suuankou);
        found
    }
}

/// 数牌が1色だけなら、その色（0..3）。字牌は無視する。
fn single_suit(tiles: &[usize]) -> Option<usize> {
    let mut suit = None;
    for &k in tiles.iter().filter(|&&k| k < 27) {
        let s = k / 9;
        match suit {
            None => suit = Some(s),
            Some(t) if t != s => return None,
            _ => {}
        }
    }
    suit
}

/// 和了の評価。役がない（`points.min_han` に届かない）、または和了形でなければ `None`。
pub fn evaluate(w: &WinInput) -> Option<WinResult> {
    if !agari::is_agari(
        &w.hand,
        w.sets.iter().all(|s| !s.is_open()) && w.sets.is_empty(),
    ) {
        return None;
    }
    Ctx::new(w).evaluate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::presets;
    use crate::tile::{Tile, kind_counts};

    fn k(s: &str) -> TileKind {
        s.parse::<Tile>().unwrap().kind()
    }

    /// テスト用の和了：手牌（和了牌を含む）、和了牌、副露、状況
    struct W {
        rules: Rules,
        hand: KindCounts,
        win: TileKind,
        sets: Vec<OpenSet>,
        tsumo: bool,
        riichi: RiichiKind,
        rinshan: bool,
        oya: bool,
        bakaze: TileKind,
        jikaze: TileKind,
        dora: Vec<TileKind>,
        ura: Vec<TileKind>,
        kita: u8,
    }

    impl W {
        fn new(rules: Rules, hand: &str, win: &str) -> Self {
            let tiles: Vec<Tile> = hand
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect();
            Self {
                rules,
                hand: kind_counts(&tiles),
                win: k(win),
                sets: Vec::new(),
                tsumo: false,
                riichi: RiichiKind::None,
                rinshan: false,
                oya: false,
                bakaze: k("E"),
                jikaze: k("S"),
                dora: Vec::new(),
                ura: Vec::new(),
                kita: 0,
            }
        }

        fn set(mut self, kind: OpenKind, t: &str) -> Self {
            self.sets.push(OpenSet { kind, tile: k(t) });
            self
        }

        fn eval(&self) -> Option<WinResult> {
            evaluate(&WinInput {
                rules: &self.rules,
                hand: self.hand,
                win_tile: self.win,
                sets: &self.sets,
                tsumo: self.tsumo,
                riichi: self.riichi,
                ippatsu: false,
                haitei: false,
                houtei: false,
                rinshan: self.rinshan,
                chankan: false,
                first_draw: false,
                oya: self.oya,
                bakaze: self.bakaze,
                jikaze: self.jikaze,
                dora_indicators: &self.dora,
                ura_indicators: &self.ura,
                aka: 0,
                kita: self.kita,
            })
        }
    }

    fn sanma() -> Rules {
        presets::tenhou_sanma()
    }

    fn with(o: &str) -> Rules {
        presets::with(&sanma(), o)
    }

    #[test]
    fn kiriage_mangan_true_and_false() {
        // 4翻30符：立直・ドラ3、白の雀頭（役牌なので平和にならない）、両面の門前ロン
        let mut w = W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 7s 8s 9s P P", "9s");
        w.riichi = RiichiKind::Single;
        w.dora = vec![k("6s"), k("1p"), k("2s")]; // 7s・2p・3s
        let r = w.eval().unwrap();
        assert_eq!((r.han, r.fu, r.base), (4, 40, 2000), "{r:?}");
        // 白の雀頭 2符で 40符になるので、雀頭を役牌でない牌にして 30符にする
        let mut w = W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 7s 8s 9s 9p 9p", "8s");
        w.riichi = RiichiKind::Single;
        w.dora = vec![k("6s"), k("1p"), k("2s")];
        let r = w.eval().unwrap();
        // 嵌張 2 符 → 32 → 40。両面にする
        assert_eq!(r.fu, 40, "{r:?}");
        let mut w = W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 7s 8s 9s 9p 9p", "9s");
        w.riichi = RiichiKind::Single;
        w.dora = vec![k("6s"), k("1p")]; // 7s・2p → 立直・平和・ドラ2 = 4翻30符
        let r = w.eval().unwrap();
        assert_eq!((r.han, r.fu, r.base), (4, 30, 1920), "{r:?}");
        assert_eq!(r.limit, Limit::None);
        w.rules = with("[points]\nkiriage_mangan = true");
        let r = w.eval().unwrap();
        assert_eq!((r.base, r.limit), (2000, Limit::Mangan));
    }

    #[test]
    fn limit_follows_base_points() {
        // (翻, 符, 役満, 切り上げ満貫, 数え役満) → (基本点, 区分)
        let cases: &[(u8, u8, u8, bool, bool, i64, Limit)] = &[
            (1, 30, 0, false, true, 240, Limit::None),
            (3, 60, 0, false, true, 1920, Limit::None),
            (3, 60, 0, true, true, 2000, Limit::Mangan),
            (3, 70, 0, false, true, 2000, Limit::Mangan),
            (4, 30, 0, false, true, 1920, Limit::None),
            (4, 30, 0, true, true, 2000, Limit::Mangan),
            (4, 25, 0, true, true, 1600, Limit::None),
            (4, 40, 0, false, true, 2000, Limit::Mangan),
            (5, 30, 0, false, true, 2000, Limit::Mangan),
            (6, 30, 0, false, true, 3000, Limit::Haneman),
            (7, 30, 0, false, true, 3000, Limit::Haneman),
            (8, 30, 0, false, true, 4000, Limit::Baiman),
            (10, 30, 0, false, true, 4000, Limit::Baiman),
            (11, 30, 0, false, true, 6000, Limit::Sanbaiman),
            (12, 30, 0, false, true, 6000, Limit::Sanbaiman),
            (13, 30, 0, false, true, 8000, Limit::KazoeYakuman),
            (13, 30, 0, false, false, 6000, Limit::Sanbaiman),
            (0, 0, 1, false, true, 8000, Limit::Yakuman),
            (0, 0, 2, false, true, 16000, Limit::Yakuman),
        ];
        for &(han, fu, yakuman, kiriage, kazoe, base, limit) in cases {
            let r = with(&format!(
                "[points]\nkiriage_mangan = {kiriage}\nkazoe_yakuman = {kazoe}"
            ));
            assert_eq!(
                base_and_limit(&r, han, fu, yakuman),
                (base, limit),
                "{han}翻{fu}符 役満{yakuman} 切り上げ{kiriage} 数え{kazoe}"
            );
            assert_eq!(base_points(&r, han, fu, yakuman), base);
        }
    }

    #[test]
    fn kazoe_yakuman_true_and_false() {
        // 清一色・立直・ドラで13翻以上
        let mut w = W::new(sanma(), "1p 1p 1p 2p 3p 4p 5p 6p 7p 8p 8p 8p 9p 9p", "9p");
        w.riichi = RiichiKind::Single;
        w.dora = vec![k("7p"), k("7p")]; // 8p ×3 ×2 = 6
        let r = w.eval().unwrap();
        assert!(r.han >= 13, "{r:?}");
        assert_eq!(
            (r.base, r.kazoe, r.yakuman, r.limit),
            (8000, true, 1, Limit::KazoeYakuman)
        );
        w.rules = with("[points]\nkazoe_yakuman = false");
        let r = w.eval().unwrap();
        assert_eq!(
            (r.base, r.kazoe, r.yakuman, r.limit),
            (6000, false, 0, Limit::Sanbaiman)
        );
    }

    #[test]
    fn min_han_excludes_dora() {
        // 役は断么九の1翻だけ（副露あり）、ドラ3
        let mut w =
            W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 5p 5p", "5s").set(OpenKind::Pon, "6s");
        w.dora = vec![k("4p"), k("4p"), k("4p")];
        assert!(w.eval().is_some());
        w.rules = with("[points]\nmin_han = 2");
        assert!(w.eval().is_none(), "ドラは飜縛りに数えない");
    }

    #[test]
    fn double_wind_pair_fu_values() {
        // 東場の東家（親）が東の雀頭（連風牌）、九萬の暗刻、両面の門前ロン
        // 4符：20 + 門前ロン 10 + 么九暗刻 8 + 連風 4 = 42 → 50符
        // 2符：20 + 10 + 8 + 2 = 40符
        let mut w = W::new(sanma(), "9m 9m 9m 4p 5p 6p 7p 8p 9p 2s 3s 4s E E", "4s");
        w.oya = true;
        w.jikaze = k("E");
        w.riichi = RiichiKind::Single; // 役は立直
        assert_eq!(w.eval().unwrap().fu, 50);
        w.rules = with("[points]\ndouble_wind_pair_fu = 2");
        assert_eq!(w.eval().unwrap().fu, 40);
    }

    #[test]
    fn rinshan_tsumo_fu_true_and_false() {
        // 明槓 5p（8符）・明刻 4s（2符）、両面の嶺上開花
        // ツモ符あり：20 + 2 + 8 + 2 = 32 → 40符、なし：30符
        let mut w = W::new(sanma(), "2s 3s 4s 6s 7s 8s 3p 3p", "8s")
            .set(OpenKind::Minkan, "5p")
            .set(OpenKind::Pon, "4p");
        w.tsumo = true;
        w.rinshan = true;
        w.dora = vec![k("2p")]; // 役は嶺上開花、ドラは 3p
        let r = w.eval().unwrap();
        assert_eq!(r.fu, 40, "{r:?}");
        w.rules = with("[points]\nrinshan_tsumo_fu = false");
        assert_eq!(w.eval().unwrap().fu, 30);
    }

    #[test]
    fn kuitan_true_and_false() {
        let mut w =
            W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 5p 5p", "5s").set(OpenKind::Pon, "6s");
        assert!(
            w.eval()
                .unwrap()
                .yaku
                .iter()
                .any(|y| y.yaku == Yaku::Tanyao)
        );
        w.rules = with("[calls]\nkuitan = false");
        assert!(w.eval().is_none(), "喰いタンなしでは役がない");
    }

    #[test]
    fn north_as_meld_otakaze_and_yakuhai() {
        // 北の刻子（三麻の席は東南西なので北は自風・場風にならない）。副露して門前ツモ・平和を消す
        let mut w = W::new(sanma(), "2p 3p 4p 3s 4s 5s N N N 9s 9s", "5s").set(OpenKind::Pon, "1p");
        let r = w.eval();
        assert!(r.is_none(), "オタ風の北には役がない: {r:?}");
        w.rules = with("[kita]\nnorth_as_meld = 'yakuhai'");
        let r = w.eval().unwrap();
        assert!(r.yaku.iter().any(|y| y.yaku == Yaku::YakuhaiNorth));
    }

    #[test]
    fn dora_per_kita_values() {
        let mut w = W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 6s 7s 8s 5p 5p", "8s");
        w.kita = 2;
        assert_eq!(w.eval().unwrap().count(Yaku::Kita), 2);
        w.rules = with("[kita]\ndora_per_kita = 0");
        assert_eq!(w.eval().unwrap().count(Yaku::Kita), 0);
        w.rules = with("[kita]\ndora_per_kita = 2");
        assert_eq!(w.eval().unwrap().count(Yaku::Kita), 4);
        // 表示牌が西なら北はドラ：抜いた北もその分数える
        w.rules = sanma();
        w.dora = vec![k("W")];
        assert_eq!(w.eval().unwrap().count(Yaku::Dora), 2);
    }

    #[test]
    fn yakuman_composite_and_double_special() {
        // 字一色・大三元・四暗刻単騎（ツモ）
        let mut w = W::new(sanma(), "P P P F F F C C C E E E S S", "S");
        w.tsumo = true;
        let r = w.eval().unwrap();
        // 天鳳三麻：複合あり・ダブル役満なし → 字一色1 + 大三元1 + 四暗刻単騎1 = 3
        assert_eq!(r.yakuman, 3, "{r:?}");
        w.rules = with("[yakuman]\ndouble_special = true");
        assert_eq!(w.eval().unwrap().yakuman, 4);
        w.rules = with("[yakuman]\ncomposite = false");
        assert_eq!(w.eval().unwrap().yakuman, 1);
    }

    #[test]
    fn dora_ura_flag() {
        let mut w = W::new(sanma(), "2p 3p 4p 6p 7p 8p 3s 4s 5s 6s 7s 8s 5p 5p", "8s");
        w.riichi = RiichiKind::Single;
        w.ura = vec![k("4p")];
        assert_eq!(w.eval().unwrap().count(Yaku::Ura), 2);
        w.rules = with("[dora]\nura = false");
        assert_eq!(w.eval().unwrap().count(Yaku::Ura), 0);
    }
}
