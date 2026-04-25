import { describe, expect, it } from "vitest";
import { feedJsonChunks } from "./ndjson";

describe("feedJsonChunks", () => {
  it("returns empty buffer and no messages for empty input", () => {
    expect(feedJsonChunks("", "")).toEqual({ buffer: "", messages: [] });
  });

  it("returns empty buffer when only whitespace", () => {
    expect(feedJsonChunks("", "   \n\t\r")).toEqual({ buffer: "", messages: [] });
  });

  it("parses a single complete JSON object", () => {
    const { buffer, messages } = feedJsonChunks("", '{"price":42}');
    expect(buffer).toBe("");
    expect(messages).toEqual([{ price: 42 }]);
  });

  it("parses empty object", () => {
    expect(feedJsonChunks("", "{}")).toEqual({ buffer: "", messages: [{}] });
  });

  it("strips leading whitespace before an object", () => {
    const { messages } = feedJsonChunks("", '\n  {"ok":true}');
    expect(messages).toEqual([{ ok: true }]);
  });

  it("parses multiple objects in one chunk separated by whitespace", () => {
    const { buffer, messages } = feedJsonChunks(
      "",
      '{"a":1}\n{"b":2}\r\n  {"c":3}'
    );
    expect(buffer).toBe("");
    expect(messages).toEqual([{ a: 1 }, { b: 2 }, { c: 3 }]);
  });

  it("parses multiple objects with no separator (concatenated)", () => {
    const { messages } = feedJsonChunks("", '{"x":1}{"y":2}');
    expect(messages).toEqual([{ x: 1 }, { y: 2 }]);
  });

  it("handles nested objects as one message", () => {
    const raw = '{"outer":{"inner":{"n":-1}}}';
    const { buffer, messages } = feedJsonChunks("", raw);
    expect(buffer).toBe("");
    expect(messages).toEqual([JSON.parse(raw)]);
  });

  it("buffers an incomplete object and emits nothing until closed", () => {
    const first = feedJsonChunks("", '{"half":');
    expect(first).toEqual({ buffer: '{"half":', messages: [] });

    const second = feedJsonChunks(first.buffer, "true}");
    expect(second.buffer).toBe("");
    expect(second.messages).toEqual([{ half: true }]);
  });

  it("buffers when chunk splits inside nested braces", () => {
    const p1 = feedJsonChunks("", '{"a":{"b"');
    expect(p1.messages).toEqual([]);
    expect(p1.buffer).toBe('{"a":{"b"');

    const p2 = feedJsonChunks(p1.buffer, ":1}}");
    expect(p2.buffer).toBe("");
    expect(p2.messages).toEqual([{ a: { b: 1 } }]);
  });

  it("returns completed messages and buffers remainder after first complete object", () => {
    const { buffer, messages } = feedJsonChunks("", '{"done":1}{"next"');
    expect(messages).toEqual([{ done: 1 }]);
    expect(buffer).toBe('{"next"');
  });

  it("accumulates prior buffer with new chunk before parsing", () => {
    const step1 = feedJsonChunks("", '{"k');
    const step2 = feedJsonChunks(step1.buffer, '": "v"}');
    expect(step2).toEqual({ buffer: "", messages: [{ k: "v" }] });
  });

  it("skips non-opening-brace bytes until next object", () => {
    const { messages } = feedJsonChunks("", 'noise{"v":1}');
    expect(messages).toEqual([{ v: 1 }]);
  });

  it("parses object after garbage prefix one byte at a time", () => {
    const { messages } = feedJsonChunks("", 'x{"only":null}');
    expect(messages).toEqual([{ only: null }]);
  });

  it("parses valid JSON with arrays and primitives inside the object", () => {
    const raw = '{"arr":[1,2,3],"flag":false,"nil":null}';
    expect(feedJsonChunks("", raw).messages).toEqual([JSON.parse(raw)]);
  });

  it("parses numbers and unicode in strings", () => {
    const raw = '{"pi":3.14,"u":"café ☕"}';
    expect(feedJsonChunks("", raw).messages).toEqual([JSON.parse(raw)]);
  });

  it("recovers from invalid JSON with balanced braces by advancing past opening brace", () => {
    const { buffer, messages } = feedJsonChunks("", "{not-json}");
    expect(messages).toEqual([]);
    expect(buffer).toBe("");
  });

  it("after invalid segment can still parse following valid object", () => {
    const { messages } = feedJsonChunks("", '{bad}{"good":1}');
    expect(messages).toEqual([{ good: 1 }]);
  });

  it("clears buffer when prior was incomplete and new chunk completes and consumes all", () => {
    const mid = feedJsonChunks('{"a":1', "}");
    expect(mid).toEqual({ buffer: "", messages: [{ a: 1 }] });
    const tail = feedJsonChunks(mid.buffer, "");
    expect(tail).toEqual({ buffer: "", messages: [] });
  });

  it("handles deep nesting without premature close", () => {
    let inner = "null";
    for (let d = 0; d < 30; d += 1) {
      inner = `{"x":${inner}}`;
    }
    const raw = `{"wrap":${inner}}`;
    const { messages, buffer } = feedJsonChunks("", raw);
    expect(buffer).toBe("");
    expect(messages).toHaveLength(1);
    expect(messages[0]).toEqual(JSON.parse(raw));
  });

  it("streams many small chunks into one object", () => {
    let buffer = "";
    const parts = ["{", '"', "x", '"', ":", "1", "}"];
    const all: unknown[] = [];
    for (const part of parts) {
      const out = feedJsonChunks(buffer, part);
      buffer = out.buffer;
      all.push(...out.messages);
    }
    expect(buffer).toBe("");
    expect(all).toEqual([{ x: 1 }]);
  });

  it("emits multiple messages when object boundaries fall across chunks", () => {
    let buffer = "";
    const chunks = ['{"a":1}', '{"b":2}'];
    const messages: unknown[] = [];
    for (const c of chunks) {
      const out = feedJsonChunks(buffer, c);
      buffer = out.buffer;
      messages.push(...out.messages);
    }
    expect(buffer).toBe("");
    expect(messages).toEqual([{ a: 1 }, { b: 2 }]);
  });

  it("treats root-level arrays as skipped characters until a brace object appears", () => {
    const { messages, buffer } = feedJsonChunks("", "[1,2,3]{\"only\":0}");
    expect(messages).toEqual([{ only: 0 }]);
    expect(buffer).toBe("");
  });

  it("parses a full object held in priorBuffer when chunk is empty", () => {
    expect(feedJsonChunks('{"solo":[]}', "")).toEqual({
      buffer: "",
      messages: [{ solo: [] }],
    });
  });

  it("parses complete object in prior then completes split object in chunk", () => {
    const { buffer, messages } = feedJsonChunks('{"first":1}{"second"', ":2}");
    expect(messages).toEqual([{ first: 1 }, { second: 2 }]);
    expect(buffer).toBe("");
  });

  it("fails to parse objects when a string value contains } (brace depth ignores quotes)", () => {
    const { buffer, messages } = feedJsonChunks("", '{"text":"}"}');
    expect(messages).toEqual([]);
    expect(buffer).toBe("");
  });
});
