//! ルールの分岐のテスト（点数の移動・祝儀・対局の進行・精算）。テスト対応表 `fixtures/rules/coverage.toml` に載せる。
//! 値ごとに、その値を変えると結果が変わる場面で期待結果を確かめる。

use mj_core::game::{TableState, progress, settle};
use mj_core::kyoku::KyokuOutcome;
use mj_core::rules::{Rules, presets};
use mj_core::score::{PayCtx, chips_to_points, hora_chips, hora_deltas, noten_deltas};
use mj_core::yaku::{WinResult, Yaku, YakuItem};

fn sanma() -> Rules {
    presets::tenhou_sanma()
}

fn with(o: &str) -> Rules {
    presets::with(&sanma(), o)
}

const CTX: PayCtx = PayCtx {
    n: 3,
    oya: 0,
    honba: 0,
};

// ------------------------------------------------------------------ 点数

#[test]
fn tsumo_payment_each_value() {
    // 子（席1）の 30符4翻ツモ（基本点 1920）：親 3900・子 2000
    let base = 1920;
    let (d, pts) = hora_deltas(&sanma(), CTX, 1, None, base, None, false);
    assert_eq!(
        d,
        vec![-3900, 5900, -2000],
        "loss：北家の 2000 は誰も払わない"
    );
    assert_eq!(pts, 5900);
    let (d, _) = hora_deltas(
        &with("[points]\ntsumo_payment = 'split'"),
        CTX,
        1,
        None,
        base,
        None,
        false,
    );
    // 北家の 2000 を2人で等分：各 +1000
    assert_eq!(d, vec![-4900, 7900, -3000]);
    // 四麻は standard：親 3900・子 2000 ×2
    let y = presets::test_yonma();
    let ctx4 = PayCtx { n: 4, ..CTX };
    let (d, _) = hora_deltas(&y, ctx4, 1, None, base, None, false);
    assert_eq!(d, vec![-3900, 7900, -2000, -2000]);
}

#[test]
fn honba_ron_and_tsumo_values() {
    let ctx = PayCtx { honba: 2, ..CTX };
    // ロン（子 1000点の手）：1本場 1000 × 2
    let (d, _) = hora_deltas(&sanma(), ctx, 1, Some(2), 250, None, true);
    assert_eq!(d, vec![0, 3000, -3000]);
    let (d, _) = hora_deltas(
        &with("[points]\nhonba_ron = 300"),
        ctx,
        1,
        Some(2),
        250,
        None,
        true,
    );
    assert_eq!(d, vec![0, 1600, -1600]);
    // ツモ：1人 500 × 2
    let (d, _) = hora_deltas(&sanma(), ctx, 1, None, 250, None, true);
    assert_eq!(d, vec![-1500, 2800, -1300]);
    let (d, _) = hora_deltas(
        &with("[points]\nhonba_tsumo_each = 100"),
        ctx,
        1,
        None,
        250,
        None,
        true,
    );
    assert_eq!(d, vec![-700, 1200, -500]);
    // 本場を払わない（ダブロンの2人目の上家取り）
    let (d, _) = hora_deltas(&sanma(), ctx, 1, Some(2), 250, None, false);
    assert_eq!(d, vec![0, 1000, -1000]);
}

#[test]
fn noten_penalty_total_values() {
    assert_eq!(
        noten_deltas(&sanma(), &[true, false, false]),
        vec![2000, -1000, -1000]
    );
    assert_eq!(
        noten_deltas(&sanma(), &[true, true, false]),
        vec![1000, 1000, -2000]
    );
    assert_eq!(noten_deltas(&sanma(), &[true, true, true]), vec![0, 0, 0]);
    let r = with("[points]\nnoten_penalty_total = 4000");
    assert_eq!(
        noten_deltas(&r, &[true, false, false]),
        vec![4000, -2000, -2000]
    );
}

#[test]
fn pao_ron_split_true_and_false() {
    // 子の大三元（基本点 8000）を席2からロン、包は席0
    let pao = Some((0u8, 8000));
    let (d, _) = hora_deltas(&sanma(), CTX, 1, Some(2), 8000, pao, false);
    assert_eq!(d, vec![-16000, 32000, -16000], "折半");
    let r = with("[yakuman]\npao_ron_split = false");
    let (d, _) = hora_deltas(&r, CTX, 1, Some(2), 8000, pao, false);
    assert_eq!(d, vec![0, 32000, -32000], "放銃者が全額");
    // ツモは包の人が全額
    let (d, _) = hora_deltas(&sanma(), CTX, 1, None, 8000, pao, false);
    assert_eq!(d, vec![-32000, 32000, 0]);
}

fn result(yaku: &[(Yaku, u8, u8)], kazoe: bool) -> WinResult {
    let yaku: Vec<YakuItem> = yaku
        .iter()
        .map(|&(y, han, yakuman)| YakuItem {
            yaku: y,
            han,
            yakuman,
        })
        .collect();
    let yakuman = yaku.iter().map(|y| y.yakuman).sum::<u8>() + u8::from(kazoe);
    WinResult {
        yaku,
        han: if kazoe { 13 } else { 3 },
        fu: 30,
        yakuman,
        kazoe,
        base: 2000,
    }
}

fn chips_on() -> Rules {
    with("[chips]\nenabled = true")
}

#[test]
fn chip_values_each_item() {
    let r = result(
        &[
            (Yaku::Riichi, 1, 0),
            (Yaku::Ippatsu, 1, 0),
            (Yaku::Aka, 2, 0),
            (Yaku::Ura, 1, 0),
        ],
        false,
    );
    // ツモ：支払う2人がそれぞれ 一発1 + 赤1×2 + 裏1×1 = 4 枚
    let (d, items) = hora_chips(&chips_on(), 3, 1, None, &r, true);
    assert_eq!(d, vec![-4, 8, -4]);
    assert_eq!(items.len(), 3);
    // 値を変える
    let rr = with("[chips]\nenabled = true\n[chips.values]\nippatsu = 2\naka = 3\nura = 5");
    let (d, _) = hora_chips(&rr, 3, 1, Some(2), &r, true);
    assert_eq!(d, vec![0, 13, -13], "ロン：放銃者が 2 + 3×2 + 5");
}

#[test]
fn chip_aka_when_open_true_and_false() {
    let r = result(&[(Yaku::YakuhaiChun, 1, 0), (Yaku::Aka, 1, 0)], false);
    let (d, _) = hora_chips(&chips_on(), 3, 1, Some(2), &r, false);
    assert_eq!(d, vec![0, 1, -1], "副露していても赤の祝儀");
    let rr = with("[chips]\nenabled = true\n[chips.values]\naka_when_open = false");
    let (d, _) = hora_chips(&rr, 3, 1, Some(2), &r, false);
    assert_eq!(d, vec![0, 0, 0]);
}

#[test]
fn chip_yakuman_ron_tsumo_and_kazoe() {
    let ym = result(&[(Yaku::Daisangen, 0, 1)], false);
    let (d, _) = hora_chips(&chips_on(), 3, 1, Some(2), &ym, true);
    assert_eq!(d, vec![0, 10, -10], "役満ロン 10枚");
    let (d, _) = hora_chips(&chips_on(), 3, 1, None, &ym, true);
    assert_eq!(d, vec![-5, 10, -5], "役満ツモ 5枚ずつ");
    let rr =
        with("[chips]\nenabled = true\n[chips.values]\nyakuman_ron = 3\nyakuman_tsumo_each = 1");
    assert_eq!(hora_chips(&rr, 3, 1, Some(2), &ym, true).0, vec![0, 3, -3]);
    assert_eq!(hora_chips(&rr, 3, 1, None, &ym, true).0, vec![-1, 2, -1]);
    // 数え役満
    let kz = result(&[(Yaku::Riichi, 1, 0)], true);
    assert_eq!(
        hora_chips(&chips_on(), 3, 1, Some(2), &kz, true).0,
        vec![0, 0, 0]
    );
    let rr = with("[chips]\nenabled = true\n[chips.values]\nkazoe_yakuman_counts = true");
    assert_eq!(
        hora_chips(&rr, 3, 1, Some(2), &kz, true).0,
        vec![0, 10, -10]
    );
}

#[test]
fn chip_point_value() {
    assert_eq!(chips_to_points(&sanma(), 3), 15000);
    assert_eq!(
        chips_to_points(&with("[chips.values]\npoint_value = 1000"), 3),
        3000
    );
}

// ------------------------------------------------------------------ 対局の進行

fn outcome(scores: &[i32], renchan: bool) -> KyokuOutcome {
    KyokuOutcome {
        hora: true,
        renchan,
        honba_next: if renchan { 1 } else { 0 },
        kyotaku_next: 0,
        scores: scores.to_vec(),
        chip_deltas: vec![0; scores.len()],
        oya_tenpai: false,
    }
}

fn state(bakaze: u8, kyoku: u8, scores: &[i32]) -> TableState {
    TableState {
        bakaze,
        kyoku_num: kyoku,
        honba: 0,
        kyotaku: 0,
        oya: kyoku,
        scores: scores.to_vec(),
    }
}

#[test]
fn length_each_value() {
    let even = [35000, 35000, 35000];
    // 1局戦：1局で終わる
    let r = with("[table]\nlength = 'single'");
    assert!(progress(
        &r,
        &mut state(0, 0, &even),
        &outcome(&even, false)
    ));
    // 東風戦：東3局が親流れで終われば（終了点数に届いていれば）終局
    let high = [45000, 30000, 30000];
    let r = with("[table]\nlength = 'tonpuu'");
    assert!(progress(
        &r,
        &mut state(0, 2, &high),
        &outcome(&high, false)
    ));
    // 東南戦：東3局の後は南1局へ
    let mut st = state(0, 2, &high);
    assert!(!progress(&sanma(), &mut st, &outcome(&high, false)));
    assert_eq!((st.bakaze, st.kyoku_num, st.oya), (1, 0, 0));
    assert!(progress(
        &sanma(),
        &mut state(1, 2, &high),
        &outcome(&high, false)
    ));
}

#[test]
fn extension_none_and_sudden_death_and_threshold() {
    // 南3局の終わりでトップが 40000（天鳳三麻の終了点数）未満
    let low = [36000, 35000, 34000];
    let r = sanma();
    let mut st = state(1, 2, &low);
    assert!(
        !progress(&r, &mut st, &outcome(&low, false)),
        "サドンデス：西入"
    );
    assert_eq!((st.bakaze, st.kyoku_num), (2, 0));
    let r = with("[table]\nextension_threshold = 40000\nextension = 'none'");
    assert!(
        progress(&r, &mut state(1, 2, &low), &outcome(&low, false)),
        "延長戦なし"
    );
    // 終了点数を 30000 に下げれば届いているので終局
    assert!(progress(
        &with("[table]\nextension_threshold = 30000"),
        &mut state(1, 2, &low),
        &outcome(&low, false)
    ));
    // 延長戦の局では、届いた人が出たら終わる
    let r = with("[table]\nextension_threshold = 40000");
    let reach = [41000, 33000, 31000];
    assert!(progress(
        &r,
        &mut state(2, 0, &reach),
        &outcome(&reach, false)
    ));
    assert!(!progress(&r, &mut state(2, 0, &low), &outcome(&low, false)));
}

#[test]
fn extension_threshold_counts_kyotaku_true_and_false() {
    // トップ 39000 + 供託2本（2000）
    let s = [39000, 33000, 31000];
    let mut o = outcome(&s, false);
    o.kyotaku_next = 2;
    let r = with("[table]\nextension_threshold = 40000");
    assert!(
        !progress(&r, &mut state(1, 2, &s), &o),
        "供託は含めない：西入"
    );
    let r = with("[table]\nextension_threshold = 40000\nextension_threshold_counts_kyotaku = true");
    assert!(progress(&r, &mut state(1, 2, &s), &o), "供託を含めると届く");
}

#[test]
fn extension_last_wind_each_value() {
    let low = [36000, 35000, 34000];
    let base = "[table]\nlength = 'tonpuu'\nextension_threshold = 40000\nextension_last_wind = ";
    // 東風戦の東3局の終わり：東なら延長戦の場がないので終局、南・西・北なら次の場へ
    for (wind, ends) in [
        ("'east'", true),
        ("'south'", false),
        ("'west'", false),
        ("'north'", false),
    ] {
        let r = with(&format!("{base}{wind}"));
        assert_eq!(
            progress(&r, &mut state(0, 2, &low), &outcome(&low, false)),
            ends,
            "{wind} at E3"
        );
    }
    // 南3局（延長戦）の終わり：南なら終局、西・北なら次の場へ
    for (wind, ends) in [("'south'", true), ("'west'", false), ("'north'", false)] {
        let r = with(&format!("{base}{wind}"));
        assert_eq!(
            progress(&r, &mut state(1, 2, &low), &outcome(&low, false)),
            ends,
            "{wind} at S3"
        );
    }
    // 西3局：北なら北場へ
    let r = with(&format!("{base}'north'"));
    let mut st = state(2, 2, &low);
    assert!(!progress(&r, &mut st, &outcome(&low, false)));
    assert_eq!(st.bakaze, 3);
}

#[test]
fn oorasu_stop_none_and_auto() {
    // 南3局（親は席2）で親がトップのまま連荘
    let s = [30000, 30000, 45000];
    let mut st = state(1, 2, &s);
    assert!(progress(&sanma(), &mut st, &outcome(&s, true)), "和了止め");
    let r = with("[table]\noorasu_stop = 'none'");
    let mut st = state(1, 2, &s);
    assert!(!progress(&r, &mut st, &outcome(&s, true)));
    assert_eq!((st.oya, st.honba), (2, 1));
    // 親がトップでなければ続く
    let s2 = [45000, 30000, 30000];
    assert!(!progress(
        &sanma(),
        &mut state(1, 2, &s2),
        &outcome(&s2, true)
    ));
}

#[test]
fn tobi_each_value() {
    let zero = [70000, 35000, 0];
    let minus = [71000, 35000, -1000];
    assert!(
        !progress(&sanma(), &mut state(0, 0, &zero), &outcome(&zero, false)),
        "0点は続行"
    );
    assert!(progress(
        &sanma(),
        &mut state(0, 0, &minus),
        &outcome(&minus, false)
    ));
    let r = with("[table]\ntobi = 'zero_or_below'");
    assert!(progress(
        &r,
        &mut state(0, 0, &zero),
        &outcome(&zero, false)
    ));
    let r = with("[table]\ntobi = 'none'");
    assert!(!progress(
        &r,
        &mut state(0, 0, &minus),
        &outcome(&minus, false)
    ));
}

#[test]
fn tobi_single_kyoku_each_value() {
    // 1局戦は1局で終わる。飛びは終局時の記録（GameEnd.tobi）だけに使う（D-012）
    let single = with("[table]\nlength = 'single'");
    let minus = [71000, 35000, -1000];
    let zero = [70000, 35000, 0];
    assert_eq!(
        settle(&single, &minus, 0, vec![0; 3]).tobi,
        vec![false; 3],
        "none（本PJ）"
    );
    let r = presets::with(&single, "[table]\ntobi_single_kyoku = 'below_zero'");
    assert_eq!(
        settle(&r, &minus, 0, vec![0; 3]).tobi,
        vec![false, false, true]
    );
    assert_eq!(settle(&r, &zero, 0, vec![0; 3]).tobi, vec![false; 3]);
    let r = presets::with(&single, "[table]\ntobi_single_kyoku = 'zero_or_below'");
    assert_eq!(
        settle(&r, &zero, 0, vec![0; 3]).tobi,
        vec![false, false, true]
    );
}

// ------------------------------------------------------------------ 精算

#[test]
fn settlement_uses_starting_returning_uma() {
    // 天鳳三麻：35000 持ちの 40000 返し、ウマ +20000/0/-20000、オカ = 5000×3 = 15000
    let e = settle(&sanma(), &[50000, 30000, 25000], 0, vec![0; 3]);
    assert_eq!(e.ranks, vec![1, 2, 3]);
    assert_eq!(
        e.settlement,
        vec![10000 + 20000 + 15000, -10000, -15000 - 20000]
    );
    let r = with("[points]\nstarting = 30000\nreturning = 30000\numa = [10000, 0, -10000]");
    let e = settle(&r, &[40000, 30000, 20000], 0, vec![0; 3]);
    assert_eq!(e.settlement, vec![20000, 0, -20000]);
}

#[test]
fn tie_break_seat_order_and_split() {
    let s = [40000, 40000, 25000];
    let e = settle(&sanma(), &s, 0, vec![0; 3]);
    assert_eq!(e.ranks, vec![1, 2, 3], "起家に近い方が上");
    assert_eq!(e.settlement[0] - e.settlement[1], 20000 + 15000);
    let r = with("[table]\ntie_break = 'split'");
    let e = settle(&r, &s, 0, vec![0; 3]);
    assert_eq!(e.settlement[0], e.settlement[1], "ウマ・オカを等分");
    assert_eq!(e.settlement.iter().sum::<i32>(), 0);
}

#[test]
fn leftover_kyotaku_top_and_discard() {
    let s = [40000, 35000, 28000];
    let e = settle(&sanma(), &s, 2, vec![0; 3]);
    assert_eq!(e.scores, vec![42000, 35000, 28000]);
    let r = with("[table]\nleftover_kyotaku = 'discard'");
    assert_eq!(settle(&r, &s, 2, vec![0; 3]).scores, s.to_vec());
    // 供託の額
    let r = with("[points]\nriichi_deposit = 500");
    assert_eq!(
        settle(&r, &s, 2, vec![0; 3]).scores,
        vec![41000, 35000, 28000]
    );
}
