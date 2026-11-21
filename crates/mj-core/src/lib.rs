//! 麻雀ルールエンジンのコア。
//!
//! 原則（CLAUDE.md）：コアに置くのは局の状態・合法手・状態遷移だけ。
//! タイマー・通信・演出・ログの整形・学習用特徴量はコアの外の crate に置く。

pub mod rules;
pub mod shanten;
pub mod tile;
pub mod wall;
