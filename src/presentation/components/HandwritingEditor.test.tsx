import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, cleanup, render } from "@testing-library/react";
import { createRef } from "react";
import { Editor, Transforms } from "slate";
import { ReactEditor } from "slate-react";
import { setupTestDom } from "../../test-setup";
import {
  getHandwritingEditorForTest,
  HandwritingEditor,
  type HandwritingEditorRef,
} from "./HandwritingEditor";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

function typeIntoEditor(ref: HandwritingEditorRef | null, text: string): void {
  act(() => {
    const editor = getHandwritingEditorForTest(ref);
    ReactEditor.focus(editor);
    Transforms.select(editor, Editor.start(editor, [0]));
    Transforms.insertText(editor, text);
  });
}

function typeMultilineIntoEditor(ref: HandwritingEditorRef | null, lines: string[]): void {
  act(() => {
    const editor = getHandwritingEditorForTest(ref);
    ReactEditor.focus(editor);
    Transforms.select(editor, Editor.start(editor, [0]));
    for (const [index, line] of lines.entries()) {
      if (index > 0) {
        editor.insertBreak();
      }
      Transforms.insertText(editor, line);
    }
  });
}

describe("HandwritingEditor", () => {
  test("renders with data-testid and hawkes-blue background", () => {
    const { getByTestId } = render(<HandwritingEditor />);
    const editor = getByTestId("handwriting-editor");
    const panel = editor.closest(".handwriting-editor-panel");

    expect(editor).toBeTruthy();
    expect(panel).toBeTruthy();
    expect((panel as HTMLElement).style.backgroundColor).toBe("var(--hawkes-blue)");
  });

  test("getPlainText returns empty string initially", () => {
    const ref = createRef<HandwritingEditorRef>();
    render(<HandwritingEditor ref={ref} />);

    expect(ref.current?.getPlainText()).toBe("");
  });

  test("reflects typed input immediately via getPlainText", () => {
    const ref = createRef<HandwritingEditorRef>();
    render(<HandwritingEditor ref={ref} />);

    typeIntoEditor(ref.current, "会議の要点");

    expect(ref.current?.getPlainText()).toBe("会議の要点");
  });

  test("getPlainText preserves line breaks between paragraphs", () => {
    const ref = createRef<HandwritingEditorRef>();
    render(<HandwritingEditor ref={ref} />);

    typeMultilineIntoEditor(ref.current, ["議題", "決定事項"]);

    expect(ref.current?.getPlainText()).toBe("議題\n決定事項");
  });

  test("does not import editorCommands (no automatic disk write)", () => {
    const source = readFileSync(join(import.meta.dir, "HandwritingEditor.tsx"), "utf8");
    expect(source).not.toContain("editorCommands");
  });

  test("uses independent Slate instances per editor", () => {
    const refA = createRef<HandwritingEditorRef>();
    const refB = createRef<HandwritingEditorRef>();
    render(
      <>
        <HandwritingEditor ref={refA} />
        <HandwritingEditor ref={refB} />
      </>,
    );

    typeIntoEditor(refA.current, "左パネル");

    expect(refA.current?.getPlainText()).toBe("左パネル");
    expect(refB.current?.getPlainText()).toBe("");
  });
});
