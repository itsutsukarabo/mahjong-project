//! riichienv-core (Rust only) throughput bench: random legal actions, no observation encoding.
//! usage: riichienv-core-bench <game_mode u8> <games_per_thread> <threads>
//!   3p: 3=single 4=east 5=half / 4p: 0,1,2
use riichienv_core::action::{Action, Phase};
use riichienv_core::rule::GameRule;
use riichienv_core::state::GameState;
use riichienv_core::state::legal_actions::GameStateLegalActions;
use riichienv_core::state_3p::GameState3P;
use riichienv_core::state_3p::legal_actions::GameState3PLegalActions;
use std::collections::HashMap;
use std::time::Instant;

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

macro_rules! run_game {
    ($st:expr, $rng:expr) => {{
        let st = &mut $st;
        let mut steps = 0u64;
        let mut kyoku = 1u64;
        let mut key = (st.round_wind, st.kyoku_idx, st.honba);
        while !st.is_done {
            let actors: Vec<u8> = if st.phase == Phase::WaitAct {
                vec![st.current_player]
            } else {
                st.active_players.clone()
            };
            let mut acts: HashMap<u8, Action> = HashMap::new();
            for pid in actors {
                let la = st._get_legal_actions_internal(pid);
                if la.is_empty() {
                    continue;
                }
                let i = ($rng.next() % la.len() as u64) as usize;
                acts.insert(pid, la[i].clone());
            }
            st.step(&acts);
            steps += 1;
            let k = (st.round_wind, st.kyoku_idx, st.honba);
            if k != key && !st.is_done {
                kyoku += 1;
                key = k;
            }
            assert!(steps < 5_000_000, "stuck");
        }
        (kyoku, steps)
    }};
}

fn worker(mode: u8, games: u64, tid: u64) -> (u64, u64) {
    let mut rng = SplitMix(0xC0FFEE ^ tid);
    let (mut k, mut s) = (0, 0);
    for g in 0..games {
        let seed = Some(tid * 1_000_000 + g);
        let (a, b) = if mode >= 3 {
            let mut st = GameState3P::new(mode, true, seed, 0, GameRule::default_tenhou());
            run_game!(st, rng)
        } else {
            let mut st = GameState::new(mode, true, seed, 0, GameRule::default_tenhou());
            run_game!(st, rng)
        };
        k += a;
        s += b;
    }
    (k, s)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode: u8 = a[1].parse().unwrap();
    let games: u64 = a[2].parse().unwrap();
    let threads: u64 = a[3].parse().unwrap();
    let t0 = Instant::now();
    let hs: Vec<_> = (0..threads)
        .map(|t| std::thread::spawn(move || worker(mode, games, t)))
        .collect();
    let (mut k, mut s) = (0, 0);
    for h in hs {
        let (a, b) = h.join().unwrap();
        k += a;
        s += b;
    }
    let dt = t0.elapsed().as_secs_f64();
    let g = games * threads;
    println!(
        "{{\"game_mode\":{mode},\"threads\":{threads},\"games\":{g},\"kyoku\":{k},\"kyoku_per_game\":{:.2},\"sec\":{dt:.3},\"kyoku_per_sec\":{:.0},\"steps_per_sec\":{:.0}}}",
        k as f64 / g as f64,
        k as f64 / dt,
        s as f64 / dt
    );
}
