import { type Editor, Node, type Operation, type Path } from "slate";
import { isTranscriptBlockElement } from "../../../domain/transcript/slateTypes";

function isAppendPath(editor: Editor, path: Path): boolean {
  return path.length === 1 && path[0] === editor.children.length;
}

export function withAppendOnlyBlocks<T extends Editor>(editor: T): T {
  const { apply } = editor;

  editor.apply = (operation: Operation) => {
    if (operation.type === "remove_node") {
      const node = Node.get(editor, operation.path);
      if (isTranscriptBlockElement(node)) {
        return;
      }
    }

    if (operation.type === "set_node") {
      const node = Node.get(editor, operation.path);
      if (isTranscriptBlockElement(node)) {
        return;
      }
    }

    if (operation.type === "insert_node" && isTranscriptBlockElement(operation.node)) {
      if (!isAppendPath(editor, operation.path)) {
        return;
      }
    }

    apply(operation);
  };

  return editor;
}
