//! ルールカタログとテスト対応表の道具（D-025）。
//!
//! ```text
//! cargo run -p rules-catalog -- gen                 # docs/spec/RULES_CATALOG.md を作り直す
//! cargo run -p rules-catalog -- check               # 生成物が最新か（CI）
//! cargo run -p rules-catalog -- coverage [--strict] [--summary FILE]
//!     # fixtures/rules/coverage.toml が「項目 × 値」をすべて含み、書かれたテストが実在するか（CI）
//!     # --strict：未実装（pending）の行があっても失敗にする
//! cargo run -p rules-catalog -- add-missing         # 対応表にない「項目 × 値」を pending で末尾に足す
//! ```
//!
//! テストの実在は `cargo test -p <package> -- --list --format terse` の出力で確かめる。

use mj_core::catalog::CATALOG;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const CATALOG_MD: &str = "docs/spec/RULES_CATALOG.md";
const COVERAGE: &str = "fixtures/rules/coverage.toml";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageFile {
    #[serde(default)]
    case: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    key: String,
    value: String,
    /// `<package>::<テストのパス>`（例：`mj-core::score::tests::tsumo_loss`）
    #[serde(default)]
    tests: Vec<String>,
    /// まだ実装していない分岐（理由・予定）。`tests` とどちらか一方
    #[serde(default)]
    pending: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("gen") => gen_catalog(),
        Some("check") => check_catalog(),
        Some("coverage") => {
            let strict = args.iter().any(|a| a == "--strict");
            let summary = args
                .iter()
                .position(|a| a == "--summary")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            coverage(strict, summary.as_deref())
        }
        Some("add-missing") => add_missing(),
        _ => Err(
            "usage: rules-catalog gen | check | coverage [--strict] [--summary FILE] | add-missing"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn gen_catalog() -> Result<(), String> {
    let path = root().join(CATALOG_MD);
    std::fs::write(&path, mj_core::catalog::render_markdown()).map_err(|e| e.to_string())?;
    println!("written {CATALOG_MD} ({} items)", CATALOG.len());
    Ok(())
}

fn check_catalog() -> Result<(), String> {
    let path = root().join(CATALOG_MD);
    let have = std::fs::read_to_string(&path).map_err(|e| format!("{CATALOG_MD}: {e}"))?;
    if have.replace("\r\n", "\n") != mj_core::catalog::render_markdown() {
        return Err(format!(
            "{CATALOG_MD} is out of date; run `cargo run -p rules-catalog -- gen`"
        ));
    }
    println!("{CATALOG_MD} is up to date ({} items)", CATALOG.len());
    Ok(())
}

/// パッケージごとのテスト名の一覧（`cargo test -p pkg -- --list --format terse`）。
fn list_tests(package: &str) -> Result<BTreeSet<String>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(root())
        .args([
            "test", "-p", package, "--locked", "--", "--list", "--format", "terse",
        ])
        .output()
        .map_err(|e| format!("cargo test --list: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo test -p {package} --list failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(String::from)
        .collect())
}

fn add_missing() -> Result<(), String> {
    let path = root().join(COVERAGE);
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    let file: CoverageFile = toml::from_str(&text).map_err(|e| format!("{COVERAGE}: {e}"))?;
    let have: BTreeSet<(String, String)> = file
        .case
        .iter()
        .map(|c| (c.key.clone(), c.value.clone()))
        .collect();
    let mut added = 0;
    for it in CATALOG {
        for v in it.kind.coverage_values() {
            if !have.contains(&(it.key.to_string(), v.to_string())) {
                text.push_str(&format!(
                    "
[[case]]
key = \"{}\"
value = \"{v}\"
pending = \"未実装\"
",
                    it.key
                ));
                added += 1;
            }
        }
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    println!("added {added} pending rows to {COVERAGE}");
    Ok(())
}

fn coverage(strict: bool, summary: Option<&Path>) -> Result<(), String> {
    let text = std::fs::read_to_string(root().join(COVERAGE)).map_err(|e| e.to_string())?;
    let file: CoverageFile = toml::from_str(&text).map_err(|e| format!("{COVERAGE}: {e}"))?;

    // 必要な行：カタログの全項目 × 値
    let mut required: BTreeMap<(String, String), &'static str> = BTreeMap::new();
    for it in CATALOG {
        for v in it.kind.coverage_values() {
            required.insert((it.key.to_string(), v.to_string()), it.title);
        }
    }

    let mut errs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut by_package: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for (i, c) in file.case.iter().enumerate() {
        let id = (c.key.clone(), c.value.clone());
        if !required.contains_key(&id) {
            errs.push(format!(
                "case #{i}: {}={} is not in the catalog",
                c.key, c.value
            ));
        }
        if !seen.insert(id) {
            errs.push(format!("case #{i}: duplicate {}={}", c.key, c.value));
        }
        match (c.tests.is_empty(), &c.pending) {
            (true, None) => errs.push(format!("{}={}: neither tests nor pending", c.key, c.value)),
            (false, Some(_)) => errs.push(format!("{}={}: both tests and pending", c.key, c.value)),
            _ => {}
        }
        for t in &c.tests {
            match t.split_once("::") {
                Some((pkg, path)) => by_package
                    .entry(pkg.to_string())
                    .or_default()
                    .push((i, path.to_string())),
                None => errs.push(format!("{}={}: test id {t} has no package", c.key, c.value)),
            }
        }
    }
    for (k, v) in required.keys() {
        if !seen.contains(&(k.clone(), v.clone())) {
            errs.push(format!("missing case: {k}={v}"));
        }
    }
    for (pkg, tests) in &by_package {
        let have = list_tests(pkg)?;
        for (i, t) in tests {
            if !have.contains(t) {
                let c = &file.case[*i];
                errs.push(format!(
                    "{}={}: test {pkg}::{t} does not exist",
                    c.key, c.value
                ));
            }
        }
    }

    let tested = file.case.iter().filter(|c| !c.tests.is_empty()).count();
    let pending: Vec<&Case> = file.case.iter().filter(|c| c.pending.is_some()).collect();
    let items_tested = CATALOG
        .iter()
        .filter(|it| {
            it.kind.coverage_values().iter().all(|v| {
                file.case
                    .iter()
                    .any(|c| c.key == it.key && c.value == *v && !c.tests.is_empty())
            })
        })
        .count();
    let report = format!(
        "rules coverage: {} items, {} item×value rows; tested {} / {} rows ({} items fully tested), pending {}",
        CATALOG.len(),
        required.len(),
        tested,
        required.len(),
        items_tested,
        pending.len()
    );
    println!("{report}");
    if let Some(path) = summary {
        let mut md = format!("### rules coverage\n\n{report}\n\n");
        if !pending.is_empty() {
            md.push_str("| key | value | pending |\n|---|---|---|\n");
            for c in &pending {
                md.push_str(&format!(
                    "| `{}` | `{}` | {} |\n",
                    c.key,
                    c.value,
                    c.pending.as_deref().unwrap_or("")
                ));
            }
        }
        std::fs::write(path, md).map_err(|e| e.to_string())?;
    }
    if strict && !pending.is_empty() {
        errs.push(format!("--strict: {} rows are pending", pending.len()));
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} problem(s):\n  {}",
            errs.len(),
            errs.join("\n  ")
        ))
    }
}
