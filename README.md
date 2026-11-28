# 麻雀ゲーム × 自己学習AI プロジェクト

自己対戦の強化学習（RL）で育てた麻雀AIを中核に、
「AI対局の再生 → 学習の配信 → 友人と遊べるゲーム → Steam・スマホアプリ」
と段階的に育てていくプロジェクト。

## ドキュメント構成

| ファイル | 内容 |
|---|---|
| `CLAUDE.md` | Claude Code向けの作業ルール（最初に読むこと） |
| `docs/00_VISION.md` | 目的と全体の進め方（フェーズ） |
| `docs/01_ARCHITECTURE.md` | システム構成（Rustエンジン共有、BE/FE、配信） |
| `docs/02_RL_DESIGN.md` | 強化学習の設計（報酬カリキュラム、評価、運用） |
| `docs/03_FRONTEND.md` | フロントエンドの進め方（2D→3D） |
| `docs/04_RELEASE.md` | リリース戦略とリスク |
| `docs/DECISIONS.md` | 決定事項の記録（なぜそう決めたか） |
| `docs/OPEN_QUESTIONS.md` | 未決事項 |
| `PROMPT_INITIAL.md` | Claude Codeへ最初に渡すプロンプト |
| `docs/spec/LOG_FORMAT.md` | 牌譜（イベント列）形式 v0.4（サンプルは `docs/spec/samples/`） |
| `docs/spec/DECISION_POINTS.md` | 判断ポイント（注釈・RL の行動選択の単位）の仕様 |
| `docs/spec/ACTION_SPACE.md` | RL の行動の番号表（全ルールの和集合、325 通り。実装は mj-features） |
| `docs/spec/OBSERVATION.md` | RL の観測（実装は mj-features、イベント列の上限 192） |
| `docs/spec/SCENARIOS.md` | 狙った局面（手で組んだ牌山）の一覧。オーナーが局面を足せる |
| `docs/spec/RL_TRAINING.md` | 学習の設計（モデル・PPO・対戦相手プール・再開・統計・評価。D-048〜D-059） |
| `docs/spec/RULES_CATALOG.md` | ルールの全項目（Rust の定義から生成。キー・値・意味・既定値） |
| `docs/spec/RULES_PRESETS.md` | ルールプリセット（天鳳三麻・雀魂三麻相当）の値と出典 |
| `docs/spec/RULES_QUESTIONS_MJSOUL.md` | 雀魂三麻の仮の値を確かめる質問票（残り1問） |
| `docs/spec/IMPLICIT_RULES_AUDIT.md` | キーのない挙動・確認なしに決めていた挙動の点検 |
| `docs/spec/SANMA_RULES_DRAFT.md` | 三麻ルールの論点と選択肢 |
| `docs/spec/REWARD_CONFIG.md` | RL の報酬設定（順位点・係数・正規化・抽選）と段位分布 |
| `docs/spec/REPO_LAYOUT.md` | リポジトリ構成 |
| `docs/research/ENGINE_SURVEY.md` | 既存エンジンの調査・性能の実測・差分テストの方針 |
| `docs/research/DEV_SETUP.md` | 開発環境の準備（Windows の長いパス、Rust、WSL） |

## 最重要の原則

1. **ルールエンジンは1つ（Rust）**。学習・サーバー・ブラウザすべてで同じ実装を使う
2. **牌譜（イベント列）がすべての契約**。再生・対戦・学習・配信は同じイベント列を読み書きする
3. **天鳳の牌譜は使わない**。学習データはすべて自己対戦で自前生成する
4. **サーバー権威型**。牌山や他家の手牌はクライアントへ送らない

## 牌譜再生画面（2D、開発者向け）
`pnpm install && pnpm build:wasm && pnpm --filter @mj/replay dev`（準備は `docs/research/DEV_SETUP.md`「牌譜再生画面」）。画面の仕様は `docs/spec/REPLAY_UI.md`。
卓の状態は Rust（`mj_log::view`）が作り、WASM（`crates/mj-wasm`）経由で画面に渡す。画面側にルールの処理は書かない。
