//! 和了・点数の層：三麻の和了形を組み、状況役・ドラ・抜きドラ・本場を付けて、
//! mj-core（`yaku::evaluate`・`score::hora_deltas`）と RiichiEnv（`HandEvaluator3P.calc`）の
//! 役・翻・符・役満・支払いを比べる。

use crate::hand::{Gen, sanma_kinds, tile_str};
use mj_core::rules::Rules;
use mj_core::score::{PayCtx, hora_deltas};
use mj_core::tile::{KindCounts, NUM_KINDS, TileKind};
use mj_core::yaku::{self, OpenKind, OpenSet, RiichiKind, WinInput, Yaku};
use serde::Serialize;
use serde_json::{Value, json};

const NORTH: usize = 30;

/// こちらの役の id → RiichiEnv の `Yaku.name_en`
pub fn yaku_name(y: Yaku) -> &'static str {
    use Yaku::*;
    match y {
        Riichi => "Riichi",
        DoubleRiichi => "Double Riichi",
        Ippatsu => "Ippatsu",
        MenzenTsumo => "Menzen Tsumo",
        Pinfu => "Pinfu",
        Tanyao => "Tanyao",
        Iipeikou => "Iipeiko",
        YakuhaiHaku => "Yakuhai (haku)",
        YakuhaiHatsu => "Yakuhai (hatsu)",
        YakuhaiChun => "Yakuhai (chun)",
        YakuhaiBakaze => "Yakuhai (round wind)",
        YakuhaiJikaze => "Yakuhai (seat wind)",
        YakuhaiNorth => "Yakuhai (north)",
        Haitei => "Haitei Raoyue",
        Houtei => "Houtei Raoyui",
        Rinshan => "Rinshan Kaihou",
        Chankan => "Chankan",
        Chiitoitsu => "Chiitoitsu",
        Chanta => "Chantai",
        Ittsu => "Ittsu",
        Sanshoku => "Sanshoku Doujun",
        SanshokuDoukou => "Sanshoku Doukou",
        Toitoi => "Toitoi",
        Sanankou => "San Ankou",
        Sankantsu => "San Kantsu",
        Shousangen => "Shou Sangen",
        Honroutou => "Honroutou",
        Honitsu => "Honitsu",
        Junchan => "Junchan",
        Ryanpeikou => "Ryanpeikou",
        Chinitsu => "Chinitsu",
        Tenhou => "Tenhou",
        Chiihou => "Chiihou",
        Kokushi => "Kokushi Musou",
        Kokushi13 => "Kokushi Musou 13-men",
        Suuankou => "Su Ankou",
        SuuankouTanki => "Su Ankou Tanki",
        Daisangen => "Dai Sangen",
        Shousuushii => "Sho Suusi",
        Daisuushii => "Dai Suusi",
        Tsuuiisou => "Tsuu iisou",
        Ryuuiisou => "Ryuu iisou",
        Chinroutou => "Chinroutou",
        Chuuren => "Chuuren Poutou",
        JunseiChuuren => "Junsei Chuuren Poutou",
        Suukantsu => "Su Kantsu",
        Dora => "Dora",
        Aka => "Aka Dora",
        Ura => "Ura Dora",
        Kita => "Nuki Dora",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HoraCase {
    pub form: &'static str,
    /// 手の中の牌（和了牌を含む）
    #[serde(skip)]
    pub hand: KindCounts,
    #[serde(skip)]
    pub red: [bool; NUM_KINDS],
    #[serde(skip)]
    pub sets: Vec<OpenSet>,
    /// 副露の中の赤（5p・5s を4枚使っていて、手の中にないとき）
    #[serde(skip)]
    pub meld_red: [bool; NUM_KINDS],
    pub win: usize,
    pub tsumo: bool,
    pub riichi: u8,
    pub ippatsu: bool,
    pub haitei: bool,
    pub houtei: bool,
    pub rinshan: bool,
    pub chankan: bool,
    pub first: bool,
    pub seat_wind: u8,
    pub round_wind: u8,
    pub honba: u32,
    pub kita: u8,
    pub dora: Vec<usize>,
    pub ura: Vec<usize>,
}

/// 比べる値
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Score {
    pub yaku: Vec<String>,
    pub han: u32,
    pub fu: u32,
    pub yakuman: bool,
    /// ロン：放銃者の支払い。ツモ：（親の支払い, 子の支払い）。親のツモは子の支払いだけ（1人あたり）
    pub pay: (i64, i64),
}

impl Gen {
    pub fn hora_case(&mut self) -> HoraCase {
        let kinds = sanma_kinds();
        let mut c = [0u8; NUM_KINDS];
        let mut sets: Vec<OpenSet> = Vec::new();
        let form = match self.below(20) {
            0 => "chiitoi",
            1 => "kokushi",
            _ => "standard",
        };
        match form {
            "chiitoi" => {
                while c.iter().filter(|&&x| x == 2).count() < 7 {
                    let k = kinds[self.below(kinds.len())];
                    if c[k] == 0 {
                        c[k] = 2;
                    }
                }
            }
            "kokushi" => {
                for k in [0usize, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33] {
                    c[k] = 1;
                }
                let k = [0usize, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33][self.below(13)];
                c[k] = 2;
            }
            _ => {
                // 字牌・么九牌に偏らせて役満・役牌も出す
                let palette: Vec<usize> = if self.below(4) == 0 {
                    kinds
                        .iter()
                        .copied()
                        .filter(|&k| k >= 27 || k == 0 || k == 8)
                        .collect()
                } else {
                    kinds.clone()
                };
                let mut n = 0;
                while n < 4 {
                    if self.add_set(&mut c, &palette) || self.add_set(&mut c, &kinds) {
                        n += 1;
                    }
                }
                loop {
                    let k = kinds[self.below(kinds.len())];
                    if c[k] + 2 <= 4 {
                        c[k] += 2;
                        break;
                    }
                }
                // 刻子の一部を副露・槓にする
                let trip: Vec<usize> = (0..NUM_KINDS).filter(|&k| c[k] >= 3).collect();
                for &k in &trip {
                    if self.below(3) != 0 {
                        continue;
                    }
                    let kind = match self.below(4) {
                        0 => OpenKind::Ankan,
                        1 => OpenKind::Minkan,
                        _ => OpenKind::Pon,
                    };
                    if kind != OpenKind::Pon && c[k] != 3 {
                        continue;
                    }
                    c[k] -= 3;
                    sets.push(OpenSet {
                        kind,
                        tile: TileKind::new(k as u8).expect("kind"),
                    });
                }
            }
        }
        let held: Vec<usize> = (0..NUM_KINDS).filter(|&k| c[k] > 0).collect();
        let win = held[self.below(held.len())];
        let closed = sets.iter().all(|s| s.kind == OpenKind::Ankan);
        let has_kan = sets.iter().any(|s| s.kind != OpenKind::Pon);
        let tsumo = self.below(2) == 0;
        let riichi = if closed {
            [0, 0, 1, 1, 2][self.below(5)]
        } else {
            0
        };
        let ippatsu = riichi > 0 && self.below(3) == 0;
        let last = self.below(8) == 0;
        let rinshan = tsumo && has_kan && self.below(3) == 0;
        let chankan = !tsumo && self.below(10) == 0;
        let first = tsumo && closed && sets.is_empty() && riichi == 0 && self.below(15) == 0;
        let n_in = usize::from(c[NORTH])
            + sets
                .iter()
                .filter(|s| s.tile.index() == NORTH)
                .map(|s| if s.kind == OpenKind::Pon { 3 } else { 4 })
                .sum::<usize>();
        let kita = self.below(5usize.saturating_sub(n_in).max(1)) as u8;
        let nd = 1 + self.below(3);
        let dora: Vec<usize> = (0..nd).map(|_| kinds[self.below(kinds.len())]).collect();
        let ura: Vec<usize> = if riichi > 0 {
            (0..nd).map(|_| kinds[self.below(kinds.len())]).collect()
        } else {
            Vec::new()
        };
        let mut red = self.reds(&c);
        // 5p・5s を手と副露で4枚使うなら1枚は赤（赤は各1枚）
        let mut meld_red = [false; NUM_KINDS];
        for k in [13usize, 22] {
            let in_sets: u8 = sets
                .iter()
                .filter(|s| s.tile.index() == k)
                .map(|s| if s.kind == OpenKind::Pon { 3 } else { 4 })
                .sum();
            if c[k] + in_sets == 4 {
                if c[k] > 0 {
                    red[k] = true;
                } else {
                    meld_red[k] = true;
                }
            }
        }
        HoraCase {
            meld_red,
            form,
            hand: c,
            red,
            sets,
            win,
            tsumo,
            riichi,
            ippatsu,
            haitei: tsumo && last && !rinshan,
            houtei: !tsumo && last && !chankan,
            rinshan,
            chankan,
            first,
            seat_wind: self.below(3) as u8,
            round_wind: self.below(2) as u8,
            honba: self.below(3) as u32,
            kita,
            dora,
            ura,
        }
    }
}

impl HoraCase {
    pub fn aka(&self) -> u8 {
        self.red
            .iter()
            .chain(&self.meld_red)
            .filter(|&&r| r)
            .count() as u8
    }

    pub fn request(&self) -> Value {
        let mut tiles = crate::hand::tiles_of(&self.hand, &self.red);
        // 和了牌を1枚抜いて別に渡す（赤でない方を抜く）
        let w = tile_str(self.win, false);
        let i = tiles.iter().position(|t| *t == w).unwrap_or_else(|| {
            tiles
                .iter()
                .position(|t| t.starts_with(&w[..2]))
                .expect("win tile in hand")
        });
        let win = tiles.remove(i);
        let melds: Vec<Value> = self
            .sets
            .iter()
            .map(|s| {
                let k = s.tile.index();
                let n = if s.kind == OpenKind::Pon { 3 } else { 4 };
                let ty = match s.kind {
                    OpenKind::Pon => "pon",
                    OpenKind::Minkan => "daiminkan",
                    OpenKind::Ankan => "ankan",
                    OpenKind::Chi => "chi",
                };
                let mut tiles = vec![tile_str(k, false); n];
                if self.meld_red[k] {
                    tiles[0] = tile_str(k, true);
                }
                json!({"type": ty, "tiles": tiles, "from": (self.seat_wind + 1) % 3})
            })
            .collect();
        json!({
            "op": "hora",
            "tiles": tiles,
            "melds": melds,
            "win": win,
            "dora": self.dora.iter().map(|&k| tile_str(k, false)).collect::<Vec<_>>(),
            "ura": self.ura.iter().map(|&k| tile_str(k, false)).collect::<Vec<_>>(),
            "cond": {
                "tsumo": self.tsumo,
                "riichi": self.riichi > 0,
                "double_riichi": self.riichi == 2,
                "ippatsu": self.ippatsu,
                "haitei": self.haitei,
                "houtei": self.houtei,
                "rinshan": self.rinshan,
                "chankan": self.chankan,
                "tsumo_first_turn": self.first,
                "player_wind": self.seat_wind,
                "round_wind": self.round_wind,
                "honba": self.honba,
                "kita_count": self.kita,
            },
        })
    }

    pub fn ours(&self, rules: &Rules) -> Option<Score> {
        // 天和で `yakuman.tenhou_win_tile = "best"`：手牌の牌のうち点数が最も高くなる牌を和了牌とみなす
        // （局進行では mj-core の `Kyoku` が選ぶ。ここでは同じ選び方をまねる）
        if self.first
            && self.seat_wind == 0
            && rules.yakuman.tenhou_win_tile == mj_core::rules::TenhouWinTile::Best
        {
            let mut best: Option<(i64, Score)> = None;
            for k in (0..NUM_KINDS).filter(|&k| self.hand[k] > 0) {
                let mut c = self.clone();
                c.win = k;
                c.first = false;
                let mut c2 = c.clone();
                c2.first = true;
                if let Some((base, s)) = c2.ours_fixed(rules)
                    && best.as_ref().is_none_or(|(b, _)| base > *b)
                {
                    best = Some((base, s));
                }
            }
            return best.map(|(_, s)| s);
        }
        self.ours_fixed(rules).map(|(_, s)| s)
    }

    /// 和了牌を `self.win` に決めたときの（基本点, 比べる値）
    fn ours_fixed(&self, rules: &Rules) -> Option<(i64, Score)> {
        let wind = |i: u8| TileKind::new(27 + i).expect("wind");
        let dora: Vec<TileKind> = self
            .dora
            .iter()
            .map(|&k| TileKind::new(k as u8).expect("k"))
            .collect();
        let ura: Vec<TileKind> = self
            .ura
            .iter()
            .map(|&k| TileKind::new(k as u8).expect("k"))
            .collect();
        let w = WinInput {
            rules,
            hand: self.hand,
            win_tile: TileKind::new(self.win as u8).expect("win"),
            sets: &self.sets,
            tsumo: self.tsumo,
            riichi: match self.riichi {
                0 => RiichiKind::None,
                1 => RiichiKind::Single,
                _ => RiichiKind::Double,
            },
            ippatsu: self.ippatsu,
            haitei: self.haitei,
            houtei: self.houtei,
            rinshan: self.rinshan,
            chankan: self.chankan,
            first_draw: self.first,
            oya: self.seat_wind == 0,
            bakaze: wind(self.round_wind),
            jikaze: wind(self.seat_wind),
            dora_indicators: &dora,
            ura_indicators: &ura,
            aka: self.aka(),
            kita: self.kita,
        };
        let r = yaku::evaluate(&w)?;
        let base = r.base;
        // 席：親 = 席0。和了者の席は自風から。ロンは下家から
        let seat = self.seat_wind;
        let loser = (!self.tsumo).then_some((seat + 1) % 3);
        let ctx = PayCtx {
            n: 3,
            oya: 0,
            honba: self.honba,
        };
        let (d, _) = hora_deltas(rules, ctx, seat, loser, r.base, None, true);
        let pay = match loser {
            Some(l) => (i64::from(-d[usize::from(l)]), 0),
            None if seat == 0 => (0, i64::from(-d[1])),
            None => {
                let other = if seat == 1 { 2 } else { 1 };
                (i64::from(-d[0]), i64::from(-d[other]))
            }
        };
        let mut yaku: Vec<String> = r
            .yaku
            .iter()
            .map(|i| yaku_name(i.yaku).to_string())
            .collect();
        yaku.sort();
        // 数え役満は RiichiEnv では役満でなく翻数で表す（`WinResult.yakuman = false`、han ≥ 13）。同じ形にそろえる
        let yakuman = r.yakuman > 0 && !r.kazoe;
        Some((
            base,
            Score {
                yaku,
                han: if yakuman { 0 } else { u32::from(r.han) },
                fu: if yakuman { 0 } else { u32::from(r.fu) },
                yakuman,
                pay,
            },
        ))
    }
}

/// 点数が同じで、片方が役満、もう片方が数え役満（13翻以上）のとき：同じ手の別の取り方（例：四暗刻か、
/// 一盃口・混一色などの数え役満か）を選んだだけで、支払いは同じ
pub fn same_points_other_reading(a: &Score, b: &Score) -> bool {
    a.pay == b.pay && a.yakuman != b.yakuman && (if a.yakuman { b.han } else { a.han }) >= 13
}

/// RiichiEnv の答えを同じ形に
pub fn theirs(c: &HoraCase, a: &Value) -> Result<Option<Score>, String> {
    if let Some(e) = a.get("error") {
        return Err(format!("harness error: {e}"));
    }
    if a["is_win"] != json!(true) {
        return Ok(None);
    }
    let yakuman = a["yakuman"].as_bool().unwrap_or(false);
    let mut yaku: Vec<String> = a["yaku"]
        .as_array()
        .ok_or("yaku")?
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    yaku.sort();
    let g = |k: &str| a[k].as_i64().unwrap_or(0);
    let pay = if !c.tsumo {
        (g("ron"), 0)
    } else if c.seat_wind == 0 {
        (0, g("tsumo_ko"))
    } else {
        (g("tsumo_oya"), g("tsumo_ko"))
    };
    Ok(Some(Score {
        yaku,
        han: if yakuman {
            0
        } else {
            a["han"].as_u64().unwrap_or(0) as u32
        },
        fu: if yakuman {
            0
        } else {
            a["fu"].as_u64().unwrap_or(0) as u32
        },
        yakuman,
        pay,
    }))
}

/// 最小化の候補：状況役・ドラ・抜きドラ・本場・赤を1つずつ外したもの
pub fn simpler(c: &HoraCase) -> Vec<HoraCase> {
    let mut out = Vec::new();
    let mut push = |f: &dyn Fn(&mut HoraCase)| {
        let mut x = c.clone();
        f(&mut x);
        out.push(x);
    };
    if c.ippatsu {
        push(&|x| x.ippatsu = false);
    }
    if c.riichi > 0 {
        push(&|x| {
            x.riichi = 0;
            x.ippatsu = false;
            x.ura.clear();
        });
    }
    if c.haitei || c.houtei {
        push(&|x| {
            x.haitei = false;
            x.houtei = false;
        });
    }
    if c.rinshan || c.chankan || c.first {
        push(&|x| {
            x.rinshan = false;
            x.chankan = false;
            x.first = false;
        });
    }
    if c.kita > 0 {
        push(&|x| x.kita = 0);
    }
    if c.honba > 0 {
        push(&|x| x.honba = 0);
    }
    if c.dora.len() > 1 || !c.ura.is_empty() {
        push(&|x| {
            x.dora.truncate(1);
            x.ura.clear();
        });
    }
    if c.red.iter().any(|&r| r) {
        // 4枚使っている種類の赤は外せない
        push(&|x| {
            for k in [13usize, 22] {
                let in_sets: u8 = x
                    .sets
                    .iter()
                    .filter(|s| s.tile.index() == k)
                    .map(|s| if s.kind == OpenKind::Pon { 3 } else { 4 })
                    .sum();
                if x.hand[k] + in_sets < 4 {
                    x.red[k] = false;
                }
            }
        });
    }
    if c.round_wind != 0 || c.seat_wind != 0 {
        push(&|x| {
            x.round_wind = 0;
            x.seat_wind = 1;
        });
    }
    out
}
