//! 行動（判断ポイントで選ぶもの）。表記と並び順は `docs/spec/DECISION_POINTS.md` §5。

use crate::tile::{Tile, TileKind};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// ツモ和了（嶺上開花を含む）
    Tsumo,
    /// ロン和了（槍槓・北抜きへのロンを含む）
    Ron,
    /// 九種九牌
    Kyushu,
    Ankan(TileKind),
    /// 加槓（加える牌）
    Kakan(Tile),
    Kita,
    Daiminkan,
    /// 手から出す2枚（並びは牌の順）
    Pon(Tile, Tile),
    Chi(Tile, Tile),
    /// 立直して切る
    Reach {
        tile: Tile,
        tsumogiri: bool,
    },
    Dahai {
        tile: Tile,
        tsumogiri: bool,
    },
    DeclareTenpai,
    DeclareNoten,
    Pass,
    /// 見送ってノーテンと宣言する
    PassNoten,
}

impl Action {
    fn rank(&self) -> u8 {
        match self {
            Action::Tsumo => 0,
            Action::Ron => 1,
            Action::Kyushu => 2,
            Action::Ankan(_) => 3,
            Action::Kakan(_) => 4,
            Action::Kita => 5,
            Action::Daiminkan => 6,
            Action::Pon(..) => 7,
            Action::Chi(..) => 8,
            Action::Reach { .. } => 9,
            Action::Dahai { .. } => 10,
            Action::DeclareTenpai => 11,
            Action::DeclareNoten => 12,
            Action::Pass => 13,
            Action::PassNoten => 14,
        }
    }

    /// 並び順の鍵：種類 → 牌（2枚なら2枚の並び）→ 手出しが先
    fn key(&self) -> (u8, Option<Tile>, Option<Tile>, bool) {
        let r = self.rank();
        match *self {
            Action::Ankan(k) => (r, Some(Tile::plain(k)), None, false),
            Action::Kakan(t) => (r, Some(t), None, false),
            Action::Pon(a, b) | Action::Chi(a, b) => (r, Some(a), Some(b), false),
            Action::Reach { tile, tsumogiri } | Action::Dahai { tile, tsumogiri } => {
                (r, Some(tile), None, tsumogiri)
            }
            _ => (r, None, None, false),
        }
    }
}

impl PartialOrd for Action {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Action {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Tsumo => write!(f, "tsumo"),
            Action::Ron => write!(f, "ron"),
            Action::Kyushu => write!(f, "kyushu"),
            Action::Ankan(k) => write!(f, "ankan:{k}"),
            Action::Kakan(t) => write!(f, "kakan:{t}"),
            Action::Kita => write!(f, "kita"),
            Action::Daiminkan => write!(f, "daiminkan"),
            Action::Pon(a, b) => write!(f, "pon:{a},{b}"),
            Action::Chi(a, b) => write!(f, "chi:{a},{b}"),
            Action::Reach { tile, tsumogiri } => {
                write!(
                    f,
                    "reach:{tile}{}",
                    if *tsumogiri { ":tsumogiri" } else { "" }
                )
            }
            Action::Dahai { tile, tsumogiri } => {
                write!(
                    f,
                    "dahai:{tile}{}",
                    if *tsumogiri { ":tsumogiri" } else { "" }
                )
            }
            Action::DeclareTenpai => write!(f, "declare:tenpai"),
            Action::DeclareNoten => write!(f, "declare:noten"),
            Action::Pass => write!(f, "pass"),
            Action::PassNoten => write!(f, "pass:noten"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseActionError(pub String);

impl fmt::Display for ParseActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid action: {}", self.0)
    }
}

impl std::error::Error for ParseActionError {}

impl FromStr for Action {
    type Err = ParseActionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || ParseActionError(s.to_string());
        let tile = |t: &str| t.parse::<Tile>().map_err(|_| bad());
        let parts: Vec<&str> = s.split(':').collect();
        let two = |p: &str| -> Result<(Tile, Tile), ParseActionError> {
            let (a, b) = p.split_once(',').ok_or_else(bad)?;
            Ok((tile(a)?, tile(b)?))
        };
        Ok(match parts.as_slice() {
            ["tsumo"] => Action::Tsumo,
            ["ron"] => Action::Ron,
            ["kyushu"] => Action::Kyushu,
            ["kita"] => Action::Kita,
            ["daiminkan"] => Action::Daiminkan,
            ["pass"] => Action::Pass,
            ["pass", "noten"] => Action::PassNoten,
            ["declare", "tenpai"] => Action::DeclareTenpai,
            ["declare", "noten"] => Action::DeclareNoten,
            ["ankan", t] => Action::Ankan(tile(t)?.kind()),
            ["kakan", t] => Action::Kakan(tile(t)?),
            ["pon", p] => {
                let (a, b) = two(p)?;
                Action::Pon(a, b)
            }
            ["chi", p] => {
                let (a, b) = two(p)?;
                Action::Chi(a, b)
            }
            ["reach", t] => Action::Reach {
                tile: tile(t)?,
                tsumogiri: false,
            },
            ["reach", t, "tsumogiri"] => Action::Reach {
                tile: tile(t)?,
                tsumogiri: true,
            },
            ["dahai", t] => Action::Dahai {
                tile: tile(t)?,
                tsumogiri: false,
            },
            ["dahai", t, "tsumogiri"] => Action::Dahai {
                tile: tile(t)?,
                tsumogiri: true,
            },
            _ => return Err(bad()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation_round_trips_and_orders_like_the_spec() {
        let strs = [
            "tsumo",
            "ron",
            "kyushu",
            "ankan:E",
            "kakan:5pr",
            "kita",
            "daiminkan",
            "pon:5pr,5p",
            "pon:5p,5p",
            "chi:3s,4s",
            "reach:2p",
            "reach:8p",
            "dahai:2p",
            "dahai:E",
            "dahai:E:tsumogiri",
            "declare:tenpai",
            "declare:noten",
            "pass",
            "pass:noten",
        ];
        let acts: Vec<Action> = strs.iter().map(|s| s.parse().unwrap()).collect();
        for (a, s) in acts.iter().zip(strs) {
            assert_eq!(a.to_string(), s);
        }
        let mut sorted = acts.clone();
        sorted.sort();
        assert_eq!(sorted, acts, "spec order (DECISION_POINTS §5)");
    }
}
