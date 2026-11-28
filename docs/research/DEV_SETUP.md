# DEV SETUP — 開発環境の準備（Windows）

対象：オーナーの開発機（Windows 11 Pro、AMD Ryzen 9 9900X 12コア/24スレッド、WSL2 Ubuntu 24.04）。
**このページのコマンドはオーナーが実行する**（Claude Code は実行していない。管理者権限が要るものを含む）。

---

## 1. Windows の長いパス（MAX_PATH 260 文字の制限）を外す

Phase 0 で、Python の venv をセッションの一時フォルダに作ろうとして
`OSError: [Errno 2] No such file or directory ... (Windows Long Path support)` で失敗した
（jedi のスタブなど深いパスを持つパッケージ）。回避のため `C:\rbench` という短いパスに作り直した。

### 1.1 OS の設定（管理者の PowerShell で1回だけ）
```powershell
# 長いパスを有効化（レジストリ）
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
  -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force

# 確認（1 なら有効）
Get-ItemPropertyValue -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled"
```
- 反映には**サインアウト／再起動**が必要
- グループポリシーで設定する場合：`gpedit.msc` →「コンピューターの構成 > 管理用テンプレート > システム > ファイル システム > Win32 の長いパスを有効にする」を「有効」

### 1.2 git の設定
```powershell
git config --global core.longpaths true

# 確認
git config --global --get core.longpaths
```
- Git for Windows は OS の設定とは別にこの設定が要る
- このリポジトリでは改行を `.gitattributes`（`* text=auto eol=lf`）で LF に固定している。牌譜サンプルのバイト比較やハッシュを OS に依存させないため

---

## 2. Rust

### 2.1 いまの状態（2026-11-26 に MSVC へ移行済み）
- `rustup`（`%USERPROFILE%\.cargo\bin`）は 2026-11-22 に Claude Code が **GNU ツールチェイン**で導入した
  （MSVC のビルドツールがなかったため）。2026-11-26 にオーナーが §2.3 の手順で **MSVC に移行**し、GNU 版は削除した
  - 既定のホスト `x86_64-pc-windows-msvc`、既定のツールチェイン `1.100.0-x86_64-pc-windows-msvc`
  - Visual Studio Build Tools 2022（17.14.45）、リンカー `VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe`
  - 確認：リポジトリで `RUSTFLAGS` なしの `cargo test --workspace --locked` が全件成功（mj-core 17、mj-log 1+1+7）
- リポジトリの `rust-toolchain.toml` で版を **1.100.0** に固定している（rustup が自動で入れる）
- CI の Windows ランナーも MSVC を使う（GitHub の標準イメージに入っている）

### 2.2 （参考）GNU ツールチェインで当たる問題と回避策
`getrandom` 0.3 以降は Windows で `raw-dylib` を使い、GNU ツールチェインではリンク時に
`dlltool` と、そこから呼ぶアセンブラ `as.exe` が要る。rustup の self-contained には `as.exe` がないので
`error calling dlltool 'dlltool.exe': program not found` になる。

ローカルでは getrandom の旧バックエンド（advapi32 の `RtlGenRandom`、通常のリンク）を使う：
```bash
# Git Bash
export RUSTFLAGS='--cfg getrandom_backend="windows_legacy"'
cargo test --workspace
```
```powershell
# PowerShell
$env:RUSTFLAGS = '--cfg getrandom_backend="windows_legacy"'
cargo test --workspace
```
- `RUSTFLAGS` を変えると全体が再ビルドされる
- この設定は**リポジトリに入れていない**（CI の MSVC / Linux では不要で、既定のバックエンドを使う）

### 2.3 MSVC ツールチェインに切り替える（実施済み）
回避策なしでビルドできるようになり、CI の Windows と同じ構成になる。
```powershell
# Visual Studio Build Tools（C++ ビルドツール）を導入（管理者、数 GB）
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 既定のホストを MSVC に切り替え、固定版を入れる
rustup set default-host x86_64-pc-windows-msvc
rustup toolchain install 1.100.0

# 既定のツールチェインも MSVC 版にする
# （set default-host は「これから入れる版」の種類を変えるだけで、既定のツールチェインは GNU のまま残る。
#   リポジトリ内は rust-toolchain.toml で MSVC 版が選ばれるが、リポジトリ外の rustc は GNU のままになる）
rustup default 1.100.0-x86_64-pc-windows-msvc

# 不要になった GNU 版を削除
rustup toolchain uninstall stable-x86_64-pc-windows-gnu 1.100.0-x86_64-pc-windows-gnu

# 確認（host: x86_64-pc-windows-msvc になっていること）
rustc -vV
```

---

## 3. WSL2（Ubuntu 24.04）

- 性能計測（`ENGINE_SURVEY.md` §3.2）は WSL2 で行った。gcc と rustup（rustc 1.98.1）が入っていた
- **WSL からは crates.io に接続できなかった**（`transfer too slow ... download of config.json failed`）。
  Windows 側で `cargo vendor` した依存を WSL にコピーし、`--offline` でビルドした
- 原因は未調査（WSL のネットワーク設定・プロキシ・DNS のどれか）。WSL を常用するなら確認が必要

---

## 4. Node・Python（版の固定）

| 道具 | 版 | 固定している場所 |
|---|---|---|
| Node.js | 24.18.0 | `.github/workflows/*.yml` の `NODE_VERSION`、`tools/difftest-majiang/node/package.json` の `engines` |
| @kobalab/majiang-core | 1.4.1 | `tools/difftest-majiang/node/package.json`（完全一致）＋ `package-lock.json` |
| Python | 3.12.10 | `.github/workflows/ci.yml` の `PYTHON_VERSION` |
| RiichiEnv（参考・隔離環境のみ） | 0.4.11（PyPI）／ 0ee1236（GitHub） | 本体には入れない（D-010）。三麻の差分テストを作るときにこの版で隔離環境を作る |

### 差分テストをローカルで回す
```bash
cd tools/difftest-majiang/node && npm ci && cd ../../..
cargo run --release -p difftest-majiang -- --cases 20000 --seed 20261122 --out difftest-out
```
- 層は2つ：シャンテン数・待ち（`--cases` 件）と、和了・点数（`--hora-cases` 件、既定は `--cases` と同じ）
- 結果は `difftest-out/summary.json`。不一致は最小化して `failures.jsonl`・`hora_failures.jsonl` に書く

- 四麻の局単位（`--game-cases N`、既定は `--cases / 500`）：ボット同士の半荘を majiang-core の `Dev.Game` で再生して比べる。結果は `game_failures.jsonl`

### RiichiEnv との三麻の差分テスト（隔離した venv）
```bash
python -m venv .venv-riichienv          # リポジトリの直下（.gitignore 済み）。本体の依存には入れない（D-010）
.venv-riichienv/Scripts/python -m pip install -r tools/difftest-riichienv/harness/requirements.txt   # Linux は bin/python
cargo run --release -p difftest-riichienv -- --cases 20000 --game-cases 200 --seed 20261122   --python .venv-riichienv/Scripts/python --out difftest-riichienv-out
```
- 結果は `difftest-riichienv-out/summary.json`（層ごとの比較・除外・不一致、変換表の件数）。不一致は `*_failures.jsonl`
- 変換表に当たった手の例（`conversions.jsonl`）は `fixtures/difftest-riichienv/` に写して回帰テストにする

### RL 環境（mj-py、PyO3・maturin）
```bash
python -m venv .venv-mjpy                       # リポジトリの直下（.gitignore 済み）
.venv-mjpy/Scripts/python -m pip install maturin numpy   # Linux は bin/python
cd crates/mj-py && VIRTUAL_ENV=../../.venv-mjpy ../../.venv-mjpy/Scripts/maturin develop --release
../../.venv-mjpy/Scripts/python python/bench_random.py --tables 256 --threads 0 --seconds 15
```
- `cargo test --workspace` は Python なしで回る（PyO3 の部分は feature `python`。CI は `cargo check -p mj-py --features python` で通ることだけ確かめる）
- WSL では Windows の作業ツリーを `git archive` で写し、WSL の中の venv でビルドした（2026-11-29 は WSL から PyPI・crates.io に接続できた）

### 牌譜再生画面（web/、TypeScript・Vite・pnpm）
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked   # crates/mj-wasm の wasm-bindgen と同じ版
corepack enable --install-directory ~/bin pnpm              # ~/bin が PATH にあること（pnpm 9.15.9、package.json の packageManager）
pnpm install
pnpm build:wasm                                             # web/packages/engine/pkg
pnpm --filter @mj/replay dev                                # http://localhost:5173 を開く
```
- コマンドはリポジトリの最上位（`package.json` のある所）で打つ
- PowerShell では Git Bash の `~/bin` が PATH にないので、`pnpm` の代わりに `corepack pnpm` と打つ（`rustup`・`cargo` の2行は同じ）
  ```powershell
  cd C:\Users\tetsu\mahjong-project
  corepack pnpm install
  corepack pnpm build:wasm
  corepack pnpm --filter @mj/replay dev
  ```
  `pnpm` だけで打ちたいときは、管理者の PowerShell で1回だけ `corepack enable pnpm`（`C:\Program Files\nodejs` に書き込むため管理者が要る）
- 画面：サンプルの牌譜を選ぶか、牌譜ファイル（`*.mjlog.jsonl`、全情報版・公開情報版）と注釈（`*.annot.jsonl`）を開く。URL で開くときは `?log=fixtures/rulebot.mjlog.jsonl&annot=fixtures/rulebot.annot.jsonl&kyoku=0&event=30&dev=1&seat=0`
- 操作：進む・戻る（← →）、前の局・次の局、局の選択、巡目のスライダー、自動再生（4段階の速さ）、開発者モード（全員の手牌）、下の席
- 検査：`pnpm typecheck`・`pnpm test`（table-model）・`pnpm build`・`cd web/apps/replay && npx playwright install chromium && npx playwright test`
- 牌譜の TS の型は `web/packages/log-types/src/generated`（mj-log から生成。手書きしない）。mj-log の型を変えたら `MJ_UPDATE_TS=1 cargo test -p mj-log --features ts --test ts_types`
- 画面のスナップショット：CI（ubuntu-24.04）で撮ったものだけを正とする（フォントで画素が変わる）。撮り直しは GitHub Actions の `web-snapshots` を手で起動 → 成果物 `replay-snapshots` を `web/apps/replay/tests/__screenshots__/` に置いてコミット（`gh workflow run web-snapshots.yml` → `gh run download <id> -n replay-snapshots -D web/apps/replay/tests/__screenshots__`）
- 試験用の牌譜（`web/apps/replay/public/fixtures`）の作り直し：`cargo run --release -p mj-py --example replay_fixtures -- web/apps/replay/public/fixtures`（mj-py の自己対戦・RuleBot 同士・その公開情報版・注釈）

### 自己対戦の速さ
```bash
cargo run --release -p mj-core --example selfplay_bench -- --seconds 10 --mode single --threads 1
```

---

## 5. 片付け（Phase 0 の作業で作ったもの）
| 場所 | 内容 | 消してよいか |
|---|---|---|
| `C:\rbench` | RiichiEnv のベンチ用 venv、ビルド成果物、依存の vendor | 消してよい（ベンチのソースは `docs/research/bench/` に保存済み） |
| WSL の `~/rbench` | RiichiEnv のコピーとベンチ | 消してよい |
| `%USERPROFILE%\.cargo`, `%USERPROFILE%\.rustup` | rustup と Rust | 開発に使うので残す |

---

## 6. push 前の検査（pre-push フック）
main に失敗する状態を push しないため、push の前に CI と同じ検査を手元で回す（2026-11-28 から）。

```bash
# 1回だけ（リポジトリごと）。.githooks/pre-push を使うようにする
git config core.hooksPath .githooks

# 手で回すとき
bash tools/check.sh
```
- `tools/check.sh`：fmt → clippy（`-D warnings`）→ `cargo test --workspace` → ルールカタログが最新か → テスト対応表（`--strict`）→ 質問票の生成部分が最新か（`questions-check`）→ 牌譜サンプルが最新か。1つでも失敗したら止まり、push もされない
- CI にだけあるもの：Windows と Ubuntu の両方で回すこと、majiang-core・RiichiEnv との差分テスト（Node・Python の環境が要るため。手元で回す手順は §4）
- 時間の目安：変更がなければ数十秒、全体の再ビルドが要るときは数分［実測：2026-11-28、Windows ネイティブ］
- フックを飛ばす `git push --no-verify` は使わない（CLAUDE.md「運用のルール」）
