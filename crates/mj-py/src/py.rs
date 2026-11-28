//! Python の呼び出し口（PyO3）。`maturin develop` / `maturin build` でビルドする（`crates/mj-py/pyproject.toml`）。
//!
//! ```python
//! import mj_py
//! env = mj_py.VecEnv(rules="tenhou-sanma", num_tables=64, seed=1, threads=0,
//!                    reward="tenhou-sanma-7dan+", record_logs=False, max_events=192, overrides="",
//!                    first_game_index=0, schedule=None)
//! b = env.observe()                       # dict of numpy arrays（行 = 判断ポイント）
//! b = env.step(b["decision_id"], actions) # actions: 行ごとの行動の番号（0〜324）
//! ```
//! - `schedule`：`[(game_id, game_seed, [席ごとに RuleBot か]), ...]`（評価。尽きたら `env.is_done`）
//! - 終わった対局（`b["finished"]`）の `stats` は席ごとの打ち方の統計（`mj_bots::stats::SeatStats` の欄の名前の dict）
//!
//! 卓の進行と観測づくりの間は GIL を手放す。

use crate::env::{Batch, EnvConfig, GameSpec, VecEnv as Inner};
use mj_bots::stats::SeatStats;
use mj_core::rules::presets;
use mj_features::obs::{AUX_SECTIONS, EVENT_FIELDS};
use mj_features::reward::RewardSpec;
use mj_features::{ActionId, NUM_ACTIONS, id_to_action};
use numpy::ndarray::{Array1, Array2, Array3};
use numpy::{IntoPyArray, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

#[pyclass(unsendable)]
struct VecEnv {
    inner: Inner,
}

fn stats_dict<'py>(py: Python<'py>, stats: &[SeatStats]) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let col = |f: fn(&SeatStats) -> i64| stats.iter().map(f).collect::<Vec<i64>>();
    d.set_item("kyokus", col(|s| i64::from(s.kyokus)))?;
    d.set_item("hora", col(|s| i64::from(s.hora)))?;
    d.set_item("tsumo", col(|s| i64::from(s.tsumo)))?;
    d.set_item("hora_points", col(|s| s.hora_points))?;
    d.set_item("deal_in", col(|s| i64::from(s.deal_in)))?;
    d.set_item("riichi", col(|s| i64::from(s.riichi)))?;
    d.set_item("call", col(|s| i64::from(s.call)))?;
    d.set_item("exhaustive", col(|s| i64::from(s.exhaustive)))?;
    d.set_item("exhaustive_tenpai", col(|s| i64::from(s.exhaustive_tenpai)))?;
    d.set_item("score_delta", col(|s| s.score_delta))?;
    Ok(d)
}

fn to_py<'py>(py: Python<'py>, inner: &Inner, b: Batch) -> PyResult<Bound<'py, PyDict>> {
    let n = b.decision_id.len();
    let d = PyDict::new(py);
    let err = |e: numpy::ndarray::ShapeError| PyValueError::new_err(e.to_string());
    d.set_item("decision_id", Array1::from(b.decision_id).into_pyarray(py))?;
    d.set_item("table", Array1::from(b.table).into_pyarray(py))?;
    d.set_item("seat", Array1::from(b.seat).into_pyarray(py))?;
    d.set_item("game_id", Array1::from(b.game_id).into_pyarray(py))?;
    d.set_item("kyoku_idx", Array1::from(b.kyoku_idx).into_pyarray(py))?;
    d.set_item("event_idx", Array1::from(b.event_idx).into_pyarray(py))?;
    d.set_item("shanten", Array1::from(b.shanten).into_pyarray(py))?;
    d.set_item(
        "policy",
        Array2::from_shape_vec((n, inner.policy_dim()), b.policy)
            .map_err(err)?
            .into_pyarray(py),
    )?;
    d.set_item(
        "events",
        Array3::from_shape_vec((n, inner.max_events(), EVENT_FIELDS), b.events)
            .map_err(err)?
            .into_pyarray(py),
    )?;
    d.set_item("events_len", Array1::from(b.events_len).into_pyarray(py))?;
    d.set_item(
        "events_overflow",
        Array1::from(b.events_overflow).into_pyarray(py),
    )?;
    d.set_item(
        "mask",
        Array2::from_shape_vec((n, NUM_ACTIONS), b.mask)
            .map_err(err)?
            .into_pyarray(py),
    )?;
    d.set_item(
        "critic",
        Array2::from_shape_vec((n, inner.critic_dim()), b.critic)
            .map_err(err)?
            .into_pyarray(py),
    )?;
    d.set_item(
        "aux",
        Array2::from_shape_vec((n, inner.aux_dim()), b.aux)
            .map_err(err)?
            .into_pyarray(py),
    )?;
    let fin = PyList::empty(py);
    for f in b.finished {
        let x = PyDict::new(py);
        x.set_item("game_id", f.game_id)?;
        x.set_item("game_seed", f.game_seed)?;
        x.set_item("table", f.table)?;
        x.set_item("scores", f.scores)?;
        x.set_item("ranks", f.ranks)?;
        x.set_item("rewards", f.rewards)?;
        x.set_item("rewards_normalized", f.rewards_normalized)?;
        x.set_item("presets", f.presets)?;
        x.set_item("kyokus", f.kyokus)?;
        x.set_item("stats", stats_dict(py, &f.stats)?)?;
        x.set_item("bots", f.bots)?;
        x.set_item("overflow", f.overflow)?;
        x.set_item("log_jsonl", f.log_jsonl)?;
        fin.append(x)?;
    }
    d.set_item("finished", fin)?;
    Ok(d)
}

#[pymethods]
impl VecEnv {
    #[new]
    #[pyo3(signature = (rules="tenhou-sanma", num_tables=64, seed=0, threads=0, reward="tenhou-sanma-7dan+", record_logs=false, max_events=192, overrides="", first_game_index=0, schedule=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        rules: &str,
        num_tables: usize,
        seed: u64,
        threads: usize,
        reward: &str,
        record_logs: bool,
        max_events: usize,
        overrides: &str,
        first_game_index: u64,
        schedule: Option<Vec<(i64, u64, Vec<bool>)>>,
    ) -> PyResult<Self> {
        let base = match rules {
            "tenhou-sanma" => presets::tenhou_sanma(),
            "mjsoul-sanma" => presets::mjsoul_sanma(),
            "test-yonma" => presets::test_yonma(),
            other => return Err(PyValueError::new_err(format!("unknown rules {other}"))),
        };
        let rules = if overrides.is_empty() {
            base
        } else {
            presets::with(&base, overrides)
        };
        let reward = RewardSpec::parse(reward)
            .ok_or_else(|| PyValueError::new_err(format!("unknown reward {reward}")))?;
        let n = usize::from(rules.table.num_players);
        let schedule = match schedule {
            None => None,
            Some(s) => {
                let mut v = Vec::with_capacity(s.len());
                for (game_id, game_seed, bots) in s {
                    if bots.len() != n || bots.iter().all(|&b| b) {
                        return Err(PyValueError::new_err(format!(
                            "game {game_id}: bots must have {n} seats and at least one non-bot seat"
                        )));
                    }
                    v.push(GameSpec {
                        game_id,
                        game_seed,
                        bots,
                    });
                }
                Some(v)
            }
        };
        Ok(Self {
            inner: Inner::new(EnvConfig {
                threads,
                record_logs,
                max_events,
                first_game_index,
                schedule,
                ..EnvConfig::new(rules, num_tables, seed, reward)
            }),
        })
    }

    #[getter]
    fn policy_dim(&self) -> usize {
        self.inner.policy_dim()
    }

    #[getter]
    fn critic_dim(&self) -> usize {
        self.inner.critic_dim()
    }

    #[getter]
    fn aux_dim(&self) -> usize {
        self.inner.aux_dim()
    }

    #[getter]
    fn max_events(&self) -> usize {
        self.inner.max_events()
    }

    /// これまでにイベント列が上限を超えた行の数（D-045）
    #[getter]
    fn overflow_total(&self) -> u64 {
        self.inner.overflow_total()
    }

    /// 次に始める対局の通し番号（チェックポイントに保存し、再開のとき `first_game_index` に渡す）
    #[getter]
    fn next_game_index(&self) -> u64 {
        self.inner.next_game_index()
    }

    /// `schedule` の対局がすべて終わった
    #[getter]
    fn is_done(&self) -> bool {
        self.inner.is_done()
    }

    #[getter]
    fn num_actions(&self) -> usize {
        NUM_ACTIONS
    }

    fn observe<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let inner = &mut self.inner;
        let b = py.detach(|| inner.observe());
        to_py(py, &self.inner, b)
    }

    fn step<'py>(
        &mut self,
        py: Python<'py>,
        decision_id: PyReadonlyArray1<'py, i64>,
        actions: PyReadonlyArray1<'py, i64>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let ids = decision_id.as_slice()?.to_vec();
        let acts = actions.as_slice()?.to_vec();
        if ids.len() != acts.len() {
            return Err(PyValueError::new_err(
                "decision_id and actions differ in length",
            ));
        }
        let inner = &mut self.inner;
        let b = py
            .detach(|| inner.step(&ids, &acts))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        to_py(py, &self.inner, b)
    }
}

/// 行動の番号（0〜324）の表記（DECISION_POINTS §5。注釈の `decision`・`choices` に使う）
#[pyfunction]
fn action_names() -> Vec<String> {
    (0..NUM_ACTIONS)
        .map(|i| id_to_action(ActionId(i as u16)).to_string())
        .collect()
}

/// 牌譜の `game_id`（環境のシードと対局の番号から。注釈のキー）
#[pyfunction]
fn log_game_id(seed: u64, game_id: i64) -> String {
    crate::env::log_game_id(seed, game_id)
}

/// 補助タスクのラベルの区画（名前, 次元）
#[pyfunction]
fn aux_sections() -> Vec<(String, usize)> {
    AUX_SECTIONS
        .iter()
        .map(|(n, d)| (n.to_string(), *d))
        .collect()
}

/// 牌譜（全情報版の JSONL）と注釈（JSONL）を検査する：注釈のキー・選択肢の形（`mj_log::validate_annotations`）に加え、
/// エンジンで再生して注釈の位置が判断ポイントで `decision`・`choices` がエンジンと一致すること。注釈の数を返す
#[pyfunction]
fn validate_annotations(log_jsonl: &str, annot_jsonl: &str) -> PyResult<usize> {
    let e = |x: mj_log::LogError| PyValueError::new_err(x.to_string());
    let log = mj_log::parse_jsonl(log_jsonl).map_err(e)?;
    let ann = mj_log::parse_jsonl(annot_jsonl).map_err(e)?;
    mj_log::validate_annotations(&ann, &log).map_err(e)?;
    let decisions = mj_log::replay::replay(&log).map_err(e)?;
    mj_log::replay::check_annotations(&ann, &decisions).map_err(e)
}

#[pymodule]
fn mj_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VecEnv>()?;
    m.add("NUM_ACTIONS", NUM_ACTIONS)?;
    m.add_function(wrap_pyfunction!(action_names, m)?)?;
    m.add_function(wrap_pyfunction!(log_game_id, m)?)?;
    m.add_function(wrap_pyfunction!(aux_sections, m)?)?;
    m.add_function(wrap_pyfunction!(validate_annotations, m)?)?;
    Ok(())
}
