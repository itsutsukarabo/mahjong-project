//! Python のハーネス（`harness/harness.py`）との1行1ケースのやり取り。

use serde_json::Value;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct Harness {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Harness {
    pub fn spawn(python: &str, script: &Path) -> Self {
        let mut child = Command::new(python)
            .arg("-I")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to start {python} {}: {e}", script.display()));
        let stdin = BufWriter::new(child.stdin.take().expect("stdin"));
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    /// まとめて送り、`case_id` で突き合わせて返す（行の並びに頼らない。CLAUDE.md）。
    pub fn ask_many(&mut self, reqs: &[Value]) -> Vec<Value> {
        let mut ids = Vec::with_capacity(reqs.len());
        for r in reqs {
            let mut r = r.clone();
            self.next_id += 1;
            let id = format!("q{}", self.next_id);
            r["case_id"] = Value::String(id.clone());
            ids.push(id);
            writeln!(self.stdin, "{r}").expect("write to harness");
        }
        self.stdin.flush().expect("flush");
        let mut by_id = std::collections::HashMap::new();
        for _ in reqs {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read harness");
            assert!(n > 0, "harness closed its output");
            let v: Value = serde_json::from_str(&line).expect("harness output is JSON");
            let id = v["case_id"].as_str().expect("case_id").to_string();
            assert!(by_id.insert(id, v).is_none(), "duplicate case_id");
        }
        ids.into_iter()
            .map(|id| {
                by_id
                    .remove(&id)
                    .unwrap_or_else(|| panic!("no answer for {id}"))
            })
            .collect()
    }

    pub fn ask(&mut self, req: Value) -> Value {
        self.ask_many(std::slice::from_ref(&req))
            .pop()
            .expect("one")
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
