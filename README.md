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
| `docs/spec/LOG_FORMAT.md` | 牌譜（イベント列）形式 v0.3（サンプルは `docs/spec/samples/`） |
| `docs/spec/DECISION_POINTS.md` | 判断ポイント（注釈・RL の行動選択の単位）の仕様 |
| `docs/spec/RULES_CATALOG.md` | ルールの全項目（Rust の定義から生成。キー・値・意味・既定値） |
| `docs/spec/RULES_PRESETS.md` | ルールプリセット（天鳳三麻・雀魂三麻相当）の値と出典 |
| `docs/spec/SANMA_RULES_DRAFT.md` | 三麻ルールの論点と選択肢 |
| `docs/spec/REPO_LAYOUT.md` | リポジトリ構成 |
| `docs/research/ENGINE_SURVEY.md` | 既存エンジンの調査・性能の実測・差分テストの方針 |
| `docs/research/DEV_SETUP.md` | 開発環境の準備（Windows の長いパス、Rust、WSL） |

## 最重要の原則

1. **ルールエンジンは1つ（Rust）**。学習・サーバー・ブラウザすべてで同じ実装を使う
2. **牌譜（イベント列）がすべての契約**。再生・対戦・学習・配信は同じイベント列を読み書きする
3. **天鳳の牌譜は使わない**。学習データはすべて自己対戦で自前生成する
4. **サーバー権威型**。牌山や他家の手牌はクライアントへ送らない
