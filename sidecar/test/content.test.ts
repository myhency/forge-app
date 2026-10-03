import { describe, expect, it } from "vitest";
import { buildContent } from "../src/content.js";

const png = Buffer.from("PNGDATA");
const read = (path: string): Buffer => {
  if (path.includes("missing")) throw new Error("ENOENT");
  return png;
};

describe("buildContent", () => {
  it("is the plain text when there are no attachments", () => {
    expect(buildContent("hello", [], read)).toBe("hello");
  });

  it("adds an @path line for a file and keeps the content a string", () => {
    expect(buildContent("look", [{ kind: "file", path: "/work/a b/notes 1.txt" }], read)).toBe(
      "look\n@/work/a b/notes 1.txt",
    );
  });

  it("makes an image a base64 image block before the text", () => {
    expect(buildContent("what is this", [{ kind: "image", path: "/work/사진 1.PNG" }], read)).toEqual([
      { type: "image", source: { type: "base64", media_type: "image/png", data: png.toString("base64") } },
      { type: "text", text: "what is this" },
    ]);
  });

  it("maps jpg, jpeg, gif and webp to their media types", () => {
    const types = ["a.jpg", "a.jpeg", "a.gif", "a.webp"].map((name) => {
      const blocks = buildContent("", [{ kind: "image", path: `/x/${name}` }], read) as any[];
      return blocks[0].source.media_type;
    });
    expect(types).toEqual(["image/jpeg", "image/jpeg", "image/gif", "image/webp"]);
  });

  it("mixes an image and a file: the file line goes in the text block", () => {
    const content = buildContent(
      "both",
      [
        { kind: "image", path: "/x/a.png" },
        { kind: "file", path: "/x/b.txt" },
      ],
      read,
    ) as any[];
    expect(content[1]).toEqual({ type: "text", text: "both\n@/x/b.txt" });
  });

  it("says so in the text when a file cannot be read, and still sends the rest", () => {
    expect(buildContent("hi", [{ kind: "image", path: "/x/missing.png" }], read)).toBe(
      "hi\n[could not read /x/missing.png]",
    );
  });

  it("sends attachments alone when the text is empty", () => {
    expect(buildContent("", [{ kind: "file", path: "/x/b.txt" }], read)).toBe("@/x/b.txt");
  });
});
