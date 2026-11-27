# REPO LAYOUT — リポジトリ構成案

状態：**承認済み（D-015：モノレポ、crate 接頭辞 `mj-`）**。Q-002 は自作（D-010）、Q-004 は Phase 3 まで保留。

## 0. いま作ってあるもの（2026-11-22）
| パス | 中身 |
|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | cargo workspace（Rust 1.100.0 に固定） |
| `crates/mj-core/` | `rules.rs`・`catalog.rs`（ルールの全項目＝正、D-025）、`tile.rs`、`wall.rs`（`chacha8-v1`）、`shanten.rs`、`agari.rs`（和了・待ちの表引き）、`yaku.rs`（役・符・点数）、`score.rs`（点数・祝儀の移動）、`action.rs`・`event.rs`・`kyoku.rs`（判断ポイント単位の局進行、D-022）、`game.rs`（対局の進行・精算）、`policy.rs`・`scenario.rs`（テスト・計測用）、`examples/selfplay_bench.rs` |
| `crates/mj-log/` | 牌譜 v0.4 の検証・`wall_commit`・`wall_salt`・redact（全情報→席視点/公開）、`write.rs`（エンジンの対局→牌譜）、`replay.rs`（牌譜→エンジンで再生・注釈の検査）、サンプルとゴールデンのテスト |
| `tools/difftest-majiang/` | Rust の差分テスト本体（シャンテン数・待ち、和了・点数。生成・比較・最小化）＋ `node/`（majiang-core 1.4.1 を呼ぶハーネス） |
| `tools/rules-catalog/` | `docs/spec/RULES_CATALOG.md` の生成と、テスト対応表（`fixtures/rules/coverage.toml`）の検査、質問票の下書き |
| `rules/` | プリセット（`tenhou-sanma.toml`, `mjsoul-sanma.toml`）。出典は `docs/spec/RULES_PRESETS.md` |
| `fixtures/` | `rules/`（テスト専用の四麻ルール `test-yonma.toml`、テスト対応表 `coverage.toml`）、`golden/walls.json`（固定シードの牌山のハッシュ）、`golden/games.json` と `golden/games/*.actions.json`（固定の行動列から作った牌譜のハッシュ） |
| `.github/workflows/` | `ci.yml`（main への push、Ubuntu/Windows）、`nightly.yml`（差分テスト大規模版） |

それ以外（mj-features, mj-sim, mj-py, mj-wasm, python/, web/ など）はまだ作っていない。

## 1. 全体像

1つのリポジトリ（モノレポ）に、Rust の cargo workspace、Python（uv）、TypeScript（pnpm workspace）を同居させる。

```
mahjong-project/
├─ Cargo.toml                 # cargo workspace（crates/* を束ねる）
├─ rust-toolchain.toml        # Rust の版を固定（wasm32 ターゲット込み）
├─ pyproject.toml             # uv workspace（python/*）
├─ pnpm-workspace.yaml        # web/* を束ねる
│
├─ crates/
│  ├─ mj-core/                # ルールエンジンのコア：局の状態・合法手・状態遷移・和了/役/点数・牌山生成（シード）
│  ├─ mj-log/                 # 牌譜（LOG_FORMAT）の型・読み書き・検証・redact（全情報→席視点/公開）・牌譜からの局面復元
│  ├─ mj-features/            # 学習用特徴量（観測テンソル・合法手マスク・補助タスクのラベル）
│  ├─ mj-sim/                 # 自己対戦ランナー：多数の卓をスレッド並列で回す、推論バッチへの受け渡し口
│  ├─ mj-bots/                # ルールベースボット（評価の基準点、Q-007）
│  ├─ mj-py/                  # PyO3 バインディング（maturin でビルド）
│  ├─ mj-wasm/                # WASM バインディング（wasm-bindgen）
│  ├─ mj-server/              # 対戦サーバー（Q-004 で Rust を選んだ場合）
│  └─ mj-napi/                # Node 向けバインディング（Q-004 で Node を選んだ場合のみ作る）
│
├─ python/
│  └─ mjrl/                   # RL 学習（PPO・対戦相手プール・評価・チェックポイント/再開・統計の配信）
│     ├─ src/mjrl/
│     ├─ configs/             # 実験設定（1実験1ファイル、「何を確かめる実験か」を冒頭に書く）
│     └─ tests/
│
├─ web/
│  ├─ packages/
│  │  ├─ engine/              # mj-wasm のビルド成果物を包む薄い TS ラッパー（Worker で動かす）
│  │  ├─ log-types/           # 牌譜の TS 型（mj-log から自動生成。手書きしない）
│  │  ├─ table-model/         # 牌譜 → WASM → 卓の状態（view model）
│  │  ├─ render-2d/           # 2D レンダラー（開発者向け：全員の手牌表示）
│  │  └─ render-3d/           # 3D レンダラー（Three.js）
│  └─ apps/
│     ├─ replay/              # 牌譜再生画面（Vite）
│     ├─ stream/              # 配信画面（OBS ブラウザソース）
│     └─ game/                # ゲームモード（Phase 3）
│
├─ server/                    # 対戦サーバー（Q-004 で Node を選んだ場合。Rust なら crates/mj-server）
│
├─ rules/                     # ルールプリセット（データファイル、D-011）
├─ tools/
│  ├─ difftest-majiang/       # Rust（生成・比較・最小化）＋ node/（majiang-core を呼ぶハーネス）
│  └─ difftest-riichienv/     # Python：RiichiEnv（隔離環境）を呼ぶ三麻の差分テストハーネス
│
├─ fixtures/                  # 回帰テストのケース（差分テストで見つかった最小ケース、ID 付き）
├─ docs/
│  ├─ spec/                   # 仕様（LOG_FORMAT, REPO_LAYOUT, SANMA_RULES ...）
│  └─ research/               # 調査メモ
└─ runs/                      # 学習の出力（.gitignore。チェックポイント・統計・牌譜シャード）
```

## 2. 依存の向き（守るルール）

```
mj-core  ←  mj-log  ←  mj-features  ←  mj-sim  ←  mj-py
   ↑           ↑                         ↑
   │           └──────── mj-wasm         mj-bots
   └──────────────────── mj-server / mj-napi（mj-log 経由で redact を必ず通す）
```

- **mj-core は本プロジェクトの他の crate に依存しない**。PyO3・wasm-bindgen・serde_json・ログ整形・タイマー・通信を入れない（原則5）
  - `serde` の derive は feature で有効にする（`mj-log` が使う）。RiichiEnv のようにコアの構造体に `#[pyclass]` を付けることはしない
- **合法手・和了判定・点数計算は mj-core にしかない**。mj-py / mj-wasm / mj-server は呼び出して形を変えるだけ（原則1）。FE（`web/`）にはルールの処理を書かない
- **牌譜の型の正は mj-log**。Python 側・TS 側の型は mj-log から生成する（TS は `ts-rs` 等、Python は JSON Schema 経由など。手段は実装時に決める）
- **他家の手牌・牌山を外へ出せるのは mj-log の redact を通った後だけ**。mj-server からクライアントへ送る関数の入力型を「redact 済みイベント」に限定し、型で漏れを防ぐ（原則4）
- mj-features は mj-core の状態を読むだけ。学習の都合でコアに変更が必要になったら、まずコアの公開 API で済むかを検討する

## 3. 各 crate の要点

| crate | 主な中身 | 性能・品質の要件 |
|---|---|---|
| mj-core | `Rules`（設定構造体）、`Tile`、`State`、`Action`（全ルールの和集合）、`legal_actions(&State, seat) -> 固定順の列`、`step(&mut State, actions) -> Vec<Event>`、牌山生成（`rng` 名と版を固定） | 決定論（HashMap を外部 API に出さない）。1局ごとのヒープ確保を最小（固定長配列・再利用バッファ）。`no_std` までは求めない |
| mj-log | イベント型、JSONL 読み書き、検証（牌の多重集合・ID の連番・点数和）、redact、`replay(log) -> State` | 全情報版↔公開情報版の3つ組一致をテストで保証 |
| mj-features | 観測テンソル、合法手マスク、補助タスクのラベル（他家手牌・待ち・シャンテン） | ゼロコピーで numpy へ渡せる形 |
| mj-sim | N 卓を rayon 等で並列実行、推論要求をまとめて Python へ返す | 局/秒を常に計測して出力 |
| mj-py | `mj-sim`・`mj-features`・`mj-log` の Python API | GIL を手放して Rust 側で回す |
| mj-wasm | 牌譜再生（イベントを1つずつ適用して卓の状態を返す）、合法手の事前チェック、オフライン対戦 | Worker 上で動かす前提の API |

## 4. ビルド・テストの単位（案）

| コマンド | 内容 |
|---|---|
| `cargo test --workspace` | Rust 単体テスト＋仕様テスト |
| `cargo bench -p mj-core` | 局/秒のベンチマーク（結果を `runs/bench/` に記録） |
| `maturin develop -m crates/mj-py/Cargo.toml` | Python から呼べるようにする |
| `wasm-pack build crates/mj-wasm --target web --out-dir ../../web/packages/engine/pkg` | WASM を FE へ |
| `pnpm -r test` / Playwright | FE のテスト・画面スナップショット |
| `just difftest`（または Makefile） | Rust で cases を生成 → `tools/difftest-*` → ID で結合して比較、件数を報告 |

## 5. 未決（オーナー確認事項）
1. モノレポでよいか（別案：エンジンだけ別リポジトリにして crates.io/npm/PyPI 風に配布）
2. crate 名の接頭辞（`mj-` は仮）
3. Q-004（サーバー言語）。この構成は両案に対応しているが、決まったら使わない側（`server/` か `crates/mj-server`、`mj-napi`）は作らない
