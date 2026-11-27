//! 多数の卓をまとめて進める RL 環境（Python に依らない本体。PyO3 の包みは `lib.rs`）。
//!
//! - 卓ごとに対局（`Game`）を持ち、判断ポイント（D-022）ごとに観測（`mj_features::obs`、D-043）・合法手マスク（325）を返す
//! - 行動は判断ポイントの ID（`decision_id`）と行動の番号（ACTION_SPACE）で受け取る。ID で突き合わせ、並びに頼らない（CLAUDE.md）
//! - 卓の進行と観測づくりは rayon のスレッドプールで並列にする
//! - 乱数：牌山は `game_seed`（`chacha8-v1`）、報酬の抽選は `game_seed` と席から決まる（REWARD_CONFIG §5）。
//!   `game_seed` は環境のシード・卓の番号・その卓の何局目かから決まるので、同じシードと同じ行動列なら同じ結果になる
//! - 対局が終わると報酬（抽選したプリセットで計算）を返し、その卓で次の対局を始める

use mj_core::action::Action;
use mj_core::game::{Game, WallSource};
use mj_core::rules::Rules;
use mj_features::obs::{EVENT_FIELDS, Observer};
use mj_features::reward::{Preset, RewardSpec};
use mj_features::{ActionId, NUM_ACTIONS, action_to_id, id_to_action};
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// 環境の設定
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub rules: Rules,
    pub num_tables: usize,
    pub seed: u64,
    /// 0 なら論理コア数
    pub threads: usize,
    pub reward: RewardSpec,
    /// 対局ごとに全情報版の牌譜（JSONL）を返す
    pub record_logs: bool,
    /// イベント列の長さの上限（D-045：1局 192）。超えた行は先頭から上限までを入れ、`events_overflow` に印を付けて
    /// 件数を数え、警告を出す（古いイベントを黙って捨てることはしない）
    pub max_events: usize,
}

/// 判断ポイントのまとまり（行は卓の番号・席の順）
#[derive(Debug, Clone, Default)]
pub struct Batch {
    pub decision_id: Vec<i64>,
    pub table: Vec<i64>,
    pub seat: Vec<i8>,
    pub game_id: Vec<i64>,
    /// 行 × policy の次元
    pub policy: Vec<f32>,
    /// 行 × max_events × EVENT_FIELDS（足りない分は -1）
    pub events: Vec<i16>,
    pub events_len: Vec<i32>,
    /// イベント列が上限を超えた行（D-045）
    pub events_overflow: Vec<bool>,
    /// 行 × 325
    pub mask: Vec<bool>,
    /// 行 × critic の次元
    pub critic: Vec<f32>,
    /// この呼び出しの間に終わった対局
    pub finished: Vec<Finished>,
}

/// 終わった対局の結果と報酬
#[derive(Debug, Clone)]
pub struct Finished {
    pub game_id: i64,
    pub game_seed: u64,
    pub table: i64,
    pub scores: Vec<i32>,
    pub ranks: Vec<u8>,
    pub rewards: Vec<f32>,
    pub rewards_normalized: Vec<f32>,
    /// 報酬の抽選の記録（席ごとのプリセット id。牌譜には書かない。REWARD_CONFIG §5）
    pub presets: Vec<&'static str>,
    pub kyokus: u32,
    pub log_jsonl: Option<String>,
}

struct Pending {
    decision_id: i64,
    seat: u8,
    choices: Vec<Action>,
}

struct Table {
    idx: usize,
    games_started: u64,
    game_id: i64,
    game_seed: u64,
    game: Game,
    presets: Vec<&'static Preset>,
    pending: Vec<Pending>,
    next_decision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvError {
    UnknownDecision(i64),
    Unanswered(i64),
    Duplicate(i64),
    BadAction { decision_id: i64, action: i64 },
    IllegalAction { decision_id: i64, action: String },
}

impl std::fmt::Display for EnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvError::UnknownDecision(d) => write!(f, "unknown decision_id {d}"),
            EnvError::Unanswered(d) => write!(f, "decision_id {d} has no action"),
            EnvError::Duplicate(d) => write!(f, "decision_id {d} answered twice"),
            EnvError::BadAction {
                decision_id,
                action,
            } => {
                write!(
                    f,
                    "action id {action} out of range (decision {decision_id})"
                )
            }
            EnvError::IllegalAction {
                decision_id,
                action,
            } => {
                write!(f, "{action} is not legal at decision {decision_id}")
            }
        }
    }
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

thread_local! {
    /// スレッドごとの観測器（シャンテン数の計算器の覚えを持つので卓ごとには作らない）。鍵はルールの指紋
    static OBSERVERS: RefCell<Vec<(String, Observer)>> = const { RefCell::new(Vec::new()) };
}

fn with_observer<R>(rules: &Rules, key: &str, f: impl FnOnce(&Observer) -> R) -> R {
    OBSERVERS.with(|o| {
        let mut v = o.borrow_mut();
        if !v.iter().any(|(k, _)| k == key) {
            v.push((key.to_string(), Observer::new(rules)));
        }
        let (_, obs) = v.iter().find(|(k, _)| k == key).expect("observer");
        f(obs)
    })
}

/// 1行分の観測
struct Row {
    decision_id: i64,
    table: i64,
    seat: i8,
    game_id: i64,
    policy: Vec<f32>,
    events: Vec<[i16; EVENT_FIELDS]>,
    mask: Vec<bool>,
    critic: Vec<f32>,
}

pub struct VecEnv {
    cfg: EnvConfig,
    rules_key: String,
    pool: rayon::ThreadPool,
    tables: Vec<Table>,
    policy_dim: usize,
    critic_dim: usize,
    /// これまでにイベント列が上限を超えた行の数（D-045：学習の統計に残す）
    overflow_total: u64,
}

impl VecEnv {
    pub fn new(cfg: EnvConfig) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cfg.threads)
            .build()
            .expect("thread pool");
        let rules_key = serde_json::to_string(&cfg.rules).expect("rules json");
        let obs = Observer::new(&cfg.rules);
        let layout = obs.layout();
        let mut env = Self {
            rules_key,
            pool,
            tables: Vec::new(),
            policy_dim: layout.policy(),
            critic_dim: layout.critic,
            overflow_total: 0,
            cfg,
        };
        env.tables = (0..env.cfg.num_tables)
            .map(|i| env.new_table(i, 0))
            .collect();
        env
    }

    pub fn policy_dim(&self) -> usize {
        self.policy_dim
    }

    pub fn critic_dim(&self) -> usize {
        self.critic_dim
    }

    pub fn max_events(&self) -> usize {
        self.cfg.max_events
    }

    pub fn overflow_total(&self) -> u64 {
        self.overflow_total
    }

    fn new_table(&self, idx: usize, games_started: u64) -> Table {
        let mut t = Table {
            idx,
            games_started,
            game_id: 0,
            game_seed: 0,
            game: Game::new(self.cfg.rules.clone(), WallSource::Seed(0)),
            presets: Vec::new(),
            pending: Vec::new(),
            next_decision: (idx as i64) << 40,
        };
        start_game(&self.cfg, &mut t);
        t
    }

    /// 今の判断ポイントの観測（行動を受け取る前）
    pub fn observe(&mut self) -> Batch {
        let cfg = &self.cfg;
        let key = &self.rules_key;
        let rows: Vec<Vec<Row>> = self.pool.install(|| {
            self.tables
                .par_iter()
                .map(|t| rows_of(cfg, key, t))
                .collect()
        });
        self.pack(rows.into_iter().flatten().collect(), Vec::new())
    }

    /// 行動を与えて進め、次の判断ポイントの観測を返す。`decision_ids` と `actions` は同じ長さ
    pub fn step(&mut self, decision_ids: &[i64], actions: &[i64]) -> Result<Batch, EnvError> {
        // ID で突き合わせる：どの卓のどの判断ポイントか
        let mut by_id: HashMap<i64, (usize, usize)> = HashMap::new();
        for (ti, t) in self.tables.iter().enumerate() {
            for (pi, p) in t.pending.iter().enumerate() {
                by_id.insert(p.decision_id, (ti, pi));
            }
        }
        let mut chosen: Vec<Vec<Option<Action>>> = self
            .tables
            .iter()
            .map(|t| vec![None; t.pending.len()])
            .collect();
        for (&d, &a) in decision_ids.iter().zip(actions) {
            let &(ti, pi) = by_id.get(&d).ok_or(EnvError::UnknownDecision(d))?;
            if !(0..NUM_ACTIONS as i64).contains(&a) {
                return Err(EnvError::BadAction {
                    decision_id: d,
                    action: a,
                });
            }
            let act = id_to_action(ActionId(a as u16));
            if !self.tables[ti].pending[pi].choices.contains(&act) {
                return Err(EnvError::IllegalAction {
                    decision_id: d,
                    action: act.to_string(),
                });
            }
            if chosen[ti][pi].replace(act).is_some() {
                return Err(EnvError::Duplicate(d));
            }
        }
        for (ti, t) in self.tables.iter().enumerate() {
            for (pi, p) in t.pending.iter().enumerate() {
                if chosen[ti][pi].is_none() {
                    return Err(EnvError::Unanswered(p.decision_id));
                }
            }
        }
        let cfg = &self.cfg;
        let key = &self.rules_key;
        let results: Vec<(Vec<Row>, Option<Finished>)> = self.pool.install(|| {
            self.tables
                .par_iter_mut()
                .zip(chosen.into_par_iter())
                .map(|(t, ch)| {
                    let acts: Vec<(u8, Action)> = t
                        .pending
                        .iter()
                        .zip(ch)
                        .map(|(p, a)| (p.seat, a.expect("checked")))
                        .collect();
                    let fin = advance(cfg, t, &acts);
                    (rows_of(cfg, key, t), fin)
                })
                .collect()
        });
        let mut rows = Vec::new();
        let mut finished = Vec::new();
        for (r, f) in results {
            rows.extend(r);
            finished.extend(f);
        }
        Ok(self.pack(rows, finished))
    }

    fn pack(&mut self, rows: Vec<Row>, finished: Vec<Finished>) -> Batch {
        let l = self.cfg.max_events;
        let mut b = Batch {
            finished,
            ..Default::default()
        };
        for r in rows {
            b.decision_id.push(r.decision_id);
            b.table.push(r.table);
            b.seat.push(r.seat);
            b.game_id.push(r.game_id);
            b.policy.extend(&r.policy);
            b.critic.extend(&r.critic);
            b.mask.extend(&r.mask);
            // 上限を超えたら先頭から l 個を入れ、印を付けて数える（D-045）
            let over = r.events.len() > l;
            if over {
                self.overflow_total += 1;
                eprintln!(
                    "warning: event sequence {} > max_events {l} (table {}, decision {})",
                    r.events.len(),
                    r.table,
                    r.decision_id
                );
            }
            b.events_overflow.push(over);
            let kept = &r.events[..r.events.len().min(l)];
            b.events_len.push(kept.len() as i32);
            for tok in kept {
                b.events.extend(tok);
            }
            b.events
                .extend(std::iter::repeat_n(-1i16, (l - kept.len()) * EVENT_FIELDS));
        }
        b
    }
}

/// 卓で新しい対局を始める（`game_seed` は環境のシード・卓・その卓の何局目かから）
fn start_game(cfg: &EnvConfig, t: &mut Table) {
    let game_index = t.games_started * cfg.num_tables as u64 + t.idx as u64;
    t.games_started += 1;
    t.game_id = game_index as i64;
    t.game_seed = splitmix64(cfg.seed ^ splitmix64(game_index)) >> 1;
    t.game = Game::new(cfg.rules.clone(), WallSource::Seed(t.game_seed));
    let n = cfg.rules.table.num_players;
    t.presets = (0..n).map(|s| cfg.reward.pick(t.game_seed, s)).collect();
    refresh_pending(t);
}

fn refresh_pending(t: &mut Table) {
    t.pending = t
        .game
        .decisions()
        .iter()
        .map(|d| {
            let id = t.next_decision;
            t.next_decision += 1;
            Pending {
                decision_id: id,
                seat: d.seat,
                choices: d.choices.clone(),
            }
        })
        .collect();
}

/// 行動を与える。対局が終わったら結果を返して次の対局を始める
fn advance(cfg: &EnvConfig, t: &mut Table, acts: &[(u8, Action)]) -> Option<Finished> {
    t.game.step(acts).expect("checked legal");
    if !t.game.is_over() {
        refresh_pending(t);
        return None;
    }
    let end = t.game.end().expect("game end").clone();
    let n = cfg.rules.table.num_players;
    let rewards: Vec<f32> = (0..usize::from(n))
        .map(|s| t.presets[s].reward(end.ranks[s], end.scores[s]))
        .collect();
    let norm: Vec<f32> = (0..usize::from(n))
        .map(|s| rewards[s] / t.presets[s].scale(n))
        .collect();
    let log_jsonl = cfg.record_logs.then(|| {
        let meta = mj_log::write::GameMeta {
            game_id: format!("mjpy-{}-{}", cfg.seed, t.game_id),
            players: (0..n).map(|s| format!("seat{s}")).collect(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            engine_git: "0000000".to_string(),
        };
        let mut salt = |_k: u64| mj_log::generate_salt().expect("salt");
        mj_log::to_jsonl(&mj_log::write::game_log(&t.game, &meta, &mut salt))
    });
    let fin = Finished {
        game_id: t.game_id,
        game_seed: t.game_seed,
        table: t.idx as i64,
        scores: end.scores.clone(),
        ranks: end.ranks.clone(),
        rewards,
        rewards_normalized: norm,
        presets: t.presets.iter().map(|p| p.id).collect(),
        kyokus: t.game.kyokus().count() as u32,
        log_jsonl,
    };
    start_game(cfg, t);
    Some(fin)
}

fn rows_of(cfg: &EnvConfig, key: &str, t: &Table) -> Vec<Row> {
    let k = t.game.current_kyoku();
    let n = cfg.rules.table.num_players;
    with_observer(&cfg.rules, key, |obs| {
        t.pending
            .iter()
            .map(|p| {
                let reward = t.presets[usize::from(p.seat)].input(n);
                let o = obs.observe(k.events(), p.seat, &p.choices, &reward);
                debug_assert!(p.choices.iter().all(|a| action_to_id(a).is_some()));
                Row {
                    decision_id: p.decision_id,
                    table: t.idx as i64,
                    seat: p.seat as i8,
                    game_id: t.game_id,
                    policy: o.policy,
                    events: o.events,
                    mask: o.mask,
                    critic: o.critic,
                }
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mj_core::rules::presets;

    fn cfg(seed: u64, threads: usize) -> EnvConfig {
        EnvConfig {
            rules: presets::with(&presets::tenhou_sanma(), "[table]\nlength = 'single'"),
            num_tables: 8,
            seed,
            threads,
            reward: RewardSpec::parse("tenhou-sanma-7dan+").unwrap(),
            record_logs: true,
            max_events: 192,
        }
    }

    /// 合法手のうち番号が最小のものを選ぶ（決定論の確認用）
    fn first_legal(b: &Batch) -> Vec<i64> {
        (0..b.decision_id.len())
            .map(|i| {
                b.mask[i * NUM_ACTIONS..(i + 1) * NUM_ACTIONS]
                    .iter()
                    .position(|&x| x)
                    .unwrap() as i64
            })
            .collect()
    }

    /// （観測の一部, 終わった対局の（game_id, 報酬, プリセット））
    type RunResult = (Vec<f32>, Vec<(i64, Vec<f32>, Vec<&'static str>)>);

    fn run(seed: u64, threads: usize, steps: usize) -> RunResult {
        let mut env = VecEnv::new(cfg(seed, threads));
        let mut b = env.observe();
        let mut fin = Vec::new();
        let mut sig = Vec::new();
        for _ in 0..steps {
            let a = first_legal(&b);
            sig.extend(b.policy.iter().take(16));
            b = env.step(&b.decision_id.clone(), &a).unwrap();
            for f in &b.finished {
                assert!(f.log_jsonl.as_ref().unwrap().contains("game_end"));
                fin.push((f.game_id, f.rewards.clone(), f.presets.clone()));
            }
        }
        (sig, fin)
    }

    #[test]
    fn deterministic_across_threads() {
        let a = run(5, 1, 300);
        let b = run(5, 4, 300);
        assert_eq!(a, b);
        assert!(!a.1.is_empty(), "some games finished");
        let mut env = VecEnv::new(cfg(5, 2));
        let mut b = env.observe();
        for _ in 0..300 {
            let a = first_legal(&b);
            b = env.step(&b.decision_id.clone(), &a).unwrap();
        }
        assert_eq!(
            env.overflow_total(),
            0,
            "event sequence exceeded max_events (D-045)"
        );
    }

    #[test]
    fn rejects_bad_answers() {
        let mut env = VecEnv::new(cfg(1, 2));
        let b = env.observe();
        let ids = b.decision_id.clone();
        let a = first_legal(&b);
        assert_eq!(
            env.step(&ids[1..], &a[1..]).unwrap_err(),
            EnvError::Unanswered(ids[0])
        );
        assert!(matches!(
            env.step(&[12345], &[0]).unwrap_err(),
            EnvError::UnknownDecision(12345)
        ));
        // 合法でない行動（マスクが false の番号）
        let bad = (0..NUM_ACTIONS).find(|&i| !b.mask[i]).unwrap() as i64;
        let mut a2 = a.clone();
        a2[0] = bad;
        assert!(matches!(
            env.step(&ids, &a2).unwrap_err(),
            EnvError::IllegalAction { .. }
        ));
    }
}
