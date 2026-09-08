import { forwardRef, useImperativeHandle, useMemo } from "react";
import { createEditor, type Descendant, Editor } from "slate";
import { Editable, type ReactEditor, Slate, withReact } from "slate-react";

export interface HandwritingEditorRef {
  getPlainText(): string;
}

const editorByRef = new WeakMap<HandwritingEditorRef, ReactEditor>();

/** @internal Resolves the Slate editor for component tests only. */
export function getHandwritingEditorForTest(ref: HandwritingEditorRef | null): ReactEditor {
  if (ref === null) {
    throw new Error("HandwritingEditor ref is not attached");
  }
  const editor = editorByRef.get(ref);
  if (editor === undefined) {
    throw new Error("HandwritingEditor Slate instance is unavailable");
  }
  return editor;
}

function createInitialValue(): Descendant[] {
  return [
    {
      type: "paragraph",
      children: [{ text: "" }],
    } as unknown as Descendant,
  ];
}

/** Slate `Editor.string(editor, [])` joins block text without `\n`; preserve paragraph breaks for save. */
function serializeHandwritingPlainText(editor: Editor): string {
  return editor.children.map((_, index) => Editor.string(editor, [index])).join("\n");
}

export const HandwritingEditor = forwardRef<HandwritingEditorRef>(
  function HandwritingEditor(_props, ref) {
    const editor = useMemo(() => withReact(createEditor()), []);
    const initialValue = useMemo(() => createInitialValue(), []);

    useImperativeHandle(ref, () => {
      const handle: HandwritingEditorRef = {
        getPlainText: () => serializeHandwritingPlainText(editor),
      };
      editorByRef.set(handle, editor);
      return handle;
    }, [editor]);

    return (
      <div className="handwriting-editor-panel" style={{ backgroundColor: "var(--hawkes-blue)" }}>
        <Slate editor={editor} initialValue={initialValue}>
          <Editable data-testid="handwriting-editor" spellCheck={false} />
        </Slate>
      </div>
    );
  },
);
