import { createElement, type ReactNode } from "react";
import { type Editor, Node, type Operation, Path, Range, Text } from "slate";
import type { RenderLeafProps } from "slate-react";
import type { CustomText } from "../../../domain/transcript/slateTypes";
import { lockAtPath, lockSelection } from "../lockManager";

export type LockedLeafStyle = {
  backgroundColor: string;
  textDecoration: string;
  textDecorationColor: string;
};

/**
 * Slate editor extended with upstream apply routing.
 * Upstream block sync MUST use `applyUpstream()` — only that path rejects
 * remove/replace on locked ranges; user `Transforms` use normal `apply`.
 */
export type LockedRangesEditor = Editor & {
  applyUpstream: (operation: Operation) => void;
};

function isLockedText(node: Node): boolean {
  return Text.isText(node) && node.locked === true;
}

function getLeafIfPresent(editor: Editor, path: Path): CustomText | null {
  try {
    const node = Node.leaf(editor, path);
    return Text.isText(node) ? node : null;
  } catch {
    return null;
  }
}

function mergeNodeAffectsLockedText(editor: Editor, path: Path): boolean {
  const node = Node.get(editor, path);
  if (isLockedText(node)) {
    return true;
  }
  const parentPath = Path.parent(path);
  const index = path.at(-1);
  if (index === undefined) {
    return false;
  }
  const siblingPath = parentPath.concat(index + 1);
  try {
    const sibling = Node.get(editor, siblingPath);
    return isLockedText(sibling);
  } catch {
    return false;
  }
}

function operationAffectsLockedText(editor: Editor, operation: Operation): boolean {
  switch (operation.type) {
    case "remove_text":
    case "insert_text": {
      const node = getLeafIfPresent(editor, operation.path);
      return node !== null && node.locked === true;
    }
    case "set_node": {
      const node = Node.get(editor, operation.path);
      if (!isLockedText(node)) {
        return false;
      }
      return "text" in operation.newProperties || "locked" in operation.newProperties;
    }
    case "split_node":
    case "remove_node": {
      const node = Node.get(editor, operation.path);
      return isLockedText(node);
    }
    case "merge_node":
      return mergeNodeAffectsLockedText(editor, operation.path);
    default:
      return false;
  }
}

/** Style object for locked leaves — consumed by renderLockedLeaf. */
function getLockedLeafStyle(leaf: CustomText): LockedLeafStyle | undefined {
  if (!leaf.locked) {
    return undefined;
  }
  return {
    backgroundColor: "var(--classic-rose)",
    textDecoration: "underline",
    textDecorationColor: "var(--plum)",
  };
}

/** Slate `renderLeaf` prop — highlights locked text with theme CSS variables. */
export function renderLockedLeaf({ attributes, children, leaf }: RenderLeafProps) {
  const style = getLockedLeafStyle(leaf);
  const childNodes = children as ReactNode;
  if (style) {
    return createElement("span", { ...attributes, style }, childNodes);
  }
  return createElement("span", attributes, childNodes);
}

export function withLockedRanges<T extends Editor>(editor: T): T & LockedRangesEditor {
  const { apply: baseApply } = editor;
  let isUpstreamApply = false;

  editor.apply = (operation: Operation) => {
    if (
      operation.type === "set_selection" &&
      editor.selection &&
      !Range.isCollapsed(editor.selection)
    ) {
      lockSelection(editor);
    }

    if (isUpstreamApply && operationAffectsLockedText(editor, operation)) {
      return;
    }

    baseApply(operation);

    if (!isUpstreamApply && operation.type === "insert_text") {
      lockAtPath(editor, operation.path);
    }
  };

  const lockedEditor = editor as T & LockedRangesEditor;
  lockedEditor.applyUpstream = (operation: Operation) => {
    isUpstreamApply = true;
    try {
      lockedEditor.apply(operation);
    } finally {
      isUpstreamApply = false;
    }
  };

  return lockedEditor;
}
