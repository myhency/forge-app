import { describe, expect, it } from "vitest";
import { toWireUsage, turnUsage } from "../src/usage.js";

const zero = { input: 0, output: 0, cache_read: 0, cache_write_5m: 0, cache_write_1h: 0 };

describe("toWireUsage", () => {
  it("maps the result's usage to the wire shape and reads the 5m/1h cache split", () => {
    expect(
      toWireUsage({
        input_tokens: 1,
        output_tokens: 2,
        cache_read_input_tokens: 3,
        cache_creation_input_tokens: 300,
        cache_creation: { ephemeral_5m_input_tokens: 100, ephemeral_1h_input_tokens: 200 },
      }),
    ).toEqual({ input: 1, output: 2, cache_read: 3, cache_write_5m: 100, cache_write_1h: 200 });
  });

  it("counts a cache write without a split as 5m", () => {
    expect(toWireUsage({ cache_creation_input_tokens: 300 })).toEqual({ ...zero, cache_write_5m: 300 });
  });

  it("does not trust a split that adds up to nothing while the plain total is not zero", () => {
    expect(
      toWireUsage({
        cache_creation_input_tokens: 300,
        cache_creation: { ephemeral_5m_input_tokens: 0, ephemeral_1h_input_tokens: 0 },
      }),
    ).toEqual({ ...zero, cache_write_5m: 300 });
  });

  it("treats missing, negative and non-numeric fields as zero", () => {
    expect(toWireUsage({ input_tokens: -5, output_tokens: "x", cache_read_input_tokens: NaN })).toEqual(zero);
    expect(toWireUsage(undefined)).toEqual(zero);
    expect(toWireUsage(null)).toEqual(zero);
  });
});

describe("turnUsage", () => {
  it("keys the turn's usage by the model that produced it", () => {
    expect(turnUsage("claude-test", { input_tokens: 4, output_tokens: 5 })).toEqual({
      "claude-test": { ...zero, input: 4, output: 5 },
    });
  });

  it("sends nothing when the model is unknown", () => {
    expect(turnUsage(null, { input_tokens: 4, output_tokens: 5 })).toEqual({});
  });

  it("sends nothing when the turn used nothing", () => {
    expect(turnUsage("claude-test", { input_tokens: 0, output_tokens: 0 })).toEqual({});
    expect(turnUsage("claude-test", undefined)).toEqual({});
  });
});
