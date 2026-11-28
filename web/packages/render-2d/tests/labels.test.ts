import { describe, expect, it } from "vitest";
import type { Hora } from "@mj/log-types";
import { limitName } from "../src/labels";

const hora = (han: number, fu: number, yakuman = 0, limit?: string, yaku: Hora["yaku"] = []): Hora => ({
  actor: 0, target: 1, pai: "1m", hand: { tehai: [], melds: [] }, yaku, han, fu, yakuman,
  ...(limit === undefined ? {} : { limit }),
  points: 0, ura_indicators: [], deltas: [], scores: [],
});

describe("limitName", () => {
  it("uses hora.limit when present (including 3・4 han mangan)", () => {
    expect(limitName(hora(4, 30, 0, "mangan"))).toBe("満貫");
    expect(limitName(hora(3, 70, 0, "mangan"))).toBe("満貫");
    expect(limitName(hora(4, 30, 0, "none"))).toBeNull();
    expect(limitName(hora(6, 30, 0, "haneman"))).toBe("跳満");
    expect(limitName(hora(8, 30, 0, "baiman"))).toBe("倍満");
    expect(limitName(hora(11, 30, 0, "sanbaiman"))).toBe("三倍満");
    expect(limitName(hora(0, 0, 2, "yakuman"))).toBe("2倍役満");
    expect(limitName(hora(13, 30, 1, "kazoe_yakuman"))).toBe("数え役満");
  });
  it("falls back to han only for old logs without limit", () => {
    expect(limitName(hora(4, 40))).toBeNull();
    expect(limitName(hora(5, 30))).toBe("満貫");
    expect(limitName(hora(7, 30))).toBe("跳満");
    expect(limitName(hora(13, 30, 1))).toBe("数え役満");
    expect(limitName(hora(0, 0, 1, undefined, [{ id: "daisangen", yakuman: 1 }]))).toBe("役満");
  });
});
