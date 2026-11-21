# LOG FORMAT — 牌譜（イベント列）形式 仕様案 v0.1（Q-003）

状態：**案**（オーナー未承認）。承認後、`docs/01_ARCHITECTURE.md` の「牌譜」節からこのファイルを参照する。

---

## 0. 方針（推奨案）

**独自の JSON Lines 形式にする。イベント名と牌の表記は MJAI に合わせ、MJAI へは変換器で落とす。**

| 案 | 長所 | 短所 |
|---|---|---|
| MJAI をそのまま使う | 既存ツール（Mortal 系、RiichiEnv の `apply_event`）がそのまま読める | ID（game_id / kyoku_idx / event_idx）、ルール設定・シードの先頭ヘッダ、全情報版／公開情報版の区別、和了の内訳（役・符・点数移動）の標準がない。北抜きは方言（`kita` / `nukidora`）。MJAI は「各プレイヤーへの通信プロトコル」で、保存形式として作られていない |
| 完全な独自形式 | 自由に設計できる | 既存ツールとの突き合わせに一から変換器が要る |
| **独自 JSONL＋MJAI 語彙（推奨）** | 要件をすべて満たせる。MJAI への変換が「余分なフィールドを落とす」程度で済み、RiichiEnv の MJAI 再生を三麻の差分テスト基準に使える | 「MJAI に似ているが MJAI ではない」ので、変換器の境界を明記する必要がある |

---

## 1. ファイルの単位

- **1ファイル = 1対局（半荘・東風・1局戦）**、UTF-8 の JSON Lines（1行1イベント）
- 拡張子 `.mjlog.jsonl`（圧縮時 `.mjlog.jsonl.gz`）
- 自己対戦の大量保存では、複数対局を連結したシャード（1シャード ≤ 10,000 対局 かつ ≤ 256 MB を目安）にし、シャードごとに `manifest.json`（game_id の一覧・対局数・イベント数・sha256）を置く。読み込み側は manifest の件数と実際の件数を照合する
- 容量の目安：ランダム打ちで 116.8 イベント/局 [実測: RiichiEnv, 20 半荘]、サンプルの通常イベント平均 122 バイト・全情報版 `kyoku_start` 903 バイト [実測: `docs/spec/samples/sample_full.jsonl`] → **約 15 KB/局、約 135 KB/半荘（無圧縮）** [推測: 上記からの換算]

## 2. 共通フィールド（全イベント必須）

| フィールド | 型 | 内容 |
|---|---|---|
| `game_id` | string | 対局 ID。全体で一意（例 `g-20261122-000001`。自己対戦ではランナー ID＋連番） |
| `kyoku_idx` | int | 対局内の局番号（0 始まり、本場・連荘も別の局として数える）。対局全体のイベント（`game_start`/`game_end`）は **-1** |
| `event_idx` | int | `kyoku_idx` 内の連番（0 始まり、欠番なし）。局は `kyoku_start` が 0。`kyoku_idx=-1` では `game_start`=0、`game_end`=1 |
| `type` | string | イベント種別（§4） |

- `(game_id, kyoku_idx, event_idx)` で全イベントが一意に決まる
- **全情報版と公開情報版で、この3つ組の集合と並びは完全に同じ**（§5）。公開情報版はフィールドを隠すだけで、イベントの追加・削除・並べ替えをしない

## 3. 表記

### 牌
MJAI と同じ文字列：`1m`〜`9m`、`1p`〜`9p`、`1s`〜`9s`、`E S W N`（東南西北）、`P F C`（白發中）、赤は末尾 `r`（`5pr`, `5sr`, `5mr`）。不明・非公開は `"?"`。

> 注意：MJAI 本家の赤表記は `5pr` 形式。majiang-core は `p0`。変換は変換器の責務。

### 席・方向
- `actor` / `target` は **絶対席番号** `0..num_players-1`。席 0 = 起家
- 親は `kyoku_start.oya`（絶対席番号）で表す。自風は席と親から計算する
- 点数の配列（`scores`, `deltas`）は常に絶対席番号順

## 4. イベント一覧

| type | 主なフィールド | 公開情報版での扱い |
|---|---|---|
| `game_start` | `format_version`, `visibility`, `engine{name,version,git}`, `rules{...}`（ルール設定の全項目）, `seed{game_seed, rng}`, `players[{seat,name}]` | `seed.game_seed` を隠す（`rng` は残す） |
| `kyoku_start` | `bakaze`, `kyoku`(1〜), `honba`, `kyotaku`, `oya`, `scores`, `dora_indicator`, `tehais`, `wall`, `wall_salt`, `wall_commit` | `tehais` を `"?"` に、`wall`・`wall_salt` を削除。`wall_commit` は残す |
| `tsumo` | `actor`, `pai`, `rinshan`(省略時 false) | `pai` を `"?"`。`rinshan` は残す |
| `dahai` | `actor`, `pai`, `tsumogiri` | そのまま |
| `reach` | `actor` | そのまま |
| `reach_accepted` | `actor`, `scores`, `kyotaku` | そのまま |
| `chi` / `pon` / `daiminkan` | `actor`, `target`, `pai`, `consumed` | そのまま |
| `ankan` | `actor`, `consumed` | そのまま（暗槓の中身の公開はルール設定で変えうる → Q-001） |
| `kakan` | `actor`, `pai`, `consumed` | そのまま |
| `kita` | `actor`, `pai` | そのまま |
| `dora` | `dora_indicator` | そのまま |
| `hora` | `actor`, `target`（ツモは actor と同じ）, `pai`, `hand{tehai, melds}`, `yaku[{id,han}]`, `han`, `fu`, `yakuman`, `points`, `ura_indicators`, `deltas`, `scores` | そのまま（和了で公開される情報） |
| `ryukyoku` | `reason`（`exhaustive`/`kyushukyuhai`/`suukaikan`/`sanchaho`/… ）, `tenpai[]`, `hands`（聴牌者のみ公開）, `deltas`, `scores` | 非聴牌者の手牌は含めない（全情報版では `hidden_hands` に全員分を持つ → 公開版で削除） |
| `kyoku_end` | `reason`, `scores`, `kyotaku`, `honba_next`, `renchan` | そのまま |
| `game_end` | `scores`, `ranks`（同点の順位決めはルール設定に従いエンジンが確定させる） | そのまま |

- 「見送り（パス）」はイベントにしない（起きたことだけを記録する）
- 立直は `reach` → `dahai` → `reach_accepted` の3イベント（宣言牌でロンされた場合は `reach_accepted` が出ない）
- 同時和了（ダブロン）は `hora` を複数並べ、`event_idx` 順に上家取りなどの順序を固定する
- 新しいイベント種別やフィールドを足すときは `format_version` を上げ、この文書を先に更新する（CLAUDE.md 原則2）

### 牌山（`kyoku_start.wall`、全情報版のみ）
```json
{"live": [...], "rinshan": [...], "dora_indicators": [5枚], "ura_indicators": [5枚]}
```
- `tehais`（配牌）と `wall` の合計が、ルール設定が定める牌の多重集合と一致すること（読み込み時に検証）
- `live` はツモ順、`rinshan` は嶺上・北抜き補充の順。配り方（何枚ずつ・どこから）に依存しない**正規化した形**で持つ。これにより、エンジンの配牌手順や RNG を後で変えても古い牌譜を再生できる
- 王牌の枚数・嶺上の枚数・北抜き補充の取り方はルール設定（Q-001）で決まる。サンプルは仮に「王牌14＝嶺上4＋ドラ表示5＋裏5」

### シードと再現性
- `game_start.seed.game_seed` と `rng`（アルゴリズム名＋版、例 `chacha8-v1`）から、エンジンが各局の牌山を生成する
- **再生は `wall` だけで完結し、シードを使わない**。シードは「同じ対局を作り直す」「デュプリケート評価で同じ牌山を席替えして使う」ためのもの
- RNG は名前と版を固定した実装を使う（`rand::StdRng` のように版間で中身が変わりうるものは使わない。RiichiEnv の調査で判明 → `docs/research/ENGINE_SURVEY.md`）
- 同じ `rules` ＋ `wall` ＋ 行動列 → 同じイベント列（決定論）。合法手の列挙順も仕様で固定する

### 牌山のコミット（`wall_commit`）
- `wall_commit = "sha256:" + hex(sha256(wall_salt + canonical_json({"haipai": tehais, "wall": wall})))`
- `canonical_json` は区切りに空白を入れない（`separators=(",",":")`）、キー順は上記のとおり
- 公開情報版にはコミットだけを載せる。対局後に全情報版を公開すれば、誰でも「牌山が途中で差し替えられていない」ことを確かめられる（対人戦での公平性の説明に使える）

## 5. 全情報版と公開情報版

| 版 | `game_start.visibility` | 用途 | 見えるもの |
|---|---|---|---|
| 全情報版 | `"full"` | 学習・デバッグ・開発者向け再生（全員の手牌表示） | すべて |
| 席視点版 | `"seat:N"` | 対戦サーバーから席 N のクライアントへ送る | 公開情報＋席 N の配牌・ツモ |
| 公開情報版 | `"public"` | 観戦・配信・SNS 共有 | 公開情報のみ |

- 生成は**全情報版からの一方向の変換だけ**（`redact(full_event, viewer) -> event`）。逆方向はない
- `redact` はエンジンのコアの外（牌譜 crate）に置く（原則5）。サーバーはクライアントへ送るすべてのイベントを `redact` に通す（原則4）
- フィールドごとの扱いは §4 の表のとおり。席視点版では `tehais[N]` と `actor==N` の `tsumo.pai` を残す
- 検証：変換後に (a) 3つ組の集合が同じ、(b) 公開情報版に `wall`・`wall_salt`・`game_seed`・他家の `tehais`／`tsumo.pai`・非聴牌者の手牌が残っていない、をテストで確かめる
- 配信（AI 同士の自己対戦）は全情報版を使ってよい（全員 AI で隠す相手がいない）。人間が参加する対局の配信は公開情報版を使う

## 6. 牌譜に載せないもの（別ファイル）

AI の行動確率・価値の推定・待ち推定などの**注釈**は牌譜に混ぜず、同じ ID で引ける別ファイル（`*.annot.jsonl`）に置く。
```json
{"game_id":"g-...","kyoku_idx":0,"event_idx":12,"model":"ckpt-001200","policy_top":[["reach",0.71],["dahai:8p",0.22]],"value":0.13}
```
- 注釈は「その event_idx の直前の局面で、その行動を選ぶとき」の値とする
- 読み込み時は `(game_id, kyoku_idx, event_idx)` で結合し、対応するイベントが存在すること・`actor` が一致することを検証する

## 7. サンプル（1局分）

条件（すべて仮）：三麻・東1局・親=席0・原点 35000・赤 5p/5s 各1・北抜きあり。ドラ表示 1m → ドラ 9m（三麻の萬子は 1m↔9m で回る）。
流れ：席1が北抜き → 席0が席2の中をポン → 席1が立直 → 席1のツモ切り 9s を席0がロン（中・ドラ2、3翻30符、親 5800＋供託 1000）。

- 生成と検証：`python -I docs/spec/samples/make_sample_log.py`
  - 検証内容：配牌＋牌山＝三麻108枚の多重集合、ツモが牌山の順と一致、打牌・副露の牌が手牌にある、打牌後の枚数 13、点数の合計 105000、公開情報版と3つ組が一致
- 点数の検算：majiang-core `Util.hule` で `fu=30, fanshu=3, defen=5800, fenpei=[6800,-5800,0,0]` [実測]
- ファイル：`docs/spec/samples/sample_full.jsonl`（4,463 バイト）、`sample_public.jsonl`（3,973 バイト）

### 7.1 全情報版（`sample_full.jsonl`）
```jsonl
{"game_id":"g-20261122-000001","kyoku_idx":-1,"event_idx":0,"type":"game_start","format_version":"0.1","visibility":"full","engine":{"name":"mahjong-core","version":"0.0.0","git":"0000000"},"rules":{"ruleset_id":"sanma-draft-0","num_players":3,"_note":"値は Q-001 未決のため仮。フィールド一覧は SANMA_RULES_DRAFT.md を参照","starting_points":35000,"kita":"nukidora","tsumo_loss":true,"red_fives":{"m":0,"p":1,"s":1},"chi":false,"length":"hanchan"},"seed":{"game_seed":"0x5eed00000001","rng":"chacha8-v1"},"players":[{"seat":0,"name":"ai-ckpt-001200"},{"seat":1,"name":"ai-ckpt-000800"},{"seat":2,"name":"rulebot-v1"}]}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":0,"type":"kyoku_start","bakaze":"E","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"scores":[35000,35000,35000],"dora_indicator":"1m","tehais":[["9m","9m","1p","2p","3p","4s","5s","6s","7s","8s","W","C","C"],["2p","3p","4p","5p","6p","7p","3s","3s","5s","6s","E","E","N"],["4p","5pr","6p","7p","8p","9p","1s","2s","3s","S","S","F","C"]],"wall":{"live":["1m","8p","P","E","2s","P","9s","5p","5p","6p","6p","7p","7p","8p","8p","9p","9p","9p","1s","1s","1s","2s","2s","3s","4s","4s","4s","5sr","5s","6s","6s","7s","7s","7s","8s","8s","8s","9s","9s","9s","E","S","S","W","W","W","N","N","N","P","P","F","F","F","C"],"rinshan":["1m","1m","9m","9m"],"dora_indicators":["1m","1p","1p","1p","2p"],"ura_indicators":["2p","3p","3p","4p","4p"]},"wall_salt":"3f9c1a7be2d04c55","wall_commit":"sha256:56954c4b43910559592433ad9b65de3a4f88eea5aad6eab5ea8f7bc1c791f0b1"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":1,"type":"tsumo","actor":0,"pai":"1m"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":2,"type":"dahai","actor":0,"pai":"W","tsumogiri":false}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":3,"type":"tsumo","actor":1,"pai":"8p"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":4,"type":"kita","actor":1,"pai":"N"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":5,"type":"tsumo","actor":1,"pai":"1m","rinshan":true}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":6,"type":"dahai","actor":1,"pai":"1m","tsumogiri":true}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":7,"type":"tsumo","actor":2,"pai":"P"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":8,"type":"dahai","actor":2,"pai":"C","tsumogiri":false}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":9,"type":"pon","actor":0,"target":2,"pai":"C","consumed":["C","C"]}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":10,"type":"dahai","actor":0,"pai":"1m","tsumogiri":false}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":11,"type":"tsumo","actor":1,"pai":"E"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":12,"type":"reach","actor":1}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":13,"type":"dahai","actor":1,"pai":"8p","tsumogiri":false}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":14,"type":"reach_accepted","actor":1,"scores":[35000,34000,35000],"kyotaku":1}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":15,"type":"tsumo","actor":2,"pai":"2s"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":16,"type":"dahai","actor":2,"pai":"F","tsumogiri":false}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":17,"type":"tsumo","actor":0,"pai":"P"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":18,"type":"dahai","actor":0,"pai":"P","tsumogiri":true}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":19,"type":"tsumo","actor":1,"pai":"9s"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":20,"type":"dahai","actor":1,"pai":"9s","tsumogiri":true}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":21,"type":"hora","actor":0,"target":1,"pai":"9s","hand":{"tehai":["9m","9m","1p","2p","3p","4s","5s","6s","7s","8s"],"melds":[{"type":"pon","pai":"C","consumed":["C","C"],"target":2}]},"yaku":[{"id":"yakuhai_chun","han":1},{"id":"dora","han":2}],"han":3,"fu":30,"yakuman":0,"points":5800,"ura_indicators":[],"deltas":[6800,-5800,0],"scores":[41800,28200,35000]}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":22,"type":"kyoku_end","reason":"hora","scores":[41800,28200,35000],"kyotaku":0,"honba_next":1,"renchan":true}
{"game_id":"g-20261122-000001","kyoku_idx":-1,"event_idx":1,"type":"game_end","scores":[41800,28200,35000],"ranks":[1,3,2],"note":"サンプルのため1局で打ち切り"}
```
（`wall.live` の 8 枚目以降は検証用に残り牌を整列して並べたもの。実際の牌山はシャッフルされる。）

### 7.2 公開情報版（`sample_public.jsonl`）
全情報版との違いは次の4種類だけ（他の行は全情報版と1文字も違わない）：
```jsonl
{"game_id":"g-20261122-000001","kyoku_idx":-1,"event_idx":0,"type":"game_start","format_version":"0.1","visibility":"public","engine":{"name":"mahjong-core","version":"0.0.0","git":"0000000"},"rules":{"ruleset_id":"sanma-draft-0","num_players":3,"_note":"値は Q-001 未決のため仮。フィールド一覧は SANMA_RULES_DRAFT.md を参照","starting_points":35000,"kita":"nukidora","tsumo_loss":true,"red_fives":{"m":0,"p":1,"s":1},"chi":false,"length":"hanchan"},"seed":{"rng":"chacha8-v1"},"players":[{"seat":0,"name":"ai-ckpt-001200"},{"seat":1,"name":"ai-ckpt-000800"},{"seat":2,"name":"rulebot-v1"}]}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":0,"type":"kyoku_start","bakaze":"E","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"scores":[35000,35000,35000],"dora_indicator":"1m","tehais":[["?","?","?","?","?","?","?","?","?","?","?","?","?"],["?","?","?","?","?","?","?","?","?","?","?","?","?"],["?","?","?","?","?","?","?","?","?","?","?","?","?"]],"wall_commit":"sha256:56954c4b43910559592433ad9b65de3a4f88eea5aad6eab5ea8f7bc1c791f0b1"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":1,"type":"tsumo","actor":0,"pai":"?"}
{"game_id":"g-20261122-000001","kyoku_idx":0,"event_idx":5,"type":"tsumo","actor":1,"pai":"?","rinshan":true}
```
- `game_start`：`visibility` が `public`、`seed.game_seed` を削除
- `kyoku_start`：`tehais` を `"?"`、`wall`・`wall_salt` を削除（`wall_commit` は残す）
- すべての `tsumo`：`pai` を `"?"`（event_idx 1, 3, 5, 7, 11, 15, 17, 19）
- それ以外（打牌・北抜き・ポン・立直・和了・局終了）はそのまま。全文は `docs/spec/samples/sample_public.jsonl`

## 8. 未決（オーナー確認事項）
1. 方針（独自 JSONL＋MJAI 語彙）でよいか
2. `kyoku_idx=-1` を対局全体のイベントに使う規約でよいか（別案：`game_start`/`game_end` を別ファイルのヘッダにする）
3. `ankan` の中身・`ryukyoku` で公開する手牌の範囲を、ルール設定で変えられるようにするか
4. 観戦の遅延（人間が参加する対局を配信するとき、何巡遅らせるか）はサーバー側の設定とし、牌譜形式には入れない、でよいか
