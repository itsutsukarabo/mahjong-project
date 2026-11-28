//! `docs/spec/SCENARIOS.md` の生成（`gen`）と、最新かの確認（`check`。CI）。
//! `show [id]`：局面を定義のルールで回し、和了・流局と点数の移動を表示する（局面を書くときの確認用）

use std::path::Path;

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = root.join("docs/spec/SCENARIOS.md");
    let defs =
        mj_scenarios::load_dir(&mj_scenarios::fixtures_dir()).unwrap_or_else(|e| panic!("{e}"));
    let cur = std::fs::read_to_string(&md).expect("SCENARIOS.md");
    let new = mj_scenarios::render_md(&cur, &defs).unwrap_or_else(|e| panic!("{e}"));
    match cmd.as_str() {
        "gen" => {
            std::fs::write(&md, new).expect("write");
            eprintln!("written {} ({} scenarios)", md.display(), defs.len());
        }
        "check" => {
            if cur != new {
                eprintln!(
                    "docs/spec/SCENARIOS.md is out of date: run `cargo run -p mj-scenarios -- gen`"
                );
                std::process::exit(1);
            }
        }
        "show" => {
            let only = std::env::args().nth(2);
            for d in defs
                .iter()
                .filter(|d| only.as_deref().is_none_or(|x| x == d.id))
            {
                if d.status != mj_scenarios::Status::Ready {
                    continue;
                }
                match mj_scenarios::run_spec(d) {
                    Ok(k) => {
                        for e in k.events() {
                            match e {
                                mj_core::event::Event::Hora(h) => println!(
                                    "{}: hora seat {} from {} yaku {:?} yakuman {} pao {:?} deltas {:?}",
                                    d.id,
                                    h.actor,
                                    h.target,
                                    h.yaku.iter().map(|y| y.yaku.id()).collect::<Vec<_>>(),
                                    h.yakuman,
                                    h.pao,
                                    h.deltas
                                ),
                                mj_core::event::Event::Ryukyoku(r) => {
                                    println!(
                                        "{}: ryukyoku {} deltas {:?}",
                                        d.id, r.reason, r.deltas
                                    )
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => println!("{}: FAILED {e}", d.id),
                }
            }
        }
        _ => {
            eprintln!("usage: mj-scenarios gen|check|show [id]");
            std::process::exit(2);
        }
    }
}
