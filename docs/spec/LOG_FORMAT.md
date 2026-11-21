# LOG FORMAT — 牌譜（イベント列）形式 v0.2

状態：**承認済みの方針（D-014）に基づく v0.2**。

## 変更履歴
| 版 | 内容 |
|---|---|
| 0.1 | 初版（Phase 0 の案） |
| 0.2 | レビューでの修正：注釈のキーに `seat` を追加し見送りの判断にも付けられるようにした／`hora.hand.tehai` に和了牌を含まないことを明記／`ryukyoku` の全情報版・公開版のフィールドを1つの表に統一（`hidden_hands` を廃止し `hands` に一本化）／`wall_salt` は暗号論的乱数で作りシードから導出しないことを明記／祝儀のフィールド（`kyoku_end.chips`、`game_end.chips_total`）を追加／暗槓の中身と流局時の手牌公開範囲をルール設定 `log_visibility` で変えられるようにした。**追加の変更（Claude Code の提案）**：牌山を `live`＋`rinshan` の2列から `pile` の1列（先頭からツモ・末尾から嶺上）に変えた（§4.2。理由：天鳳三麻の「嶺上牌は8枚」と、嶺上を引くたびにハイテイがずれる処理を、王牌の枚数をルール値にして表すため）。`seed.rng = "manual"`（手で組んだ牌山）を追加 |

---

## 0. 方針
**独自の JSON Lines 形式。イベント名と牌の表記は MJAI に合わせ、MJAI へは変換器で落とす**（D-014）。

## 1. ファイルの単位
- **1ファイル = 1対局**、UTF-8 の JSON Lines（1行1イベント）、改行は LF
- 拡張子 `.mjlog.jsonl`（圧縮時 `.mjlog.jsonl.gz`）
- 大量保存はシャード（1シャード ≤ 10,000 対局かつ ≤ 256 MB を目安）にし、シャードごとに `manifest.json`（game_id の一覧・対局数・イベント数・sha256）を置く。読み込み側は manifest の件数と実際の件数を照合する
- 容量の目安：約 15 KB/局、約 135 KB/半荘（無圧縮）[推測：v0.1 のサンプルと RiichiEnv のイベント数 116.8/局 からの換算]

## 2. 共通フィールド（全イベント必須）
| フィールド | 型 | 内容 |
|---|---|---|
| `game_id` | string | 対局 ID。全体で一意 |
| `kyoku_idx` | int | 対局内の局番号（0 始まり。本場・連荘も別の局として数える）。対局全体のイベント（`game_start`/`game_end`）は **-1** |
| `event_idx` | int | `kyoku_idx` 内の連番（0 始まり、欠番なし）。局は `kyoku_start` が 0。`kyoku_idx=-1` では `game_start`=0、`game_end`=1 |
| `type` | string | イベント種別（§4） |

- `(game_id, kyoku_idx, event_idx)` で全イベントが一意に決まる
- **全情報版・席視点版・公開情報版で、この3つ組の集合と並びは完全に同じ**（§5）

## 3. 表記
- 牌：MJAI と同じ文字列。`1m`〜`9m`、`1p`〜`9p`、`1s`〜`9s`、`E S W N`（東南西北）、`P F C`（白發中）、赤は末尾 `r`（`5mr`, `5pr`, `5sr`）。非公開は `"?"`
- 手牌の並び：種類の昇順、同じ種類では赤が先（`mj-core` の `Tile` の順序）
- 席：`actor`/`target`/`seat` は**絶対席番号** `0..num_players-1`（席 0 = 起家）。点数・祝儀の配列は絶対席番号順

## 4. イベント

### 4.1 一覧
| type | 主なフィールド |
|---|---|
| `game_start` | `format_version`("0.2"), `visibility`, `engine{name,version,git}`, `rules`（§4.3）, `seed{game_seed, rng}`, `players[{seat,name}]` |
| `kyoku_start` | `bakaze`, `kyoku`(1〜), `honba`, `kyotaku`, `oya`, `scores`, `dora_indicator`, `tehais`（席ごと13枚・整列済み）, `wall`（§4.2）, `wall_salt`, `wall_commit` |
| `tsumo` | `actor`, `pai`, `rinshan`（省略時 false。槓・北抜きの補充） |
| `dahai` | `actor`, `pai`, `tsumogiri` |
| `reach` / `reach_accepted` | `actor`（accepted は `scores`, `kyotaku` も） |
| `chi` / `pon` / `daiminkan` | `actor`, `target`, `pai`, `consumed` |
| `ankan` | `actor`, `consumed` |
| `kakan` | `actor`, `pai`, `consumed` |
| `kita` | `actor`, `pai` |
| `dora` | `dora_indicator` |
| `hora` | `actor`, `target`（ツモは actor と同じ）, `pai`（和了牌）, `hand{tehai, melds}`, `yaku[{id,han}]`, `han`, `fu`, `yakuman`, `points`, `ura_indicators`, `deltas`, `scores` |
| `ryukyoku` | §4.4 |
| `kyoku_end` | `reason`, `scores`, `kyotaku`, `honba_next`, `renchan`, `chips`（§4.5） |
| `game_end` | `scores`, `ranks`, `chips_total`（§4.5） |

- **`hora.hand.tehai` は和了牌を含まない**（ロンでもツモでも、和了牌は `pai` だけに書く。ツモ和了なら直前の `tsumo` で引いた牌が `pai`）。`hand.melds` は副露（暗槓・北抜きは含めない。北抜きは `kita` イベントで分かる）
- 「見送り（パス）」はイベントにしない（起きたことだけを記録する）。判断の記録は注釈（§6）に置く
- 立直は `reach` → `dahai` → `reach_accepted`。宣言牌でロンされた場合は `reach_accepted` が出ない
- 同時和了は `hora` を複数並べ、放銃者から見た上家順に並べる
- 形式を変えるときは `format_version` を上げ、この文書を先に更新する（CLAUDE.md 原則2）

### 4.2 牌山（`kyoku_start.wall`、全情報版のみ）
```json
{"pile": [...], "dora_indicators": [...], "ura_indicators": [...]}
```
- `pile`：配牌を除いた山の残り。**通常のツモは先頭から、嶺上ツモ（槓・北抜きの補充）は末尾から**取る
- ツモ山の残りが `rules.wall.reserve` 枚になったら通常のツモは終わり（ハイテイ）。嶺上を引くと残りが減るので、ハイテイの位置が1枚ずつ手前にずれる。嶺上ツモの上限は `rules.wall.rinshan_max`
- `dora_indicators` / `ura_indicators`：表示牌の枠（`rules.wall.dora_indicator_slots` 枚ずつ）。i 番目のカンドラは `dora_indicators[i]`
- 配牌 `tehais` ＋ `pile` ＋表示牌の合計が、ルールの牌の多重集合（`mj-core::wall::tile_multiset`）と一致すること（読み込み時に検証）
- 配り方（何枚ずつ・どこから）に依存しない**正規化した形**。エンジンの配牌手順や RNG を後で変えても古い牌譜を再生できる

### 4.3 ルールとシード
- `game_start.rules`：`mj-core::rules::Rules` をそのまま JSON にしたもの（**全項目必須**。プリセット名ではなく値そのものを書く）。キーは `rules/*.toml` と同じ
- `game_start.seed.rng`：牌山の作り方
  - `"chacha8-v1"`：`game_seed`（u64 を `"0x…"` の16進文字列）と `kyoku_idx` から `mj-core::wall::generate_wall` で生成（定義は `crates/mj-core/src/wall.rs` の冒頭）
  - `"manual"`：手で組んだ牌山（テスト・サンプル・状況再現）。`game_seed` は `null`
- **再生は `wall` だけで完結し、シードを使わない**。シードは対局の作り直しとデュプリケート評価（同じ牌山を席替えして使う）のためのもの
- 同じ `rules` ＋ `wall` ＋行動列 → 同じイベント列（決定論）。合法手の列挙順も仕様で固定する

### 4.4 流局（`ryukyoku`）
全情報版と公開版のフィールドを1つの表で定義する（v0.1 の `hands`／`hidden_hands` の不統一を解消）。

| フィールド | 型 | 全情報版 | 席視点版（seat:N） | 公開情報版 |
|---|---|---|---|---|
| `reason` | string | `exhaustive` / `kyushu_kyuhai` / `suukaikan` / `all_riichi` / `same_wind_discards` / `all_ron` / `nagashi_mangan` | 同じ | 同じ |
| `tenpai` | bool[人数] | 全員分 | 同じ | 同じ |
| `hands` | (string[] \| null)[人数] | **全員の門前手牌** | 公開範囲の席＋自席。他は `null` | 公開範囲の席。他は `null` |
| `deltas` | int[人数] | 聴牌料・流し満貫の点数移動 | 同じ | 同じ |
| `scores` | int[人数] | 流局後の点数 | 同じ | 同じ |

公開範囲は `rules.log_visibility.ryukyoku_reveal`：`tenpai_only`（既定。聴牌者だけ）／`all`／`none`。九種九牌は宣言者の手牌を公開範囲に含める。

### 4.5 祝儀（D-013）
祝儀は点数とは**別の帳簿**。`scores`・`deltas` には混ぜない。RL の報酬にも入れない。

| イベント | フィールド | 内容 |
|---|---|---|
| `kyoku_end` | `chips.deltas` | int[人数]。その局の祝儀の枚数の移動（合計 0）。`rules.chips.enabled = false` のときは全員 0 |
| `kyoku_end` | `chips.items` | `[{seat, kind, count}]`。`kind` は `ippatsu` / `aka` / `ura` / `yakuman`。和了者が受け取った枚数の内訳（`count` は1人あたり） |
| `game_end` | `chips_total` | int[人数]。対局全体の祝儀の枚数の合計 |

点数への換算（`rules.chips.values.point_value`）は表示側で行い、牌譜には枚数だけを書く。

### 4.6 牌山のコミットと塩（`wall_commit`, `wall_salt`）
- `wall_commit = "sha256:" + hex(sha256(wall_salt ‖ canonical_json({"haipai": tehais, "wall": wall})))`
  - `wall_salt` は16進文字列のまま連結する。`canonical_json` は区切りに空白を入れない（`{"a":1,"b":[2]}`）、キー順は上の順、非 ASCII はエスケープしない
- **`wall_salt` は暗号論的に安全な乱数（OS の CSPRNG）で作った 16 バイトを 32 文字の16進にしたもの。シードから導出しない**
  - 理由：シードはデュプリケート評価や対局の作り直しのために公開・共有することがある。塩をシードから作ると、シードを知る人がコミットから牌山を逆算・照合できてしまい、コミットの意味がなくなる
- 公開情報版にはコミットだけを載せる。対局後に全情報版を公開すれば、牌山が途中で差し替えられていないことを誰でも確かめられる

## 5. 全情報版・席視点版・公開情報版
| 版 | `game_start.visibility` | 用途 |
|---|---|---|
| 全情報版 | `"full"` | 学習・デバッグ・開発者向け再生 |
| 席視点版 | `"seat:N"` | 対戦サーバーから席 N のクライアントへ送る |
| 公開情報版 | `"public"` | 観戦・配信・SNS 共有 |

- 生成は全情報版からの一方向の変換だけ（`mj-log::redact`）。サーバーはクライアントへ送るすべてのイベントをこれに通す（原則4）
- 変換の規則（これ以外のフィールドはそのまま）：

| イベント | フィールド | 席視点版（seat:N） | 公開情報版 |
|---|---|---|---|
| `game_start` | `visibility` | `"seat:N"` | `"public"` |
| `game_start` | `seed.game_seed` | 削除 | 削除 |
| `kyoku_start` | `tehais[i]` | i ≠ N は13個の `"?"` | 全員13個の `"?"` |
| `kyoku_start` | `wall`, `wall_salt` | 削除 | 削除 |
| `tsumo` | `pai` | `actor` ≠ N なら `"?"` | `"?"` |
| `ankan` | `consumed` | `log_visibility.ankan = "closed"` かつ `actor` ≠ N なら4個の `"?"` | `closed` なら4個の `"?"` |
| `ryukyoku` | `hands` | §4.4 | §4.4 |

- 観戦の遅延（何巡遅らせるか）はサーバーの設定で、牌譜形式には入れない（D-014）
- 検証：変換後に (a) 3つ組の集合が同じ、(b) 隠すべき値が残っていない、をテストで確かめる（`crates/mj-log/tests/`）

## 6. 注釈（`*.annot.jsonl`、牌譜とは別ファイル）
AI の判断の記録・行動確率・価値の推定などは牌譜に混ぜず、別ファイルに置く。

- **キー：`(game_id, kyoku_idx, event_idx, seat)`**
- 意味：**「`event_idx` のイベントをきっかけに、`seat` が行った判断」**
  - 自分のツモ（`tsumo`）をきっかけに打牌・立直・北抜き・ツモ和了を選ぶ
  - 他家の打牌（`dahai`・`kakan`・`kita`）をきっかけにポン・ロンなどを**するか見送るか**を選ぶ。**見送りの判断にも注釈を付けられる**（牌譜には見送りのイベントがないので、注釈だけが記録になる）
- フィールド：`decision`（選んだ行動。`dahai:W`, `pon`, `pass`, `hora`, `reach`, `kita` など）、任意で `model`, `policy`（`[[行動, 確率], ...]`）, `value`
- 読み込み時の検証：キーが一意、対応するイベントが存在する、`seat` が人数の範囲内

## 7. サンプル（1局分）
- ファイル：`docs/spec/samples/sample_full.jsonl`（全情報版）、`sample_public.jsonl`（公開情報版）、`sample_annot.jsonl`（注釈）
- 生成と検証：`python -I docs/spec/samples/make_sample_log.py`（`--check` で、コミット済みのファイルと一致するかも確かめる。CI で実行）
  - 検証内容：配牌＋牌山＝三麻108枚、ツモが `pile` の先頭・嶺上が末尾と一致、打牌・副露の牌が手牌にある、打牌後の枚数、点数の合計、公開版と3つ組が一致、注釈のキーが存在するイベントを指す
- Rust 側の検証：`cargo test -p mj-log`（`redact(全情報版)` が `sample_public.jsonl` と1行ずつ一致、`wall_commit` の再計算、`rules` が `Rules` として読めること）
- ルール：`rules/tenhou-sanma.toml` ＋ `fixtures/rules/tenhou-sanma.test-overrides.toml`（未確認項目を埋める**テスト用の仮値**）
- 牌山は手で組んだもの（`seed.rng = "manual"`）

流れ：東1局・親=席0。席1が北抜き → 席0が席2の中をポン → 席1が立直 → 席0の白を席2が見送り（注釈） → 席1のツモ切り 9s を席0がロン（中・ドラ2、3翻30符、親 5800＋供託 1000）。

v0.2 からサンプルの本文はこの文書に貼らず、`docs/spec/samples/` のファイルを正とする（文書とファイルの食い違いを CI で防げないため）。