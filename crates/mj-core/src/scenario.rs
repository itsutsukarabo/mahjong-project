//! テスト用：牌山を手で組んで局面を作り、行動を表記で与えて進める。
//!
//! ```ignore
//! let mut k = Scenario::new(rules, &["1p 2p 3p ...", "...", "..."])
//!     .draws("5s 1m")          // 通常のツモの順（pile の先頭から）
//!     .rinshan("9p")           // 嶺上ツモの順（pile の末尾から）
//!     .dora("1m")              // ドラ表示牌（先頭から。残りの枠は埋める）
//!     .kyoku();
//! act(&mut k, 0, "reach:5s");  // 席0の判断ポイントで行動する（同じ場のほかの席は既定の行動）
//! ```
//! 既定の行動（[`default_action`]）：見送り → ツモ切り → 最初の選択肢。指定しなかった牌は残りの牌を
//! 種類の順に並べて埋める。

use crate::action::Action;
use crate::kyoku::{Kyoku, KyokuSetup};
use crate::rules::Rules;
use crate::tile::Tile;
use crate::wall::{HAND_SIZE, Wall, tile_multiset};
use std::sync::Arc;

pub fn tiles(s: &str) -> Vec<Tile> {
    s.split_whitespace()
        .map(|t| t.parse().unwrap_or_else(|e| panic!("{t}: {e}")))
        .collect()
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub rules: Rules,
    pub hands: Vec<Vec<Tile>>,
    pub draws: Vec<Tile>,
    pub rinshan: Vec<Tile>,
    pub dora: Vec<Tile>,
    pub ura: Vec<Tile>,
    /// 最後の通常のツモ（ハイテイ）にする牌
    pub haitei: Option<Tile>,
    pub oya: u8,
    pub bakaze: u8,
    pub honba: u32,
    pub kyotaku: u32,
    pub scores: Option<Vec<i32>>,
}

impl Scenario {
    pub fn new(rules: Rules, hands: &[&str]) -> Self {
        assert_eq!(hands.len(), usize::from(rules.table.num_players));
        let hands: Vec<Vec<Tile>> = hands.iter().map(|h| tiles(h)).collect();
        for h in &hands {
            assert_eq!(h.len(), HAND_SIZE, "hand {h:?}");
        }
        Scenario {
            rules,
            hands,
            draws: Vec::new(),
            rinshan: Vec::new(),
            dora: Vec::new(),
            ura: Vec::new(),
            haitei: None,
            oya: 0,
            bakaze: 0,
            honba: 0,
            kyotaku: 0,
            scores: None,
        }
    }

    pub fn draws(mut self, s: &str) -> Self {
        self.draws = tiles(s);
        self
    }

    pub fn rinshan(mut self, s: &str) -> Self {
        self.rinshan = tiles(s);
        self
    }

    pub fn dora(mut self, s: &str) -> Self {
        self.dora = tiles(s);
        self
    }

    pub fn ura(mut self, s: &str) -> Self {
        self.ura = tiles(s);
        self
    }

    pub fn haitei(mut self, t: &str) -> Self {
        self.haitei = Some(t.parse().expect("tile"));
        self
    }

    pub fn oya(mut self, s: u8) -> Self {
        self.oya = s;
        self
    }

    pub fn honba(mut self, h: u32, kyotaku: u32) -> Self {
        self.honba = h;
        self.kyotaku = kyotaku;
        self
    }

    pub fn scores(mut self, s: &[i32]) -> Self {
        self.scores = Some(s.to_vec());
        self
    }

    /// 指定どおりの牌山（残りは種類の順に埋める）
    pub fn wall(&self) -> Wall {
        let mut pool = tile_multiset(&self.rules.tiles);
        let mut take = |t: Tile, what: &str| {
            let i = pool
                .iter()
                .position(|x| *x == t)
                .unwrap_or_else(|| panic!("{what}: no {t} left"));
            pool.remove(i)
        };
        for h in &self.hands {
            for t in h {
                take(*t, "hand");
            }
        }
        let draws: Vec<Tile> = self.draws.iter().map(|t| take(*t, "draws")).collect();
        let rinshan: Vec<Tile> = self.rinshan.iter().map(|t| take(*t, "rinshan")).collect();
        let dora: Vec<Tile> = self.dora.iter().map(|t| take(*t, "dora")).collect();
        let ura: Vec<Tile> = self.ura.iter().map(|t| take(*t, "ura")).collect();
        let haitei = self.haitei.map(|t| take(t, "haitei"));
        let slots = usize::from(self.rules.wall.dora_indicator_slots);
        let fill = |v: Vec<Tile>, pool: &mut Vec<Tile>| {
            let mut v = v;
            while v.len() < slots {
                v.push(pool.remove(0));
            }
            v
        };
        let dora_indicators = fill(dora, &mut pool);
        let ura_indicators = fill(ura, &mut pool);
        let pile_len = pool.len() + draws.len() + rinshan.len() + usize::from(haitei.is_some());
        let mut pile = draws;
        let back: Vec<Tile> = rinshan.into_iter().rev().collect();
        let middle = pile_len - pile.len() - back.len();
        let mut mid: Vec<Tile> = pool.drain(..).collect();
        if let Some(h) = haitei {
            // ハイテイ：通常のツモの最後（pile の残りが reserve 枚になる直前）
            let live = pile_len - self.rules.wall.reserve();
            let pos = live - 1 - pile.len();
            mid.insert(pos, h);
        }
        assert_eq!(mid.len(), middle);
        pile.extend(mid);
        pile.extend(back);
        Wall {
            haipai: self.hands.clone(),
            pile,
            dora_indicators,
            ura_indicators,
        }
    }

    pub fn setup(&self) -> KyokuSetup {
        let n = usize::from(self.rules.table.num_players);
        KyokuSetup {
            bakaze: self.bakaze,
            kyoku: self.oya,
            honba: self.honba,
            kyotaku: self.kyotaku,
            oya: self.oya,
            scores: self
                .scores
                .clone()
                .unwrap_or_else(|| vec![self.rules.points.starting; n]),
            wall: self.wall(),
        }
    }

    pub fn kyoku(&self) -> Kyoku {
        Kyoku::new(Arc::new(self.rules.clone()), self.setup())
    }
}

/// 既定の行動：見送り → ツモ切り → 最初の選択肢。
pub fn default_action(choices: &[Action]) -> Action {
    if choices.contains(&Action::Pass) {
        return Action::Pass;
    }
    if let Some(a) = choices.iter().find(|a| {
        matches!(
            a,
            Action::Dahai {
                tsumogiri: true,
                ..
            }
        )
    }) {
        return *a;
    }
    choices[0]
}

/// 席 `seat` の判断ポイントで `a` を選ぶ（同じ場のほかの席は既定の行動）。`seat` に判断ポイントがなければ panic。
pub fn act(k: &mut Kyoku, seat: u8, a: &str) {
    let a: Action = a.parse().unwrap_or_else(|e| panic!("{e}"));
    let ds = k.decisions().to_vec();
    let mine = ds
        .iter()
        .find(|d| d.seat == seat)
        .unwrap_or_else(|| panic!("seat {seat} has no decision; decisions = {}", show(&ds)));
    assert!(
        mine.choices.contains(&a),
        "seat {seat}: {a} not in {}",
        show(&ds)
    );
    let acts: Vec<(u8, Action)> = ds
        .iter()
        .map(|d| {
            if d.seat == seat {
                (seat, a)
            } else {
                (d.seat, default_action(&d.choices))
            }
        })
        .collect();
    k.step(&acts).expect("legal step");
}

/// 今の場を既定の行動で1回進める。
pub fn auto(k: &mut Kyoku) {
    let acts: Vec<(u8, Action)> = k
        .decisions()
        .iter()
        .map(|d| (d.seat, default_action(&d.choices)))
        .collect();
    k.step(&acts).expect("legal step");
}

/// 席 `seat` に判断ポイントが来るまで既定の行動で進める（局が終われば false）。
pub fn auto_until_seat(k: &mut Kyoku, seat: u8) -> bool {
    for _ in 0..10_000 {
        if k.is_done() {
            return false;
        }
        if k.decisions().iter().any(|d| d.seat == seat) {
            return true;
        }
        auto(k);
    }
    panic!("no progress");
}

/// 局が終わるまで既定の行動で進める。
pub fn auto_to_end(k: &mut Kyoku) {
    while !k.is_done() {
        auto(k);
    }
}

/// 判断ポイントの席の選択肢
pub fn choices(k: &Kyoku, seat: u8) -> Vec<String> {
    k.decisions()
        .iter()
        .find(|d| d.seat == seat)
        .map(|d| d.choices.iter().map(|a| a.to_string()).collect())
        .unwrap_or_default()
}

fn show(ds: &[crate::kyoku::Decision]) -> String {
    ds.iter()
        .map(|d| {
            format!(
                "seat{}:[{}]",
                d.seat,
                d.choices
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}
