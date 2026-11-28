//! 牌譜の TS の型（`web/packages/log-types/src/generated`）が `mj_log::schema`・`mj_log::view` から生成したものと一致するか。
//! 作り直し：`MJ_UPDATE_TS=1 cargo test -p mj-log --features ts --test ts_types`（手書きしない。REPO_LAYOUT §2）
#![cfg(feature = "ts")]

use mj_log::schema::{Annotation, LogEvent};
use mj_log::view::{ReplayMeta, TableView};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ts_rs::{Config, TS};

fn read_dir(d: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(d)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "ts"))
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        std::fs::read_to_string(e.path())
                            .unwrap()
                            .replace("\r\n", "\n"),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn ts_types_are_up_to_date() {
    let tmp = std::env::temp_dir().join(format!("mj-log-ts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let cfg = Config::new().with_out_dir(&tmp);
    LogEvent::export_all(&cfg).unwrap();
    Annotation::export_all(&cfg).unwrap();
    TableView::export_all(&cfg).unwrap();
    ReplayMeta::export_all(&cfg).unwrap();
    let generated = read_dir(&tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    let dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/packages/log-types/src/generated");
    if std::env::var("MJ_UPDATE_TS").is_ok() {
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        let mut index = String::from(
            "// 生成：cargo test -p mj-log --features ts --test ts_types（MJ_UPDATE_TS=1）。手で書き換えない\n",
        );
        for (name, body) in &generated {
            std::fs::write(dest.join(name), body).unwrap();
            let stem = name.trim_end_matches(".ts");
            index.push_str(&format!("export type {{ {stem} }} from \"./{stem}\";\n"));
        }
        std::fs::write(dest.join("index.ts"), index).unwrap();
        return;
    }
    let mut committed = read_dir(&dest);
    committed.remove("index.ts");
    assert_eq!(
        committed.keys().collect::<Vec<_>>(),
        generated.keys().collect::<Vec<_>>(),
        "generated TS files differ: run MJ_UPDATE_TS=1 cargo test -p mj-log --features ts --test ts_types"
    );
    for (k, v) in &generated {
        assert_eq!(
            &committed[k], v,
            "{k} is out of date: run MJ_UPDATE_TS=1 cargo test -p mj-log --features ts --test ts_types"
        );
    }
}
