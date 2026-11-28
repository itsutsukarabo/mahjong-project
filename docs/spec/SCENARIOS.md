# SCENARIOS — 狙った局面（手で組んだ牌山）

状態：D-047 の初版（2026-11-29）。

自己対戦ではめったに起きない局面（槍槓・北抜きへのロン・包・流し満貫など）を、牌山を手で組んで必ず起こし、
本PJのエンジンの結果を確かめ、比較相手（三麻は RiichiEnv、四麻は majiang-core）と同じ牌山・同じ行動で並べて比べる。

## 仕組み
- 局面の定義は `fixtures/scenarios/<id>.toml`（1局面1ファイル）。読み込み・台本・期待の確認は `crates/mj-scenarios`
- **仕様テスト**（`cargo test -p mj-scenarios`）：定義のルール（天鳳三麻・test-yonma など）で回し、期待される結果（和了者・放銃者・役・包・点数の移動・流局の種類・珍しい局面の名前）を確かめる
- **差分テスト**：`difftest-riichienv` と `difftest-majiang` が毎回すべて回す（CI も同じ）。比較用のルール（`fixtures/rules/riichienv-sanma.toml`・`majiang-yonma-game.toml`）で台本が通らない局面は「除外」として数える。比較相手と本PJのルールが違う項目は比較用のルールで合わせ、本PJの値は仕様テストで確かめる
- 台本：1要素がその場の1回の判断で `席=行動`（例 `1=pon:5p,5p`、`0=ron`、同じ場で2人なら `1=ron 2=ron`）。台本の次の1歩が「その席に判断ポイントがあり、行動が選択肢にある」場で使い、それ以外の場は既定の行動（見送り → ツモ切り）で進める。台本を使い切らずに局が終わればテストは失敗
- 局面の結果の確認：`cargo run -p mj-scenarios -- show [id]`

## 局面を足す（オーナー向け）
`fixtures/scenarios/<id>.toml` を作り、文章の欄だけを書いて `status = "requested"` にする。配牌・台本・期待の数値は実装側が埋める。

```toml
id = "my-scenario"                 # ファイル名と同じ
category = "槍槓"                   # 珍しい局面の表の行
title = "短い名前"
description = "局面の説明（麻雀の用語で）"
expect_text = "期待される結果"
source = "出典（ルールの文書・オーナー確認など）"
status = "requested"
```
その後 `cargo run -p mj-scenarios -- gen` でこの文書の一覧が更新される（CI は最新かを確かめる）。

## 一覧（生成）
<!-- BEGIN GENERATED: cargo run -p mj-scenarios -- gen -->

| 局面 | 表の行 | ルール | 比べる相手 | 状態 |
|---|---|---|---|---|
| [槍槓（三麻）](#chankan-sanma) | 槍槓 | tenhou-sanma | riichienv | テストあり |
| [槍槓（四麻）](#chankan-yonma) | 槍槓 | test-yonma | majiang | テストあり |
| [地和（三麻）](#chiihou-sanma) | 天和・地和 | tenhou-sanma | riichienv | テストあり |
| [地和（四麻）](#chiihou-yonma) | 天和・地和 | test-yonma | majiang | テストあり |
| [北抜きの補充での嶺上開花とカンドラ](#kita-rinshan-kan-dora) | 北抜きの補充での嶺上開花 | tenhou-sanma | riichienv | テストあり |
| [流し満貫（三麻）](#nagashi-sanma) | 流し満貫 | tenhou-sanma | riichienv | テストあり |
| [流し満貫（四麻）](#nagashi-yonma) | 流し満貫 | test-yonma | majiang | テストあり |
| [包：複合役満（大三元＋字一色）](#pao-compound) | 包 | tenhou-sanma | riichienv | テストあり |
| [包：大三元のロン](#pao-daisangen-ron) | 包 | tenhou-sanma | riichienv | テストあり |
| [包：大三元のツモ](#pao-daisangen-tsumo) | 包 | tenhou-sanma | riichienv | テストあり |
| [包：大四喜のロン（四麻）](#pao-daisuushii-ron) | 包 | test-yonma | majiang | テストあり |
| [包：大四喜のツモ（四麻）](#pao-daisuushii-tsumo) | 包 | test-yonma | majiang | テストあり |
| [北抜きへのロンでの一発](#ron-on-kita-ippatsu) | 北抜きへのロンでの一発 | tenhou-sanma | riichienv | テストあり |
| [北抜きへのロン](#ron-on-kita) | 北抜きへのロン | tenhou-sanma | riichienv | テストあり |
| [四槓散了（三麻）](#suukaikan-sanma) | 四槓散了 | tenhou-sanma | riichienv | テストあり |
| [四槓散了（四麻）](#suukaikan-yonma) | 四槓散了 | test-yonma | majiang | テストあり |
| [天和（三麻）](#tenhou-sanma) | 天和・地和 | tenhou-sanma | riichienv | テストあり |
| [天和（四麻）](#tenhou-yonma) | 天和・地和 | test-yonma | majiang | テストあり |

### 槍槓（三麻）
<a id="chankan-sanma"></a>`fixtures/scenarios/chankan-sanma.toml`

- **局面**：席0（親）は 4p・6p の嵌張で 5p 待ち（一気通貫）。席2 が切った 5p を席1 がポンする（席0 はこのときロンを見送る）。席1 が後で赤の 5p をツモって加槓し、その 5p で席0 がロンする
- **期待される結果**：席0 の和了に槍槓が付く（一気通貫と合わせて）。見送りの同巡フリテンは席0 の次の打牌で解けている
- **出典**：天鳳 ルール（https://tenhou.net/man/）「槍槓」

### 槍槓（四麻）
<a id="chankan-yonma"></a>`fixtures/scenarios/chankan-yonma.toml`

- **局面**：三麻の槍槓と同じ形に、何もしない席3 を足したもの。席1 が 5p をポンして加槓し、席0 が槍槓でロンする
- **期待される結果**：席0 の和了に槍槓が付く
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 地和（三麻）
<a id="chiihou-sanma"></a>`fixtures/scenarios/chiihou-sanma.toml`

- **局面**：鳴きのない第1巡に、子の席1 が第1ツモ（東）で和了する
- **期待される結果**：地和（役満）
- **出典**：天鳳 ルール（https://tenhou.net/man/）

### 地和（四麻）
<a id="chiihou-yonma"></a>`fixtures/scenarios/chiihou-yonma.toml`

- **局面**：鳴きのない第1巡に、子の席1 が第1ツモで和了する（四麻）
- **期待される結果**：地和（役満）
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 北抜きの補充での嶺上開花とカンドラ
<a id="kita-rinshan-kan-dora"></a>`fixtures/scenarios/kita-rinshan-kan-dora.toml`

- **局面**：席1 が席0 の中を大明槓し、嶺上牌の北をそのまま抜く。北抜きの補充の嶺上牌 2s でツモ和了する
- **期待される結果**：大明槓のカンドラは、北抜きの補充の嶺上牌をツモる直前にめくる（D-040）。和了は嶺上開花
- **出典**：オーナー確認（2026-11-28）D-040

### 流し満貫（三麻）
<a id="nagashi-sanma"></a>`fixtures/scenarios/nagashi-sanma.toml`

- **局面**：席2 のツモ（18回）をすべて么九牌（北を除く）にして、全員がツモ切りで荒牌まで進める。席2 の河は么九牌だけで、鳴かれていない
- **期待される結果**：流し満貫：子の満貫ツモと同じ払い（ツモ損）。聴牌料の清算はない
- **出典**：天鳳 ルール（https://tenhou.net/man/）「流し満貫あり」・オーナー確認（2026-11-28）D-033

### 流し満貫（四麻）
<a id="nagashi-yonma"></a>`fixtures/scenarios/nagashi-yonma.toml`

- **局面**：席3 のツモ（17回）をすべて么九牌（北を除く）にして、全員がツモ切りで荒牌まで進める
- **期待される結果**：流し満貫：子の満貫ツモと同じ払い
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 包：複合役満（大三元＋字一色）
<a id="pao-compound"></a>`fixtures/scenarios/pao-compound.toml`

- **局面**：席1 が白・發・中をポンし（中は席0から）、東の暗刻と南の単騎で大三元＋字一色をツモる
- **期待される結果**：天鳳三麻は複合役満の全額（ダブル役満のツモ 48000）を包の席0 が払う（pao_scope = full）
- **出典**：オーナー確認（2026-11-28）I-3「複合役満は全額が責任払い」

### 包：大三元のロン
<a id="pao-daisangen-ron"></a>`fixtures/scenarios/pao-daisangen-ron.toml`

- **局面**：大三元のツモと同じ形で、包の席0 ではない席2 が 4p を切り、席1 がロンする
- **期待される結果**：ロンは折半：放銃の席2 と包の席0 が半分ずつ払う（pao_ron_split）
- **出典**：天鳳 ルール「ツモ＝全額・ロン＝折半」

### 包：大三元のツモ
<a id="pao-daisangen-tsumo"></a>`fixtures/scenarios/pao-daisangen-tsumo.toml`

- **局面**：席1 が白（席0から）・發（席2から）・中（席0から）をポンして大三元を確定させ、4p 単騎をツモる。3つ目の中を鳴かせた席0 が包
- **期待される結果**：包の席0 が、ツモ損の子の役満ツモの合計（24000）を1人で払う
- **出典**：天鳳 ルール「大三元/大四喜のパオあり」・オーナー確認（2026-11-28）回答 E

### 包：大四喜のロン（四麻）
<a id="pao-daisuushii-ron"></a>`fixtures/scenarios/pao-daisuushii-ron.toml`

- **局面**：大四喜のツモと同じ形で、包の席0 ではない席2 が 4p を切り、席1 がロンする
- **期待される結果**：ロンは折半：放銃の席2 と包の席0 が半分ずつ払う
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 包：大四喜のツモ（四麻）
<a id="pao-daisuushii-tsumo"></a>`fixtures/scenarios/pao-daisuushii-tsumo.toml`

- **局面**：席1 が東（席0から）・南（席2から）・西（席3から）・北（席0から）をポンし、4p 単騎をツモる。4つ目の北を鳴かせた席0 が包
- **期待される結果**：包の席0 が払う（四麻の比較用ルールは包の役満の分だけ。majiang-core と同じ）
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 北抜きへのロンでの一発
<a id="ron-on-kita-ippatsu"></a>`fixtures/scenarios/ron-on-kita-ippatsu.toml`

- **局面**：席1 が北の単騎で立直する。一発の巡目のうちに席0 が北を抜き、席1 がその北でロンする
- **期待される結果**：席1 の和了にダブル立直（第1巡の立直）・一発が付く（D-046：天鳳三麻・雀魂三麻とも「つく」）
- **出典**：オーナー確認（2026-11-28）D-046

### 北抜きへのロン
<a id="ron-on-kita"></a>`fixtures/scenarios/ron-on-kita.toml`

- **局面**：席1 は北の単騎待ち（一気通貫）。親の席0 が第1ツモの後に北を抜き、席1 がその北でロンする
- **期待される結果**：席1 のロン和了。槍槓は付かない（kita.ron_counts_as_chankan = false）
- **出典**：オーナー確認（2026-11-28）・天鳳 ルール（https://tenhou.net/man/）

### 四槓散了（三麻）
<a id="suukaikan-sanma"></a>`fixtures/scenarios/suukaikan-sanma.toml`

- **局面**：席0 が東・南、席1 が西・白を暗槓し、合わせて4つの槓が2人に分かれた。4つ目の槓の後の打牌が通る
- **期待される結果**：四槓散了で流局
- **出典**：天鳳 ルール（https://tenhou.net/man/）

### 四槓散了（四麻）
<a id="suukaikan-yonma"></a>`fixtures/scenarios/suukaikan-yonma.toml`

- **局面**：三麻と同じ形に席3 を足したもの
- **期待される結果**：四槓散了で流局
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

### 天和（三麻）
<a id="tenhou-sanma"></a>`fixtures/scenarios/tenhou-sanma.toml`

- **局面**：親の席0 が配牌と第1ツモ（東）で和了の形になり、ツモ和了する
- **期待される結果**：天和（役満）
- **出典**：天鳳 ルール（https://tenhou.net/man/）

### 天和（四麻）
<a id="tenhou-yonma"></a>`fixtures/scenarios/tenhou-yonma.toml`

- **局面**：親の席0 が第1ツモで和了する（四麻）
- **期待される結果**：天和（役満）
- **出典**：test-yonma（四麻の比較用ルール）・majiang-core の lib/hule.js

<!-- END GENERATED -->
