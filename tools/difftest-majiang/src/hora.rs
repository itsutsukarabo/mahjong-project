//! 和了・点数の差分テスト（四麻、`Majiang.Util.hule`）。
//!
//! 和了形を組み（使う牌の種類を偏らせて役を出しやすくする）、状況役・ドラ・赤をランダムに付け、
//! mj-core の [`mj_core::yaku::evaluate`] と majiang-core の役・翻・符・役満・得点（本場・供託なし）を比べる。
//! ルールは `fixtures/rules/test-yonma.toml`（majiang-core の既定のルールに合わせてある）。

use mj_core::rules::{Rules, presets};
use mj_core::tile::{KindCounts, NUM_KINDS, Tile, TileKind};
use mj_core::wall::{tile_multiset, uniform_below};
use mj_core::yaku::{self, OpenKind, OpenSet, RiichiKind, WinInput, round_up_100};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct HoraCase {
    pub case_id: String,
    pub palette: &'static str,
    /// majiang-core に渡す入力
    pub shoupai: String,
    pub rongpai: Option<String>,
    pub param: Value,
    #[serde(skip)]
    pub ours: Ours,
}

#[derive(Debug, Clone)]
pub struct Ours {
    pub hand: KindCounts,
    pub win: Option<TileKind>,
    pub sets: Vec<OpenSet>,
    pub aka: u8,
    pub tsumo: bool,
    pub riichi: u8,
    pub ippatsu: bool,
    pub haitei: bool,
    pub houtei: bool,
    pub rinshan: bool,
    pub chankan: bool,
    pub tianhu: bool,
    pub zhuangfeng: u8,
    pub menfeng: u8,
    pub dora: Vec<TileKind>,
    pub ura: Vec<TileKind>,
}

#[derive(Debug, Deserialize)]
pub struct HoraAnswer {
    pub case_id: String,
    pub hule: Option<Value>,
    pub error: Option<String>,
}

/// 比べる値：役（id, 翻, 役満の倍数）の並び、翻、符、役満、得点。和了にならなければ `None`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Score {
    pub yaku: Vec<(String, u8, u8)>,
    pub han: u8,
    pub fu: u8,
    pub yakuman: u8,
    pub defen: i64,
}

fn mj_letter(k: usize) -> (char, usize) {
    match k {
        0..=8 => ('m', k + 1),
        9..=17 => ('p', k - 8),
        18..=26 => ('s', k - 17),
        _ => ('z', k - 26),
    }
}

fn mj_digit(t: Tile) -> char {
    let (_, n) = mj_letter(t.kind().index());
    if t.is_red() {
        '0'
    } else {
        char::from(b'0' + n as u8)
    }
}

fn mj_tile(t: Tile) -> String {
    let (l, _) = mj_letter(t.kind().index());
    format!("{l}{}", mj_digit(t))
}

/// 門前の牌の並び（色ごと）。`last` があれば最後に別に付ける（ツモ牌）。
fn mj_concealed(tiles: &[Tile], last: Option<Tile>) -> String {
    let mut s = String::new();
    for letter in ['m', 'p', 's', 'z'] {
        let mut part: Vec<Tile> = tiles
            .iter()
            .copied()
            .filter(|t| mj_letter(t.kind().index()).0 == letter)
            .collect();
        part.sort();
        if !part.is_empty() {
            s.push(letter);
            for t in part {
                s.push(mj_digit(t));
            }
        }
    }
    if let Some(t) = last {
        s.push_str(&mj_tile(t));
    }
    s
}

const PALETTES: &[&str] = &[
    "all",
    "all",
    "all",
    "one_suit_honors",
    "one_suit",
    "honors",
    "green",
    "terminals",
    "tanyao",
    "dragons",
    "winds",
    "terminal_seq",
];

fn palette_kinds(p: &str, suit: usize) -> Vec<usize> {
    match p {
        "one_suit_honors" => (suit * 9..suit * 9 + 9).chain(27..34).collect(),
        "one_suit" => (suit * 9..suit * 9 + 9).collect(),
        "honors" => (27..34).collect(),
        "green" => vec![19, 20, 21, 23, 25, 32],
        "terminals" => vec![0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33],
        "tanyao" => (0..27)
            .filter(|&k: &usize| !k.is_multiple_of(9) && k % 9 != 8)
            .collect(),
        "dragons" => (0..34).filter(|&k| k >= 31 || k % 3 == 0).collect(),
        "winds" => (27..31).chain(suit * 9..suit * 9 + 9).collect(),
        "terminal_seq" => (0..27).filter(|k| ![3, 4, 5].contains(&(k % 9))).collect(),
        _ => (0..34).collect(),
    }
}

pub struct HoraGen {
    pub rng: ChaCha8Rng,
    pool: Vec<Tile>,
}

impl HoraGen {
    pub fn new(rng: ChaCha8Rng) -> Self {
        Self {
            rng,
            pool: Vec::new(),
        }
    }

    fn below(&mut self, n: usize) -> usize {
        uniform_below(&mut self.rng, n as u32) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn left(&self, k: usize) -> usize {
        self.pool.iter().filter(|t| t.kind().index() == k).count()
    }

    /// 種類 k の牌を1枚取る（赤があればランダムに）
    fn take(&mut self, k: usize) -> Tile {
        let idx: Vec<usize> = (0..self.pool.len())
            .filter(|&i| self.pool[i].kind().index() == k)
            .collect();
        let i = idx[self.below(idx.len())];
        self.pool.swap_remove(i)
    }

    fn take_any(&mut self) -> Tile {
        let i = self.below(self.pool.len());
        self.pool.swap_remove(i)
    }

    pub fn case(&mut self, i: usize, seed: u64, rules: &Rules) -> HoraCase {
        self.pool = tile_multiset(&rules.tiles);
        let id = format!("h{seed}-{i:07}");
        let palette = PALETTES[self.below(PALETTES.len())];
        let suit = self.below(3);
        let kinds = palette_kinds(palette, suit);
        let mut o = Ours {
            hand: [0; NUM_KINDS],
            win: None,
            sets: Vec::new(),
            aka: 0,
            tsumo: false,
            riichi: 0,
            ippatsu: false,
            haitei: false,
            houtei: false,
            rinshan: false,
            chankan: false,
            tianhu: false,
            zhuangfeng: self.below(4) as u8,
            menfeng: self.below(4) as u8,
            dora: Vec::new(),
            ura: Vec::new(),
        };
        let mut concealed: Vec<Tile> = Vec::new();
        let mut melds_str: Vec<String> = Vec::new();
        let form = self.below(100);
        if form < 12 {
            // 七対子
            let mut pairs = 0;
            let mut guard = 0;
            while pairs < 7 && guard < 1000 {
                guard += 1;
                let k = kinds[self.below(kinds.len())];
                if self.left(k) >= 2 && !concealed.iter().any(|t| t.kind().index() == k) {
                    concealed.push(self.take(k));
                    concealed.push(self.take(k));
                    pairs += 1;
                }
            }
            if pairs < 7 {
                return self.case(i, seed ^ 0x9e37, rules);
            }
        } else if form < 20 && palette == "one_suit" {
            // 九蓮宝燈形：1112345678999 ＋ 同じ色の1枚
            let need = [3, 1, 1, 1, 1, 1, 1, 1, 3];
            for (n, &c) in need.iter().enumerate() {
                for _ in 0..c {
                    concealed.push(self.take(suit * 9 + n));
                }
            }
            let extra = suit * 9 + self.below(9);
            if self.left(extra) == 0 {
                return self.case(i, seed ^ 0x3c, rules);
            }
            concealed.push(self.take(extra));
        } else if form < 17 {
            // 国士無双
            for k in TileKind::all().filter(|k| k.is_terminal_or_honor()) {
                concealed.push(self.take(k.index()));
            }
            let yao: Vec<usize> = TileKind::all()
                .filter(|k| k.is_terminal_or_honor())
                .map(|k| k.index())
                .collect();
            let k = yao[self.below(yao.len())];
            concealed.push(self.take(k));
        } else {
            // 一般形：雀頭＋4面子、うちいくつかを副露
            let n_open = [0, 0, 0, 1, 1, 2, 3, 4][self.below(8)];
            let mut guard = 0;
            loop {
                guard += 1;
                if guard > 1000 {
                    return self.case(i, seed ^ 0x51, rules);
                }
                let k = kinds[self.below(kinds.len())];
                if self.left(k) >= 2 {
                    concealed.push(self.take(k));
                    concealed.push(self.take(k));
                    break;
                }
            }
            let mut sets = 0;
            while sets < 4 {
                guard += 1;
                if guard > 3000 {
                    return self.case(i, seed ^ 0x77, rules);
                }
                let k = kinds[self.below(kinds.len())];
                let open = sets < n_open;
                let shuntsu = k < 27
                    && k % 9 <= 6
                    && kinds.contains(&(k + 1))
                    && kinds.contains(&(k + 2))
                    && self.chance(55);
                if shuntsu {
                    if self.left(k) == 0 || self.left(k + 1) == 0 || self.left(k + 2) == 0 {
                        continue;
                    }
                    let ts = [self.take(k), self.take(k + 1), self.take(k + 2)];
                    if open {
                        let called = self.below(3);
                        let (l, _) = mj_letter(k);
                        let mut s = format!("{l}");
                        for (j, t) in ts.iter().enumerate() {
                            s.push(mj_digit(*t));
                            if j == called {
                                s.push('-');
                            }
                        }
                        melds_str.push(s);
                        o.sets.push(OpenSet {
                            kind: OpenKind::Chi,
                            tile: TileKind::new(k as u8).unwrap(),
                        });
                        o.aka += ts.iter().filter(|t| t.is_red()).count() as u8;
                    } else {
                        concealed.extend(ts);
                    }
                } else {
                    let kan = open && self.chance(30) || !open && self.chance(6);
                    let need = if kan { 4 } else { 3 };
                    if self.left(k) < need {
                        continue;
                    }
                    let ts: Vec<Tile> = (0..need).map(|_| self.take(k)).collect();
                    let (l, _) = mj_letter(k);
                    let digits: String = ts.iter().map(|t| mj_digit(*t)).collect();
                    let mark = ["+", "=", "-"][self.below(3)];
                    if open && kan {
                        melds_str.push(format!("{l}{digits}{mark}"));
                        o.sets.push(OpenSet {
                            kind: OpenKind::Minkan,
                            tile: TileKind::new(k as u8).unwrap(),
                        });
                    } else if open {
                        melds_str.push(format!("{l}{digits}{mark}"));
                        o.sets.push(OpenSet {
                            kind: OpenKind::Pon,
                            tile: TileKind::new(k as u8).unwrap(),
                        });
                    } else if kan {
                        melds_str.push(format!("{l}{digits}"));
                        o.sets.push(OpenSet {
                            kind: OpenKind::Ankan,
                            tile: TileKind::new(k as u8).unwrap(),
                        });
                    } else {
                        concealed.extend(ts.iter().copied());
                    }
                    if open || kan {
                        o.aka += ts.iter().filter(|t| t.is_red()).count() as u8;
                    }
                }
                sets += 1;
            }
        }
        // 和了牌は門前の牌から選ぶ
        let wi = self.below(concealed.len());
        let win = concealed.remove(wi);
        o.win = Some(win.kind());
        let closed = o.sets.iter().all(|s| s.kind == OpenKind::Ankan);
        let has_kan = o
            .sets
            .iter()
            .any(|s| matches!(s.kind, OpenKind::Minkan | OpenKind::Ankan));
        o.tsumo = self.chance(50);
        if closed && self.chance(55) {
            o.riichi = if self.chance(12) { 2 } else { 1 };
            o.ippatsu = self.chance(30);
        }
        o.rinshan = o.tsumo && has_kan && self.chance(25);
        if !o.rinshan && self.chance(6) {
            if o.tsumo {
                o.haitei = true;
            } else {
                o.houtei = true;
            }
        }
        o.chankan = !o.tsumo && !o.houtei && self.chance(5);
        o.tianhu = o.tsumo && o.sets.is_empty() && o.riichi == 0 && !o.haitei && self.chance(3);
        for t in concealed.iter().chain(std::iter::once(&win)) {
            o.aka += u8::from(t.is_red());
            o.hand[t.kind().index()] += 1;
        }
        let n_dora = 1 + self.below(5);
        let dora: Vec<Tile> = (0..n_dora).map(|_| self.take_any()).collect();
        let ura: Vec<Tile> = if o.riichi > 0 {
            (0..n_dora).map(|_| self.take_any()).collect()
        } else {
            Vec::new()
        };
        o.dora = dora.iter().map(|t| t.kind()).collect();
        o.ura = ura.iter().map(|t| t.kind()).collect();

        let mut shoupai = if o.tsumo {
            mj_concealed(&concealed, Some(win))
        } else {
            mj_concealed(&concealed, None)
        };
        for m in &melds_str {
            shoupai.push(',');
            shoupai.push_str(m);
        }
        let rongpai =
            (!o.tsumo).then(|| format!("{}{}", mj_tile(win), ["+", "=", "-"][self.below(3)]));
        let param = json!({
            "zhuangfeng": o.zhuangfeng, "menfeng": o.menfeng,
            "lizhi": o.riichi, "yifa": o.ippatsu, "qianggang": o.chankan, "lingshang": o.rinshan,
            "haidi": if o.haitei { 1 } else if o.houtei { 2 } else { 0 },
            "tianhu": if o.tianhu { if o.menfeng == 0 { 1 } else { 2 } } else { 0 },
            "baopai": dora.iter().map(|t| mj_tile(*t)).collect::<Vec<_>>(),
            "fubaopai": if o.riichi > 0 { Value::from(ura.iter().map(|t| mj_tile(*t)).collect::<Vec<_>>()) } else { Value::Null },
        });
        HoraCase {
            case_id: id,
            palette,
            shoupai,
            rongpai,
            param,
            ours: o,
        }
    }
}

/// 状況役・ドラを外した、より単純なケース（最小化に使う）。
pub fn simpler(c: &HoraCase) -> Vec<HoraCase> {
    let mut out = Vec::new();
    let mut push = |f: &dyn Fn(&mut HoraCase)| {
        let mut d = c.clone();
        f(&mut d);
        d.case_id = format!("{}-m", c.case_id);
        if d.param != c.param || d.rongpai != c.rongpai {
            out.push(d);
        }
    };
    push(&|d| {
        d.param["yifa"] = json!(false);
        d.ours.ippatsu = false;
    });
    push(&|d| {
        d.param["lizhi"] = json!(0);
        d.param["yifa"] = json!(false);
        d.param["fubaopai"] = Value::Null;
        d.ours.riichi = 0;
        d.ours.ippatsu = false;
        d.ours.ura.clear();
    });
    push(&|d| {
        d.param["haidi"] = json!(0);
        d.ours.haitei = false;
        d.ours.houtei = false;
    });
    push(&|d| {
        d.param["lingshang"] = json!(false);
        d.ours.rinshan = false;
    });
    push(&|d| {
        d.param["qianggang"] = json!(false);
        d.ours.chankan = false;
    });
    push(&|d| {
        d.param["tianhu"] = json!(0);
        d.ours.tianhu = false;
    });
    push(&|d| {
        if d.ours.dora.len() > 1 {
            d.ours.dora.truncate(1);
            let b = d.param["baopai"].as_array().unwrap()[..1].to_vec();
            d.param["baopai"] = Value::from(b);
        }
    });
    out
}

pub fn ours_score(rules: &Rules, c: &HoraCase) -> Option<Score> {
    let o = &c.ours;
    let wind = |i: u8| TileKind::new(27 + i).unwrap();
    let w = WinInput {
        rules,
        hand: o.hand,
        win_tile: o.win.unwrap(),
        sets: &o.sets,
        tsumo: o.tsumo,
        riichi: match o.riichi {
            0 => RiichiKind::None,
            1 => RiichiKind::Single,
            _ => RiichiKind::Double,
        },
        ippatsu: o.ippatsu,
        haitei: o.haitei,
        houtei: o.houtei,
        rinshan: o.rinshan,
        chankan: o.chankan,
        first_draw: o.tianhu,
        oya: o.menfeng == 0,
        bakaze: wind(o.zhuangfeng),
        jikaze: wind(o.menfeng),
        dora_indicators: &o.dora,
        ura_indicators: &o.ura,
        aka: o.aka,
        kita: 0,
    };
    let mut r = yaku::evaluate(&w)?;
    // 最小化の仕組みを確かめるための故障注入（--inject-fault）：混一色の符を 10 ずらす
    if crate::INJECT_FAULT.load(std::sync::atomic::Ordering::Relaxed)
        && r.yaku.iter().any(|y| y.yaku == yaku::Yaku::Honitsu)
    {
        r.fu += 10;
    }
    let oya = o.menfeng == 0;
    let defen = if !o.tsumo {
        round_up_100(r.base * if oya { 6 } else { 4 })
    } else if oya {
        round_up_100(r.base * 2) * 3
    } else {
        round_up_100(r.base * 2) + 2 * round_up_100(r.base)
    };
    let mut yaku: Vec<(String, u8, u8)> = r
        .yaku
        .iter()
        .map(|y| (y.yaku.id().to_string(), y.han, y.yakuman))
        .collect();
    yaku.sort();
    Some(Score {
        yaku,
        han: r.han,
        fu: if r.yakuman > 0 && !r.kazoe { 0 } else { r.fu },
        yakuman: if r.kazoe { 0 } else { r.yakuman },
        defen,
    })
}

fn map_name(n: &str) -> Option<&'static str> {
    Some(match n {
        "立直" => "riichi",
        "ダブル立直" => "double_riichi",
        "一発" => "ippatsu",
        "海底摸月" => "haitei",
        "河底撈魚" => "houtei",
        "嶺上開花" => "rinshan",
        "槍槓" => "chankan",
        "天和" => "tenhou",
        "地和" => "chiihou",
        "門前清自摸和" => "menzen_tsumo",
        "翻牌 白" => "yakuhai_haku",
        "翻牌 發" => "yakuhai_hatsu",
        "翻牌 中" => "yakuhai_chun",
        "平和" => "pinfu",
        "断幺九" => "tanyao",
        "一盃口" => "iipeikou",
        "三色同順" => "sanshoku",
        "一気通貫" => "ittsu",
        "混全帯幺九" => "chanta",
        "七対子" => "chiitoitsu",
        "対々和" => "toitoi",
        "三暗刻" => "sanankou",
        "三槓子" => "sankantsu",
        "三色同刻" => "sanshoku_doukou",
        "混老頭" => "honroutou",
        "小三元" => "shousangen",
        "混一色" => "honitsu",
        "純全帯幺九" => "junchan",
        "二盃口" => "ryanpeikou",
        "清一色" => "chinitsu",
        "国士無双" => "kokushi",
        "国士無双十三面" => "kokushi_13",
        "四暗刻" => "suuankou",
        "四暗刻単騎" => "suuankou_tanki",
        "大三元" => "daisangen",
        "小四喜" => "shousuushii",
        "大四喜" => "daisuushii",
        "字一色" => "tsuuiisou",
        "緑一色" => "ryuuiisou",
        "清老頭" => "chinroutou",
        "四槓子" => "suukantsu",
        "九蓮宝燈" => "chuuren",
        "純正九蓮宝燈" => "junsei_chuuren",
        "ドラ" => "dora",
        "赤ドラ" => "aka",
        "裏ドラ" => "ura",
        _ if n.starts_with("場風") => "yakuhai_bakaze",
        _ if n.starts_with("自風") => "yakuhai_jikaze",
        _ => return None,
    })
}

pub fn theirs_score(a: &HoraAnswer) -> Result<Option<Score>, String> {
    if let Some(e) = &a.error {
        return Err(format!("harness error for {}: {e}", a.case_id));
    }
    let Some(h) = a.hule.as_ref().filter(|h| !h.is_null()) else {
        return Ok(None);
    };
    if h["defen"].as_i64() == Some(0) {
        return Ok(None);
    }
    let mut yaku = Vec::new();
    for y in h["hupai"].as_array().ok_or("hupai")? {
        let name = y["name"].as_str().ok_or("name")?;
        let id = map_name(name).ok_or_else(|| format!("unknown yaku {name}"))?;
        let (han, ym) = match &y["fanshu"] {
            Value::String(s) => (0, s.len() as u8),
            v => (v.as_u64().ok_or("fanshu")? as u8, 0),
        };
        yaku.push((id.to_string(), han, ym));
    }
    yaku.sort();
    Ok(Some(Score {
        yaku,
        han: h["fanshu"].as_u64().unwrap_or(0) as u8,
        fu: h["fu"].as_u64().unwrap_or(0) as u8,
        yakuman: h["damanguan"].as_u64().unwrap_or(0) as u8,
        defen: h["defen"].as_i64().ok_or("defen")?,
    }))
}

pub fn rules() -> Rules {
    presets::test_yonma()
}
