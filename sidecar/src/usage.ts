import type { WireTokenUsage } from "./protocol.js";

const count = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;

const isZero = (u: WireTokenUsage): boolean =>
  u.input + u.output + u.cache_read + u.cache_write_5m + u.cache_write_1h === 0;

/** The wire shape of one `result.usage`: the main loop's usage for that turn (spec §20.4, §20.10). */
export function toWireUsage(usage: any): WireTokenUsage {
  const plainWrite = count(usage?.cache_creation_input_tokens);
  const split = usage?.cache_creation;
  let write5m = plainWrite;
  let write1h = 0;
  if (typeof split === "object" && split !== null) {
    const five = count(split.ephemeral_5m_input_tokens);
    const hour = count(split.ephemeral_1h_input_tokens);
    // A split that adds up to nothing while the plain total is not zero is not trusted.
    if (five + hour > 0) {
      write5m = five;
      write1h = hour;
    }
  }
  return {
    input: count(usage?.input_tokens),
    output: count(usage?.output_tokens),
    cache_read: count(usage?.cache_read_input_tokens),
    cache_write_5m: write5m,
    cache_write_1h: write1h,
  };
}

/** What a finished turn used, keyed by the model that produced it; nothing when either is unknown. */
export function turnUsage(model: string | null, usage: unknown): Record<string, WireTokenUsage> {
  if (!model) return {};
  const wire = toWireUsage(usage);
  return isZero(wire) ? {} : { [model]: wire };
}
