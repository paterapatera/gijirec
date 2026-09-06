import { Editor, Node, type Path, Range, Text, Transforms } from "slate";
import { isTranscriptBlockElement } from "../../domain/transcript/slateTypes";
import type { LockRange } from "../../domain/transcript/types";

function getBlockIdForPath(editor: Editor, path: number[]): string | null {
  const blockPath = path.slice(0, 1);
  if (blockPath.length === 0) {
    return null;
  }
  const node = Node.get(editor, blockPath);
  if (isTranscriptBlockElement(node)) {
    return node.blockId;
  }
  return null;
}

/** Applies `locked: true` to the current non-collapsed selection. */
export function lockSelection(editor: Editor): LockRange | null {
  const { selection } = editor;
  if (!selection || Range.isCollapsed(selection)) {
    return null;
  }

  Transforms.setNodes(editor, { locked: true }, { at: selection, match: Text.isText, split: true });

  const blockId = getBlockIdForPath(editor, selection.anchor.path);
  if (blockId === null) {
    return null;
  }

  return { range: selection, blockId };
}

/** Applies `locked: true` to the text leaf at `path` (direct-input lock). */
export function lockAtPath(editor: Editor, path: Path): LockRange | null {
  try {
    const node = Node.leaf(editor, path);
    if (!Text.isText(node)) {
      return null;
    }
  } catch {
    return null;
  }

  Transforms.setNodes(editor, { locked: true }, { at: path, match: Text.isText });

  const blockId = getBlockIdForPath(editor, path);
  if (blockId === null) {
    return null;
  }

  return { range: Editor.range(editor, path), blockId };
}
