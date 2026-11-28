import { describe, expect, it } from "vitest";
import type { Annotation, SeatView, TableView } from "@mj/log-types";
import { actionOfSlot, bandSlots, cuesBetween, detectSelectionMode, layoutAnnotation, placeKey, slotOfAction } from "../src/index";

const seat = (hand: string[], drawn: string | null, extra: Partial<SeatView> = {}): SeatView => ({
  seat: 0, name: "a", wind: "E", score: 35000, hand, drawn, river: [], melds: [], kita: [], riichi: false, ...extra,
});

const view = (event_idx: number, event_type: string, actor: number | null, seats: SeatView[]): TableView => ({
  kyoku_idx: 0, event_idx, event_type, actor, bakaze: "E", kyoku: 1, honba: 0, kyotaku: 0, oya: 0,
  dora_indicators: ["1m"], live_remaining: 40, junme: 1, seats, result: null,
});

describe("band slots", () => {
  it("maps each hand tile and the drawn tile to a slot and an action", () => {
    const s = bandSlots(seat(["1m", "5p", "5pr", "5p"], "5p"));
    expect(s.map((x) => x.action)).toEqual(["dahai:1m", "dahai:5p", "dahai:5pr", "dahai:5p", "dahai:5p:tsumogiri"]);
    expect(s[4]?.drawn).toBe(true);
    expect(slotOfAction(s, "dahai:5p")).toBe(1);
    expect(slotOfAction(s, "dahai:5p:tsumogiri")).toBe(4);
    expect(slotOfAction(s, "dahai:9s")).toBeNull();
    expect(actionOfSlot(s, 2)).toBe("dahai:5pr");
  });

  it("has no drawn slot after a call", () => {
    expect(bandSlots(seat(["1m", "2m"], null)).length).toBe(2);
  });

  it("splits an annotation into slot choices and other choices", () => {
    const a: Annotation = {
      game_id: "g", kyoku_idx: 0, event_idx: 1, seat: 0, decision: "dahai:5p:tsumogiri",
      choices: ["kita", "reach:5p", "dahai:1m", "dahai:5p", "dahai:5p:tsumogiri"],
      policy: [["kita", 0.1], ["reach:5p", 0.2], ["dahai:1m", 0.4], ["dahai:5p", 0.05], ["dahai:5p:tsumogiri", 0.25]],
      value: 0.3,
    };
    const l = layoutAnnotation(a, seat(["1m", "5p", "N"], "5p"));
    expect(l.others.map((c) => c.action)).toEqual(["kita", "reach:5p"]);
    expect(l.bySlot.get(0)?.action).toBe("dahai:1m");
    expect(l.bySlot.get(1)?.action).toBe("dahai:5p");
    expect(l.bySlot.get(3)?.action).toBe("dahai:5p:tsumogiri");
    expect(l.bySlot.has(2)).toBe(false);
    expect(l.bestAction).toBe("dahai:1m");
    expect(l.differsFromBest).toBe(true);
  });

  it("puts hidden-hand discards in the other list", () => {
    const a: Annotation = { game_id: "g", kyoku_idx: 0, event_idx: 1, seat: 0, decision: "dahai:1m", choices: ["dahai:1m", "dahai:2m"] };
    const l = layoutAnnotation(a, seat(["?", "?"], "?"));
    expect(l.bySlot.size).toBe(0);
    expect(l.others.length).toBe(2);
    expect(l.differsFromBest).toBe(false);
  });
});

describe("selection mode", () => {
  const ann = (decision: string, policy: [string, number][]): Annotation => ({ game_id: "g", kyoku_idx: 0, event_idx: 1, seat: 0, decision, policy });
  it("uses the selection field first", () => {
    const a = (selection: string): Annotation => ({ ...ann("a", [["a", 0.2], ["b", 0.8]]), selection });
    expect(detectSelectionMode([a("greedy")], "x.sampled.annot.jsonl")).toMatchObject({ kind: "argmax", source: "field" });
    expect(detectSelectionMode([a("sample")])).toMatchObject({ kind: "sample", source: "field" });
    expect(detectSelectionMode([a("sample"), a("greedy")]).kind).toBe("unknown");
  });
  it("uses the file name next (not the word sample in sample.annot.jsonl)", () => {
    expect(detectSelectionMode([], "fixtures/sample.annot.jsonl").kind).toBe("unknown");
    expect(detectSelectionMode([], "x.greedy.annot.jsonl").kind).toBe("argmax");
    expect(detectSelectionMode([], "x.sampled.annot.jsonl").kind).toBe("sample");
  });
  it("infers sampling when a non-best action was chosen", () => {
    const m = detectSelectionMode([ann("a", [["a", 0.2], ["b", 0.8]])]);
    expect(m).toMatchObject({ kind: "sample", source: "inferred" });
  });
  it("does not call uniform policies argmax", () => {
    const list = Array.from({ length: 20 }, () => ann("a", [["a", 0.5], ["b", 0.5]]));
    expect(detectSelectionMode(list).kind).toBe("unknown");
    const peaked = Array.from({ length: 20 }, () => ann("a", [["a", 0.9], ["b", 0.1]]));
    expect(detectSelectionMode(peaked).kind).toBe("argmax");
  });
});

describe("animation cues", () => {
  it("moves a tedashi discard from its hand slot and shifts the drawn tile into the hand", () => {
    const prev = view(1, "tsumo", 0, [seat(["1m", "3m", "9p"], "2m")]);
    const next = view(2, "dahai", 0, [seat(["1m", "2m", "3m"], null, { river: [{ pai: "9p", tsumogiri: false, riichi: false, called: false, ron: false }] })]);
    const c = cuesBetween(prev, next);
    expect(c.moves.map((m) => `${placeKey(m.from)}>${placeKey(m.to)}:${m.role}`)).toEqual([
      "h:0:2>r:0:0:main",
      "d:0>h:0:1:shift",
      "h:0:1>h:0:2:shift",
    ]);
  });

  it("moves a tsumogiri discard from the drawn slot", () => {
    const prev = view(1, "tsumo", 0, [seat(["1m"], "1m")]);
    const next = view(2, "dahai", 0, [seat(["1m"], null, { river: [{ pai: "1m", tsumogiri: true, riichi: false, called: false, ron: false }] })]);
    expect(placeKey(cuesBetween(prev, next).moves[0]!.from)).toBe("d:0");
  });

  it("moves a called tile from the river to the meld and shouts", () => {
    const r = { pai: "C", tsumogiri: false, riichi: false, called: false, ron: false };
    const prev = view(5, "dahai", 1, [seat(["C", "C", "1m"], null), seat([], null, { seat: 1, river: [r] })]);
    const next = view(6, "pon", 0, [
      seat(["1m"], null, { melds: [{ type: "pon", pai: "C", consumed: ["C", "C"], target: 1 }] }),
      seat([], null, { seat: 1, river: [{ ...r, called: true }] }),
    ]);
    const c = cuesBetween(prev, next);
    expect(c.shout).toEqual({ seat: 0, kind: "pon" });
    expect(placeKey(c.moves[0]!.from)).toBe("r:1:0");
    expect(placeKey(c.moves[0]!.to)).toBe("m:0:0");
  });

  it("does nothing when not stepping forward by one", () => {
    const a = view(1, "tsumo", 0, [seat([], "1m")]);
    expect(cuesBetween(a, view(3, "dahai", 0, [seat([], null)])).moves).toEqual([]);
    expect(cuesBetween(null, a).appear).toEqual([]);
  });
});
