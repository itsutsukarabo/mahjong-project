# RULES PRESETS — 天鳳三麻相当・雀魂三麻相当の値と出典

状態：D-011・D-025・D-026 に基づくプリセットの値の記録。データファイルは `rules/tenhou-sanma.toml`（メイン）と `rules/mjsoul-sanma.toml`（次点）。
項目の意味・取りうる値は `docs/spec/RULES_CATALOG.md`（Rust の定義から生成）。

## 原則
- **プリセットはカタログの全項目（85）に値を書く**（D-025）。足りないと `Rules` を作れない（`RulesError::Missing`、テスト `presets_are_complete_and_valid`）
- 値ごとに出典を残す：**公式のルール説明（URL）／オーナー確認＋日付／本PJの値**のどれか
- 公式説明・オーナーの確認がない値は**仮**とし、TOML の `[meta] provisional` に列挙する（テストで、列挙したキーがカタログにあることを確かめる）。仮の値はサービスのルールとして確認したものではない
- 天鳳の牌譜・データは参照していない（ルール説明のページのみ、2026-11-22 取得）
- 「4人打ちを適用」：天鳳の3人打ちの節は「段位戦4人打と異なる部分のみ記載」と明記しているので、3人打ちの節にない項目は4人打ちの節の記述を適用した
- 「雀荘戦との差から」：天鳳の雀荘戦の節は「段位戦と異なる部分のみ記載」と明記しているので、雀荘戦の節にだけある規則（祝儀・切り上げ満貫など）は段位戦にはないと読んだ

## 出典
| 記号 | 出典 | URL |
|---|---|---|
| T3 | 天鳳マニュアル「ルール / 段位戦3人打ち」 | https://tenhou.net/man/#RULE |
| T4 | 天鳳マニュアル「ルール / 段位戦4人打ち」（3人打ちに適用） | https://tenhou.net/man/#RULE |
| TJ | 天鳳マニュアル「ルール / 雀荘戦」「卓種別」 | https://tenhou.net/man/#RULE |
| TY | 天鳳マニュアル「役一覧」 | https://tenhou.net/man/#YAKU |
| オーナー確認（2026-11-22） | 公式説明にない項目・卓の選択についてのオーナーの回答（D-026） | — |
| M1 | 雀魂公式 X「三人麻雀段位戦ではツモ損が有効です」 | https://x.com/MahjongSoul_JP/status/1152125980267315201 |
| M2 | 雀魂公式 X「三人麻雀段位戦では役がある場合、抜き北に対してロンできますが、槍槓の役はつかない仕様」 | https://x.com/MahjongSoul_JP/status/1280051121281626112 |
| M3 | 雀魂公式 X「段位戦では、オーラス終了後に1位が30000点(三人麻雀の場合は40000点)未満の場合南入(半荘戦の場合は西入)」 | https://x.com/mahjongsoul_jp/status/1243085277452591104 |
| M0 | 雀魂公式 X「公式HPに『段位戦のルール説明』を掲載しました」（リンク先は JavaScript で描画され本文を取得できなかった） | https://x.com/mahjongsoul_jp/status/1404725374932582407 |
| 本PJ | 本プロジェクトの決定・牌譜の設定で、サービスのルールではない | — |

- 雀魂の M1〜M3 の本文は、検索結果に表示された投稿本文で確認した（X はログインなしでは本文ページを取得できない）
- 雀魂のルール説明の本体はゲーム内（段位戦「～の間」の「！」）にある。未確認の項目は質問票 `docs/spec/RULES_QUESTIONS_MJSOUL.md` で確認する

## オーナーの回答の設定値への落とし方（2026-11-22 の回答 → キー）
| 回答 | キーと値 | 補足 |
|---|---|---|
| ツモの支払い：ツモ損あり | `points.tsumo_payment = "loss"` | |
| 本場：ロン 1000点、ツモ 1人500点（1本場あたり） | `points.honba_ron = 1000`、`points.honba_tsumo_each = 500` | |
| 流局時の聴牌料：場に 2000点 | `points.noten_penalty_total = 2000` | 聴牌1人なら 2000 を受け取り（ノーテン2人が1000ずつ）、聴牌2人なら 1000 ずつ受け取る（ノーテン1人が2000） |
| 立直の供託：1000点 | `points.riichi_deposit = 1000` | 数値の設定として持つ |
| 西入の終了条件：~~30000点~~ → **40000点**以上の人が出たら終了（2026-11-26 にオーナーが訂正：三人打ちのトップ条件は40000点）。供託を含めるかもフラグ、既定は含めない | `table.extension_threshold = 40000`、`table.extension_threshold_counts_kyotaku = false` | 同じ点数を「南3局の終了時に西入するか」の判定にも使う（カタログ `table.extension_threshold` の定義。トップ条件として入る・抜けるの両方に使うことは訂正で確定） |
| 王牌：常に14枚。表示牌の枠10枚＋pile の末尾4枚。嶺上ツモのたびにハイテイが1枚手前にずれる | `wall.dead_wall = 14`（`wall.dora_indicator_slots = 5` と合わせて、pile の末尾に残す枚数 = 14 − 10 = 4） | `wall.reserve` は廃止し、`WallRules::reserve()` で導く（D-017） |
| 北抜き：任意 | `kita.mandatory = false` | 面子に使えばオタ風は既存の `kita.north_as_meld = "otakaze"`（T3） |
| 北へのロン：抜いたとき・切ったとき、北待ちで役がある人はロンできる | `kita.ron_on_kita = true`、`kita.ron_on_discard = true` | 2つの分岐として別のキーにした |
| 北ロンの槍槓：付かない | `kita.ron_counts_as_chankan = false` | |
| 立直後の北抜き：ツモってきた北を抜くか抜かないかを選べる | `kita.after_riichi = "drawn_only"` | 抜かない場合はツモ切り |
| 3人全員の立直：流局しない | `draws.all_riichi = false` | |
| 第1打が3人とも同じ風牌：流局しない | `draws.same_wind_discards = false` | |
| 放銃者以外の2人が同時にロン：両方とも和了 | `draws.all_ron = false`、`calls.max_ron = 2` | |
| ノーテン宣言：なし | `draws.noten_declaration = false` | |
| 卓：喰いタンあり・赤あり（5p・5s 各1）、東南戦 | `calls.kuitan = true`、`tiles.red_fives = {m=0,p=1,s=1}`、`table.length = "hanchan"` | 旧 `owner_choice` の4キー |

---

## 値の一覧
凡例：*仮* = 公式説明・オーナー確認のない本PJの仮の値（天鳳三麻と同じ値を入れてある。`meta.provisional`）

### A. 対局の形（`table`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `table.num_players` | `3` | T3 | `3` | M1〜M3 |
| `table.length` | `hanchan` | **オーナー確認（2026-11-22）**（東南戦）。T4「東風戦の南入、東南戦の西入あり」（卓で選ぶ） | `hanchan` | *仮*（質問票の回答待ち） |
| `table.extension` | `sudden_death` | T4「東南戦の西入あり、以降サドンデスルールで終了」 | `sudden_death` | *仮*（質問票の回答待ち） |
| `table.extension_threshold` | `40000` | **オーナー確認（2026-11-26 訂正）**「3人麻雀に限っては40000がトップ条件」（2026-11-22 の回答「30000点以上の人が出たら終了」を訂正） | `40000` | M3「1位が30000点(三人麻雀の場合は40000点)未満の場合南入(半荘戦の場合は西入)」 |
| `table.extension_threshold_counts_kyotaku` | `false` | **オーナー確認（2026-11-22）**「供託を判定に含めるかもフラグにし、既定は含めない」（T4 の4人打ちも「30000点(供託未収)以上」） | `false` | *仮*（質問票の回答待ち） |
| `table.extension_last_wind` | `west` | T4 補足「最大局数=…三南:西3局」 | `west` | *仮*（質問票の回答待ち） |
| `table.oorasu_stop` | `auto` | T4「ラス親トップの自動和了り止め、自動聴牌止めあり」 | `auto` | *仮*（質問票の回答待ち） |
| `table.tobi` | `below_zero` | T4「点数がマイナスで飛び終了、マイナス点数も集計、0点は続行」 | `below_zero` | *仮*（質問票の回答待ち） |
| `table.tobi_single_kyoku` | `none` | 本PJ（D-012） | `none` | 本PJ（D-012） |
| `table.tie_break` | `seat_order` | T4「終了時に同点の場合は東1局の風順で順位を決定」 | `seat_order` | *仮*（質問票の回答待ち） |
| `table.leftover_kyotaku` | `top` | T4「終了時の供託はトップ取り」 | `top` | *仮*（質問票の回答待ち） |

### B. 点数（`points`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `points.starting` | `35000` | T3「35000開始の40000点返し」 | `35000` | *仮*（質問票の回答待ち） |
| `points.returning` | `40000` | T3「35000開始の40000点返し」 | `40000` | *仮*（質問票の回答待ち） |
| `points.uma` | `[20000, 0, -20000]` | T3「順位ウマ +20, 0, -20」 | `[20000, 0, -20000]` | *仮*（質問票の回答待ち） |
| `points.tsumo_payment` | `loss` | **オーナー確認（2026-11-22）**「ツモ損あり」 | `loss` | M1「三人麻雀段位戦ではツモ損が有効です」 |
| `points.honba_ron` | `1000` | **オーナー確認（2026-11-22）**「ロン和了 1000点（1本場あたり）」 | `1000` | *仮*（質問票の回答待ち） |
| `points.honba_tsumo_each` | `500` | **オーナー確認（2026-11-22）**「ツモ和了 1人500点（1本場あたり）」 | `500` | *仮*（質問票の回答待ち） |
| `points.noten_penalty_total` | `2000` | **オーナー確認（2026-11-22）**「場に 2000点」 | `2000` | *仮*（質問票の回答待ち） |
| `points.riichi_deposit` | `1000` | **オーナー確認（2026-11-22）**「1000点」 | `1000` | *仮*（質問票の回答待ち） |
| `points.kiriage_mangan` | `false` | 雀荘戦との差から（TJ「切り上げ満貫」） | `false` | *仮*（質問票の回答待ち） |
| `points.kazoe_yakuman` | `true` | TY「数え役満は13翻以上」 | `true` | *仮*（質問票の回答待ち） |
| `points.min_han` | `1` | T4「2飜縛りなし/常に1飜縛り」 | `1` | *仮*（質問票の回答待ち） |
| `points.double_wind_pair_fu` | `4` | T4「連風牌は4符」 | `4` | *仮*（質問票の回答待ち） |
| `points.rinshan_tsumo_fu` | `true` | T4「嶺上ツモも2符加算」 | `true` | *仮*（質問票の回答待ち） |

### C. 牌（`tiles`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `tiles.manzu` | `terminals_only` | T3「二萬～八萬を使用しない」「ドラ表示『一萬』のドラは『九萬』」 | `terminals_only` | *仮*（質問票の回答待ち） |
| `tiles.red_fives.m` | `0` | T3（5m を使わない） | `0` | *仮*（質問票の回答待ち） |
| `tiles.red_fives.p` | `1` | **オーナー確認（2026-11-22）**「赤あり（5p・5s 各1）」。T4「赤ありルールでの枚数：各1枚」 | `1` | *仮*（質問票の回答待ち） |
| `tiles.red_fives.s` | `1` | **オーナー確認（2026-11-22）**「赤あり（5p・5s 各1）」 | `1` | *仮*（質問票の回答待ち） |

### C'. 王牌（`wall`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `wall.dora_indicator_slots` | `5` | T4「カンドラあり」「四槓散了」から 1＋4 枚と導いた（直接の記載なし） | `5` | *仮*（質問票の回答待ち） |
| `wall.dead_wall` | `14` | **オーナー確認（2026-11-22）**「王牌は常に14枚」 | `14` | *仮*（質問票の回答待ち） |
| `wall.rinshan_max` | `8` | T3「嶺上牌は8枚」 | `8` | *仮*（質問票の回答待ち） |

### E. 鳴き・和了（`calls`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `calls.chi` | `false` | T3「チーはできない」 | `false` | *仮*（質問票の回答待ち） |
| `calls.kuitan` | `true` | **オーナー確認（2026-11-22）**「喰いタンあり」。T4「喰い断なし/あり選択」 | `true` | *仮*（質問票の回答待ち） |
| `calls.atozuke` | `true` | T4「後付けあり」 | `true` | *仮*（質問票の回答待ち） |
| `calls.kuikae` | `forbidden` | T4「喰い替えなし」 | `forbidden` | *仮*（質問票の回答待ち） |
| `calls.haitei_call` | `false` | T4「海底牌は鳴けない」 | `false` | *仮*（質問票の回答待ち） |
| `calls.max_ron` | `2` | T4「ダブロンあり」、**オーナー確認（2026-11-22）**「放銃者以外の2人が同時にロン：両方とも和了」 | `2` | *仮*（質問票の回答待ち） |
| `calls.multi_ron_deposit` | `upstream` | T4「積み棒/供託は上家取り」 | `upstream` | *仮*（質問票の回答待ち） |

### E'. 立直（`riichi`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `riichi.min_points` | `1000` | T4「得点が1000点以上でツモ巡がある場合に可能」 | `1000` | *仮*（質問票の回答待ち） |
| `riichi.requires_draw` | `true` | T4（同上） | `true` | *仮*（質問票の回答待ち） |
| `riichi.furiten_riichi` | `true` | T4「振聴リーチあり」 | `true` | *仮*（質問票の回答待ち） |
| `riichi.ankan_after_riichi` | `wait_unchanged` | T4「待ちが変わらない場合のみ。送り槓不可、牌姿や役の増減は不問」 | `wait_unchanged` | *仮*（質問票の回答待ち） |
| `riichi.no_deposit_on_deal_in` | `true` | T4「リーチ宣言牌で放銃した場合は供託料は発生しない」 | `true` | *仮*（質問票の回答待ち） |

### E''. ドラ・一発（`dora`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `dora.ippatsu` | `true` | TY【懸賞】一発 | `true` | *仮*（質問票の回答待ち） |
| `dora.ura` | `true` | TY【懸賞】裏ドラ | `true` | *仮*（質問票の回答待ち） |
| `dora.kan_dora` | `true` | T4「カンドラ/カン裏あり」 | `true` | *仮*（質問票の回答待ち） |
| `dora.kan_ura` | `true` | T4「カンドラ/カン裏あり」 | `true` | *仮*（質問票の回答待ち） |
| `dora.kan_dora_timing` | `ankan_immediate` | T4「暗槓は即乗り、明槓/加槓は後めくり」 | `ankan_immediate` | *仮*（質問票の回答待ち） |

### D. 北（`kita`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `kita.mode` | `nukidora` | T3「北は抜きドラ」 | `nukidora` | M2（抜き北を前提とした説明） |
| `kita.mandatory` | `false` | **オーナー確認（2026-11-22）**「任意（抜かずに手牌で使える）」 | `false` | *仮*（質問票の回答待ち） |
| `kita.after_pon` | `false` | T3「ポンの直後は抜けない」 | `false` | *仮*（質問票の回答待ち） |
| `kita.after_riichi` | `drawn_only` | **オーナー確認（2026-11-22）**「ツモってきた北を抜くか抜かないかを選べる」 | `drawn_only` | *仮*（質問票の回答待ち） |
| `kita.ron_on_kita` | `true` | T3「抜いた北は役満以外でも和了可能」、**オーナー確認（2026-11-22）**「北を抜いたとき…北待ちで役がある人はロンできる」 | `true` | M2「役がある場合、抜き北に対してロンできます」 |
| `kita.ron_on_discard` | `true` | **オーナー確認（2026-11-22）**「北を切ったとき、北待ちで役がある人はロンできる」 | `true` | *仮*（質問票の回答待ち） |
| `kita.ron_counts_as_chankan` | `false` | **オーナー確認（2026-11-22）**「付かない」 | `false` | M2「槍槓の役はつかない仕様」 |
| `kita.breaks_ippatsu` | `true` | T3「抜きは鳴きと同じ扱い(一発/地和/九種/両立直は消える)」 | `true` | *仮*（質問票の回答待ち） |
| `kita.breaks_first_turn` | `true` | T3（同上） | `true` | *仮*（質問票の回答待ち） |
| `kita.north_as_meld` | `otakaze` | T3「北を面子で使用した場合はオタ風」 | `otakaze` | *仮*（質問票の回答待ち） |
| `kita.dora_per_kita` | `1` | T3「北は抜きドラ」（1枚でドラ1） | `1` | *仮*（質問票の回答待ち） |

### F. 流局（`draws`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `draws.kyushu_kyuhai` | `true` | T4 途中流局「九種九牌」 | `true` | *仮*（質問票の回答待ち） |
| `draws.suukaikan` | `true` | T4 途中流局「四槓散了」 | `true` | *仮*（質問票の回答待ち） |
| `draws.all_riichi` | `false` | **オーナー確認（2026-11-22）**「3人全員の立直：流局しない」 | `false` | *仮*（質問票の回答待ち） |
| `draws.same_wind_discards` | `false` | **オーナー確認（2026-11-22）**「第1打が3人とも同じ風牌：流局しない」 | `false` | *仮*（質問票の回答待ち） |
| `draws.all_ron` | `false` | **オーナー確認（2026-11-22）**「放銃者以外の2人が同時にロン：両方とも和了（ダブロン）」 | `false` | *仮*（質問票の回答待ち） |
| `draws.abortive_renchan` | `true` | T4「すべて連荘」 | `true` | *仮*（質問票の回答待ち） |
| `draws.nagashi_mangan` | `true` | T4「流し満貫あり」 | `true` | *仮*（質問票の回答待ち） |
| `draws.renchan` | `tenpai` | T4「ノーテン親流れ、聴牌連荘」 | `tenpai` | *仮*（質問票の回答待ち） |
| `draws.keishiki_tenpai` | `true` | T4「形式聴牌あり。5枚目を待つ聴牌あり」 | `true` | *仮*（質問票の回答待ち） |
| `draws.noten_declaration` | `false` | **オーナー確認（2026-11-22）**「ノーテン宣言なし」 | `false` | *仮*（質問票の回答待ち） |

### G. 役満（`yakuman`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `yakuman.composite` | `true` | TY「役満は複合あり」 | `true` | *仮*（質問票の回答待ち） |
| `yakuman.double_special` | `false` | TY「四暗刻単騎・国士無双十三面待ち等はすべてシングル役満」 | `false` | *仮*（質問票の回答待ち） |
| `yakuman.pao` | `[daisangen, daisuushii]` | T4「大三元/大四喜のパオあり」「四槓子のパオなし」 | `[daisangen, daisuushii]` | *仮*（質問票の回答待ち） |
| `yakuman.pao_ron_split` | `true` | T4「ツモ＝全額・ロン＝折半」 | `true` | *仮*（質問票の回答待ち） |
| `yakuman.kokushi_ron_on_ankan` | `false` | TY「他家が暗槓した牌では国士無双はあがれない」 | `false` | *仮*（質問票の回答待ち） |

### H. 祝儀（`chips`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `chips.enabled` | `false` | 雀荘戦との差から（TJ「祝儀は一発/赤/裏/役満」は雀荘戦だけ） | `false` | *仮*（質問票の回答待ち） |
| `chips.values.ippatsu` | `1` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `1` | *仮*（質問票の回答待ち） |
| `chips.values.aka` | `1` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `1` | *仮*（質問票の回答待ち） |
| `chips.values.ura` | `1` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `1` | *仮*（質問票の回答待ち） |
| `chips.values.aka_when_open` | `true` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `true` | *仮*（質問票の回答待ち） |
| `chips.values.yakuman_ron` | `10` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `10` | *仮*（質問票の回答待ち） |
| `chips.values.yakuman_tsumo_each` | `5` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `5` | *仮*（質問票の回答待ち） |
| `chips.values.kazoe_yakuman_counts` | `false` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `false` | *仮*（質問票の回答待ち） |
| `chips.values.point_value` | `5000` | 本PJ（祝儀なしなので使わない。値は参考として TJ の雀荘戦） | `5000` | *仮*（質問票の回答待ち） |

### I. 牌譜の公開範囲（`log_visibility`）

| キー | 天鳳三麻 | 出典・根拠 | 雀魂三麻 | 出典 |
|---|---|---|---|---|
| `log_visibility.ankan` | `open` | 本PJ（LOG_FORMAT §5） | `open` | 本PJ |
| `log_visibility.ryukyoku_reveal` | `tenpai_only` | 本PJ（LOG_FORMAT §4.4） | `tenpai_only` | 本PJ |

---

## 集計（`python` で `rules/*.toml` を数えた値。全 85 キー）

| | 公式で確認 | オーナー確認（2026-11-22） | 本PJの値 | 仮 |
|---|---|---|---|---|
| 天鳳三麻 | 52 | 22 | 11（`tobi_single_kyoku`・`log_visibility` 2つ・祝儀の値 8つ） | **0** |
| 雀魂三麻 | 6 | 0 | 3 | **76** |

- 天鳳三麻の未確認の項目は**なくなった**（テスト `tenhou_preset_has_no_provisional_values`）
- 祝儀の値（`chips.values.*`）は、天鳳三麻では `chips.enabled = false` なので使わない。参考として天鳳の雀荘戦（TJ）の値を入れ、本PJの値として数えた

## 両プリセットの差分（確認済みの値どうし）
なし（2026-11-26 の訂正で `table.extension_threshold` は両方 40000 になり、差分ではなくなった）。

確認済みの値（`num_players`・`tsumo_payment`・`table.extension_threshold`・`kita.mode`・`kita.ron_on_kita`・`kita.ron_counts_as_chankan`）は同じ。

## 確認してほしい点
（なし。2026-11-26 の訂正で `table.extension_threshold` の解釈は「トップ条件＝入る・抜けるの両方」に確定）
