import { readFileSync } from "node:fs";
import { extname } from "node:path";
import type { WireAttachment } from "./protocol.js";

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; source: { type: "base64"; media_type: string; data: string } };

const MEDIA_TYPES: Record<string, string> = {
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".webp": "image/webp",
};

// The content of a user message: a plain string unless an image is attached. A file is an `@path`
// line for the model's Read tool. A file that cannot be read is named in the text instead of
// failing the message: the rest still goes.
export function buildContent(
  text: string,
  attachments: WireAttachment[],
  read: (path: string) => Buffer = readFileSync,
): string | ContentBlock[] {
  const lines = text === "" ? [] : [text];
  const images: ContentBlock[] = [];
  for (const attachment of attachments) {
    if (attachment.kind === "file") {
      lines.push(`@${attachment.path}`);
      continue;
    }
    try {
      const mediaType = MEDIA_TYPES[extname(attachment.path).toLowerCase()] ?? "image/png";
      images.push({
        type: "image",
        source: { type: "base64", media_type: mediaType, data: read(attachment.path).toString("base64") },
      });
    } catch {
      lines.push(`[could not read ${attachment.path}]`);
    }
  }
  const joined = lines.join("\n");
  if (images.length === 0) return joined;
  // The API refuses an empty text block, so an image sent alone has none.
  return joined === "" ? images : [...images, { type: "text", text: joined }];
}
