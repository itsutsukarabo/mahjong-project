//! RL のための変換（mj-core の外。CLAUDE.md 原則5）。
//!
//! いまは行動の番号表（`docs/spec/ACTION_SPACE.md`）だけ：選択肢の表記（mj-core の [`Action`]）と、
//! RL の方策の出力の番号（[`ActionId`]、0〜324）の相互変換と、合法手マスク。
//!
//! 番号は一度付けたら変えない背番号（D-038）。行動を足すときは末尾に足す。
//! 初版の 325 個の対応は `fixtures/features/action_ids.txt` に固定し、テストで確かめる。

use mj_core::action::Action;
use mj_core::tile::{Tile, TileKind};
use std::collections::HashMap;
use std::sync::OnceLock;

/// 行動の数（方策の出力の次元、D-039）
pub const NUM_ACTIONS: usize = 325;

/// 牌の番号の数（赤を別に数える。ACTION_SPACE §2）
pub const NUM_TILES: usize = 37;

/// 行動の番号（0〜324）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u16);

/// 牌 → 牌の番号（ACTION_SPACE §2：種類の昇順、5 は赤が先）
pub fn tile_index(t: Tile) -> usize {
    let k = t.kind().index();
    // 5m・5p・5s の前までに入る赤の数（萬子の後は1、筒子の後は2…）
    let reds_before = match k {
        0..=4 => 0,
        5..=13 => 1,
        14..=22 => 2,
        _ => 3,
    };
    let is_five = matches!(k, 4 | 13 | 22);
    k + reds_before + usize::from(is_five && !t.is_red())
}

/// 牌の番号 → 牌
pub fn tile_of_index(i: usize) -> Tile {
    assert!(i < NUM_TILES, "tile index {i}");
    let (k, red) = match i {
        0..=3 => (i, false),
        4 => (4, true),
        5..=13 => (i - 1, false),
        14 => (13, true),
        15..=23 => (i - 2, false),
        24 => (22, true),
        _ => (i - 3, false),
    };
    Tile::new(TileKind::new(k as u8).expect("kind"), red).expect("tile")
}

fn kind(k: usize) -> TileKind {
    TileKind::new(k as u8).expect("kind")
}

fn plain(k: usize) -> Tile {
    Tile::plain(kind(k))
}

fn red(k: usize) -> Tile {
    Tile::new(kind(k), true).expect("five")
}

/// 番号の順の行動の一覧（ACTION_SPACE §3）
fn build() -> Vec<Action> {
    let mut v = vec![Action::Tsumo, Action::Ron, Action::Kyushu];
    v.extend((0..34).map(|k| Action::Ankan(kind(k))));
    v.extend((0..NUM_TILES).map(|i| Action::Kakan(tile_of_index(i))));
    v.push(Action::Kita);
    v.push(Action::Daiminkan);
    // ポン（§3.1）：5 の種類は 赤赤・赤・赤なし の3つ
    for k in 0..34 {
        if matches!(k, 4 | 13 | 22) {
            v.push(Action::Pon(red(k), red(k)));
            v.push(Action::Pon(red(k), plain(k)));
        }
        v.push(Action::Pon(plain(k), plain(k)));
    }
    // チー（§3.2）：色ごとに 19 通り
    for suit in 0..3 {
        let t = |n: usize, r: bool| {
            let k = suit * 9 + n - 1;
            if r { red(k) } else { plain(k) }
        };
        let pairs: [(usize, bool, usize, bool); 19] = [
            (1, false, 2, false),
            (1, false, 3, false),
            (2, false, 3, false),
            (2, false, 4, false),
            (3, false, 4, false),
            (3, false, 5, true),
            (3, false, 5, false),
            (4, false, 5, true),
            (4, false, 5, false),
            (4, false, 6, false),
            (5, true, 6, false),
            (5, false, 6, false),
            (5, true, 7, false),
            (5, false, 7, false),
            (6, false, 7, false),
            (6, false, 8, false),
            (7, false, 8, false),
            (7, false, 9, false),
            (8, false, 9, false),
        ];
        for (a, ar, b, br) in pairs {
            v.push(Action::Chi(t(a, ar), t(b, br)));
        }
    }
    for i in 0..NUM_TILES {
        for tsumogiri in [false, true] {
            v.push(Action::Reach {
                tile: tile_of_index(i),
                tsumogiri,
            });
        }
    }
    for i in 0..NUM_TILES {
        for tsumogiri in [false, true] {
            v.push(Action::Dahai {
                tile: tile_of_index(i),
                tsumogiri,
            });
        }
    }
    v.extend([
        Action::DeclareTenpai,
        Action::DeclareNoten,
        Action::Pass,
        Action::PassNoten,
    ]);
    assert_eq!(v.len(), NUM_ACTIONS);
    v
}

struct Table {
    by_id: Vec<Action>,
    by_action: HashMap<Action, u16>,
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let by_id = build();
        let by_action = by_id
            .iter()
            .enumerate()
            .map(|(i, a)| (*a, i as u16))
            .collect::<HashMap<_, _>>();
        assert_eq!(by_action.len(), NUM_ACTIONS, "actions are distinct");
        Table { by_id, by_action }
    })
}

/// 行動 → 番号。番号表にない行動なら `None`（エンジンの選択肢はすべて番号表にある。テストで確かめる）
pub fn action_to_id(a: &Action) -> Option<ActionId> {
    table().by_action.get(a).map(|&i| ActionId(i))
}

/// 番号 → 行動
pub fn id_to_action(id: ActionId) -> Action {
    table().by_id[usize::from(id.0)]
}

/// 選択肢（判断ポイント）→ 合法手マスク（325 次元。選択肢の番号が true）
pub fn legal_mask(choices: &[Action]) -> Vec<bool> {
    let mut m = vec![false; NUM_ACTIONS];
    for a in choices {
        let id = action_to_id(a).unwrap_or_else(|| panic!("{a} is not in the action table"));
        m[usize::from(id.0)] = true;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_index_roundtrip_and_order() {
        for i in 0..NUM_TILES {
            assert_eq!(tile_index(tile_of_index(i)), i);
        }
        assert_eq!(tile_of_index(4).to_string(), "5mr");
        assert_eq!(tile_of_index(14).to_string(), "5pr");
        assert_eq!(tile_of_index(24).to_string(), "5sr");
        assert_eq!(tile_of_index(36).to_string(), "C");
    }

    #[test]
    fn offsets_match_the_spec() {
        // ACTION_SPACE §3 の表の範囲
        let at = |i: u16| id_to_action(ActionId(i)).to_string();
        assert_eq!(at(0), "tsumo");
        assert_eq!(at(3), "ankan:1m");
        assert_eq!(at(36), "ankan:C");
        assert_eq!(at(37), "kakan:1m");
        assert_eq!(at(73), "kakan:C");
        assert_eq!(at(74), "kita");
        assert_eq!(at(75), "daiminkan");
        assert_eq!(at(76), "pon:1m,1m");
        assert_eq!(at(115), "pon:C,C");
        assert_eq!(at(116), "chi:1m,2m");
        assert_eq!(at(172), "chi:8s,9s");
        assert_eq!(at(173), "reach:1m");
        assert_eq!(at(174), "reach:1m:tsumogiri");
        assert_eq!(at(247), "dahai:1m");
        assert_eq!(at(320), "dahai:C:tsumogiri");
        assert_eq!(at(321), "declare:tenpai");
        assert_eq!(at(324), "pass:noten");
    }
}
