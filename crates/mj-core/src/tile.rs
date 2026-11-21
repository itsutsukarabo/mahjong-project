//! 牌の型。
//!
//! 牌の種類（kind）は 0..34 の連番：
//! `0..9` = 1m..9m、`9..18` = 1p..9p、`18..27` = 1s..9s、`27..34` = 東南西北白發中。
//! 表記は MJAI と同じ（`1m`, `5pr`, `E`, `P` など。`docs/spec/LOG_FORMAT.md` §3）。

use std::fmt;
use std::str::FromStr;

/// 牌の種類の数。
pub const NUM_KINDS: usize = 34;

/// 字牌の表記（東南西北白發中）。
const HONOR_NAMES: [&str; 7] = ["E", "S", "W", "N", "P", "F", "C"];

/// 数牌の色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suit {
    Man,
    Pin,
    Sou,
}

impl Suit {
    pub const ALL: [Suit; 3] = [Suit::Man, Suit::Pin, Suit::Sou];

    fn letter(self) -> char {
        match self {
            Suit::Man => 'm',
            Suit::Pin => 'p',
            Suit::Sou => 's',
        }
    }
}

/// 牌の種類（赤の区別なし）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKind(u8);

impl TileKind {
    pub fn new(index: u8) -> Option<Self> {
        (usize::from(index) < NUM_KINDS).then_some(Self(index))
    }

    pub fn number(suit: Suit, n: u8) -> Self {
        assert!((1..=9).contains(&n), "number tile out of range: {n}");
        Self(suit as u8 * 9 + n - 1)
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn all() -> impl Iterator<Item = TileKind> {
        (0..NUM_KINDS as u8).map(TileKind)
    }

    pub fn suit(self) -> Option<Suit> {
        match self.0 {
            0..=8 => Some(Suit::Man),
            9..=17 => Some(Suit::Pin),
            18..=26 => Some(Suit::Sou),
            _ => None,
        }
    }

    /// 数牌なら 1..=9。
    pub fn num(self) -> Option<u8> {
        self.suit().map(|_| self.0 % 9 + 1)
    }

    pub fn is_honor(self) -> bool {
        self.0 >= 27
    }

    pub fn is_terminal_or_honor(self) -> bool {
        self.is_honor() || matches!(self.num(), Some(1 | 9))
    }
}

impl fmt::Display for TileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.suit(), self.num()) {
            (Some(s), Some(n)) => write!(f, "{n}{}", s.letter()),
            _ => f.write_str(HONOR_NAMES[usize::from(self.0 - 27)]),
        }
    }
}

/// 1枚の牌（種類＋赤）。赤は数牌の 5 にだけ付く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tile {
    kind: TileKind,
    red: bool,
}

impl Tile {
    pub fn new(kind: TileKind, red: bool) -> Option<Self> {
        if red && kind.num() != Some(5) {
            return None;
        }
        Some(Self { kind, red })
    }

    pub fn plain(kind: TileKind) -> Self {
        Self { kind, red: false }
    }

    pub fn kind(self) -> TileKind {
        self.kind
    }

    pub fn is_red(self) -> bool {
        self.red
    }

    /// 正規化した並び順のキー：種類の昇順、同じ種類では赤が先。
    pub fn sort_key(self) -> (TileKind, bool) {
        (self.kind, !self.red)
    }
}

impl PartialOrd for Tile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Tile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if self.red {
            f.write_str("r")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTileError(pub String);

impl fmt::Display for ParseTileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid tile notation: {:?}", self.0)
    }
}

impl std::error::Error for ParseTileError {}

impl FromStr for Tile {
    type Err = ParseTileError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseTileError(s.to_string());
        if let Some(i) = HONOR_NAMES.iter().position(|&h| h == s) {
            return Ok(Tile::plain(TileKind(27 + i as u8)));
        }
        let b = s.as_bytes();
        let (n, suit, red) = match b {
            [n, su] => (*n, *su, false),
            [n, su, b'r'] => (*n, *su, true),
            _ => return Err(err()),
        };
        let suit = match suit {
            b'm' => Suit::Man,
            b'p' => Suit::Pin,
            b's' => Suit::Sou,
            _ => return Err(err()),
        };
        if !(b'1'..=b'9').contains(&n) {
            return Err(err());
        }
        Tile::new(TileKind::number(suit, n - b'0'), red).ok_or_else(err)
    }
}

impl serde::Serialize for Tile {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Tile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// 牌の種類ごとの枚数（手牌など）。
pub type KindCounts = [u8; NUM_KINDS];

pub fn kind_counts<'a>(tiles: impl IntoIterator<Item = &'a Tile>) -> KindCounts {
    let mut c = [0u8; NUM_KINDS];
    for t in tiles {
        c[t.kind().index()] += 1;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation_round_trip_for_every_kind() {
        for k in TileKind::all() {
            let t = Tile::plain(k);
            assert_eq!(t.to_string().parse::<Tile>().unwrap(), t);
        }
        for s in ["5mr", "5pr", "5sr"] {
            assert_eq!(s.parse::<Tile>().unwrap().to_string(), s);
        }
    }

    #[test]
    fn rejects_invalid_notation() {
        for s in ["0m", "5zr", "6pr", "Er", "x", "", "10m", "5m "] {
            assert!(s.parse::<Tile>().is_err(), "{s}");
        }
    }

    #[test]
    fn red_sorts_before_plain_of_same_kind() {
        let red: Tile = "5pr".parse().unwrap();
        let plain: Tile = "5p".parse().unwrap();
        let six: Tile = "6p".parse().unwrap();
        assert!(red < plain && plain < six);
    }
}
