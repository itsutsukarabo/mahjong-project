//! 狙った局面（`fixtures/scenarios`）を定義のルールで回し、期待どおりかを確かめる（D-047）。

use mj_scenarios::{Status, fixtures_dir, load_dir, run_spec};

#[test]
fn all_scenarios_meet_expectations() {
    let defs = load_dir(&fixtures_dir()).unwrap();
    assert!(!defs.is_empty());
    let mut failed = Vec::new();
    for d in defs.iter().filter(|d| d.status == Status::Ready) {
        if let Err(e) = run_spec(d) {
            failed.push(format!("{}: {e}", d.id));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
