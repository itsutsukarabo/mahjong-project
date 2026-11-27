//! Python の呼び出し口（PyO3）。`maturin develop` / `maturin build` でビルドする（`crates/mj-py/pyproject.toml`）。
//!
//! ```python
//! import mj_py
//! env = mj_py.VecEnv(rules="tenhou-sanma", num_tables=64, seed=1, threads=0,
//!                    reward="tenhou-sanma-7dan+", record_logs=False, max_events=160, overrides="")
//! b = env.observe()                       # dict of numpy arrays（行 = 判断ポイント）
//! b = env.step(b["decision_id"], actions) # actions: 行ごとの行動の番号（0〜324）
//! ```
//! 卓の進行と観測づくりの間は GIL を手放す。

use crate::env::{Batch, EnvConfig, VecEnv as Inner};
use mj_core::rules::presets;
use mj_features::NUM_ACTIONS;
use mj_features::obs::EVENT_FIELDS;
use mj_features::reward::RewardSpec;
use numpy::ndarray::{Array1, Array2, Array3};
use numpy::{IntoPyArray, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

#[pyclass(unsendable)]
struct VecEnv {
    inner: Inner,
}

fn to_py<'py>(py: Python<'py>, inner: &Inner, b: Batch) -> PyResult<Bound<'py, PyDict>> {
    let n = b.decision_id.len();
    let d = PyDict::new(py);
    let err = |e: numpy::ndarray::ShapeError| PyValueError::new_err(e.to_string());
    d.set_item("decision_id", Array1::from(b.decision_id).into_pyarray(py))?;
    d.set_item("table", Array1::from(b.table).into_pyarray(py))?;
    d.set_item("seat", Array1::from(b.seat).into_pyarray(py))?;
    d.set_item("game_id", Array1::from(b.game_id).into_pyarray(py))?;
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
        x.set_item("log_jsonl", f.log_jsonl)?;
        fin.append(x)?;
    }
    d.set_item("finished", fin)?;
    Ok(d)
}

#[pymethods]
impl VecEnv {
    #[new]
    #[pyo3(signature = (rules="tenhou-sanma", num_tables=64, seed=0, threads=0, reward="tenhou-sanma-7dan+", record_logs=false, max_events=160, overrides=""))]
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
        Ok(Self {
            inner: Inner::new(EnvConfig {
                rules,
                num_tables,
                seed,
                threads,
                reward,
                record_logs,
                max_events,
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

#[pymodule]
fn mj_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VecEnv>()?;
    m.add("NUM_ACTIONS", NUM_ACTIONS)?;
    Ok(())
}
