# RULES CATALOG — ルールの全項目

<!-- このファイルは生成物。手で編集しない。`cargo run -p rules-catalog -- gen` で作り直す（CI の `rules-catalog -- check` で最新か確かめる） -->

状態：D-025 に基づくルールの体系。**正は Rust の定義**（`crates/mj-core/src/rules.rs` の `Rules` と列挙型、`crates/mj-core/src/catalog.rs`）。

- キーは `rules/*.toml` と牌譜の `game_start.rules` で同じ
- **既定値**はメインのプリセット（天鳳三麻相当、D-011）の値。プリセットはすべての項目に値を書く（`docs/spec/RULES_PRESETS.md`）
- テスト対応表：`fixtures/rules/coverage.toml`。下の「値」の行ごとに、その分岐を確かめるテストを対応付ける（数値の項目は1つ以上）

項目数：**85**、テスト対応表の行数（項目 × 値）：**156**

## A. 対局の形（`table`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `table.num_players` | 人数<br>三麻は3、四麻は4。席0が起家 | 整数 | 3〜4（人） | `3` | `tiles.manzu`<br>`kita.mode`<br>`points.tsumo_payment`<br>`points.uma` |
| `table.length` | 対局の長さ<br>規定の局数。1局戦の飛びは `table.tobi_single_kyoku` で決める（D-012） | 列挙 | `single`：1局戦（1局だけで終わる。本場・連荘なし）<br>`tonpuu`：東風戦（東場だけ）<br>`hanchan`：東南戦（半荘。東場と南場） | `"hanchan"` | `table.extension`<br>`table.tobi_single_kyoku` |
| `table.extension` | 延長戦（南入・西入）<br>規定の局数が終わったときに終了点数（`table.extension_threshold`）以上の人がいない場合の扱い。1局戦では使わない | 列挙 | `none`：延長戦なし。規定の局数で必ず終わる<br>`sudden_death`：延長戦あり。規定の局数が終わった時点で終了点数に届いた人がいなければ次の場に入り、届いた人が出た局の終わりで終了（サドンデス） | `"sudden_death"` | `table.extension_threshold`<br>`table.extension_threshold_counts_kyotaku`<br>`table.extension_last_wind` |
| `table.extension_threshold` | 終了点数（延長戦に入る・抜ける点数）<br>規定の局数が終わったとき、この点数以上の人がいなければ延長戦に入る。延長戦ではこの点数以上の人が出た局の終わりで終了。和了止め・聴牌止めの「終了の条件」にも使う | 整数 | 0〜1000000（点） | `30000` | `table.extension`<br>`table.extension_threshold_counts_kyotaku`<br>`table.oorasu_stop` |
| `table.extension_threshold_counts_kyotaku` | 終了点数の判定に供託を含める<br>終了点数（`table.extension_threshold`）に届いたかの判定のしかた | 真偽値 | `true`：場に残っている供託をトップが受け取るものとして持ち点に足してから判定する<br>`false`：持ち点だけで判定する（供託は含めない） | `false` | `table.extension_threshold`<br>`table.leftover_kyotaku` |
| `table.extension_last_wind` | 延長戦の最後の場<br>延長戦でも、この場の最後の局（三麻は3局、四麻は4局）が終わったら必ず終了する | 列挙 | `east`：東<br>`south`：南<br>`west`：西<br>`north`：北 | `"west"` | `table.extension` |
| `table.oorasu_stop` | 和了止め・聴牌止め<br>最終局（延長戦の局を含む）で親が和了・聴牌したときに終わるかどうか。「終了の条件」は、延長戦ありなら親が終了点数以上であること | 列挙 | `none`：和了止め・聴牌止めなし（最終局で親が和了・聴牌すれば続行）<br>`auto`：最終局で親が和了または聴牌で終えたとき、親が単独トップで終了の条件を満たしていれば自動で終了する | `"auto"` | `table.extension_threshold`<br>`draws.renchan` |
| `table.tobi` | 飛び<br>局の終わりに持ち点がこの条件になった人がいたら対局を終える（東風戦・東南戦） | 列挙 | `none`：飛びなし（持ち点がマイナスでも続ける）<br>`below_zero`：誰かの持ち点が0点未満になったら終了（ちょうど0点は続行）<br>`zero_or_below`：誰かの持ち点が0点以下になったら終了 | `"below_zero"` | `table.tobi_single_kyoku` |
| `table.tobi_single_kyoku` | 1局戦での飛び<br>1局戦（`table.length = "single"`）での飛び。本PJでは飛びなし（D-012）。1局戦は1局で終わるので、飛びは終了の判定ではなく「飛んだ」記録だけに使う | 列挙 | `none`：飛びなし（持ち点がマイナスでも続ける）<br>`below_zero`：誰かの持ち点が0点未満になったら終了（ちょうど0点は続行）<br>`zero_or_below`：誰かの持ち点が0点以下になったら終了 | `"none"` | `table.length`<br>`table.tobi` |
| `table.tie_break` | 同点のときの順位<br>終了時の持ち点が同じ人の順位とウマ・オカの分け方 | 列挙 | `seat_order`：同点なら起家に近い席（東1局の風順）が上位<br>`split`：同点の人どうしでウマ・オカを等分する（順位は起家に近い方を上として表示） | `"seat_order"` | `points.uma`<br>`points.returning` |
| `table.leftover_kyotaku` | 終了時に残った供託<br>対局の終了時に場に残っている立直棒の行き先 | 列挙 | `top`：終了時に場に残った供託（立直棒）はトップが受け取る<br>`discard`：終了時に場に残った供託は誰も受け取らない | `"top"` | `points.riichi_deposit` |

## B. 点数（`points`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `points.starting` | 配給原点<br>開始時の持ち点。1局戦も同じ（D-012） | 整数 | 0〜1000000（点） | `35000` | `points.returning` |
| `points.returning` | 返し点<br>精算の基準点。（返し点 − 配給原点）× 人数をトップがオカとして受け取る | 整数 | 0〜1000000（点） | `40000` | `points.starting`<br>`points.uma` |
| `points.uma` | 順位ウマ<br>精算で順位ごとに足す点数（1位から順、合計0）。RL の報酬の順位点とは別物（D-019） | 整数の配列（人数分） | 各要素 -1000000〜1000000（点）。1位から順に人数分 | `[20000, 0, -20000]` | `table.tie_break`<br>`points.returning` |
| `points.tsumo_payment` | ツモ和了の支払い<br>三麻でのツモ和了の払い方。四麻は `standard` だけ | 列挙 | `standard`：四麻の支払い（子のツモは親が2人分・子が1人分、親のツモは子が2人分ずつ）。四麻専用<br>`loss`：ツモ損。四麻と同じ1人あたりの額を払い、いない北家の分は誰も払わない<br>`split`：いない北家の分を、残りの支払う人で等分して上乗せする（100点単位に切り上げ） | `"loss"` | `table.num_players`<br>`points.honba_tsumo_each` |
| `points.honba_ron` | 本場（ロン）<br>1本場あたり、ロン和了で放銃者が和了者に払う点数 | 整数 | 0〜100000（点） | `1000` | `points.honba_tsumo_each`<br>`calls.multi_ron_deposit` |
| `points.honba_tsumo_each` | 本場（ツモ）<br>1本場あたり、ツモ和了で支払う人1人が払う点数 | 整数 | 0〜100000（点） | `500` | `points.honba_ron` |
| `points.noten_penalty_total` | 聴牌料（ノーテン罰符）<br>荒牌流局のとき場に出る合計。ノーテン者が等分して払い、聴牌者が等分して受け取る（全員聴牌・全員ノーテンなら移動なし） | 整数 | 0〜100000（点） | `2000` | `draws.keishiki_tenpai`<br>`draws.noten_declaration` |
| `points.riichi_deposit` | 立直の供託<br>立直が成立したときに場に出す点数（立直棒）。次の和了者が受け取る | 整数 | 0〜100000（点） | `1000` | `riichi.min_points`<br>`riichi.no_deposit_on_deal_in`<br>`table.leftover_kyotaku` |
| `points.kiriage_mangan` | 切り上げ満貫 | 真偽値 | `true`：4翻30符・3翻60符（子 7700・親 11600）を満貫にする<br>`false`：切り上げない（子 7700・親 11600 のまま） | `false` |  |
| `points.kazoe_yakuman` | 数え役満<br>ドラを含めた翻数で判定する | 真偽値 | `true`：13翻以上を役満にする<br>`false`：13翻以上も三倍満 | `true` | `chips.values.kazoe_yakuman_counts` |
| `points.min_han` | 飜縛り<br>和了に必要な翻数（ドラ・抜きドラは数えない） | 整数 | 1〜13（翻） | `1` |  |
| `points.double_wind_pair_fu` | 連風牌の雀頭の符<br>場風と自風が同じ牌の雀頭の符 | 整数 | 2〜4（符） | `4` |  |
| `points.rinshan_tsumo_fu` | 嶺上開花のツモ符 | 真偽値 | `true`：嶺上開花でもツモ符2符を付ける<br>`false`：嶺上開花にはツモ符を付けない | `true` |  |

## C. 牌（`tiles`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `tiles.manzu` | 萬子<br>四麻は `all` だけ | 列挙 | `all`：萬子を1m〜9mまですべて使う（四麻）<br>`terminals_only`：2m〜8mを抜き、1m・9mだけを使う（三麻）。ドラは1m→9m→1mと回る | `"terminals_only"` | `table.num_players`<br>`tiles.red_fives.m` |
| `tiles.red_fives.m` | 赤5mの枚数<br>萬子が `terminals_only` なら0 | 整数 | 0〜4（枚） | `0` | `tiles.manzu`<br>`chips.values.aka` |
| `tiles.red_fives.p` | 赤5pの枚数 | 整数 | 0〜4（枚） | `1` | `chips.values.aka` |
| `tiles.red_fives.s` | 赤5sの枚数 | 整数 | 0〜4（枚） | `1` | `chips.values.aka` |

## C'. 王牌（`wall`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `wall.dora_indicator_slots` | ドラ表示牌の枠<br>表ドラ1枚＋カンドラの最大数。裏ドラの枠も同じ枚数 | 整数 | 1〜5（枚） | `5` | `wall.dead_wall`<br>`dora.kan_dora` |
| `wall.dead_wall` | 王牌の枚数<br>王牌は常にこの枚数を保つ。牌譜の形では表示牌の枠（`2 × dora_indicator_slots` 枚）＋ `pile` の末尾（残りの枚数。ハイテイの位置）。嶺上ツモ（槓・北抜き）は `pile` の末尾から取るので、そのたびにハイテイが1枚手前にずれる（D-017） | 整数 | 0〜40（枚） | `14` | `wall.dora_indicator_slots`<br>`wall.rinshan_max` |
| `wall.rinshan_max` | 嶺上ツモの最大回数<br>槓と北抜きの補充の合計の上限。上限に達したら槓・北抜きはできない | 整数 | 0〜16（回） | `8` | `wall.dead_wall`<br>`draws.suukaikan` |

## E. 鳴き・和了（`calls`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `calls.chi` | チー | 真偽値 | `true`：上家の打牌をチーできる<br>`false`：チーできない | `false` | `calls.kuikae` |
| `calls.kuitan` | 喰いタン | 真偽値 | `true`：副露していても断么九が付く<br>`false`：断么九は門前だけ | `true` |  |
| `calls.atozuke` | 後付け | 真偽値 | `true`：副露した手は、和了の時点で役があればよい<br>`false`：副露した手は、待ちのどの牌で和了っても役が付く形でなければ和了できない（片和了りの禁止）。判定に使う役は手の形・場風・自風・ドラ以外で決まるもの | `true` | `points.min_han` |
| `calls.kuikae` | 喰い替え<br>ポン・チーの直後の打牌の制限 | 列挙 | `forbidden`：喰い替えなし。鳴いた牌と同じ種類の牌も、チーの筋の牌も、その打牌では切れない<br>`suji_allowed`：鳴いた牌と同じ種類の牌（現物）は切れないが、チーの筋の牌は切れる<br>`allowed`：喰い替えあり。制限なし | `"forbidden"` | `calls.chi` |
| `calls.haitei_call` | 河底牌の鳴き | 真偽値 | `true`：最後のツモ牌を切った牌（河底牌）をポン・チー・大明槓できる<br>`false`：河底牌はロン以外できない | `false` |  |
| `calls.max_ron` | 同時に和了できる人数<br>1つの打牌に対して和了できる最大人数。1 は頭ハネ（放銃者から見て下家から順に優先）。これを超えた人は和了できない | 整数 | 1〜3（人） | `2` | `draws.all_ron`<br>`calls.multi_ron_deposit` |
| `calls.multi_ron_deposit` | 複数の和了の積み棒・供託 | 列挙 | `upstream`：上家取り。複数の和了のとき、積み棒と供託は頭ハネで最優先の和了者（放銃者の下家から数えて最初の和了者。ほかの和了者から見て上家）が全部受け取る<br>`split`：積み棒は各和了者がそれぞれ受け取り、供託は和了者で等分する（1000点単位、余りは上家側から） | `"upstream"` | `calls.max_ron`<br>`points.honba_ron` |

## E'. 立直（`riichi`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `riichi.min_points` | 立直に要る持ち点<br>この点数以上あれば立直できる | 整数 | 0〜100000（点） | `1000` | `points.riichi_deposit` |
| `riichi.requires_draw` | ツモ番のない立直 | 真偽値 | `true`：ツモ山の残り（通常のツモで引ける枚数）が人数以上のときだけ立直できる<br>`false`：残りが何枚でも立直できる | `true` |  |
| `riichi.furiten_riichi` | 振聴立直 | 真偽値 | `true`：振聴でも立直できる<br>`false`：振聴では立直できない | `true` |  |
| `riichi.ankan_after_riichi` | 立直後の暗槓 | 列挙 | `forbidden`：立直後の暗槓はできない<br>`wait_unchanged`：ツモった牌での暗槓で、待ちが変わらなければできる（牌姿・役の増減は問わない。送り槓は不可）<br>`form_unchanged`：ツモった牌での暗槓で、待ちが変わらず、どの面子の取り方でもその牌が刻子のときだけできる | `"wait_unchanged"` |  |
| `riichi.no_deposit_on_deal_in` | 宣言牌での放銃の供託 | 真偽値 | `true`：立直の宣言牌でロンされたら立直は不成立で、供託を出さない<br>`false`：宣言牌でロンされても供託を出す | `true` | `points.riichi_deposit` |

## E''. ドラ・一発（`dora`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `dora.ippatsu` | 一発 | 真偽値 | `true`：一発あり<br>`false`：一発なし | `true` | `kita.breaks_ippatsu`<br>`chips.values.ippatsu` |
| `dora.ura` | 裏ドラ | 真偽値 | `true`：立直して和了したら裏ドラを見る<br>`false`：裏ドラなし | `true` | `dora.kan_ura`<br>`chips.values.ura` |
| `dora.kan_dora` | カンドラ | 真偽値 | `true`：槓のたびにドラ表示牌を1枚めくる<br>`false`：カンドラなし | `true` | `dora.kan_dora_timing`<br>`wall.dora_indicator_slots` |
| `dora.kan_ura` | カン裏 | 真偽値 | `true`：カンドラの下の裏ドラも見る<br>`false`：裏ドラは最初の1枚だけ | `true` | `dora.ura`<br>`dora.kan_dora` |
| `dora.kan_dora_timing` | カンドラをめくる時機 | 列挙 | `ankan_immediate`：暗槓はその場でめくる（即乗り）。大明槓・加槓は嶺上ツモの後の打牌（または次の槓）でめくる（後めくり）<br>`all_immediate`：どの槓もその場でめくる<br>`all_delayed`：どの槓も嶺上ツモの後の打牌（または次の槓）でめくる | `"ankan_immediate"` | `dora.kan_dora` |

## D. 北（`kita`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `kita.mode` | 北抜き<br>四麻は `none` だけ | 列挙 | `none`：北抜きなし（北はふつうの字牌）<br>`nukidora`：北抜きあり。北を抜いて脇に置くとドラになり、嶺上牌を1枚ツモる | `"nukidora"` | `table.num_players` |
| `kita.mandatory` | 北抜きの強制 | 真偽値 | `true`：北を持っていたら抜かなければならない（手牌に残せない）<br>`false`：抜くかどうかは任意。抜かずに手牌で使える | `false` | `kita.north_as_meld` |
| `kita.after_pon` | ポンの直後の北抜き | 真偽値 | `true`：ポンの直後（打牌の前）に北を抜ける<br>`false`：ポンの直後は抜けない | `false` |  |
| `kita.after_riichi` | 立直後の北抜き | 列挙 | `forbidden`：立直後は北を抜けない<br>`drawn_only`：立直後は、ツモってきた北だけを抜ける（抜かずにツモ切りも選べる） | `"drawn_only"` | `kita.mandatory` |
| `kita.ron_on_kita` | 北抜きへのロン | 真偽値 | `true`：誰かが北を抜いたとき、北待ちで役がある人はロンできる<br>`false`：北抜きにはロンできない | `true` | `kita.ron_on_discard`<br>`kita.ron_counts_as_chankan` |
| `kita.ron_on_discard` | 切った北へのロン | 真偽値 | `true`：捨てられた北に、北待ちで役がある人はロンできる<br>`false`：捨てられた北にはロンできない | `true` | `kita.ron_on_kita` |
| `kita.ron_counts_as_chankan` | 北抜きへのロンの槍槓 | 真偽値 | `true`：北抜きへのロンに槍槓（1翻）が付く<br>`false`：槍槓は付かない | `false` | `kita.ron_on_kita` |
| `kita.breaks_ippatsu` | 北抜きで一発が消える | 真偽値 | `true`：誰かが北を抜いたら全員の一発が消える（鳴きと同じ）<br>`false`：北抜きでは一発は消えない | `true` | `dora.ippatsu` |
| `kita.breaks_first_turn` | 北抜きで第1巡が終わる | 真偽値 | `true`：誰かが北を抜いたら地和・九種九牌・ダブル立直の条件（第1巡で鳴きがない）が消える<br>`false`：北抜きでは消えない | `true` | `draws.kyushu_kyuhai` |
| `kita.north_as_meld` | 北を面子に使ったとき | 列挙 | `otakaze`：北を刻子・雀頭に使ったら、自風・場風でなければ役も符もない字牌（オタ風）<br>`yakuhai`：北の刻子は常に役牌（1翻） | `"otakaze"` | `kita.mandatory` |
| `kita.dora_per_kita` | 抜いた北1枚のドラ<br>抜いた北1枚あたりのドラの数（北がドラ表示でドラになっていれば、さらにその分を足す） | 整数 | 0〜4（翻） | `1` | `kita.mode` |

## F. 流局（`draws`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `draws.kyushu_kyuhai` | 九種九牌 | 真偽値 | `true`：第1ツモで么九牌が9種以上あれば流局を宣言できる（鳴き・北抜きの前）<br>`false`：九種九牌の流局なし | `true` | `draws.abortive_renchan`<br>`kita.breaks_first_turn` |
| `draws.suukaikan` | 四槓散了 | 真偽値 | `true`：2人以上で合わせて4回槓したら、4回目の槓の後の打牌が通った時点で流局<br>`false`：四槓散了なし（槓は合わせて4回まで） | `true` | `wall.rinshan_max` |
| `draws.all_riichi` | 全員の立直（四家立直・三家立直） | 真偽値 | `true`：全員が立直したら、最後の立直が成立した時点で流局<br>`false`：流局しない | `false` | `draws.abortive_renchan` |
| `draws.same_wind_discards` | 第1打が全員同じ風牌（四風連打） | 真偽値 | `true`：鳴き・北抜きのない第1巡で全員の第1打が同じ風牌なら流局<br>`false`：流局しない | `false` | `draws.abortive_renchan` |
| `draws.all_ron` | 放銃者以外の全員のロン（三家和・三麻のダブロン） | 真偽値 | `true`：放銃者以外の全員がロンしたら和了にせず流局<br>`false`：流局にしない（`calls.max_ron` まで和了） | `false` | `calls.max_ron` |
| `draws.abortive_renchan` | 途中流局の連荘<br>九種九牌・四槓散了・全員立直・四風連打・全員のロンの流局 | 真偽値 | `true`：途中流局は親の連荘（本場を1つ積む）<br>`false`：途中流局は親流れ（本場は積む） | `true` |  |
| `draws.nagashi_mangan` | 流し満貫<br>本場・供託は動かない。成立したら聴牌料のやりとりはしない（majiang-core と同じ）。連荘の判定はふつうの荒牌流局と同じ | 真偽値 | `true`：荒牌流局のとき、河が么九牌だけで鳴かれていない人は満貫（ツモ和了と同じ払い方）を受け取る<br>`false`：流し満貫なし | `true` | `points.tsumo_payment` |
| `draws.renchan` | 連荘の条件 | 列挙 | `agari`：和了連荘。親が和了したときだけ連荘（流局なら聴牌でも親流れ）<br>`tenpai`：聴牌連荘。親が和了または流局時に聴牌なら連荘（ノーテンなら親流れ）<br>`always`：流局なら親が聴牌でもノーテンでも連荘 | `"tenpai"` | `table.oorasu_stop` |
| `draws.keishiki_tenpai` | 形式聴牌<br>荒牌流局の聴牌の判定 | 真偽値 | `true`：役のない聴牌・手の中の4枚目を待つ聴牌も聴牌とする<br>`false`：役のある聴牌で、待ちの牌が残っている場合だけ聴牌とする | `true` | `points.noten_penalty_total` |
| `draws.noten_declaration` | ノーテン宣言 | 真偽値 | `true`：荒牌流局のとき、聴牌の人は「ノーテン」と宣言して手牌を伏せられる（聴牌料・連荘はノーテン扱い）<br>`false`：聴牌なら必ず聴牌扱い | `false` | `points.noten_penalty_total` |

## G. 役満（`yakuman`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `yakuman.composite` | 役満の複合 | 真偽値 | `true`：複数の役満を足す（ダブル役満など）<br>`false`：役満は1つだけ数える | `true` | `yakuman.double_special` |
| `yakuman.double_special` | ダブル役満の役 | 真偽値 | `true`：四暗刻単騎・国士無双十三面待ち・純正九蓮宝燈・大四喜を2倍役満にする<br>`false`：すべてシングル役満 | `false` | `yakuman.composite` |
| `yakuman.pao` | 包（責任払い）の対象<br>包の人はツモなら全額、ロンなら `yakuman.pao_ron_split` に従って払う | 列挙の集合 | `daisangen`：大三元（三元牌の3つ目の刻子・槓子を鳴かせた人が責任を負う）<br>`daisuushii`：大四喜（風牌の4つ目の刻子・槓子を鳴かせた人が責任を負う）<br>`suukantsu`：四槓子（4つ目の槓子を大明槓させた人が責任を負う） | `["daisangen", "daisuushii"]` | `yakuman.pao_ron_split` |
| `yakuman.pao_ron_split` | 包のロンの払い | 真偽値 | `true`：放銃者と包の人が半分ずつ払う<br>`false`：放銃者が全額払う | `true` | `yakuman.pao` |
| `yakuman.kokushi_ron_on_ankan` | 暗槓への国士無双のロン | 真偽値 | `true`：他家の暗槓に国士無双でロンできる（槍槓）<br>`false`：暗槓にはロンできない | `false` |  |

## H. 祝儀（`chips`、D-013）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `chips.enabled` | 祝儀 | 真偽値 | `true`：祝儀あり。点数とは別の帳簿で枚数を数える（RL の報酬には入れない）<br>`false`：祝儀なし（`chips.values` は使わない） | `false` | `chips.values.point_value` |
| `chips.values.ippatsu` | 一発の祝儀<br>一発で和了したとき、支払う人1人あたり | 整数 | 0〜100（枚） | `1` | `chips.enabled` |
| `chips.values.aka` | 赤牌の祝儀<br>和了した手の赤牌1枚あたり、支払う人1人あたり | 整数 | 0〜100（枚） | `1` | `chips.values.aka_when_open` |
| `chips.values.ura` | 裏ドラの祝儀<br>裏ドラ1つあたり、支払う人1人あたり | 整数 | 0〜100（枚） | `1` | `chips.enabled` |
| `chips.values.aka_when_open` | 副露した手の赤牌の祝儀 | 真偽値 | `true`：副露していても赤牌の祝儀が付く<br>`false`：門前のときだけ付く | `true` | `chips.values.aka` |
| `chips.values.yakuman_ron` | 役満のロンの祝儀<br>役満をロンしたとき、放銃者が払う枚数 | 整数 | 0〜100（枚） | `10` | `chips.values.kazoe_yakuman_counts` |
| `chips.values.yakuman_tsumo_each` | 役満のツモの祝儀<br>役満をツモしたとき、支払う人1人あたり | 整数 | 0〜100（枚） | `5` | `chips.values.kazoe_yakuman_counts` |
| `chips.values.kazoe_yakuman_counts` | 数え役満の役満祝儀 | 真偽値 | `true`：数え役満にも役満の祝儀が付く<br>`false`：数え役満には役満の祝儀が付かない | `false` | `points.kazoe_yakuman` |
| `chips.values.point_value` | 祝儀1枚の点数換算<br>表示用。牌譜の `scores` には混ぜない（LOG_FORMAT §4.5） | 整数 | 0〜1000000（点） | `5000` | `chips.enabled` |

## I. 牌譜の公開範囲（`log_visibility`）

| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |
|---|---|---|---|---|---|
| `log_visibility.ankan` | 暗槓の公開<br>本PJの牌譜の設定で、サービスのルールではない（LOG_FORMAT §5） | 列挙 | `open`：暗槓した牌の種類を公開版の牌譜にも書く<br>`closed`：暗槓した牌を、本人以外の牌譜では "?" にする | `"open"` |  |
| `log_visibility.ryukyoku_reveal` | 流局時の手牌の公開<br>本PJの牌譜の設定で、サービスのルールではない（LOG_FORMAT §4.4） | 列挙 | `tenpai_only`：流局時は聴牌者の手牌だけを公開する（九種九牌は宣言者の手牌も）<br>`all`：流局時は全員の手牌を公開する<br>`none`：流局時に手牌を公開しない（九種九牌の宣言者を除く） | `"tenpai_only"` |  |

