//! ルールカタログ：全項目のキー・型・取りうる値・意味・既定値・関連項目（D-025）。
//!
//! `docs/spec/RULES_CATALOG.md` はここから生成する（`cargo run -p rules-catalog -- gen`）。
//! 列挙型の値と意味は [`crate::rules`] の `rule_enum!` の定義をそのまま参照するので、
//! 値を足すと自動でカタログ・テスト対応表の検査（`fixtures/rules/coverage.toml`）に現れる。
//!
//! 既定値はメインのプリセット（天鳳三麻相当、D-011）の値。`defaults_equal_main_preset` で一致を確かめる。

use crate::rules::*;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// 真偽値。`on` / `off` はそれぞれの意味
    Bool { on: &'static str, off: &'static str },
    /// 整数
    Int {
        min: i64,
        max: i64,
        unit: &'static str,
    },
    /// 整数の配列（長さは人数）
    IntPerRank {
        min: i64,
        max: i64,
        unit: &'static str,
    },
    /// 列挙値（表記, 意味）
    Enum(&'static [(&'static str, &'static str)]),
    /// 列挙値の集合（配列）
    EnumSet(&'static [(&'static str, &'static str)]),
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Bool { .. } => "真偽値",
            Kind::Int { .. } => "整数",
            Kind::IntPerRank { .. } => "整数の配列（人数分）",
            Kind::Enum(_) => "列挙",
            Kind::EnumSet(_) => "列挙の集合",
        }
    }

    /// テスト対応表で、それぞれにテストが要る「値」。数値は `*`（少なくとも1つ）。
    pub fn coverage_values(&self) -> Vec<&'static str> {
        match self {
            Kind::Bool { .. } => vec!["true", "false"],
            Kind::Int { .. } | Kind::IntPerRank { .. } => vec!["*"],
            Kind::Enum(v) | Kind::EnumSet(v) => v.iter().map(|(s, _)| *s).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub key: &'static str,
    /// 平易な名前
    pub title: &'static str,
    pub kind: Kind,
    /// 説明（麻雀の用語で）
    pub desc: &'static str,
    /// 既定値（TOML の表記）
    pub default: &'static str,
    pub related: &'static [&'static str],
}

pub struct Section {
    pub prefix: &'static str,
    pub title: &'static str,
}

pub const SECTIONS: &[Section] = &[
    Section {
        prefix: "table",
        title: "A. 対局の形（`table`）",
    },
    Section {
        prefix: "points",
        title: "B. 点数（`points`）",
    },
    Section {
        prefix: "tiles",
        title: "C. 牌（`tiles`）",
    },
    Section {
        prefix: "wall",
        title: "C'. 王牌（`wall`）",
    },
    Section {
        prefix: "calls",
        title: "E. 鳴き・和了（`calls`）",
    },
    Section {
        prefix: "riichi",
        title: "E'. 立直（`riichi`）",
    },
    Section {
        prefix: "dora",
        title: "E''. ドラ・一発（`dora`）",
    },
    Section {
        prefix: "kita",
        title: "D. 北（`kita`）",
    },
    Section {
        prefix: "draws",
        title: "F. 流局（`draws`）",
    },
    Section {
        prefix: "yakuman",
        title: "G. 役満（`yakuman`）",
    },
    Section {
        prefix: "chips",
        title: "H. 祝儀（`chips`、D-013）",
    },
    Section {
        prefix: "log_visibility",
        title: "I. 牌譜の公開範囲（`log_visibility`）",
    },
];

const fn b(on: &'static str, off: &'static str) -> Kind {
    Kind::Bool { on, off }
}

const fn int(min: i64, max: i64, unit: &'static str) -> Kind {
    Kind::Int { min, max, unit }
}

/// 全項目（`Rules` を TOML にしたときの葉のキーの集合と同じ。テストで確かめる）。並びは構造体の定義の順。
pub const CATALOG: &[Item] = &[
    // ---- A. table
    Item {
        key: "table.num_players",
        title: "人数",
        kind: int(3, 4, "人"),
        desc: "三麻は3、四麻は4。席0が起家",
        default: "3",
        related: &[
            "tiles.manzu",
            "kita.mode",
            "points.tsumo_payment",
            "points.uma",
        ],
    },
    Item {
        key: "table.length",
        title: "対局の長さ",
        kind: Kind::Enum(GameLength::VALUES),
        desc: "規定の局数。1局戦の飛びは `table.tobi_single_kyoku` で決める（D-012）",
        default: "\"hanchan\"",
        related: &["table.extension", "table.tobi_single_kyoku"],
    },
    Item {
        key: "table.extension",
        title: "延長戦（南入・西入）",
        kind: Kind::Enum(Extension::VALUES),
        desc: "規定の局数が終わったときに終了点数（`table.extension_threshold`）以上の人がいない場合の扱い。1局戦では使わない",
        default: "\"sudden_death\"",
        related: &[
            "table.extension_threshold",
            "table.extension_threshold_counts_kyotaku",
            "table.extension_last_wind",
        ],
    },
    Item {
        key: "table.extension_threshold",
        title: "終了点数（延長戦に入る・抜ける点数）",
        kind: int(0, 1_000_000, "点"),
        desc: "規定の局数が終わったとき、この点数以上の人がいなければ延長戦に入る。延長戦ではこの点数以上の人が出た局の終わりで終了。和了止め・聴牌止めの「終了の条件」にも使う",
        default: "30000",
        related: &[
            "table.extension",
            "table.extension_threshold_counts_kyotaku",
            "table.oorasu_stop",
        ],
    },
    Item {
        key: "table.extension_threshold_counts_kyotaku",
        title: "終了点数の判定に供託を含める",
        kind: b(
            "場に残っている供託をトップが受け取るものとして持ち点に足してから判定する",
            "持ち点だけで判定する（供託は含めない）",
        ),
        desc: "終了点数（`table.extension_threshold`）に届いたかの判定のしかた",
        default: "false",
        related: &["table.extension_threshold", "table.leftover_kyotaku"],
    },
    Item {
        key: "table.extension_last_wind",
        title: "延長戦の最後の場",
        kind: Kind::Enum(Wind::VALUES),
        desc: "延長戦でも、この場の最後の局（三麻は3局、四麻は4局）が終わったら必ず終了する",
        default: "\"west\"",
        related: &["table.extension"],
    },
    Item {
        key: "table.oorasu_stop",
        title: "和了止め・聴牌止め",
        kind: Kind::Enum(OorasuStop::VALUES),
        desc: "最終局（延長戦の局を含む）で親が和了・聴牌したときに終わるかどうか。「終了の条件」は、延長戦ありなら親が終了点数以上であること",
        default: "\"auto\"",
        related: &["table.extension_threshold", "draws.renchan"],
    },
    Item {
        key: "table.tobi",
        title: "飛び",
        kind: Kind::Enum(Tobi::VALUES),
        desc: "局の終わりに持ち点がこの条件になった人がいたら対局を終える（東風戦・東南戦）",
        default: "\"below_zero\"",
        related: &["table.tobi_single_kyoku"],
    },
    Item {
        key: "table.tobi_single_kyoku",
        title: "1局戦での飛び",
        kind: Kind::Enum(Tobi::VALUES),
        desc: "1局戦（`table.length = \"single\"`）での飛び。本PJでは飛びなし（D-012）。1局戦は1局で終わるので、飛びは終了の判定ではなく「飛んだ」記録だけに使う",
        default: "\"none\"",
        related: &["table.length", "table.tobi"],
    },
    Item {
        key: "table.tie_break",
        title: "同点のときの順位",
        kind: Kind::Enum(TieBreak::VALUES),
        desc: "終了時の持ち点が同じ人の順位とウマ・オカの分け方",
        default: "\"seat_order\"",
        related: &["points.uma", "points.returning"],
    },
    Item {
        key: "table.leftover_kyotaku",
        title: "終了時に残った供託",
        kind: Kind::Enum(LeftoverKyotaku::VALUES),
        desc: "対局の終了時に場に残っている立直棒の行き先",
        default: "\"top\"",
        related: &["points.riichi_deposit"],
    },
    // ---- B. points
    Item {
        key: "points.starting",
        title: "配給原点",
        kind: int(0, 1_000_000, "点"),
        desc: "開始時の持ち点。1局戦も同じ（D-012）",
        default: "35000",
        related: &["points.returning"],
    },
    Item {
        key: "points.returning",
        title: "返し点",
        kind: int(0, 1_000_000, "点"),
        desc: "精算の基準点。（返し点 − 配給原点）× 人数をトップがオカとして受け取る",
        default: "40000",
        related: &["points.starting", "points.uma"],
    },
    Item {
        key: "points.uma",
        title: "順位ウマ",
        kind: Kind::IntPerRank {
            min: -1_000_000,
            max: 1_000_000,
            unit: "点",
        },
        desc: "精算で順位ごとに足す点数（1位から順、合計0）。RL の報酬の順位点とは別物（D-019）",
        default: "[20000, 0, -20000]",
        related: &["table.tie_break", "points.returning"],
    },
    Item {
        key: "points.tsumo_payment",
        title: "ツモ和了の支払い",
        kind: Kind::Enum(TsumoPayment::VALUES),
        desc: "三麻でのツモ和了の払い方。四麻は `standard` だけ",
        default: "\"loss\"",
        related: &["table.num_players", "points.honba_tsumo_each"],
    },
    Item {
        key: "points.honba_ron",
        title: "本場（ロン）",
        kind: int(0, 100_000, "点"),
        desc: "1本場あたり、ロン和了で放銃者が和了者に払う点数",
        default: "1000",
        related: &["points.honba_tsumo_each", "calls.multi_ron_deposit"],
    },
    Item {
        key: "points.honba_tsumo_each",
        title: "本場（ツモ）",
        kind: int(0, 100_000, "点"),
        desc: "1本場あたり、ツモ和了で支払う人1人が払う点数",
        default: "500",
        related: &["points.honba_ron"],
    },
    Item {
        key: "points.noten_penalty_total",
        title: "聴牌料（ノーテン罰符）",
        kind: int(0, 100_000, "点"),
        desc: "荒牌流局のとき場に出る合計。ノーテン者が等分して払い、聴牌者が等分して受け取る（全員聴牌・全員ノーテンなら移動なし）",
        default: "2000",
        related: &["draws.keishiki_tenpai", "draws.noten_declaration"],
    },
    Item {
        key: "points.riichi_deposit",
        title: "立直の供託",
        kind: int(0, 100_000, "点"),
        desc: "立直が成立したときに場に出す点数（立直棒）。次の和了者が受け取る",
        default: "1000",
        related: &[
            "riichi.min_points",
            "riichi.no_deposit_on_deal_in",
            "table.leftover_kyotaku",
        ],
    },
    Item {
        key: "points.kiriage_mangan",
        title: "切り上げ満貫",
        kind: b(
            "4翻30符・3翻60符（子 7700・親 11600）を満貫にする",
            "切り上げない（子 7700・親 11600 のまま）",
        ),
        desc: "",
        default: "false",
        related: &[],
    },
    Item {
        key: "points.kazoe_yakuman",
        title: "数え役満",
        kind: b("13翻以上を役満にする", "13翻以上も三倍満"),
        desc: "ドラを含めた翻数で判定する",
        default: "true",
        related: &["chips.values.kazoe_yakuman_counts"],
    },
    Item {
        key: "points.min_han",
        title: "飜縛り",
        kind: int(1, 13, "翻"),
        desc: "和了に必要な翻数（ドラ・抜きドラは数えない）",
        default: "1",
        related: &[],
    },
    Item {
        key: "points.double_wind_pair_fu",
        title: "連風牌の雀頭の符",
        kind: int(2, 4, "符"),
        desc: "場風と自風が同じ牌の雀頭の符",
        default: "4",
        related: &[],
    },
    Item {
        key: "points.rinshan_tsumo_fu",
        title: "嶺上開花のツモ符",
        kind: b(
            "嶺上開花でもツモ符2符を付ける",
            "嶺上開花にはツモ符を付けない",
        ),
        desc: "",
        default: "true",
        related: &[],
    },
    // ---- C. tiles
    Item {
        key: "tiles.manzu",
        title: "萬子",
        kind: Kind::Enum(Manzu::VALUES),
        desc: "四麻は `all` だけ",
        default: "\"terminals_only\"",
        related: &["table.num_players", "tiles.red_fives.m"],
    },
    Item {
        key: "tiles.red_fives.m",
        title: "赤5mの枚数",
        kind: int(0, 4, "枚"),
        desc: "萬子が `terminals_only` なら0",
        default: "0",
        related: &["tiles.manzu", "chips.values.aka"],
    },
    Item {
        key: "tiles.red_fives.p",
        title: "赤5pの枚数",
        kind: int(0, 4, "枚"),
        desc: "",
        default: "1",
        related: &["chips.values.aka"],
    },
    Item {
        key: "tiles.red_fives.s",
        title: "赤5sの枚数",
        kind: int(0, 4, "枚"),
        desc: "",
        default: "1",
        related: &["chips.values.aka"],
    },
    // ---- wall
    Item {
        key: "wall.dora_indicator_slots",
        title: "ドラ表示牌の枠",
        kind: int(1, 5, "枚"),
        desc: "表ドラ1枚＋カンドラの最大数。裏ドラの枠も同じ枚数",
        default: "5",
        related: &["wall.dead_wall", "dora.kan_dora"],
    },
    Item {
        key: "wall.dead_wall",
        title: "王牌の枚数",
        kind: int(0, 40, "枚"),
        desc: "王牌は常にこの枚数を保つ。牌譜の形では表示牌の枠（`2 × dora_indicator_slots` 枚）＋ `pile` の末尾（残りの枚数。ハイテイの位置）。嶺上ツモ（槓・北抜き）は `pile` の末尾から取るので、そのたびにハイテイが1枚手前にずれる（D-017）",
        default: "14",
        related: &["wall.dora_indicator_slots", "wall.rinshan_max"],
    },
    Item {
        key: "wall.rinshan_max",
        title: "嶺上ツモの最大回数",
        kind: int(0, 16, "回"),
        desc: "槓と北抜きの補充の合計の上限。上限に達したら槓・北抜きはできない",
        default: "8",
        related: &["wall.dead_wall", "draws.suukaikan"],
    },
    // ---- E. calls
    Item {
        key: "calls.chi",
        title: "チー",
        kind: b("上家の打牌をチーできる", "チーできない"),
        desc: "",
        default: "false",
        related: &["calls.kuikae"],
    },
    Item {
        key: "calls.kuitan",
        title: "喰いタン",
        kind: b("副露していても断么九が付く", "断么九は門前だけ"),
        desc: "",
        default: "true",
        related: &[],
    },
    Item {
        key: "calls.atozuke",
        title: "後付け",
        kind: b(
            "副露した手は、和了の時点で役があればよい",
            "副露した手は、待ちのどの牌で和了っても役が付く形でなければ和了できない（片和了りの禁止）。判定に使う役は手の形・場風・自風・ドラ以外で決まるもの",
        ),
        desc: "",
        default: "true",
        related: &["points.min_han"],
    },
    Item {
        key: "calls.kuikae",
        title: "喰い替え",
        kind: Kind::Enum(Kuikae::VALUES),
        desc: "ポン・チーの直後の打牌の制限",
        default: "\"forbidden\"",
        related: &["calls.chi"],
    },
    Item {
        key: "calls.haitei_call",
        title: "河底牌の鳴き",
        kind: b(
            "最後のツモ牌を切った牌（河底牌）をポン・チー・大明槓できる",
            "河底牌はロン以外できない",
        ),
        desc: "",
        default: "false",
        related: &[],
    },
    Item {
        key: "calls.max_ron",
        title: "同時に和了できる人数",
        kind: int(1, 3, "人"),
        desc: "1つの打牌に対して和了できる最大人数。1 は頭ハネ（放銃者から見て下家から順に優先）。これを超えた人は和了できない",
        default: "2",
        related: &["draws.all_ron", "calls.multi_ron_deposit"],
    },
    Item {
        key: "calls.multi_ron_deposit",
        title: "複数の和了の積み棒・供託",
        kind: Kind::Enum(MultiRonDeposit::VALUES),
        desc: "",
        default: "\"upstream\"",
        related: &["calls.max_ron", "points.honba_ron"],
    },
    // ---- riichi
    Item {
        key: "riichi.min_points",
        title: "立直に要る持ち点",
        kind: int(0, 100_000, "点"),
        desc: "この点数以上あれば立直できる",
        default: "1000",
        related: &["points.riichi_deposit"],
    },
    Item {
        key: "riichi.requires_draw",
        title: "ツモ番のない立直",
        kind: b(
            "ツモ山の残り（通常のツモで引ける枚数）が人数以上のときだけ立直できる",
            "残りが何枚でも立直できる",
        ),
        desc: "",
        default: "true",
        related: &[],
    },
    Item {
        key: "riichi.furiten_riichi",
        title: "振聴立直",
        kind: b("振聴でも立直できる", "振聴では立直できない"),
        desc: "",
        default: "true",
        related: &[],
    },
    Item {
        key: "riichi.ankan_after_riichi",
        title: "立直後の暗槓",
        kind: Kind::Enum(RiichiAnkan::VALUES),
        desc: "",
        default: "\"wait_unchanged\"",
        related: &[],
    },
    Item {
        key: "riichi.no_deposit_on_deal_in",
        title: "宣言牌での放銃の供託",
        kind: b(
            "立直の宣言牌でロンされたら立直は不成立で、供託を出さない",
            "宣言牌でロンされても供託を出す",
        ),
        desc: "",
        default: "true",
        related: &["points.riichi_deposit"],
    },
    // ---- dora
    Item {
        key: "dora.ippatsu",
        title: "一発",
        kind: b("一発あり", "一発なし"),
        desc: "",
        default: "true",
        related: &["kita.breaks_ippatsu", "chips.values.ippatsu"],
    },
    Item {
        key: "dora.ura",
        title: "裏ドラ",
        kind: b("立直して和了したら裏ドラを見る", "裏ドラなし"),
        desc: "",
        default: "true",
        related: &["dora.kan_ura", "chips.values.ura"],
    },
    Item {
        key: "dora.kan_dora",
        title: "カンドラ",
        kind: b("槓のたびにドラ表示牌を1枚めくる", "カンドラなし"),
        desc: "",
        default: "true",
        related: &["dora.kan_dora_timing", "wall.dora_indicator_slots"],
    },
    Item {
        key: "dora.kan_ura",
        title: "カン裏",
        kind: b("カンドラの下の裏ドラも見る", "裏ドラは最初の1枚だけ"),
        desc: "",
        default: "true",
        related: &["dora.ura", "dora.kan_dora"],
    },
    Item {
        key: "dora.kan_dora_timing",
        title: "カンドラをめくる時機",
        kind: Kind::Enum(KanDoraTiming::VALUES),
        desc: "",
        default: "\"ankan_immediate\"",
        related: &["dora.kan_dora"],
    },
    // ---- D. kita
    Item {
        key: "kita.mode",
        title: "北抜き",
        kind: Kind::Enum(KitaMode::VALUES),
        desc: "四麻は `none` だけ",
        default: "\"nukidora\"",
        related: &["table.num_players"],
    },
    Item {
        key: "kita.mandatory",
        title: "北抜きの強制",
        kind: b(
            "北を持っていたら抜かなければならない（手牌に残せない）",
            "抜くかどうかは任意。抜かずに手牌で使える",
        ),
        desc: "",
        default: "false",
        related: &["kita.north_as_meld"],
    },
    Item {
        key: "kita.after_pon",
        title: "ポンの直後の北抜き",
        kind: b("ポンの直後（打牌の前）に北を抜ける", "ポンの直後は抜けない"),
        desc: "",
        default: "false",
        related: &[],
    },
    Item {
        key: "kita.after_riichi",
        title: "立直後の北抜き",
        kind: Kind::Enum(KitaAfterRiichi::VALUES),
        desc: "",
        default: "\"drawn_only\"",
        related: &["kita.mandatory"],
    },
    Item {
        key: "kita.ron_on_kita",
        title: "北抜きへのロン",
        kind: b(
            "誰かが北を抜いたとき、北待ちで役がある人はロンできる",
            "北抜きにはロンできない",
        ),
        desc: "",
        default: "true",
        related: &["kita.ron_on_discard", "kita.ron_counts_as_chankan"],
    },
    Item {
        key: "kita.ron_on_discard",
        title: "切った北へのロン",
        kind: b(
            "捨てられた北に、北待ちで役がある人はロンできる",
            "捨てられた北にはロンできない",
        ),
        desc: "",
        default: "true",
        related: &["kita.ron_on_kita"],
    },
    Item {
        key: "kita.ron_counts_as_chankan",
        title: "北抜きへのロンの槍槓",
        kind: b("北抜きへのロンに槍槓（1翻）が付く", "槍槓は付かない"),
        desc: "",
        default: "false",
        related: &["kita.ron_on_kita"],
    },
    Item {
        key: "kita.breaks_ippatsu",
        title: "北抜きで一発が消える",
        kind: b(
            "誰かが北を抜いたら全員の一発が消える（鳴きと同じ）",
            "北抜きでは一発は消えない",
        ),
        desc: "",
        default: "true",
        related: &["dora.ippatsu"],
    },
    Item {
        key: "kita.breaks_first_turn",
        title: "北抜きで第1巡が終わる",
        kind: b(
            "誰かが北を抜いたら地和・九種九牌・ダブル立直の条件（第1巡で鳴きがない）が消える",
            "北抜きでは消えない",
        ),
        desc: "",
        default: "true",
        related: &["draws.kyushu_kyuhai"],
    },
    Item {
        key: "kita.north_as_meld",
        title: "北を面子に使ったとき",
        kind: Kind::Enum(NorthAsMeld::VALUES),
        desc: "",
        default: "\"otakaze\"",
        related: &["kita.mandatory"],
    },
    Item {
        key: "kita.dora_per_kita",
        title: "抜いた北1枚のドラ",
        kind: int(0, 4, "翻"),
        desc: "抜いた北1枚あたりのドラの数（北がドラ表示でドラになっていれば、さらにその分を足す）",
        default: "1",
        related: &["kita.mode"],
    },
    // ---- F. draws
    Item {
        key: "draws.kyushu_kyuhai",
        title: "九種九牌",
        kind: b(
            "第1ツモで么九牌が9種以上あれば流局を宣言できる（鳴き・北抜きの前）",
            "九種九牌の流局なし",
        ),
        desc: "",
        default: "true",
        related: &["draws.abortive_renchan", "kita.breaks_first_turn"],
    },
    Item {
        key: "draws.suukaikan",
        title: "四槓散了",
        kind: b(
            "2人以上で合わせて4回槓したら、4回目の槓の後の打牌が通った時点で流局",
            "四槓散了なし（槓は合わせて4回まで）",
        ),
        desc: "",
        default: "true",
        related: &["wall.rinshan_max"],
    },
    Item {
        key: "draws.all_riichi",
        title: "全員の立直（四家立直・三家立直）",
        kind: b(
            "全員が立直したら、最後の立直が成立した時点で流局",
            "流局しない",
        ),
        desc: "",
        default: "false",
        related: &["draws.abortive_renchan"],
    },
    Item {
        key: "draws.same_wind_discards",
        title: "第1打が全員同じ風牌（四風連打）",
        kind: b(
            "鳴き・北抜きのない第1巡で全員の第1打が同じ風牌なら流局",
            "流局しない",
        ),
        desc: "",
        default: "false",
        related: &["draws.abortive_renchan"],
    },
    Item {
        key: "draws.all_ron",
        title: "放銃者以外の全員のロン（三家和・三麻のダブロン）",
        kind: b(
            "放銃者以外の全員がロンしたら和了にせず流局",
            "流局にしない（`calls.max_ron` まで和了）",
        ),
        desc: "",
        default: "false",
        related: &["calls.max_ron"],
    },
    Item {
        key: "draws.abortive_renchan",
        title: "途中流局の連荘",
        kind: b(
            "途中流局は親の連荘（本場を1つ積む）",
            "途中流局は親流れ（本場は積む）",
        ),
        desc: "九種九牌・四槓散了・全員立直・四風連打・全員のロンの流局",
        default: "true",
        related: &[],
    },
    Item {
        key: "draws.nagashi_mangan",
        title: "流し満貫",
        kind: b(
            "荒牌流局のとき、河が么九牌だけで鳴かれていない人は満貫（ツモ和了と同じ払い方）を受け取る",
            "流し満貫なし",
        ),
        desc: "本場・供託は動かない。聴牌料と連荘の判定はふつうの荒牌流局と同じ",
        default: "true",
        related: &["points.tsumo_payment"],
    },
    Item {
        key: "draws.renchan",
        title: "連荘の条件",
        kind: Kind::Enum(Renchan::VALUES),
        desc: "",
        default: "\"tenpai\"",
        related: &["table.oorasu_stop"],
    },
    Item {
        key: "draws.keishiki_tenpai",
        title: "形式聴牌",
        kind: b(
            "役のない聴牌・手の中の4枚目を待つ聴牌も聴牌とする",
            "役のある聴牌で、待ちの牌が残っている場合だけ聴牌とする",
        ),
        desc: "荒牌流局の聴牌の判定",
        default: "true",
        related: &["points.noten_penalty_total"],
    },
    Item {
        key: "draws.noten_declaration",
        title: "ノーテン宣言",
        kind: b(
            "荒牌流局のとき、聴牌の人は「ノーテン」と宣言して手牌を伏せられる（聴牌料・連荘はノーテン扱い）",
            "聴牌なら必ず聴牌扱い",
        ),
        desc: "",
        default: "false",
        related: &["points.noten_penalty_total"],
    },
    // ---- G. yakuman
    Item {
        key: "yakuman.composite",
        title: "役満の複合",
        kind: b("複数の役満を足す（ダブル役満など）", "役満は1つだけ数える"),
        desc: "",
        default: "true",
        related: &["yakuman.double_special"],
    },
    Item {
        key: "yakuman.double_special",
        title: "ダブル役満の役",
        kind: b(
            "四暗刻単騎・国士無双十三面待ち・純正九蓮宝燈・大四喜を2倍役満にする",
            "すべてシングル役満",
        ),
        desc: "",
        default: "false",
        related: &["yakuman.composite"],
    },
    Item {
        key: "yakuman.pao",
        title: "包（責任払い）の対象",
        kind: Kind::EnumSet(PaoYaku::VALUES),
        desc: "包の人はツモなら全額、ロンなら `yakuman.pao_ron_split` に従って払う",
        default: "[\"daisangen\", \"daisuushii\"]",
        related: &["yakuman.pao_ron_split"],
    },
    Item {
        key: "yakuman.pao_ron_split",
        title: "包のロンの払い",
        kind: b("放銃者と包の人が半分ずつ払う", "放銃者が全額払う"),
        desc: "",
        default: "true",
        related: &["yakuman.pao"],
    },
    Item {
        key: "yakuman.kokushi_ron_on_ankan",
        title: "暗槓への国士無双のロン",
        kind: b(
            "他家の暗槓に国士無双でロンできる（槍槓）",
            "暗槓にはロンできない",
        ),
        desc: "",
        default: "false",
        related: &[],
    },
    // ---- H. chips
    Item {
        key: "chips.enabled",
        title: "祝儀",
        kind: b(
            "祝儀あり。点数とは別の帳簿で枚数を数える（RL の報酬には入れない）",
            "祝儀なし（`chips.values` は使わない）",
        ),
        desc: "",
        default: "false",
        related: &["chips.values.point_value"],
    },
    Item {
        key: "chips.values.ippatsu",
        title: "一発の祝儀",
        kind: int(0, 100, "枚"),
        desc: "一発で和了したとき、支払う人1人あたり",
        default: "1",
        related: &["chips.enabled"],
    },
    Item {
        key: "chips.values.aka",
        title: "赤牌の祝儀",
        kind: int(0, 100, "枚"),
        desc: "和了した手の赤牌1枚あたり、支払う人1人あたり",
        default: "1",
        related: &["chips.values.aka_when_open"],
    },
    Item {
        key: "chips.values.ura",
        title: "裏ドラの祝儀",
        kind: int(0, 100, "枚"),
        desc: "裏ドラ1つあたり、支払う人1人あたり",
        default: "1",
        related: &["chips.enabled"],
    },
    Item {
        key: "chips.values.aka_when_open",
        title: "副露した手の赤牌の祝儀",
        kind: b("副露していても赤牌の祝儀が付く", "門前のときだけ付く"),
        desc: "",
        default: "true",
        related: &["chips.values.aka"],
    },
    Item {
        key: "chips.values.yakuman_ron",
        title: "役満のロンの祝儀",
        kind: int(0, 100, "枚"),
        desc: "役満をロンしたとき、放銃者が払う枚数",
        default: "10",
        related: &["chips.values.kazoe_yakuman_counts"],
    },
    Item {
        key: "chips.values.yakuman_tsumo_each",
        title: "役満のツモの祝儀",
        kind: int(0, 100, "枚"),
        desc: "役満をツモしたとき、支払う人1人あたり",
        default: "5",
        related: &["chips.values.kazoe_yakuman_counts"],
    },
    Item {
        key: "chips.values.kazoe_yakuman_counts",
        title: "数え役満の役満祝儀",
        kind: b(
            "数え役満にも役満の祝儀が付く",
            "数え役満には役満の祝儀が付かない",
        ),
        desc: "",
        default: "false",
        related: &["points.kazoe_yakuman"],
    },
    Item {
        key: "chips.values.point_value",
        title: "祝儀1枚の点数換算",
        kind: int(0, 1_000_000, "点"),
        desc: "表示用。牌譜の `scores` には混ぜない（LOG_FORMAT §4.5）",
        default: "5000",
        related: &["chips.enabled"],
    },
    // ---- I. log_visibility
    Item {
        key: "log_visibility.ankan",
        title: "暗槓の公開",
        kind: Kind::Enum(AnkanVisibility::VALUES),
        desc: "本PJの牌譜の設定で、サービスのルールではない（LOG_FORMAT §5）",
        default: "\"open\"",
        related: &[],
    },
    Item {
        key: "log_visibility.ryukyoku_reveal",
        title: "流局時の手牌の公開",
        kind: Kind::Enum(RyukyokuReveal::VALUES),
        desc: "本PJの牌譜の設定で、サービスのルールではない（LOG_FORMAT §4.4）",
        default: "\"tenpai_only\"",
        related: &[],
    },
];

pub fn item(key: &str) -> Option<&'static Item> {
    CATALOG.iter().find(|i| i.key == key)
}

/// 数値の項目が範囲内かを確かめる（[`Rules::validate`] から呼ぶ）。
pub fn range_errors(rules: &Rules) -> Vec<String> {
    let t = rules_to_table(rules);
    let mut errs = Vec::new();
    for it in CATALOG {
        let v = lookup(&t, it.key);
        let check = |x: i64, min: i64, max: i64, errs: &mut Vec<String>| {
            if x < min || x > max {
                errs.push(format!("{} = {x} is out of range {min}..={max}", it.key));
            }
        };
        match (it.kind, v) {
            (Kind::Int { min, max, .. }, Some(toml::Value::Integer(x))) => {
                check(*x, min, max, &mut errs)
            }
            (Kind::IntPerRank { min, max, .. }, Some(toml::Value::Array(a))) => {
                for x in a.iter().filter_map(|x| x.as_integer()) {
                    check(x, min, max, &mut errs);
                }
            }
            _ => {}
        }
    }
    errs
}

/// 既定値だけで作った `Rules`。
pub fn default_rules() -> Rules {
    let mut t = toml::Table::new();
    for it in CATALOG {
        let doc: toml::Table = format!("v = {}", it.default)
            .parse()
            .unwrap_or_else(|e| panic!("default of {}: {e}", it.key));
        set_key(&mut t, it.key, doc["v"].clone());
    }
    rules_from_table(t).expect("catalog defaults form valid rules")
}

/// `docs/spec/RULES_CATALOG.md` の本文。
pub fn render_markdown() -> String {
    let mut s = String::new();
    s.push_str("# RULES CATALOG — ルールの全項目\n\n");
    s.push_str(
        "<!-- このファイルは生成物。手で編集しない。`cargo run -p rules-catalog -- gen` で作り直す（CI の `rules-catalog -- check` で最新か確かめる） -->\n\n",
    );
    s.push_str("状態：D-025 に基づくルールの体系。**正は Rust の定義**（`crates/mj-core/src/rules.rs` の `Rules` と列挙型、`crates/mj-core/src/catalog.rs`）。\n\n");
    s.push_str("- キーは `rules/*.toml` と牌譜の `game_start.rules` で同じ\n");
    s.push_str("- **既定値**はメインのプリセット（天鳳三麻相当、D-011）の値。プリセットはすべての項目に値を書く（`docs/spec/RULES_PRESETS.md`）\n");
    s.push_str("- テスト対応表：`fixtures/rules/coverage.toml`。下の「値」の行ごとに、その分岐を確かめるテストを対応付ける（数値の項目は1つ以上）\n\n");
    let total_values: usize = CATALOG.iter().map(|i| i.kind.coverage_values().len()).sum();
    s.push_str(&format!(
        "項目数：**{}**、テスト対応表の行数（項目 × 値）：**{}**\n\n",
        CATALOG.len(),
        total_values
    ));
    for sec in SECTIONS {
        s.push_str(&format!("## {}\n\n", sec.title));
        s.push_str("| キー | 名前 | 型 | 取りうる値と意味 | 既定値 | 関連項目 |\n|---|---|---|---|---|---|\n");
        for it in CATALOG
            .iter()
            .filter(|i| i.key.split('.').next() == Some(sec.prefix))
        {
            let values = match it.kind {
                Kind::Bool { on, off } => format!("`true`：{on}<br>`false`：{off}"),
                Kind::Int { min, max, unit } => format!("{min}〜{max}（{unit}）"),
                Kind::IntPerRank { min, max, unit } => {
                    format!("各要素 {min}〜{max}（{unit}）。1位から順に人数分")
                }
                Kind::Enum(v) | Kind::EnumSet(v) => v
                    .iter()
                    .map(|(k, d)| format!("`{k}`：{d}"))
                    .collect::<Vec<_>>()
                    .join("<br>"),
            };
            let desc = if it.desc.is_empty() {
                String::new()
            } else {
                format!("<br>{}", it.desc)
            };
            let related = it
                .related
                .iter()
                .map(|r| format!("`{r}`"))
                .collect::<Vec<_>>()
                .join("<br>");
            s.push_str(&format!(
                "| `{}` | {}{} | {} | {} | `{}` | {} |\n",
                it.key,
                it.title,
                desc,
                it.kind.type_name(),
                values.replace('|', "\\|"),
                it.default,
                related
            ));
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_equal_main_preset() {
        assert_eq!(default_rules(), presets::tenhou_sanma());
    }

    #[test]
    fn catalog_keys_are_unique_and_related_keys_exist() {
        let mut seen = std::collections::BTreeSet::new();
        for it in CATALOG {
            assert!(seen.insert(it.key), "duplicate {}", it.key);
            for r in it.related {
                assert!(item(r).is_some(), "{}: unknown related {r}", it.key);
            }
            assert!(
                SECTIONS
                    .iter()
                    .any(|s| it.key.starts_with(&format!("{}.", s.prefix))),
                "{} has no section",
                it.key
            );
        }
    }

    #[test]
    fn enum_values_round_trip_through_serde() {
        // カタログに載せた表記で各列挙値が読めること（rule_enum! の定義と食い違わない）
        fn check<T: serde::de::DeserializeOwned + Copy + PartialEq + std::fmt::Debug>(
            all: &[T],
            values: &[(&str, &str)],
            as_str: fn(T) -> &'static str,
        ) {
            assert_eq!(all.len(), values.len());
            for (v, (s, _)) in all.iter().zip(values) {
                assert_eq!(as_str(*v), *s);
                let t: toml::Table = format!("v = \"{s}\"").parse().unwrap();
                let back: T = t["v"].clone().try_into().unwrap();
                assert_eq!(back, *v);
            }
        }
        check(
            TsumoPayment::ALL,
            TsumoPayment::VALUES,
            TsumoPayment::as_str,
        );
        check(Kuikae::ALL, Kuikae::VALUES, Kuikae::as_str);
        check(PaoYaku::ALL, PaoYaku::VALUES, PaoYaku::as_str);
        check(Renchan::ALL, Renchan::VALUES, Renchan::as_str);
        check(
            RyukyokuReveal::ALL,
            RyukyokuReveal::VALUES,
            RyukyokuReveal::as_str,
        );
    }

    #[test]
    fn range_check_uses_catalog_bounds() {
        let r = presets::with(&presets::tenhou_sanma(), "[points]\nmin_han = 2");
        assert!(range_errors(&r).is_empty());
        let mut t = rules_to_table(&r);
        set_key(&mut t, "points.min_han", toml::Value::Integer(14));
        let e = rules_from_table(t).unwrap_err();
        assert!(format!("{e}").contains("points.min_han = 14"), "{e}");
    }
}
