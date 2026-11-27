//! ルールの分岐のテスト（局進行）。牌山を手で組み（`mj_core::scenario`）、値ごとに結果が変わる場面を確かめる。
//! テスト対応表 `fixtures/rules/coverage.toml` に載せる。指定しない判断は「見送り → ツモ切り」で進む。

use mj_core::event::Event;
use mj_core::kyoku::Kyoku;
use mj_core::rules::{Rules, presets};
use mj_core::scenario::{Scenario, act, auto, auto_to_end, auto_until_seat, choices};
use mj_core::yaku::Yaku;

fn sanma() -> Rules {
    presets::tenhou_sanma()
}

fn with(o: &str) -> Rules {
    presets::with(&sanma(), o)
}

fn yonma_with(o: &str) -> Rules {
    presets::with(&presets::test_yonma(), o)
}

/// 席0（親）：1〜9p ＋ 2s3s ＋ 5s5s（1s・4s 待ち、一気通貫）
const T0: &str = "1p 2p 3p 4p 5p 6p 7p 8p 9p 2s 3s 5s 5s";
/// 何もできない手
const J1: &str = "1m 1m 9m 2p 6p 9p 3s 7s S W F F C";
const J2: &str = "9m 9m 1p 4p 8p 2s 6s 9s S W P P C";

fn has(k: &Kyoku, seat: u8, a: &str) -> bool {
    choices(k, seat).iter().any(|c| c == a)
}

fn last_hora(k: &Kyoku) -> &mj_core::event::Hora {
    k.events()
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Hora(h) => Some(&**h),
            _ => None,
        })
        .expect("hora")
}

fn ryukyoku(k: &Kyoku) -> &mj_core::event::Ryukyoku {
    k.events()
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Ryukyoku(r) => Some(&**r),
            _ => None,
        })
        .expect("ryukyoku")
}

fn types(k: &Kyoku) -> Vec<&'static str> {
    k.events()
        .iter()
        .map(|e| match e {
            Event::KyokuStart(_) => "kyoku_start",
            Event::Tsumo { .. } => "tsumo",
            Event::Dahai { .. } => "dahai",
            Event::Reach { .. } => "reach",
            Event::ReachAccepted { .. } => "reach_accepted",
            Event::Chi { .. } => "chi",
            Event::Pon { .. } => "pon",
            Event::Daiminkan { .. } => "daiminkan",
            Event::Ankan { .. } => "ankan",
            Event::Kakan { .. } => "kakan",
            Event::Kita { .. } => "kita",
            Event::Dora { .. } => "dora",
            Event::Hora(_) => "hora",
            Event::Ryukyoku(_) => "ryukyoku",
            Event::KyokuEnd(_) => "kyoku_end",
        })
        .collect()
}

// ------------------------------------------------------------------ 立直

#[test]
fn riichi_min_points() {
    let s = Scenario::new(sanma(), &[T0, J1, J2]).draws("E");
    assert!(has(&s.kyoku(), 0, "reach:E:tsumogiri"));
    let k = s.clone().scores(&[900, 52000, 52100]).kyoku();
    assert!(!has(&k, 0, "reach:E:tsumogiri"), "持ち点が 1000 未満");
    let k = Scenario {
        rules: with("[riichi]\nmin_points = 500"),
        ..s
    }
    .scores(&[900, 52000, 52100])
    .kyoku();
    assert!(has(&k, 0, "reach:E:tsumogiri"));
}

#[test]
fn riichi_furiten_riichi_true_and_false() {
    // 1s をツモ切りして振聴（1s・4s 待ち）にしてから、次のツモで立直できるか
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2])
            .draws("1s 1m 9m E")
            .kyoku();
        act(&mut k, 0, "dahai:1s:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        has(&k, 0, "reach:E:tsumogiri")
    };
    assert!(run(sanma()));
    assert!(!run(with("[riichi]\nfuriten_riichi = false")));
}

#[test]
fn riichi_no_deposit_on_deal_in_true_and_false() {
    // 席0の立直の宣言牌（東）を席1が場風の東でロン（東・白のシャンポン）
    let s1 = "E E 2p 3p 4p 5p 6p 7p 2s 3s 4s P P";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, s1, J1]).draws("E").kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        act(&mut k, 1, "ron");
        k
    };
    let k = run(sanma());
    assert!(!types(&k).contains(&"reach_accepted"));
    assert_eq!(last_hora(&k).deltas[0], -last_hora(&k).deltas[1]);
    let k = run(with("[riichi]\nno_deposit_on_deal_in = false"));
    assert!(types(&k).contains(&"reach_accepted"));
    // 供託 1000 は和了者へ
    let h = last_hora(&k);
    assert_eq!(h.deltas[1], -h.deltas[0] + 1000);
}

#[test]
fn riichi_requires_draw_true_and_false() {
    // 王牌を 40 枚にしてツモを 29 回に縮め、最後のツモの番（残り 1 枚）で立直できるか
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2]).kyoku();
        loop {
            let my_turn = choices(&k, 0).iter().any(|c| c.starts_with("dahai:"));
            if my_turn && k.live_remaining() < 3 {
                break;
            }
            auto(&mut k);
        }
        choices(&k, 0).iter().any(|c| c.starts_with("reach:"))
    };
    assert!(!run(with("[wall]\ndead_wall = 40")));
    assert!(run(with(
        "[wall]\ndead_wall = 40\n[riichi]\nrequires_draw = false"
    )));
}

/// 席0：東の暗刻＋1〜9p＋5s（5s 単騎）。立直してから東をツモって暗槓、嶺上で 5s をツモる
fn riichi_ankan(rules: Rules) -> Kyoku {
    let s0 = "E E E 1p 2p 3p 4p 5p 6p 7p 8p 9p 5s";
    let mut k = Scenario::new(rules, &[s0, J1, J2])
        .draws("S 1m 9m E")
        .rinshan("5s")
        .kyoku();
    act(&mut k, 0, "reach:S:tsumogiri");
    assert!(auto_until_seat(&mut k, 0));
    k
}

#[test]
fn riichi_ankan_after_riichi_each_value() {
    let k = riichi_ankan(sanma());
    assert!(has(&k, 0, "ankan:E"), "待ちが変わらない暗槓");
    let k = riichi_ankan(with("[riichi]\nankan_after_riichi = 'forbidden'"));
    assert!(!has(&k, 0, "ankan:E"));
    let k = riichi_ankan(with("[riichi]\nankan_after_riichi = 'form_unchanged'"));
    assert!(has(&k, 0, "ankan:E"), "東は常に刻子");
    // 5p 666p 777p 888p 999p（4p〜8p 待ち）で 9p を暗槓：待ちは変わらないが、9p が刻子にならない取り方がある
    let run = |rules: Rules| {
        let s0 = "5p 6p 6p 6p 7p 7p 7p 8p 8p 8p 9p 9p 9p";
        let s1 = "1m 1m 9m 2p 3p 3s 7s S W F F C 1s";
        let s2 = "9m 9m 1p 4p 2s 6s 9s S W P P C 4s";
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("E 1m 9m 9p")
            .kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        has(&k, 0, "ankan:9p")
    };
    assert!(run(sanma()));
    assert!(!run(with(
        "[riichi]\nankan_after_riichi = 'form_unchanged'"
    )));
}

// ------------------------------------------------------------------ ドラ・一発

#[test]
fn dora_ippatsu_true_and_false() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2])
            .draws("E 1m 9m 1s")
            .kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "tsumo");
        last_hora(&k).yaku.iter().any(|y| y.yaku == Yaku::Ippatsu)
    };
    assert!(run(sanma()));
    assert!(!run(with("[dora]\nippatsu = false")));
}

#[test]
fn dora_kan_ura_true_and_false() {
    let run = |rules: Rules| {
        let mut k = riichi_ankan(rules);
        act(&mut k, 0, "ankan:E");
        act(&mut k, 0, "tsumo");
        last_hora(&k).ura_indicators.len()
    };
    assert_eq!(run(sanma()), 2, "カン裏あり：表示牌 2 枚分");
    assert_eq!(run(with("[dora]\nkan_ura = false")), 1);
}

/// 席0の暗槓（東）の後と、席0の打牌への席1の大明槓（中）の後の、ドラの出る位置
fn kan_dora_events(rules: Rules) -> Vec<&'static str> {
    let s0 = "E E E 1p 2p 3p 4p 5p 6p 7p 8p C 5s";
    let s1 = "C C C 9m 1p 4p 8p 2s 6s 9s S W P";
    let s2 = "1m 1m 9m 6p 9p 3s 7s S W F F N 2s";
    let mut k = Scenario::new(rules, &[s0, s1, s2])
        .draws("E")
        .rinshan("9s 1s")
        .kyoku();
    act(&mut k, 0, "ankan:E");
    act(&mut k, 0, "dahai:C");
    act(&mut k, 1, "daiminkan");
    act(&mut k, 1, "dahai:1s:tsumogiri");
    let t = types(&k);
    // 最後は席2 のツモ（判断ポイントで止まる）
    t[1..].to_vec()
}

#[test]
fn kan_dora_timing_after_pass_and_ron() {
    // 席1 が大明槓（中）の後の打牌 1s を席2 にロンされる
    let s0 = "E E E 1p 2p 3p 4p 5p 6p 7p 8p C 5s";
    let s1 = "C C C 9m 1p 4p 8p 2s 6s 9s S W P";
    let s2 = "2s 3s F F F 5p 6p 7p 7p 8p 9p 9m 9m";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("E")
            .rinshan("9s 1s")
            .kyoku();
        act(&mut k, 0, "ankan:E");
        act(&mut k, 0, "dahai:C");
        act(&mut k, 1, "daiminkan");
        act(&mut k, 1, "dahai:1s:tsumogiri");
        act(&mut k, 2, "ron");
        types(&k)[5..].to_vec()
    };
    // 打牌が通らなかったので、大明槓のカンドラはめくらない（オーナー確認 2026-11-28 回答 C）
    assert_eq!(
        run(sanma()),
        vec!["dahai", "daiminkan", "tsumo", "dahai", "hora", "kyoku_end"]
    );
    // ankan_immediate：打牌の直後（応答の前）にめくる（majiang-core と同じ）
    assert_eq!(
        run(with("[dora]\nkan_dora_timing = 'ankan_immediate'")),
        vec![
            "dahai",
            "daiminkan",
            "tsumo",
            "dahai",
            "dora",
            "hora",
            "kyoku_end"
        ]
    );
}

#[test]
fn dora_kan_dora_true_and_false_and_timing() {
    // 天鳳三麻：暗槓は即乗り、大明槓は後めくり（打牌の後）
    assert_eq!(
        kan_dora_events(sanma()),
        vec![
            "tsumo",
            "ankan",
            "dora",
            "tsumo",
            "dahai",
            "daiminkan",
            "tsumo",
            "dahai",
            "dora",
            "tsumo"
        ]
    );
    assert_eq!(
        kan_dora_events(with("[dora]\nkan_dora_timing = 'all_immediate'")),
        vec![
            "tsumo",
            "ankan",
            "dora",
            "tsumo",
            "dahai",
            "daiminkan",
            "dora",
            "tsumo",
            "dahai",
            "tsumo"
        ]
    );
    assert_eq!(
        kan_dora_events(with("[dora]\nkan_dora_timing = 'all_delayed'")),
        vec![
            "tsumo",
            "ankan",
            "tsumo",
            "dahai",
            "dora",
            "daiminkan",
            "tsumo",
            "dahai",
            "dora",
            "tsumo"
        ]
    );
    assert_eq!(
        kan_dora_events(with("[dora]\nkan_dora = false")),
        vec![
            "tsumo",
            "ankan",
            "tsumo",
            "dahai",
            "daiminkan",
            "tsumo",
            "dahai",
            "tsumo"
        ]
    );
}

// ------------------------------------------------------------------ 北

/// 席0 が北を1枚持つ手
const N0: &str = "N 1m 9m 2p 6p 9p 3s 7s S W F F C";

#[test]
fn kita_mode_none_and_nukidora() {
    let s = Scenario::new(sanma(), &[N0, J1, J2]).draws("E");
    assert!(has(&s.kyoku(), 0, "kita"));
    let k = Scenario {
        rules: with("[kita]\nmode = 'none'"),
        ..s
    }
    .kyoku();
    assert!(!has(&k, 0, "kita"));
    assert!(has(&k, 0, "dahai:N"), "北はふつうの字牌");
}

#[test]
fn kita_mandatory_true_and_false() {
    let s = Scenario::new(sanma(), &[N0, J1, J2]).draws("E");
    assert!(has(&s.kyoku(), 0, "dahai:N"));
    let k = Scenario {
        rules: with("[kita]\nmandatory = true"),
        ..s
    }
    .kyoku();
    assert!(has(&k, 0, "kita"));
    assert!(!has(&k, 0, "dahai:N"), "北は手牌に残せない");
}

#[test]
fn kita_after_pon_true_and_false() {
    // 席0 が席1 の白をポンした直後に北を抜けるか
    let s0 = "N P P 9m 2p 6p 9p 3s 7s S W F C";
    let s1 = "P 1m 9m 1p 4p 8p 2s 6s 9s S W F C";
    let s2 = "1m 1p 5p 7p 3s 4s 8s E W W 1s 2s 6s";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2]).draws("E 1m").kyoku();
        act(&mut k, 0, "dahai:E:tsumogiri");
        act(&mut k, 1, "dahai:P");
        act(&mut k, 0, "pon:P,P");
        has(&k, 0, "kita")
    };
    assert!(!run(sanma()));
    assert!(run(with("[kita]\nafter_pon = true")));
}

#[test]
fn kita_after_riichi_forbidden_and_drawn_only() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2])
            .draws("E 1m 9m N")
            .kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        has(&k, 0, "kita")
    };
    assert!(run(sanma()), "ツモってきた北は抜ける");
    assert!(!run(with("[kita]\nafter_riichi = 'forbidden'")));
}

/// 席1：北の単騎（一気通貫）。席0 が北を抜く／切る
const N1: &str = "N 1p 2p 3p 4p 5p 6p 7p 8p 9p 2s 3s 4s";
const N0B: &str = "N 1m 9m 2p 6p 9p 3s 7s S W F F C";

#[test]
fn kita_ron_on_kita_true_and_false_and_chankan() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[N0B, N1, J2]).draws("E").kyoku();
        act(&mut k, 0, "kita");
        k
    };
    let mut k = run(sanma());
    assert!(has(&k, 1, "ron"));
    act(&mut k, 1, "ron");
    assert!(
        !last_hora(&k).yaku.iter().any(|y| y.yaku == Yaku::Chankan),
        "槍槓は付かない"
    );
    let mut k = run(with("[kita]\nron_counts_as_chankan = true"));
    act(&mut k, 1, "ron");
    assert!(last_hora(&k).yaku.iter().any(|y| y.yaku == Yaku::Chankan));
    let k = run(with("[kita]\nron_on_kita = false"));
    assert!(!has(&k, 1, "ron"));
}

#[test]
fn kita_ron_on_discard_true_and_false() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[N0B, N1, J2]).draws("E").kyoku();
        act(&mut k, 0, "dahai:N");
        has(&k, 1, "ron")
    };
    assert!(run(sanma()));
    assert!(!run(with("[kita]\nron_on_discard = false")));
}

#[test]
fn kita_breaks_ippatsu_true_and_false() {
    // 席0 が立直、席1 が北抜き、席0 が一発でツモれるか
    let s1 = "N 1m 9m 1p 4p 8p 2s 6s 9s S W P C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, s1, J2])
            .draws("E 9m 1m 1s")
            .rinshan("F")
            .kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        act(&mut k, 1, "kita");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "tsumo");
        last_hora(&k).yaku.iter().any(|y| y.yaku == Yaku::Ippatsu)
    };
    assert!(!run(sanma()));
    assert!(run(with("[kita]\nbreaks_ippatsu = false")));
}

#[test]
fn kita_breaks_first_turn_true_and_false() {
    // 席0 が第1巡に北を抜いた後、席1 の第1ツモで九種九牌を宣言できるか
    let s1 = "1m 9m 1p 9p 1s 9s E S W 2p 3p 4p 5p";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[N0B, s1, J2])
            .draws("E 6p")
            .rinshan("6s")
            .kyoku();
        act(&mut k, 0, "kita");
        auto(&mut k);
        has(&k, 1, "kyushu")
    };
    assert!(!run(sanma()));
    assert!(run(with("[kita]\nbreaks_first_turn = false")));
}

#[test]
fn rinshan_max_values() {
    let s0 = "N N 9m 2p 6p 9p 3s 7s S W F F C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, J1, J2])
            .draws("E")
            .rinshan("6s")
            .kyoku();
        act(&mut k, 0, "kita");
        has(&k, 0, "kita")
    };
    assert!(run(sanma()), "8 回まで");
    assert!(!run(with("[wall]\nrinshan_max = 1")));
}

#[test]
fn dead_wall_values() {
    // 三麻の pile は 59 枚。王牌 14（表示牌の枠 10 + 末尾 4）なら通常のツモは 55 回、親が1回引いた後は 54
    let k = Scenario::new(sanma(), &[T0, J1, J2]).draws("E").kyoku();
    assert_eq!(k.live_remaining(), 54);
    let k = Scenario::new(with("[wall]\ndead_wall = 20"), &[T0, J1, J2])
        .draws("E")
        .kyoku();
    assert_eq!(k.live_remaining(), 48);
}

// ------------------------------------------------------------------ 途中流局

const KYUSHU: &str = "1m 9m 1p 9p 1s 9s E S W 2p 3p 4p 5p";

#[test]
fn kyushu_kyuhai_true_and_false() {
    let s = Scenario::new(sanma(), &[KYUSHU, J1, J2]).draws("6p");
    assert!(has(&s.kyoku(), 0, "kyushu"));
    let k = Scenario {
        rules: with("[draws]\nkyushu_kyuhai = false"),
        ..s
    }
    .kyoku();
    assert!(!has(&k, 0, "kyushu"));
}

#[test]
fn abortive_renchan_true_and_false() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[KYUSHU, J1, J2])
            .draws("6p")
            .honba(1, 0)
            .kyoku();
        act(&mut k, 0, "kyushu");
        assert_eq!(ryukyoku(&k).reason, "kyushu_kyuhai");
        let o = k.outcome().unwrap();
        (o.renchan, o.honba_next)
    };
    assert_eq!(run(sanma()), (true, 2));
    assert_eq!(run(with("[draws]\nabortive_renchan = false")), (false, 2));
}

#[test]
fn suukaikan_true_and_false() {
    let s0 = "E E E E S S S S 1p 2p 3p 4p 5p";
    let s1 = "W W W W P P P P 1s 2s 3s 4s 9m";
    let run = |rules: Rules| {
        let s2 = "1m 1m 9m 6p 9p 3s 7s F F C C N 2s";
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("6p 6s")
            .rinshan("7p 8p 7s 8s")
            .kyoku();
        act(&mut k, 0, "ankan:E");
        act(&mut k, 0, "ankan:S");
        act(&mut k, 0, "dahai:8p:tsumogiri");
        act(&mut k, 1, "ankan:W");
        act(&mut k, 1, "ankan:P");
        act(&mut k, 1, "dahai:8s:tsumogiri");
        k
    };
    let k = run(sanma());
    assert!(k.is_done());
    assert_eq!(ryukyoku(&k).reason, "suukaikan");
    let k = run(with("[draws]\nsuukaikan = false"));
    assert!(!k.is_done(), "流局しない");
}

#[test]
fn all_riichi_true_and_false() {
    let s1 = "1s 2s 3s 4s 5s 6s 7s 8s 9s 2p 3p 7p 7p";
    let s2 = "E E E S S S W W W 6p 8p C C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, s1, s2]).draws("F F F").kyoku();
        act(&mut k, 0, "reach:F:tsumogiri");
        act(&mut k, 1, "reach:F:tsumogiri");
        act(&mut k, 2, "reach:F:tsumogiri");
        k
    };
    let k = run(sanma());
    assert!(!k.is_done(), "天鳳三麻：流局しない");
    let k = run(with("[draws]\nall_riichi = true"));
    assert_eq!(ryukyoku(&k).reason, "all_riichi");
    // 途中流局は親が聴牌でも聴牌止めの対象にしない（四麻の局単位の差分テストで見つけた。IMPLICIT_RULES_AUDIT B17）
    let o = k.outcome().unwrap();
    assert!(o.renchan && !o.hora && !o.oya_tenpai);
}

#[test]
fn same_wind_discards_true_and_false() {
    let s0 = "E 1m 9m 2p 6p 9p 3s 7s S W F F C";
    let s1 = "E 9m 9m 1p 4p 8p 2s 6s 9s W P P C";
    let s2 = "E 1m 1p 5p 7p 3s 4s 8s S W F P C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("6s 6p 2s")
            .kyoku();
        act(&mut k, 0, "dahai:E");
        act(&mut k, 1, "dahai:E");
        act(&mut k, 2, "dahai:E");
        k
    };
    assert!(!run(sanma()).is_done());
    let k = run(with("[draws]\nsame_wind_discards = true"));
    assert_eq!(ryukyoku(&k).reason, "same_wind_discards");
}

// ------------------------------------------------------------------ 同時のロン

/// 席0 の 4s に席1（1s・4s 待ち）と席2（4s 単騎・場風の東の暗刻）がロンできる
fn double_ron(rules: Rules) -> Kyoku {
    let s0 = "4s 1m 9m 2p 6p 9p 3s 7s S W F F C";
    let s2 = "1s 2s 3s 1m 1m 1m 9m 9m 9m E E E 4s";
    let mut k = Scenario::new(rules, &[s0, T0, s2])
        .draws("8p")
        .honba(1, 3)
        .kyoku();
    act(&mut k, 0, "dahai:4s");
    let acts: Vec<_> = k
        .decisions()
        .iter()
        .map(|d| (d.seat, "ron".parse().unwrap()))
        .collect();
    assert_eq!(acts.len(), 2, "2人に判断ポイント");
    k.step(&acts).unwrap();
    k
}

fn hora_seats(k: &Kyoku) -> Vec<u8> {
    k.events()
        .iter()
        .filter_map(|e| match e {
            Event::Hora(h) => Some(h.actor),
            _ => None,
        })
        .collect()
}

#[test]
fn all_ron_true_and_false() {
    let k = double_ron(sanma());
    assert_eq!(hora_seats(&k), vec![1, 2], "ダブロン");
    let k = double_ron(with("[draws]\nall_ron = true"));
    assert_eq!(ryukyoku(&k).reason, "all_ron");
}

#[test]
fn max_ron_values() {
    let k = double_ron(with("[calls]\nmax_ron = 1"));
    assert_eq!(hora_seats(&k), vec![1], "頭ハネ：放銃者の下家");
}

#[test]
fn multi_ron_deposit_upstream_and_split() {
    // 供託 3 本・1 本場。上家取り：席1 が供託 3000 と本場 1000
    let k = double_ron(sanma());
    let hs: Vec<_> = k
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Hora(h) => Some((h.deltas.clone(), h.points)),
            _ => None,
        })
        .collect();
    assert_eq!(hs[0].0[1], hs[0].1 as i32 + 1000 + 3000);
    assert_eq!(hs[1].0[2], hs[1].1 as i32);
    let k = double_ron(with("[calls]\nmulti_ron_deposit = 'split'"));
    let hs: Vec<_> = k
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Hora(h) => Some((h.deltas.clone(), h.points)),
            _ => None,
        })
        .collect();
    // 本場はそれぞれ、供託は 2本・1本（余りは上家側）
    assert_eq!(hs[0].0[1], hs[0].1 as i32 + 1000 + 2000);
    assert_eq!(hs[1].0[2], hs[1].1 as i32 + 1000 + 1000);
}

// ------------------------------------------------------------------ 鳴き（四麻）

#[test]
fn chi_true_and_false() {
    let s0 = "3m 1m 9m 2p 6p 9p 3s 7s S W F F C";
    let s1 = "4m 5m 9m 1p 4p 8p 2s 6s 9s S W P C";
    let s2 = "1m 1p 5p 7p 3s 4s 8s E S W P N C";
    let s3 = "2m 7m 2p 8p 1s 5s 9s E E N N W F";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2, s3]).draws("6s").kyoku();
        act(&mut k, 0, "dahai:3m");
        has(&k, 1, "chi:4m,5m")
    };
    assert!(run(yonma_with("")));
    assert!(!run(yonma_with("[calls]\nchi = false")));
}

#[test]
fn kuikae_each_value() {
    // 3m を 45m でチー：喰い替えなしは 3m・6m とも切れない、筋は切れる、ありはどちらも切れる
    let s0 = "3m 1m 9m 2p 6p 9p 3s 7s S W F F C";
    let s1 = "4m 5m 3m 6m 1p 4p 8p 2s 6s 9s S W P";
    let s2 = "1m 1p 5p 7p 3s 4s 8s E S W P N C";
    let s3 = "2m 7m 2p 8p 1s 5s 9s E E N N W F";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2, s3]).draws("6s").kyoku();
        act(&mut k, 0, "dahai:3m");
        act(&mut k, 1, "chi:4m,5m");
        (has(&k, 1, "dahai:3m"), has(&k, 1, "dahai:6m"))
    };
    assert_eq!(run(yonma_with("")), (false, false));
    assert_eq!(
        run(yonma_with("[calls]\nkuikae = 'suji_allowed'")),
        (false, true)
    );
    assert_eq!(run(yonma_with("[calls]\nkuikae = 'allowed'")), (true, true));
}

#[test]
fn atozuke_true_and_false() {
    // 席1：5p をポンして 中・9s のシャンポン待ち。中は役牌、9s は役なし（片和了り）
    let s1 = "5p 5p C C 9s 9s 2p 3p 4p 6s 7s 8s E";
    let s0 = "5p 1m 9m 2p 6p 9p 3s 7s S W F F 1s";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, J2]).draws("8p C").kyoku();
        act(&mut k, 0, "dahai:5p");
        act(&mut k, 1, "pon:5p,5p");
        act(&mut k, 1, "dahai:E");
        // 席2 が中をツモ切り
        auto(&mut k);
        has(&k, 1, "ron")
    };
    assert!(run(sanma()));
    assert!(!run(with("[calls]\natozuke = false")));
}

// ------------------------------------------------------------------ 荒牌流局（ツモを 29 回に縮める）

const SHORT: &str = "[wall]\ndead_wall = 40";

#[test]
fn haitei_call_true_and_false() {
    // 最後のツモ（席1）の 5s を、5s の対子を持つ席0 がポンできるか
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2]).haitei("5s").kyoku();
        loop {
            if k.live_remaining() == 0 && k.decisions().iter().any(|d| d.seat == 0) {
                break has(&k, 0, "pon:5s,5s");
            }
            if k.is_done() {
                break false;
            }
            auto(&mut k);
        }
    };
    assert!(!run(with(SHORT)));
    assert!(run(with(&format!("{SHORT}\n[calls]\nhaitei_call = true"))));
}

#[test]
fn renchan_each_value() {
    // 親（席0）が聴牌のまま荒牌流局
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2]).kyoku();
        auto_to_end(&mut k);
        let r = ryukyoku(&k);
        assert_eq!(r.reason, "exhaustive");
        assert!(r.tenpai[0]);
        k.outcome().unwrap().renchan
    };
    assert!(run(with(SHORT)), "聴牌連荘");
    assert!(!run(with(&format!("{SHORT}\n[draws]\nrenchan = 'agari'"))));
    // 親がノーテンでも連荘
    let run_noten = |rules: Rules| {
        let mut k = Scenario::new(rules, &[J1, T0, J2]).kyoku();
        auto_to_end(&mut k);
        assert!(!ryukyoku(&k).tenpai[0]);
        k.outcome().unwrap().renchan
    };
    assert!(!run_noten(with(SHORT)));
    assert!(run_noten(with(&format!(
        "{SHORT}\n[draws]\nrenchan = 'always'"
    ))));
}

#[test]
fn keishiki_tenpai_each_value() {
    let s0 = "9m 9m 2p 4p 8p 2s 6s 9s S W P P F";
    let run = |s1: &str, v: &str| {
        let rules = with(&format!("{SHORT}\n[draws]\nkeishiki_tenpai = '{v}'"));
        let mut k = Scenario::new(rules, &[s0, s1, J1]).kyoku();
        auto_to_end(&mut k);
        ryukyoku(&k).tenpai[1]
    };
    // 席1：1p を4枚使った単騎（5枚目待ち）だけの形。役（東・中）はある
    let pure_karaten = "1p 1p 1p 1p 5s 6s 7s E E E C C C";
    // 席1：役のない聴牌（6s・9s 待ち。東の雀頭は場風なので平和にならない）
    let no_yaku = "2p 3p 4p 6p 7p 8p 2s 3s 4s 7s 8s E E";
    assert!(run(pure_karaten, "any"));
    assert!(!run(pure_karaten, "no_pure_karaten"));
    assert!(!run(pure_karaten, "yaku_required"));
    assert!(run(no_yaku, "any"));
    assert!(run(no_yaku, "no_pure_karaten"));
    assert!(!run(no_yaku, "yaku_required"));
}

#[test]
fn noten_declaration_true_and_false() {
    // 河底の打牌（席1）への応答で、聴牌の席0 が「見送ってノーテン」を選べるか
    let run = |rules: Rules, declare: bool| {
        let mut k = Scenario::new(rules, &[T0, J1, J2]).kyoku();
        let mut offered = false;
        while !k.is_done() {
            if k.live_remaining() == 0 && has(&k, 0, "pass:noten") {
                offered = true;
                act(&mut k, 0, if declare { "pass:noten" } else { "pass" });
                continue;
            }
            auto(&mut k);
        }
        (offered, ryukyoku(&k).tenpai[0])
    };
    assert_eq!(run(with(SHORT), true), (false, true));
    let r = with(&format!("{SHORT}\n[draws]\nnoten_declaration = true"));
    assert_eq!(run(r.clone(), true), (true, false), "ノーテン宣言");
    assert_eq!(run(r, false), (true, true));
}

#[test]
fn nagashi_mangan_true_and_false() {
    // 席2 のツモ（9回）をすべて么九牌にし、ツモ切りで河を么九牌だけにする。席0・席1 は中張牌をツモ切り
    let yao = ["1m", "9m", "1p", "9p", "1s", "9s", "E", "S", "W"];
    let simple = [
        "2p", "3p", "4p", "5p", "6p", "7p", "8p", "2s", "3s", "4s", "5s", "6s", "7s", "8s", "2p",
        "3p", "4p", "6p", "7p", "8p",
    ];
    let mut draws = Vec::new();
    let (mut y, mut s) = (0, 0);
    for i in 0..29 {
        if i % 3 == 2 {
            draws.push(yao[y]);
            y += 1;
        } else {
            draws.push(simple[s]);
            s += 1;
        }
    }
    let s0 = "1m 1m 2p 6p 3s 7s S W F F C N N";
    let s1 = "9m 9m 1p 4p 8p 2s 6s 9s P P C N N";
    let s2 = "1m 9m 1p 5p 7p 3s 4s 8s E S P F C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws(&draws.join(" "))
            .kyoku();
        auto_to_end(&mut k);
        let r = ryukyoku(&k);
        (r.reason, r.deltas.clone())
    };
    let (reason, d) = run(with(SHORT));
    assert_eq!(reason, "nagashi_mangan");
    assert_eq!(d, vec![-4000, -2000, 6000], "子の満貫ツモ（ツモ損）");
    let (reason, _) = run(with(&format!("{SHORT}\n[draws]\nnagashi_mangan = false")));
    assert_eq!(reason, "exhaustive");
}

// ------------------------------------------------------------------ 役満の包・槍槓

#[test]
fn kokushi_ron_on_ankan_true_and_false() {
    let s0 = "E E E 2m 3m 4m 5p 6p 7p 2s 3s 4s 5s";
    let s1 = "1m 9m 1p 9p 1s 9s S W N P F C C";
    let s2 = "2m 7m 2p 8p 3s 5s 7s 8s S W F N P";
    let s3 = "3m 6m 3p 4p 6s 6s 8s 9s W W N F C";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2, s3]).draws("E").kyoku();
        act(&mut k, 0, "ankan:E");
        has(&k, 1, "ron")
    };
    assert!(!run(yonma_with("")));
    assert!(run(yonma_with("[yakuman]\nkokushi_ron_on_ankan = true")));
}

/// 席1 が 白（席0から）・發（席2から）・中（席0から）をポンして大三元、4p 単騎をツモ
fn daisangen(rules: Rules) -> Kyoku {
    let s0 = "P C 9m 2p 6p 9p 3s 7s S W 1s 2s 6s";
    let s1 = "P P F F C C 1s 2s 3s 4p 5p 9s 9m";
    let s2 = "F 1m 1p 5p 7p 3s 4s 8s S W E E N";
    let mut k = Scenario::new(rules, &[s0, s1, s2])
        .draws("8s 6p 8p 7p 7s 4s 4p")
        .kyoku();
    act(&mut k, 0, "dahai:P");
    act(&mut k, 1, "pon:P,P");
    act(&mut k, 1, "dahai:9m");
    act(&mut k, 2, "dahai:F");
    act(&mut k, 1, "pon:F,F");
    act(&mut k, 1, "dahai:9s");
    assert!(auto_until_seat(&mut k, 0));
    act(&mut k, 0, "dahai:C");
    act(&mut k, 1, "pon:C,C");
    act(&mut k, 1, "dahai:5p");
    assert!(auto_until_seat(&mut k, 1));
    act(&mut k, 1, "tsumo");
    k
}

#[test]
fn pao_daisangen() {
    let k = daisangen(sanma());
    let h = last_hora(&k);
    assert!(h.yaku.iter().any(|y| y.yaku == Yaku::Daisangen));
    assert_eq!(h.pao, Some(0));
    // ツモ損の子の役満ツモ（親 16000 ＋ 子 8000）の合計を包の人が払う（オーナー確認 2026-11-28 回答 E）
    assert_eq!(h.deltas, vec![-24000, 24000, 0], "包の人がツモ損後の合計額");
    let k = daisangen(with("[yakuman]\npao = ['daisuushii']"));
    let h = last_hora(&k);
    assert_eq!(h.pao, None);
    assert_eq!(h.deltas, vec![-16000, 24000, -8000]);
}

#[test]
fn pao_scope_full_and_pao_yaku_only() {
    // 席1 が 白（席0から）・發（席2から）・中（席0から）をポンし、東の暗刻と南の単騎で大三元＋字一色をツモ
    let s0 = "P C 9m 2p 6p 9p 3s 7s S W 1s 2s 6s";
    let s1 = "P P F F C C E E E S 9m 9s 5p";
    let s2 = "F 1m 1p 5p 7p 3s 4s 8s S W 9p 1s N";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("8s 6p 8p 7p 7s 4s S")
            .kyoku();
        act(&mut k, 0, "dahai:P");
        act(&mut k, 1, "pon:P,P");
        act(&mut k, 1, "dahai:9m");
        act(&mut k, 2, "dahai:F");
        act(&mut k, 1, "pon:F,F");
        act(&mut k, 1, "dahai:9s");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "dahai:C");
        act(&mut k, 1, "pon:C,C");
        act(&mut k, 1, "dahai:5p");
        assert!(auto_until_seat(&mut k, 1));
        act(&mut k, 1, "tsumo");
        let h = last_hora(&k);
        assert_eq!((h.yakuman, h.pao), (2, Some(0)));
        h.deltas.clone()
    };
    // 天鳳：全額（ツモ損のダブル役満ツモ 32000 ＋ 16000）を包の人
    assert_eq!(run(sanma()), vec![-48000, 48000, 0]);
    // 雀魂：大三元の分 24000 を包の人、字一色の分はふつうのツモ（親 16000・子 8000）
    assert_eq!(
        run(with("[yakuman]\npao_scope = 'pao_yaku_only'")),
        vec![-40000, 48000, -8000]
    );
}

#[test]
fn tenhou_win_tile_drawn_and_best() {
    // 親（席0）の天和：四暗刻。1p をツモって 1p の刻子が完成（シャンポン形）、W は雀頭
    let s0 = "1p 1p 2s 2s 2s 9s 9s 9s E E E W W";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, J1, J2]).draws("1p").kyoku();
        act(&mut k, 0, "tsumo");
        let h = last_hora(&k);
        (h.pai.to_string(), h.yakuman)
    };
    let dbl = "[yakuman]\ndouble_special = true\ntenhou_win_tile = ";
    // best：W を和了牌とみなすと四暗刻単騎（2倍）＋天和
    assert_eq!(run(with(&format!("{dbl}'best'"))), ("W".into(), 3));
    assert_eq!(run(with(&format!("{dbl}'drawn'"))), ("1p".into(), 2));
}

#[test]
fn pao_daisuushii() {
    // 席1 が 東・南・西・北 をポン（北は席2から＝包）して大四喜、9p 単騎をツモ
    let s0 = "E S 1m 9m 2p 6p 3s 7s P F C 1s 2s";
    let s1 = "E E S S W W N N 9p 4p 5p 9s 9m";
    let s2 = "W N 1p 5p 7p 3s 4s 8s P F C 6s 8p";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2])
            .draws("8s 6p 7p 7s 4s 2p 3p 9p")
            .kyoku();
        act(&mut k, 0, "dahai:E");
        act(&mut k, 1, "pon:E,E");
        act(&mut k, 1, "dahai:9m");
        act(&mut k, 2, "dahai:W");
        act(&mut k, 1, "pon:W,W");
        act(&mut k, 1, "dahai:9s");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "dahai:S");
        act(&mut k, 1, "pon:S,S");
        act(&mut k, 1, "dahai:5p");
        act(&mut k, 2, "dahai:N");
        act(&mut k, 1, "pon:N,N");
        act(&mut k, 1, "dahai:4p");
        assert!(auto_until_seat(&mut k, 1));
        act(&mut k, 1, "tsumo");
        last_hora(&k).pao
    };
    assert_eq!(run(sanma()), Some(2));
    assert_eq!(run(with("[yakuman]\npao = ['daisangen']")), None);
}

// ------------------------------------------------------------------ 祝儀

#[test]
fn chips_enabled_true_and_false() {
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[T0, J1, J2])
            .draws("E 1m 9m 1s")
            .kyoku();
        act(&mut k, 0, "reach:E:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "tsumo");
        k.outcome().unwrap().chip_deltas.clone()
    };
    assert_eq!(run(sanma()), vec![0, 0, 0]);
    let d = run(with("[chips]\nenabled = true"));
    assert!(d[0] > 0, "一発の祝儀: {d:?}");
    assert_eq!(d.iter().sum::<i32>(), 0);
}

#[test]
fn pao_suukantsu() {
    // 四麻：席1 が 1p・2p・4p を暗槓し、席0 の 7s を大明槓（4つ目の槓＝包は席0）、嶺上で 5s 単騎をツモ
    let s0 = "3m 1m 9m 5p 6p 9p 3s 8s S W F F C";
    let s1 = "1p 1p 1p 1p 2p 2p 2p 2p 4p 4p 4p 4p 5s";
    let s2 = "1m 3p 5p 7p 3s 4s 8s E S W P N C";
    let s3 = "2m 7m 3p 8p 1s 6s 9s E E N N W F";
    let run = |rules: Rules| {
        let mut k = Scenario::new(rules, &[s0, s1, s2, s3])
            .draws("6m 7s 6m 6m 7s")
            .rinshan("7s 7s 9m 5s")
            .kyoku();
        auto(&mut k);
        act(&mut k, 1, "ankan:1p");
        act(&mut k, 1, "ankan:2p");
        act(&mut k, 1, "ankan:4p");
        act(&mut k, 1, "dahai:9m:tsumogiri");
        assert!(auto_until_seat(&mut k, 0));
        act(&mut k, 0, "dahai:7s:tsumogiri");
        act(&mut k, 1, "daiminkan");
        act(&mut k, 1, "tsumo");
        let h = last_hora(&k);
        assert!(
            h.yaku.iter().any(|y| y.yaku == Yaku::Suukantsu),
            "{:?}",
            h.yaku
        );
        (h.pao, h.deltas.clone())
    };
    let (pao, d) = run(yonma_with(""));
    assert_eq!(pao, Some(0));
    assert_eq!(d[0], -d[1], "包の人が全額");
    let (pao, _) = run(yonma_with("[yakuman]\npao = ['daisangen', 'daisuushii']"));
    assert_eq!(pao, None);
}
