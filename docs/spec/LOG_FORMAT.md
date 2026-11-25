# LOG FORMAT — 牌譜（イベント列）形式 v0.3

状態：**承認済みの方針（D-014・D-017・D-022）に基づく v0.3**。

## 変更履歴
| 版 | 内容 |
|---|---|
| 0.1 | 初版（Phase 0 の案） |
| 0.2 | レビューでの修正：注釈のキーに `seat` を追加し見送りの判断にも付けられるようにした／`hora.hand.tehai` に和了牌を含まないことを明記／`ryukyoku` の全情報版・公開版のフィールドを1つの表に統一（`hidden_hands` を廃止し `hands` に一本化）／`wall_salt` は暗号論的乱数で作りシードから導出しないことを明記／祝儀のフィールド（`kyoku_end.chips`、`game_end.chips_total`）を追加／暗槓の中身と流局時の手牌公開範囲をルール設定 `log_visibility` で変えられるようにした。**追加の変更（Claude Code の提案）**：牌山を `live`＋`rinshan` の2列から `pile` の1列（先頭からツモ・末尾から嶺上）に変えた（§4.2。理由：天鳳三麻の「嶺上牌は8枚」と、嶺上を引くたびにハイテイがずれる処理を、王牌の枚数をルール値にして表すため）。`seed.rng = "manual"`（手で組んだ牌山）を追加 |
| 0.3 | 注釈を「判断ポイント」で定義した（D-022、§6、`docs/spec/DECISION_POINTS.md`）：キーの `event_idx` は判断ポイントの直前のイベント、`decision` の表記を DECISION_POINTS §5 に統一（`hora` → `ron`／`tsumo`、`pon` → `pon:C,C` など）、任意のフィールド `choices` を追加／牌山の1列化を承認（D-017）し、ハイテイの位置を `rules.wall.dead_wall`（王牌の枚数）から導く形にした（`rules.wall.reserve` は廃止）／同時和了の `hora` の並びを「放銃者の下家から順（頭ハネの優先順）」と書き直した（v0.2 の「上家順」は誤り）／ルールの北ロンのキーを `kita.ron_on_kita`・`kita.ron_on_discard` に分けた。**局進行（mj-core）の実装に合わせた追加**：`hora.pao`（包の人）、役満の役は `{id, yakuman}`（倍数）で書く、`hora.hand.melds` に暗槓も含める、`ryukyoku.actor`（九種九牌の宣言者）、`game_end.settlement`（精算）・`game_end.tobi`（飛んだ席） |

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
| `game_start` | `format_version`("0.3"), `visibility`, `engine{name,version,git}`, `rules`（§4.3）, `seed{game_seed, rng}`, `players[{seat,name}]` |
| `kyoku_start` | `bakaze`, `kyoku`(1〜), `honba`, `kyotaku`, `oya`, `scores`, `dora_indicator`, `tehais`（席ごと13枚・整列済み）, `wall`（§4.2）, `wall_salt`, `wall_commit` |
| `tsumo` | `actor`, `pai`, `rinshan`（省略時 false。槓・北抜きの補充） |
| `dahai` | `actor`, `pai`, `tsumogiri` |
| `reach` / `reach_accepted` | `actor`（accepted は `scores`, `kyotaku` も） |
| `chi` / `pon` / `daiminkan` | `actor`, `target`, `pai`, `consumed` |
| `ankan` | `actor`, `consumed` |
| `kakan` | `actor`, `pai`（加えた牌）, `consumed`（元のポンの3枚） |
| `kita` | `actor`, `pai` |
| `dora` | `dora_indicator` |
| `hora` | `actor`, `target`（ツモは actor と同じ）, `pai`（和了牌）, `hand{tehai, melds}`, `yaku`（役は `{id, han}`、役満は `{id, yakuman}`＝倍数）, `han`（合計。役満なら 0）, `fu`, `yakuman`（役満の倍数の合計。数え役満は 1）, `points`（和了者が受け取る手の値。本場・供託を除く）, `ura_indicators`（立直していなければ空）, `pao`（包の人。あるときだけ）, `deltas`, `scores` |
| `ryukyoku` | §4.4（九種九牌なら宣言者 `actor`） |
| `kyoku_end` | `reason`, `scores`, `kyotaku`, `honba_next`, `renchan`, `chips`（§4.5） |
| `game_end` | `scores`（残った供託の精算後）, `ranks`（1始まり）, `settlement`（持ち点 − 返し点 ＋ ウマ ＋ オカ、点）, `tobi`（飛んだ席、bool[人数]。1局戦は `rules.table.tobi_single_kyoku` で記録だけ）, `chips_total`（§4.5） |

- **`hora.hand.tehai` は和了牌を含まない**（ロンでもツモでも、和了牌は `pai` だけに書く。ツモ和了なら直前の `tsumo` で引いた牌が `pai`）。`hand.melds` は副露と暗槓（`{type, pai, consumed, target}`。`type` は `chi`/`pon`/`daiminkan`/`kakan`/`ankan`、暗槓は `pai`・`target` なしで `consumed` に4枚）。北抜きは含めない（`kita` イベントで分かる）
- 役の `id` は `crates/mj-core/src/yaku.rs` の `Yaku::id`（例：`riichi`, `menzen_tsumo`, `yakuhai_chun`, `yakuhai_bakaze`, `chiitoitsu`, `daisangen`, `dora`, `aka`, `ura`, `kita`＝抜きドラ）
- 「見送り（パス）」はイベントにしない（起きたことだけを記録する）。判断の記録は注釈（§6）に置く
- 大明槓・加槓のカンドラを後めくりにするルールでは、槓の後の打牌の直後（応答の前）に `dora` を出す（majiang-core と同じ順）
- 立直は `reach` → `dahai` → `reach_accepted`。宣言牌でロンされた場合は `reach_accepted` が出ない
- 同時和了は `hora` を複数並べ、放銃者の下家から順（頭ハネの優先順、DECISION_POINTS §6）に並べる
- 形式を変えるときは `format_version` を上げ、この文書を先に更新する（CLAUDE.md 原則2）

### 4.2 牌山（`kyoku_start.wall`、全情報版のみ）
```json
{"pile": [...], "dora_indicators": [...], "ura_indicators": [...]}
```
- `pile`：配牌を除いた山の残り。**通常のツモは先頭から、嶺上ツモ（槓・北抜きの補充）は末尾から**取る
- 王牌は常に `rules.wall.dead_wall` 枚（D-017）。そのうち表示牌の枠が `2 × rules.wall.dora_indicator_slots` 枚で、残りの `reserve = dead_wall − 2 × dora_indicator_slots` 枚が `pile` の末尾にある
- ツモ山の残り（`pile` のまだ引いていない枚数）が `reserve` 枚になったら通常のツモは終わり（ハイテイ）。嶺上を引くと残りが減るので、ハイテイの位置が1枚ずつ手前にずれる（王牌が14枚を保つ）。嶺上ツモの上限は `rules.wall.rinshan_max`
- 天鳳三麻相当：`dead_wall = 14`、枠 5 → `reserve = 4`。`pile` 59 枚のうち通常のツモは最大 55 枚
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

- **注釈は「判断ポイント」ごとに1つ**（D-022）。判断ポイント＝エンジンがある席に選択肢（見送りを含む）を2つ以上提示した瞬間。種類の一覧と、キーが一意になる理由は `docs/spec/DECISION_POINTS.md`
- **キー：`(game_id, kyoku_idx, event_idx, seat)`**。`event_idx` は**判断ポイントの直前に起きたイベント**
  - 例：ツモ後の打牌 → その `tsumo`／他家の打牌へのポン・ロン・見送り → その `dahai`／ポン後の打牌 → その `pon`／北抜きへのロン → その `kita`／後めくりのカンドラがある打牌への応答 → その直後の `dora`
  - 同じ (event_idx, seat) に判断ポイントが2つ生じないことはエンジンの設計で保証する（DECISION_POINTS §3：1場1席1判断、立直などの複合行動、場の解決で必ずイベントが出る）
  - **見送りの判断にも注釈を付けられる**（牌譜には見送りのイベントがないので、注釈だけが記録になる）
- フィールド

| フィールド | 必須 | 内容 |
|---|---|---|
| `game_id`, `kyoku_idx`, `event_idx`, `seat` | 必須 | キー |
| `decision` | 必須 | 選んだ行動。表記は DECISION_POINTS §5（`dahai:W`, `dahai:9s:tsumogiri`, `reach:8p`, `pon:C,C`, `ron`, `tsumo`, `pass` など） |
| `choices` | 任意 | 提示された選択肢の全部（DECISION_POINTS §5 の並び順）。書く場合は2つ以上で、`decision` を含む |
| `model` | 任意 | 判断した打ち手（モデルのチェックポイント名・ボット名） |
| `policy` | 任意 | `[[行動, 確率], ...]`。行動は `choices` と同じ表記 |
| `value` | 任意 | 価値の推定 |

- 読み込み時の検証：キーが一意、対応するイベントが存在する、`seat` が人数の範囲内、`choices` があれば2つ以上で `decision` を含む、`policy` の行動が `choices` に含まれる
- エンジンで牌譜を再生し、注釈の位置が実際の判断ポイントで `choices` がエンジンの選択肢と一致することを確かめる検証は、局進行の実装（`mj-core`）に合わせて足す

## 7. サンプル（1局分）
- ファイル：`docs/spec/samples/sample_full.jsonl`（全情報版）、`sample_public.jsonl`（公開情報版）、`sample_annot.jsonl`（注釈）
- 生成と検証：`python -I docs/spec/samples/make_sample_log.py`（`--check` で、コミット済みのファイルと一致するかも確かめる。CI で実行）
  - 検証内容：配牌＋牌山＝三麻108枚、ツモが `pile` の先頭・嶺上が末尾と一致、打牌・副露の牌が手牌にある、打牌後の枚数、点数の合計、公開版と3つ組が一致、注釈のキーが存在するイベントを指す
- Rust 側の検証：`cargo test -p mj-log`（`redact(全情報版)` が `sample_public.jsonl` と1行ずつ一致、`wall_commit` の再計算、`rules` が `Rules` として読めること）
- ルール：`rules/tenhou-sanma.toml`（天鳳三麻相当のプリセットそのまま）
- 牌山は手で組んだもの（`seed.rng = "manual"`）

流れ：東1局・親=席0。席1が北抜き → 席0が席2の中をポン → 席1が立直 → 席0の白を席2がポンせず見送り（注釈） → 席1のツモ切り 9s を席0がロン（中・ドラ2、3翻30符、親 5800＋供託 1000）。

v0.2 からサンプルの本文はこの文書に貼らず、`docs/spec/samples/` のファイルを正とする（文書とファイルの食い違いを CI で防げないため）。