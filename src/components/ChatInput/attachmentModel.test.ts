import { describe, expect, it } from "vitest";
import { isImageFile, isImagePath } from "./attachmentModel";

describe("attachment model helpers", () => {
  it("recognizes image extensions and MIME types", () => {
    expect(isImagePath("router.PNG")).toBe(true);
    expect(isImageFile(new File(["data"], "router.txt", { type: "image/png" }))).toBe(true);
    expect(isImagePath("router.conf")).toBe(false);
  });
});
