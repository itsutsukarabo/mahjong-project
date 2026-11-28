import { describe, expect, it } from "vitest";
import type { ReplayMeta, TableView } from "@mj/log-types";
import { ReplayModel } from "../src/index";

const state = (kyoku_idx: number, event_idx: number, junme: number, event_type = "tsumo"): TableView => ({
  kyoku_idx, event_idx, event_type, actor: 0, bakaze: "E", kyoku: 1, honba: 0, kyotaku: 0, oya: 0,
  dora_indicators: [], live_remaining: 0, junme, seats: [], result: null,
});

const meta: ReplayMeta = {
  game_id: "g", visibility: "full", num_players: 3, players: ["a", "b", "c"], game_end: null,
  kyokus: [
    { kyoku_idx: 0, bakaze: "E", kyoku: 1, honba: 0, end: "hora", events: 4 },
    { kyoku_idx: 1, bakaze: "E", kyoku: 2, honba: 0, end: "exhaustive", events: 3 },
  ],
};
const loader = async (k: number) =>
  k === 0 ? [state(0, 0, 1, "kyoku_start"), state(0, 1, 1), state(0, 2, 2), state(0, 3, 2, "hora")]
          : [state(1, 0, 1, "kyoku_start"), state(1, 1, 1), state(1, 2, 1, "ryukyoku")];

describe("ReplayModel", () => {
  it("steps across kyoku boundaries both ways", async () => {
    const m = await ReplayModel.open(meta, loader);
    const seen: string[] = [];
    do seen.push(`${m.view.kyoku_idx}:${m.view.event_idx}`); while (await m.next());
    expect(seen).toEqual(["0:0", "0:1", "0:2", "0:3", "1:0", "1:1", "1:2"]);
    expect(m.atEnd).toBe(true);
    await m.prev();
    await m.prev();
    await m.prev();
    expect(`${m.view.kyoku_idx}:${m.view.event_idx}`).toBe("0:3");
  });

  it("seeks to a junme and matches annotations by key", async () => {
    const m = await ReplayModel.open(meta, loader);
    m.seekJunme(2);
    expect(m.view.event_idx).toBe(2);
    m.setAnnotations([{ game_id: "g", kyoku_idx: 0, event_idx: 2, seat: 1, decision: "pass" }]);
    expect(m.annotations.map((a) => a.seat)).toEqual([1]);
    expect(() => m.setAnnotations([{ game_id: "x", kyoku_idx: 0, event_idx: 2, seat: 1, decision: "pass" }])).toThrow();
  });

  it("rejects states whose keys do not match their position", async () => {
    await expect(ReplayModel.open(meta, async () => [state(0, 1, 1)])).rejects.toThrow();
  });
});
