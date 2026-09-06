import { Editor, type Operation, type Path, Range, Transforms } from "slate";
import { isTranscriptBlockElement } from "../../../domain/transcript/slateTypes";

/** Scroll container style for AiTranscriptEditor — keeps read position via overflow-anchor (req 4.1). */
export const AI_TRANSCRIPT_SCROLL_STYLE = {
  overflowY: "auto",
  overflowAnchor: "auto",
} as const;

export type StableSelectionEditor = Editor & {
  stableSelectionRef: { current: Range | null };
};

function isAppendPath(editor: Editor, path: Path): boolean {
  return path.length === 1 && path[0] === editor.children.length;
}

function isEndAppendInsert(operation: Operation, editor: Editor): boolean {
  return (
    operation.type === "insert_node" &&
    isTranscriptBlockElement(operation.node) &&
    isAppendPath(editor, operation.path)
  );
}

function isAtDocumentEnd(editor: Editor, selection: Range): boolean {
  const lastBlockIndex = editor.children.length - 1;
  if (lastBlockIndex < 0) {
    return true;
  }

  const lastBlock = editor.children[lastBlockIndex];
  if (lastBlock === undefined || !isTranscriptBlockElement(lastBlock)) {
    return false;
  }

  const endPoint = Editor.end(editor, [lastBlockIndex]);
  return (
    Range.isCollapsed(selection) && Range.equals(selection, { anchor: endPoint, focus: endPoint })
  );
}

function isNonEndEditSelection(editor: Editor): boolean {
  const { selection } = editor;
  if (!selection) {
    return false;
  }
  return !isAtDocumentEnd(editor, selection);
}

function cloneRange(range: Range): Range {
  return {
    anchor: { path: [...range.anchor.path], offset: range.anchor.offset },
    focus: { path: [...range.focus.path], offset: range.focus.offset },
  };
}

function updateStableSelectionRef(ref: { current: Range | null }, selection: Range | null): void {
  ref.current = selection ? cloneRange(selection) : null;
}

export function withStableSelection<T extends Editor>(editor: T): T & StableSelectionEditor {
  const { apply: baseApply } = editor;
  const stableSelectionRef: { current: Range | null } = { current: null };

  const stableEditor = editor as T & StableSelectionEditor;
  stableEditor.stableSelectionRef = stableSelectionRef;

  editor.apply = (operation: Operation) => {
    let selectionToRestore: Range | null = null;

    if (isEndAppendInsert(operation, editor) && editor.selection && isNonEndEditSelection(editor)) {
      selectionToRestore = cloneRange(editor.selection);
    }

    baseApply(operation);

    if (selectionToRestore) {
      Transforms.select(editor, selectionToRestore);
      updateStableSelectionRef(stableSelectionRef, selectionToRestore);
      return;
    }

    if (operation.type === "set_selection") {
      updateStableSelectionRef(stableSelectionRef, editor.selection);
    }
  };

  return stableEditor;
}
