import { describe, it, expect } from "vitest";
import { extractJsonBlocks } from "./helpers";

describe("extractJsonBlocks", () => {
  it("should extract simple JSON blocks", () => {
    const text = 'Some text {"key": "value"} some other text';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "value"}']);
  });

  it("should handle nested objects/braces", () => {
    const text = 'Nested block: {"outer": {"inner": "val"}} ok';
    expect(extractJsonBlocks(text)).toEqual(['{"outer": {"inner": "val"}}']);
  });

  it("should ignore braces inside string literals", () => {
    const text = 'Braces inside string: {"key": "val {with} braces", "another": "val2"}';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "val {with} braces", "another": "val2"}']);

    const textUnbalanced = 'Braces inside string: {"key": "val {with braces", "another": "val2"}';
    expect(extractJsonBlocks(textUnbalanced)).toEqual(['{"key": "val {with braces", "another": "val2"}']);
  });

  it("should handle escaped quotes inside string literals", () => {
    const text = 'Escaped quotes: {"key": "escaped \\" quote and {braces}"}';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "escaped \\" quote and {braces}"}']);
  });

  it("should extract multiple JSON blocks", () => {
    const text = 'First {"a": 1} second {"b": 2}';
    expect(extractJsonBlocks(text)).toEqual(['{"a": 1}', '{"b": 2}']);
  });

});
