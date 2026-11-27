//! RL 環境（`docs/spec/OBSERVATION.md`・`REWARD_CONFIG.md`）。本体は [`env`]（Python に依らない）。
//! Python からの呼び出し口は feature `python`（maturin でビルドするとき）の [`py`]。

pub mod env;

#[cfg(feature = "python")]
mod py;
