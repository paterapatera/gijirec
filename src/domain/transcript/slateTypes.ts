import { Element, type Node } from "slate";

export type CustomText = {
  text: string;
  locked?: boolean;
};

export type TranscriptBlockElement = {
  type: "transcript-block";
  blockId: string;
  sequence: number;
  upstreamText: string;
  startTimestampMs: number;
  language: string;
  children: CustomText[];
};

/** Slate CustomTypes の Element は TranscriptBlockElement のみ。 */
export function isTranscriptBlockElement(node: Node): node is TranscriptBlockElement {
  // Runtime documents may still contain other element types (e.g. handwriting paragraphs).
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- keep runtime discriminant
  return Element.isElement(node) && node.type === "transcript-block";
}

declare module "slate" {
  interface CustomTypes {
    Text: CustomText;
    Element: TranscriptBlockElement;
  }
}
