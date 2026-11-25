//! ルール設定。
//!
//! 項目は `docs/spec/SANMA_RULES_DRAFT.md` に対応する。プリセットは `rules/*.toml`
//! （値の出典は `docs/spec/RULES_PRESETS.md`）。
//!
//! プリセットでは公式説明で確認できない項目を**書かない**（推測で埋めない）。
//! そのため読み込みは `PresetFile`（TOML の木）→ 上書きの合成 → `Rules` の2段階で行い、
//! 足りない項目があれば全件を `RulesError::Missing` で返す。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

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
    /// 延長戦（西入・南入）
    pub extension: Extension,
    /// 延長戦・オーラスで終了と判定する点数（返し点と同じとは限らないので別に持つ）
    pub extension_threshold: i32,
    /// 延長戦の最終場（東風戦なら南、東南戦なら西、など）
    pub extension_last_wind: Wind,
    pub oorasu_stop: OorasuStop,
    pub tobi: Tobi,
    /// 1局戦モードでの飛び（D-012：1局戦のみ飛びなし）
    pub tobi_single_kyoku: Tobi,
    pub tie_break: TieBreak,
    /// 終了時に残った供託の行き先
    pub leftover_kyotaku: LeftoverKyotaku,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameLength {
    Single,
    Tonpuu,
    Hanchan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extension {
    None,
    SuddenDeath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wind {
    East,
    South,
    West,
    North,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OorasuStop {
    None,
    /// ラス親がトップなら和了止め・聴牌止めを自動で行う
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tobi {
    None,
    /// 0 点未満で終了（0 点は続行）
    BelowZero,
    /// 0 点以下で終了
    ZeroOrBelow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TieBreak {
    /// 起家に近い方が上位（東1局の風順）
    SeatOrder,
    /// 順位点を等分
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftoverKyotaku {
    Top,
    Discard,
}

/// B. 点数
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointRules {
    pub starting: i32,
    pub returning: i32,
    /// 順位ウマ（点数、1位から順。長さ = 人数）
    pub uma: Vec<i32>,
    pub tsumo_payment: TsumoPayment,
    /// 1本場あたりロンで和了者が受け取る点数
    pub honba_ron: i32,
    /// 1本場あたりツモで支払者1人が払う点数
    pub honba_tsumo_each: i32,
    /// 流局時の聴牌料（場に出る合計）
    pub noten_penalty_total: i32,
    pub riichi_deposit: i32,
    pub kiriage_mangan: bool,
    /// 数え役満（13翻以上）
    pub kazoe_yakuman: bool,
    /// 飜縛り
    pub min_han: u8,
    /// 連風牌の雀頭の符
    pub double_wind_pair_fu: u8,
    /// 嶺上ツモにもツモ符 2 符を付ける
    pub rinshan_tsumo_fu: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsumoPayment {
    /// 四麻の通常の支払い
    Standard,
    /// ツモ損（いない北家の分を誰も払わない）
    Loss,
    /// いない北家の分を残りの支払者で折半
    Split,
}

/// C. 牌
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileRules {
    pub manzu: Manzu,
    pub red_fives: RedFives,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Manzu {
    /// 1m〜9m をすべて使う（四麻）
    All,
    /// 2m〜8m を抜き、1m・9m だけを使う（ドラは 1m↔9m で回る）
    TerminalsOnly,
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
    /// ドラ表示牌の枠（裏も同数）
    pub dora_indicator_slots: u8,
    /// 嶺上牌（槓・北抜きの補充）の最大枚数
    pub rinshan_max: u8,
    /// ツモ山の残りがこの枚数になったら流局（ハイテイの位置）。
    /// 嶺上ツモはツモ山の末尾から取るので、嶺上を引くたびにハイテイが1枚ずれる
    pub reserve: u8,
}

/// E. 鳴き
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallRules {
    pub chi: bool,
    pub kuitan: bool,
    pub atozuke: bool,
    pub kuikae: Kuikae,
    /// 海底牌を鳴ける
    pub haitei_call: bool,
    /// 同時に和了できる最大人数（1 = 頭ハネ）
    pub max_ron: u8,
    /// 複数和了時の積み棒・供託の行き先
    pub multi_ron_deposit: MultiRonDeposit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kuikae {
    Forbidden,
    SujiAllowed,
    Allowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiRonDeposit {
    /// 放銃者から見て上家（頭ハネ順で最初の和了者）が取る
    Upstream,
    Split,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiichiRules {
    pub min_points: i32,
    /// ツモ番がないと立直できない
    pub requires_draw: bool,
    pub furiten_riichi: bool,
    pub ankan_after_riichi: RiichiAnkan,
    /// 宣言牌で放銃したら供託を出さない
    pub no_deposit_on_deal_in: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiichiAnkan {
    Forbidden,
    /// 待ちが変わらなければ可（牌姿・役の増減は不問）
    WaitUnchanged,
    /// 牌姿も変わらない場合のみ可
    FormUnchanged,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KanDoraTiming {
    /// 暗槓は即乗り、明槓・加槓は後めくり
    AnkanImmediate,
    AllImmediate,
    AllDelayed,
}

/// D. 北
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitaRules {
    pub mode: KitaMode,
    /// 北を持ったら必ず抜く（false なら抜くかどうかは任意）
    pub mandatory: bool,
    /// ポンの直後に抜ける
    pub after_pon: bool,
    pub after_riichi: KitaAfterRiichi,
    /// 抜いた北でロンできる
    pub ron: bool,
    /// 抜いた北へのロンに槍槓の役が付く
    pub ron_counts_as_chankan: bool,
    /// 抜きで一発が消える
    pub breaks_ippatsu: bool,
    /// 抜きで第1ツモ扱い（地和・九種九牌・ダブル立直）が消える
    pub breaks_first_turn: bool,
    /// 北を面子に使ったときの扱い
    pub north_as_meld: NorthAsMeld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitaMode {
    None,
    Nukidora,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitaAfterRiichi {
    Forbidden,
    /// ツモってきた北だけ抜ける
    DrawnOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NorthAsMeld {
    /// 自風・場風でなければ役なし（オタ風）
    Otakaze,
    /// 常に役牌
    Yakuhai,
}

/// F. 流局
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawRules {
    pub kyushu_kyuhai: bool,
    pub suukaikan: bool,
    /// 全員立直（四麻の四家立直、三麻なら三家）
    pub all_riichi: bool,
    /// 第1打牌が全員同じ風牌（四麻の四風連打）
    pub same_wind_discards: bool,
    /// 放銃者以外の全員がロン（四麻の三家和）
    pub all_ron: bool,
    pub abortive_renchan: bool,
    pub nagashi_mangan: bool,
    pub renchan: Renchan,
    /// 形式聴牌（役なし・5枚目待ちを含む）を聴牌と認める
    pub keishiki_tenpai: bool,
    pub noten_declaration: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Renchan {
    /// 和了連荘（聴牌でも親流れ）
    Agari,
    /// 聴牌連荘（ノーテン親流れ）
    Tenpai,
    /// ノーテンでも連荘
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YakumanRules {
    /// 役満の複合
    pub composite: bool,
    /// 四暗刻単騎・国士13面・純正九蓮・大四喜をダブル役満にする
    pub double_special: bool,
    /// 包（責任払い）の対象
    pub pao: Vec<PaoYaku>,
    /// 包のロン：放銃者と折半
    pub pao_ron_split: bool,
    /// 他家の暗槓に国士無双でロンできる
    pub kokushi_ron_on_ankan: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaoYaku {
    Daisangen,
    Daisuushii,
    Suukantsu,
}

/// 祝儀（D-013：点数とは別の帳簿。RL の報酬には入れない）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipRules {
    pub enabled: bool,
    /// `enabled = true` のときだけ必要
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<ChipValues>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipValues {
    pub ippatsu: u8,
    pub aka: u8,
    pub ura: u8,
    /// 副露していても赤の祝儀が付く
    pub aka_when_open: bool,
    pub yakuman_ron: u8,
    pub yakuman_tsumo_each: u8,
    pub kazoe_yakuman_counts: bool,
    /// 祝儀1枚の点数換算（表示用。scores には混ぜない）
    pub point_value: i32,
}

/// 牌譜での公開範囲（ルール設定で変えられる。LOG_FORMAT §5）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogVisibility {
    pub ankan: AnkanVisibility,
    pub ryukyoku_reveal: RyukyokuReveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnkanVisibility {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RyukyokuReveal {
    TenpaiOnly,
    All,
    None,
}

impl Rules {
    /// 設定値どうしの整合を確かめる（型だけでは表せない制約）。
    pub fn validate(&self) -> Result<(), RulesError> {
        let mut errs = Vec::new();
        let n = self.table.num_players;
        if !(n == 3 || n == 4) {
            errs.push(format!("table.num_players must be 3 or 4, got {n}"));
        }
        if self.points.uma.len() != usize::from(n) {
            errs.push(format!("points.uma must have {n} entries"));
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
        for (s, c) in [
            ("m", self.tiles.red_fives.m),
            ("p", self.tiles.red_fives.p),
            ("s", self.tiles.red_fives.s),
        ] {
            if c > 4 {
                errs.push(format!("tiles.red_fives.{s} must be <= 4"));
            }
        }
        if n == 4 && self.kita.mode != KitaMode::None {
            errs.push("kita.mode must be none for 4 players".into());
        }
        if self.calls.max_ron == 0 || self.calls.max_ron >= n {
            errs.push("calls.max_ron must be in 1..num_players".into());
        }
        if self.chips.enabled != self.chips.values.is_some() {
            errs.push("chips.values must be present iff chips.enabled".into());
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(RulesError::Invalid(errs))
        }
    }
}

/// `Rules` を埋めるのに必要な葉のキー（`chips.values` は `chips.enabled` に従属するので除く）。
/// `tests::required_keys_cover_every_field` で構造体と一致することを確かめる。
pub const REQUIRED_KEYS: &[&str] = &[
    "table.num_players",
    "table.length",
    "table.extension",
    "table.extension_threshold",
    "table.extension_last_wind",
    "table.oorasu_stop",
    "table.tobi",
    "table.tobi_single_kyoku",
    "table.tie_break",
    "table.leftover_kyotaku",
    "points.starting",
    "points.returning",
    "points.uma",
    "points.tsumo_payment",
    "points.honba_ron",
    "points.honba_tsumo_each",
    "points.noten_penalty_total",
    "points.riichi_deposit",
    "points.kiriage_mangan",
    "points.kazoe_yakuman",
    "points.min_han",
    "points.double_wind_pair_fu",
    "points.rinshan_tsumo_fu",
    "tiles.manzu",
    "tiles.red_fives.m",
    "tiles.red_fives.p",
    "tiles.red_fives.s",
    "wall.dora_indicator_slots",
    "wall.rinshan_max",
    "wall.reserve",
    "calls.chi",
    "calls.kuitan",
    "calls.atozuke",
    "calls.kuikae",
    "calls.haitei_call",
    "calls.max_ron",
    "calls.multi_ron_deposit",
    "riichi.min_points",
    "riichi.requires_draw",
    "riichi.furiten_riichi",
    "riichi.ankan_after_riichi",
    "riichi.no_deposit_on_deal_in",
    "dora.ippatsu",
    "dora.ura",
    "dora.kan_dora",
    "dora.kan_ura",
    "dora.kan_dora_timing",
    "kita.mode",
    "kita.mandatory",
    "kita.after_pon",
    "kita.after_riichi",
    "kita.ron",
    "kita.ron_counts_as_chankan",
    "kita.breaks_ippatsu",
    "kita.breaks_first_turn",
    "kita.north_as_meld",
    "draws.kyushu_kyuhai",
    "draws.suukaikan",
    "draws.all_riichi",
    "draws.same_wind_discards",
    "draws.all_ron",
    "draws.abortive_renchan",
    "draws.nagashi_mangan",
    "draws.renchan",
    "draws.keishiki_tenpai",
    "draws.noten_declaration",
    "yakuman.composite",
    "yakuman.double_special",
    "yakuman.pao",
    "yakuman.pao_ron_split",
    "yakuman.kokushi_ron_on_ankan",
    "chips.enabled",
    "log_visibility.ankan",
    "log_visibility.ryukyoku_reveal",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesError {
    Parse(String),
    /// プリセット＋上書きで値が決まっていないキー（未確認項目など）
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
    /// `[meta]` を除いたルールの木（未確認の項目は欠けている）
    pub body: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetMeta {
    pub id: String,
    pub name: String,
    /// 公式説明で確認できず、ファイルに書いていないキー
    #[serde(default)]
    pub unconfirmed: Vec<String>,
    /// 公式説明に選択肢があり、どれを採るかをオーナーが決めるキー（値は書いてある）
    #[serde(default)]
    pub owner_choice: Vec<String>,
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

    /// まだ値がないキー（`REQUIRED_KEYS` のうち木にないもの）。
    pub fn missing_keys(&self) -> Vec<String> {
        missing_keys(&self.body)
    }

    /// 上書きを重ねてから `Rules` にする。上書きは後ろほど優先。
    pub fn resolve(&self, overrides: &[&toml::Table]) -> Result<Rules, RulesError> {
        let mut body = self.body.clone();
        for o in overrides {
            merge(&mut body, o);
        }
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

    /// 牌の集合と牌山の生成に要る部分だけを取り出す（他の項目が未確認でも使える）。
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

fn is_known_key(k: &str) -> bool {
    REQUIRED_KEYS.contains(&k) || k.starts_with("chips.values.")
}

fn missing_keys(body: &toml::Table) -> Vec<String> {
    let have: BTreeSet<String> = leaf_keys(body).into_iter().collect();
    REQUIRED_KEYS
        .iter()
        .filter(|k| !have.contains(**k))
        .map(|k| k.to_string())
        .collect()
}

/// 木の葉のキーを `a.b.c` 形式で列挙する（配列は葉として扱う）。
fn leaf_keys(t: &toml::Table) -> Vec<String> {
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

fn lookup<'a>(t: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    let mut parts = key.split('.');
    let mut cur = t.get(parts.next()?)?;
    for p in parts {
        cur = cur.as_table()?.get(p)?;
    }
    Some(cur)
}

fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENHOU: &str = include_str!("../../../rules/tenhou-sanma.toml");
    const MJSOUL: &str = include_str!("../../../rules/mjsoul-sanma.toml");

    fn test_overrides() -> toml::Table {
        include_str!("../../../fixtures/rules/tenhou-sanma.test-overrides.toml")
            .parse()
            .unwrap()
    }

    fn tenhou_with_test_overrides() -> Rules {
        PresetFile::parse(TENHOU)
            .unwrap()
            .resolve(&[&test_overrides()])
            .unwrap()
    }

    #[test]
    fn presets_list_exactly_their_missing_keys() {
        // 「未確認」として文書に挙げたキー ＝ ファイルに書いていないキー
        for text in [TENHOU, MJSOUL] {
            let p = PresetFile::parse(text).unwrap();
            let mut listed = p.meta.unconfirmed.clone();
            listed.sort();
            let mut missing = p.missing_keys();
            missing.sort();
            assert_eq!(missing, listed, "preset {}", p.meta.id);
            assert_eq!(
                p.resolve(&[]).unwrap_err(),
                RulesError::Missing(p.missing_keys())
            );
        }
    }

    #[test]
    fn tenhou_resolves_with_test_overrides() {
        let r = tenhou_with_test_overrides();
        assert_eq!(r.table.num_players, 3);
        assert_eq!(r.wall.rinshan_max, 8);
        assert_eq!(r.tiles.manzu, Manzu::TerminalsOnly);
    }

    #[test]
    fn required_keys_cover_every_field() {
        // テスト用の完全なルールを直列化し、葉のキーが REQUIRED_KEYS と一致することを確かめる
        let rules = tenhou_with_test_overrides();
        let mut v: toml::Table = toml::Value::try_from(&rules).unwrap().try_into().unwrap();
        if let Some(toml::Value::Table(c)) = v.get_mut("chips") {
            c.remove("values");
        }
        let mut got = leaf_keys(&v);
        got.sort();
        let mut want: Vec<String> = REQUIRED_KEYS.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(got, want);
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
    fn override_fills_missing_key() {
        let over = test_overrides();
        let mut preset = PresetFile::parse(TENHOU).unwrap();
        merge(&mut preset.body, &over);
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
        let over: toml::Table = "[points]\nhonba_ron = 200\n".parse().unwrap();
        assert_eq!(preset.resolve(&[&over]).unwrap().points.honba_ron, 200);
    }
}
