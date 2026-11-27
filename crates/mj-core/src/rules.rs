//! ルール設定。
//!
//! **この構造体と列挙型がルールの「正」**（D-025）。項目の説明・取りうる値・既定値は
//! [`crate::catalog`] にあり、そこから `docs/spec/RULES_CATALOG.md` を生成する。
//! プリセットは `rules/*.toml`（値の出典は `docs/spec/RULES_PRESETS.md`）。
//!
//! 読み込みは `PresetFile`（TOML の木）→ 上書きの合成 → `Rules` の2段階で行い、
//! 足りない項目があれば全件を `RulesError::Missing` で返す。

use crate::catalog;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// ルールの列挙型を定義する。値の名前（TOML・牌譜での表記）と意味をここに一緒に書き、
/// カタログはこの定義から作る（値の一覧が二重管理にならないように）。
macro_rules! rule_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $s:literal => $desc:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name {
            $(#[doc = $desc] #[serde(rename = $s)] $var),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            /// （TOML での表記, 意味）
            pub const VALUES: &'static [(&'static str, &'static str)] = &[$(($s, $desc)),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$var => $s),+
                }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub table: TableRules,
    pub points: PointRules,
    pub tiles: TileRules,
    pub wall: WallRules,
    pub calls: CallRules,
    pub riichi: RiichiRules,
    pub dora: DoraRules,
    pub kita: KitaRules,
    pub draws: DrawRules,
    pub yakuman: YakumanRules,
    pub chips: ChipRules,
    pub log_visibility: LogVisibility,
}

/// A. 対局の形
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableRules {
    pub num_players: u8,
    pub length: GameLength,
    pub extension: Extension,
    pub extension_threshold: i32,
    pub extension_threshold_counts_kyotaku: bool,
    pub extension_last_wind: Wind,
    pub extension_renchan: ExtensionRenchan,
    pub oorasu_stop: OorasuStop,
    pub tobi: Tobi,
    pub tobi_single_kyoku: Tobi,
    pub tie_break: TieBreak,
    pub leftover_kyotaku: LeftoverKyotaku,
}

rule_enum! {
    ExtensionRenchan {
        Continue = "continue" => "延長戦の局で誰かが終了点数に届いていても、親が連荘するときは続ける（和了止め・聴牌止めと飛びを除く。連荘優先）",
        EndIfReached = "end_if_reached" => "延長戦の局の終わりに終了点数以上の人がいれば、親が連荘しても終わる",
    }
}

rule_enum! {
    GameLength {
        Single = "single" => "1局戦（1局だけで終わる。本場・連荘なし）",
        Tonpuu = "tonpuu" => "東風戦（東場だけ）",
        Hanchan = "hanchan" => "東南戦（半荘。東場と南場）",
    }
}

rule_enum! {
    Extension {
        None = "none" => "延長戦なし。規定の局数で必ず終わる",
        SuddenDeath = "sudden_death" => "延長戦あり。規定の局数が終わった時点で終了点数に届いた人がいなければ次の場に入り、届いた人が出た局の終わりで終了（サドンデス）",
    }
}

rule_enum! {
    Wind {
        East = "east" => "東",
        South = "south" => "南",
        West = "west" => "西",
        North = "north" => "北",
    }
}

rule_enum! {
    OorasuStop {
        None = "none" => "和了止め・聴牌止めなし（最終局で親が和了・聴牌すれば続行）",
        Auto = "auto" => "最終局で親が和了または聴牌で終えたとき、親が単独トップで終了の条件を満たしていれば自動で終了する",
    }
}

rule_enum! {
    Tobi {
        None = "none" => "飛びなし（持ち点がマイナスでも続ける）",
        BelowZero = "below_zero" => "誰かの持ち点が0点未満になったら終了（ちょうど0点は続行）",
        ZeroOrBelow = "zero_or_below" => "誰かの持ち点が0点以下になったら終了",
    }
}

rule_enum! {
    TieBreak {
        SeatOrder = "seat_order" => "同点なら起家に近い席（東1局の風順）が上位",
        Split = "split" => "同点の人どうしでウマ・オカを等分する（順位は起家に近い方を上として表示）",
    }
}

rule_enum! {
    LeftoverKyotaku {
        Top = "top" => "終了時に場に残った供託（立直棒）はトップが受け取る",
        Discard = "discard" => "終了時に場に残った供託は誰も受け取らない",
    }
}

/// B. 点数
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointRules {
    pub starting: i32,
    pub returning: i32,
    pub uma: Vec<i32>,
    pub tsumo_payment: TsumoPayment,
    pub honba_ron: i32,
    pub honba_tsumo_each: i32,
    pub noten_penalty_total: i32,
    pub riichi_deposit: i32,
    pub kiriage_mangan: bool,
    pub kazoe_yakuman: bool,
    pub min_han: u8,
    pub double_wind_pair_fu: u8,
    pub rinshan_tsumo_fu: bool,
}

rule_enum! {
    TsumoPayment {
        Standard = "standard" => "四麻の支払い（子のツモは親が2人分・子が1人分、親のツモは子が2人分ずつ）。四麻専用",
        Loss = "loss" => "ツモ損。四麻と同じ1人あたりの額を払い、いない北家の分は誰も払わない",
        Split = "split" => "いない北家の分を、残りの支払う人で等分して上乗せする（100点単位に切り上げ）",
    }
}

/// C. 牌
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileRules {
    pub manzu: Manzu,
    pub red_fives: RedFives,
}

rule_enum! {
    Manzu {
        All = "all" => "萬子を1m〜9mまですべて使う（四麻）",
        TerminalsOnly = "terminals_only" => "2m〜8mを抜き、1m・9mだけを使う（三麻）。ドラは1m→9m→1mと回る",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedFives {
    pub m: u8,
    pub p: u8,
    pub s: u8,
}

/// 王牌
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallRules {
    pub dora_indicator_slots: u8,
    pub dead_wall: u8,
    pub rinshan_max: u8,
}

impl WallRules {
    /// `pile` の末尾に残す枚数（ハイテイの位置）＝王牌 −（ドラ表示の枠 ＋ 裏の枠）。
    /// 嶺上ツモは `pile` の末尾から取るので、嶺上を引くたびにハイテイが1枚手前にずれ、
    /// 王牌は常に `dead_wall` 枚を保つ（LOG_FORMAT §4.2、D-017）。
    pub fn reserve(&self) -> usize {
        usize::from(self.dead_wall) - 2 * usize::from(self.dora_indicator_slots)
    }
}

/// E. 鳴き
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallRules {
    pub chi: bool,
    pub kuitan: bool,
    pub atozuke: bool,
    pub kuikae: Kuikae,
    pub haitei_call: bool,
    pub max_ron: u8,
    pub multi_ron_deposit: MultiRonDeposit,
}

rule_enum! {
    Kuikae {
        Forbidden = "forbidden" => "喰い替えなし。鳴いた牌と同じ種類の牌も、チーの筋の牌も、その打牌では切れない",
        SujiAllowed = "suji_allowed" => "鳴いた牌と同じ種類の牌（現物）は切れないが、チーの筋の牌は切れる",
        Allowed = "allowed" => "喰い替えあり。制限なし",
    }
}

rule_enum! {
    MultiRonDeposit {
        Upstream = "upstream" => "上家取り。複数の和了のとき、積み棒と供託は頭ハネで最優先の和了者（放銃者の下家から数えて最初の和了者。ほかの和了者から見て上家）が全部受け取る",
        Split = "split" => "積み棒は各和了者がそれぞれ受け取り、供託は和了者で等分する（1000点単位、余りは上家側から）",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiichiRules {
    pub min_points: i32,
    pub requires_draw: bool,
    pub furiten_riichi: bool,
    pub ankan_after_riichi: RiichiAnkan,
    pub no_deposit_on_deal_in: bool,
}

rule_enum! {
    RiichiAnkan {
        Forbidden = "forbidden" => "立直後の暗槓はできない",
        WaitUnchanged = "wait_unchanged" => "ツモった牌での暗槓で、待ちが変わらなければできる（牌姿・役の増減は問わない。送り槓は不可）",
        FormUnchanged = "form_unchanged" => "ツモった牌での暗槓で、待ちが変わらず、どの面子の取り方でもその牌が刻子のときだけできる",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoraRules {
    pub ippatsu: bool,
    pub ura: bool,
    pub kan_dora: bool,
    pub kan_ura: bool,
    pub kan_dora_timing: KanDoraTiming,
}

rule_enum! {
    KanDoraTiming {
        AnkanImmediate = "ankan_immediate" => "暗槓はその場でめくる（即乗り）。大明槓・加槓は嶺上ツモの後の打牌（または次の槓）でめくる（後めくり）",
        AllImmediate = "all_immediate" => "どの槓もその場でめくる",
        AllDelayed = "all_delayed" => "どの槓も嶺上ツモの後の打牌（または次の槓）でめくる",
        AnkanImmediateAfterPass = "ankan_immediate_after_pass" => "暗槓はその場でめくる（即乗り）。大明槓・加槓は嶺上ツモの後の打牌が通った後（ロンがなかった場合。ロンされたらめくらない）、または次の槓の前にめくる",
    }
}

/// D. 北
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitaRules {
    pub mode: KitaMode,
    pub mandatory: bool,
    pub after_pon: bool,
    pub after_riichi: KitaAfterRiichi,
    pub ron_on_kita: bool,
    pub ron_on_discard: bool,
    pub ron_counts_as_chankan: bool,
    pub breaks_ippatsu: bool,
    pub breaks_first_turn: bool,
    pub north_as_meld: NorthAsMeld,
    pub dora_per_kita: u8,
}

rule_enum! {
    KitaMode {
        None = "none" => "北抜きなし（北はふつうの字牌）",
        Nukidora = "nukidora" => "北抜きあり。北を抜いて脇に置くとドラになり、嶺上牌を1枚ツモる",
    }
}

rule_enum! {
    KitaAfterRiichi {
        Forbidden = "forbidden" => "立直後は北を抜けない",
        DrawnOnly = "drawn_only" => "立直後は、ツモってきた北だけを抜ける（抜かずにツモ切りも選べる）",
    }
}

rule_enum! {
    NorthAsMeld {
        Otakaze = "otakaze" => "北を刻子・雀頭に使ったら、自風・場風でなければ役も符もない字牌（オタ風）",
        Yakuhai = "yakuhai" => "北の刻子は常に役牌（1翻）",
    }
}

/// F. 流局
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawRules {
    pub kyushu_kyuhai: bool,
    pub suukaikan: bool,
    pub all_riichi: bool,
    pub same_wind_discards: bool,
    pub all_ron: bool,
    pub abortive_renchan: bool,
    pub nagashi_mangan: bool,
    pub renchan: Renchan,
    pub keishiki_tenpai: KeishikiTenpai,
    pub noten_declaration: bool,
}

rule_enum! {
    KeishikiTenpai {
        Any = "any" => "役のない聴牌も、自分で4枚使っている牌の単騎待ち（5枚目を待つ形）も聴牌とする",
        NoPureKaraten = "no_pure_karaten" => "役のない聴牌は聴牌とする。自分で4枚使っている牌しか待ちがない形（4枚使いの単騎待ち）はノーテン",
        YakuRequired = "yaku_required" => "役のある聴牌で、自分で4枚使っていない待ちがある場合だけ聴牌とする",
    }
}

rule_enum! {
    Renchan {
        Agari = "agari" => "和了連荘。親が和了したときだけ連荘（流局なら聴牌でも親流れ）",
        Tenpai = "tenpai" => "聴牌連荘。親が和了または流局時に聴牌なら連荘（ノーテンなら親流れ）",
        Always = "always" => "流局なら親が聴牌でもノーテンでも連荘",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YakumanRules {
    pub composite: bool,
    pub double_special: bool,
    pub pao: Vec<PaoYaku>,
    pub pao_ron_split: bool,
    pub pao_scope: PaoScope,
    pub pao_honba: PaoHonba,
    pub pao_tsumo_honba: PaoTsumoHonba,
    pub kokushi_ron_on_ankan: bool,
    pub tenhou_win_tile: TenhouWinTile,
}

rule_enum! {
    PaoScope {
        Full = "full" => "包の役満を含む和了の全額が責任払いの対象（複合役満なら全部）",
        PaoYakuOnly = "pao_yaku_only" => "包の対象の役満の分だけが責任払い。複合したほかの役満の分は通常の和了として払う（ツモなら包の人も分担する）",
    }
}

rule_enum! {
    PaoHonba {
        Pao = "pao" => "包があれば、ロンでもツモでも積み棒は包の人が全部払う",
        Normal = "normal" => "積み棒は通常どおり（ロンは放銃者、ツモは支払う人それぞれ）。ただしツモの全額が包の対象なら包の人が払う",
    }
}

rule_enum! {
    PaoTsumoHonba {
        TsumoTotal = "tsumo_total" => "包の人が払うツモの積み棒は、支払う人全員の分の合計（`points.honba_tsumo_each` × 本場 × 支払う人数）",
        RonAmount = "ron_amount" => "包の人が払うツモの積み棒は、ロンと同じ額（`points.honba_ron` × 本場）",
    }
}

rule_enum! {
    TenhouWinTile {
        Drawn = "drawn" => "天和の和了牌は親の第1ツモの牌",
        Best = "best" => "天和は手牌14枚のうち、和了牌とみなしたときに最も高い点数になる牌を和了牌とする",
    }
}

rule_enum! {
    PaoYaku {
        Daisangen = "daisangen" => "大三元（三元牌の3つ目の刻子・槓子を鳴かせた人が責任を負う）",
        Daisuushii = "daisuushii" => "大四喜（風牌の4つ目の刻子・槓子を鳴かせた人が責任を負う）",
        Suukantsu = "suukantsu" => "四槓子（4つ目の槓子を大明槓させた人が責任を負う）",
    }
}

/// 祝儀（D-013：点数とは別の帳簿。RL の報酬には入れない）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipRules {
    pub enabled: bool,
    /// `enabled = false` のときは使わない（値は書いておく）
    pub values: ChipValues,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipValues {
    pub ippatsu: u8,
    pub aka: u8,
    pub ura: u8,
    pub aka_when_open: bool,
    pub yakuman_ron: u8,
    pub yakuman_tsumo_each: u8,
    pub kazoe_yakuman_counts: bool,
    pub point_value: i32,
}

/// 牌譜での公開範囲（LOG_FORMAT §5）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogVisibility {
    pub ankan: AnkanVisibility,
    pub ryukyoku_reveal: RyukyokuReveal,
}

rule_enum! {
    AnkanVisibility {
        Open = "open" => "暗槓した牌の種類を公開版の牌譜にも書く",
        Closed = "closed" => "暗槓した牌を、本人以外の牌譜では \"?\" にする",
    }
}

rule_enum! {
    RyukyokuReveal {
        TenpaiOnly = "tenpai_only" => "流局時は聴牌者の手牌だけを公開する（九種九牌は宣言者の手牌も）",
        All = "all" => "流局時は全員の手牌を公開する",
        None = "none" => "流局時に手牌を公開しない（九種九牌の宣言者を除く）",
    }
}

impl Rules {
    /// 設定値どうしの整合を確かめる（型だけでは表せない制約）。
    /// 数値の範囲は [`catalog::CATALOG`] の `Int { min, max }` で一括して確かめる。
    pub fn validate(&self) -> Result<(), RulesError> {
        let mut errs = catalog::range_errors(self);
        let n = self.table.num_players;
        if self.points.uma.len() != usize::from(n) {
            errs.push(format!("points.uma must have {n} entries"));
        }
        if self.points.uma.iter().sum::<i32>() != 0 {
            errs.push("points.uma must sum to 0".into());
        }
        if n == 3 && self.points.tsumo_payment == TsumoPayment::Standard {
            errs.push("points.tsumo_payment = standard is only for 4 players".into());
        }
        if n == 4 && self.points.tsumo_payment != TsumoPayment::Standard {
            errs.push("points.tsumo_payment must be standard for 4 players".into());
        }
        if n == 4 && self.tiles.manzu != Manzu::All {
            errs.push("tiles.manzu must be all for 4 players".into());
        }
        if self.tiles.manzu == Manzu::TerminalsOnly && self.tiles.red_fives.m != 0 {
            errs.push("tiles.red_fives.m must be 0 when 5m is not used".into());
        }
        if n == 4 && self.kita.mode != KitaMode::None {
            errs.push("kita.mode must be none for 4 players".into());
        }
        if usize::from(self.wall.dead_wall) < 2 * usize::from(self.wall.dora_indicator_slots) {
            errs.push("wall.dead_wall must be >= 2 * wall.dora_indicator_slots".into());
        }
        if self.calls.max_ron >= n {
            errs.push("calls.max_ron must be < num_players".into());
        }
        // 聴牌料は、聴牌者・ノーテン者が何人でも等分できること
        for k in 1..i32::from(n) {
            if self.points.noten_penalty_total % k != 0 {
                errs.push(format!(
                    "points.noten_penalty_total must be divisible by {k}"
                ));
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(RulesError::Invalid(errs))
        }
    }
}

/// `Rules` を埋めるのに必要な葉のキー（カタログの順）。
pub fn required_keys() -> impl Iterator<Item = &'static str> {
    catalog::CATALOG.iter().map(|i| i.key)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesError {
    Parse(String),
    /// プリセット＋上書きで値が決まっていないキー
    Missing(Vec<String>),
    /// 定義にないキー
    Unknown(Vec<String>),
    Invalid(Vec<String>),
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RulesError::Parse(e) => write!(f, "rules parse error: {e}"),
            RulesError::Missing(k) => write!(f, "rules keys not set: {}", k.join(", ")),
            RulesError::Unknown(k) => write!(f, "unknown rules keys: {}", k.join(", ")),
            RulesError::Invalid(e) => write!(f, "invalid rules: {}", e.join("; ")),
        }
    }
}

impl std::error::Error for RulesError {}

/// プリセットの TOML ファイル（`[meta]` ＋ルールの部分木）。
#[derive(Debug, Clone)]
pub struct PresetFile {
    pub meta: PresetMeta,
    /// `[meta]` を除いたルールの木
    pub body: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetMeta {
    pub id: String,
    pub name: String,
    /// 値はあるが、サービスの公式説明・オーナーの確認がまだのキー（本PJの仮の値）
    #[serde(default)]
    pub provisional: Vec<String>,
    /// テスト専用のファイルなら true（本番のプリセットとして使わない）
    #[serde(default)]
    pub test_only: bool,
}

impl PresetFile {
    pub fn parse(text: &str) -> Result<Self, RulesError> {
        let mut t: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| RulesError::Parse(e.to_string()))?;
        let meta = t
            .remove("meta")
            .ok_or_else(|| RulesError::Parse("missing [meta]".into()))?;
        let meta: PresetMeta = meta
            .try_into()
            .map_err(|e: toml::de::Error| RulesError::Parse(format!("[meta]: {e}")))?;
        let unknown: Vec<String> = leaf_keys(&t)
            .into_iter()
            .filter(|k| !is_known_key(k))
            .collect();
        if !unknown.is_empty() {
            return Err(RulesError::Unknown(unknown));
        }
        Ok(Self { meta, body: t })
    }

    /// まだ値がないキー（カタログのキーのうち木にないもの）。
    pub fn missing_keys(&self) -> Vec<String> {
        missing_keys(&self.body)
    }

    /// 上書きを重ねてから `Rules` にする。上書きは後ろほど優先。
    pub fn resolve(&self, overrides: &[&toml::Table]) -> Result<Rules, RulesError> {
        let mut body = self.body.clone();
        for o in overrides {
            merge(&mut body, o);
        }
        rules_from_table(body)
    }

    /// 牌の集合と牌山の生成に要る部分だけを取り出す（他の項目が欠けていても使える）。
    pub fn wall_spec(&self) -> Result<crate::wall::WallSpec, RulesError> {
        let get = |k: &str| -> Result<toml::Value, RulesError> {
            lookup(&self.body, k)
                .cloned()
                .ok_or_else(|| RulesError::Missing(vec![k.to_string()]))
        };
        let num_players = get("table.num_players")?
            .try_into()
            .map_err(|e: toml::de::Error| RulesError::Parse(e.to_string()))?;
        let tiles: TileRules = get("tiles")?
            .try_into()
            .map_err(|e: toml::de::Error| RulesError::Parse(e.to_string()))?;
        let dora_indicator_slots = get("wall.dora_indicator_slots")?
            .try_into()
            .map_err(|e: toml::de::Error| RulesError::Parse(e.to_string()))?;
        Ok(crate::wall::WallSpec {
            num_players,
            tiles,
            dora_indicator_slots,
        })
    }
}

/// TOML の木（全項目）から `Rules` を作る。
pub fn rules_from_table(body: toml::Table) -> Result<Rules, RulesError> {
    let unknown: Vec<String> = leaf_keys(&body)
        .into_iter()
        .filter(|k| !is_known_key(k))
        .collect();
    if !unknown.is_empty() {
        return Err(RulesError::Unknown(unknown));
    }
    let missing = missing_keys(&body);
    if !missing.is_empty() {
        return Err(RulesError::Missing(missing));
    }
    let rules: Rules = toml::Value::Table(body)
        .try_into()
        .map_err(|e: toml::de::Error| RulesError::Parse(e.to_string()))?;
    rules.validate()?;
    Ok(rules)
}

/// `Rules` を TOML の木にする（キーはプリセットと同じ）。
pub fn rules_to_table(rules: &Rules) -> toml::Table {
    toml::Value::try_from(rules)
        .expect("Rules serializes")
        .try_into()
        .expect("Rules is a table")
}

fn is_known_key(k: &str) -> bool {
    catalog::item(k).is_some()
}

fn missing_keys(body: &toml::Table) -> Vec<String> {
    let have: BTreeSet<String> = leaf_keys(body).into_iter().collect();
    required_keys()
        .filter(|k| !have.contains(*k))
        .map(|k| k.to_string())
        .collect()
}

/// 木の葉のキーを `a.b.c` 形式で列挙する（配列は葉として扱う）。
pub fn leaf_keys(t: &toml::Table) -> Vec<String> {
    fn walk(prefix: &str, t: &toml::Table, out: &mut Vec<String>) {
        for (k, v) in t {
            let key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            match v {
                toml::Value::Table(sub) => walk(&key, sub, out),
                _ => out.push(key),
            }
        }
    }
    let mut out = Vec::new();
    walk("", t, &mut out);
    out
}

pub fn lookup<'a>(t: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    let mut parts = key.split('.');
    let mut cur = t.get(parts.next()?)?;
    for p in parts {
        cur = cur.as_table()?.get(p)?;
    }
    Some(cur)
}

/// `key`（`a.b.c`）に値を入れる（途中の表がなければ作る）。
pub fn set_key(t: &mut toml::Table, key: &str, v: toml::Value) {
    let mut parts: Vec<&str> = key.split('.').collect();
    let last = parts.pop().expect("non-empty key");
    let mut cur = t;
    for p in parts {
        cur = cur
            .entry(p)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("intermediate key is a table");
    }
    cur.insert(last.to_string(), v);
}

pub fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

/// テストで使うプリセット（リポジトリに含まれるファイル）。
pub mod presets {
    use super::*;

    pub const TENHOU_SANMA: &str = include_str!("../../../rules/tenhou-sanma.toml");
    pub const MJSOUL_SANMA: &str = include_str!("../../../rules/mjsoul-sanma.toml");
    /// テスト専用の四麻（majiang-core との差分テストに合わせた値）
    pub const TEST_YONMA: &str = include_str!("../../../fixtures/rules/test-yonma.toml");

    pub fn tenhou_sanma() -> Rules {
        PresetFile::parse(TENHOU_SANMA)
            .and_then(|p| p.resolve(&[]))
            .expect("rules/tenhou-sanma.toml is complete")
    }

    pub fn mjsoul_sanma() -> Rules {
        PresetFile::parse(MJSOUL_SANMA)
            .and_then(|p| p.resolve(&[]))
            .expect("rules/mjsoul-sanma.toml is complete")
    }

    pub fn test_yonma() -> Rules {
        PresetFile::parse(TEST_YONMA)
            .and_then(|p| p.resolve(&[]))
            .expect("fixtures/rules/test-yonma.toml is complete")
    }

    /// `base` の一部のキーを TOML の断片で上書きしたルール（テスト用）。
    pub fn with(base: &Rules, overrides: &str) -> Rules {
        let mut t = rules_to_table(base);
        let o: toml::Table = overrides.parse().expect("override TOML");
        merge(&mut t, &o);
        rules_from_table(t).expect("valid override")
    }
}

#[cfg(test)]
mod tests {
    use super::presets::*;
    use super::*;

    #[test]
    fn presets_are_complete_and_valid() {
        for text in [TENHOU_SANMA, MJSOUL_SANMA, TEST_YONMA] {
            let p = PresetFile::parse(text).unwrap();
            assert_eq!(
                p.missing_keys(),
                Vec::<String>::new(),
                "preset {}",
                p.meta.id
            );
            p.resolve(&[]).unwrap();
            for k in &p.meta.provisional {
                assert!(
                    catalog::item(k).is_some(),
                    "{}: unknown provisional {k}",
                    p.meta.id
                );
            }
        }
    }

    #[test]
    fn tenhou_preset_has_no_provisional_values() {
        // D-026：オーナーの回答で天鳳三麻の値はすべて決まった
        let p = PresetFile::parse(TENHOU_SANMA).unwrap();
        assert!(p.meta.provisional.is_empty());
        let r = tenhou_sanma();
        assert_eq!(r.table.num_players, 3);
        assert_eq!(r.wall.rinshan_max, 8);
        assert_eq!(r.wall.reserve(), 4);
        assert_eq!(r.tiles.manzu, Manzu::TerminalsOnly);
    }

    #[test]
    fn serialized_keys_equal_catalog_keys() {
        // 構造体の葉のキー ＝ カタログのキー
        for r in [tenhou_sanma(), mjsoul_sanma(), test_yonma()] {
            let got: BTreeSet<String> = leaf_keys(&rules_to_table(&r)).into_iter().collect();
            let want: BTreeSet<String> = required_keys().map(String::from).collect();
            assert_eq!(got, want);
        }
    }

    #[test]
    fn rejects_unknown_keys() {
        let text = "[meta]\nid='x'\nname='x'\n[table]\nnum_playres=3\n";
        assert_eq!(
            PresetFile::parse(text).unwrap_err(),
            RulesError::Unknown(vec!["table.num_playres".into()])
        );
    }

    #[test]
    fn missing_key_is_reported_and_override_fills_it() {
        let mut preset = PresetFile::parse(TENHOU_SANMA).unwrap();
        preset
            .body
            .get_mut("points")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .remove("honba_ron");
        assert_eq!(
            preset.resolve(&[]).unwrap_err(),
            RulesError::Missing(vec!["points.honba_ron".into()])
        );
        let over: toml::Table = "[points]\nhonba_ron = 300\n".parse().unwrap();
        assert_eq!(preset.resolve(&[&over]).unwrap().points.honba_ron, 300);
    }

    #[test]
    fn validate_rejects_inconsistent_values() {
        let base = tenhou_sanma();
        let bad = |o: &str| {
            let mut t = rules_to_table(&base);
            merge(&mut t, &o.parse().unwrap());
            rules_from_table(t).unwrap_err()
        };
        for o in [
            "[points]\ntsumo_payment = 'standard'",
            "[points]\numa = [10, 0]",
            "[points]\numa = [10, 0, 0]",
            "[points]\nnoten_penalty_total = 1001",
            "[tiles]\nred_fives = { m = 1, p = 1, s = 1 }",
            "[wall]\ndead_wall = 9",
            "[calls]\nmax_ron = 3",
            "[table]\nnum_players = 5",
        ] {
            assert!(matches!(bad(o), RulesError::Invalid(_)), "{o}");
        }
    }
}
