//! 牌譜の再生を WASM から呼ぶ（`web/packages/engine` が Worker で読み込む）。
//!
//! ```js
//! const r = new Replay(jsonlText);          // 型で読み、局ごとに分ける（読めなければ例外）
//! const meta = JSON.parse(r.meta());         // ReplayMeta（局の一覧・席・対局の結果）
//! const states = JSON.parse(r.kyoku_states(kyokuIdx)); // TableView[]（各イベントを当てた直後の卓）
//! ```
//! 返す JSON の型は `web/packages/log-types`（mj-log から生成）。卓の状態を作るのは `mj_log::view` だけで、
//! 画面は描くだけにする（原則1）。

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Replay {
    inner: mj_log::view::Replay,
}

#[wasm_bindgen]
impl Replay {
    #[wasm_bindgen(constructor)]
    pub fn new(jsonl: &str) -> Result<Replay, JsError> {
        mj_log::view::Replay::from_jsonl(jsonl)
            .map(|inner| Replay { inner })
            .map_err(|e| JsError::new(&e))
    }

    /// `ReplayMeta` の JSON
    pub fn meta(&self) -> String {
        serde_json::to_string(&self.inner.meta()).expect("serialize")
    }

    /// `TableView[]` の JSON
    pub fn kyoku_states(&self, kyoku_idx: i32) -> Result<String, JsError> {
        let v = self
            .inner
            .kyoku_states(kyoku_idx)
            .map_err(|e| JsError::new(&e))?;
        Ok(serde_json::to_string(&v).expect("serialize"))
    }
}

/// 注釈（`*.annot.jsonl`）を型で読み、`Annotation[]` の JSON を返す（読めなければ例外）
#[wasm_bindgen]
pub fn parse_annotations(jsonl: &str) -> Result<String, JsError> {
    let a = mj_log::schema::parse_annotations(jsonl).map_err(|e| JsError::new(&e))?;
    Ok(serde_json::to_string(&a).expect("serialize"))
}
