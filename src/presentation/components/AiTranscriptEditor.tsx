import { forwardRef, useCallback, useImperativeHandle, useLayoutEffect, useMemo } from "react";
import { type Descendant, Editor, Node } from "slate";
import { Editable, type RenderElementProps, Slate } from "slate-react";
import {
  type AiTranscriptEditorInstance,
  createAiTranscriptEditor,
} from "../../application/transcript/createAiTranscriptEditor";
import { renderLockedLeaf } from "../../application/transcript/plugins/withLockedRanges";
import { AI_TRANSCRIPT_SCROLL_STYLE } from "../../application/transcript/plugins/withStableSelection";
import {
  isTranscriptBlockElement,
  type TranscriptBlockElement,
} from "../../domain/transcript/slateTypes";
import type { TranscriptBlockView } from "../../domain/transcript/types";

export interface AiTranscriptEditorRef {
  getBlocks(): TranscriptBlockView[];
  appendBlock(block: TranscriptBlockView): void;
}

export interface AiTranscriptEditorProps {
  readonly blocks?: readonly TranscriptBlockView[];
}

const EMPTY_BLOCKS: readonly TranscriptBlockView[] = [];

const editorByRef = new WeakMap<AiTranscriptEditorRef, AiTranscriptEditorInstance>();

/** @internal Resolves the Slate editor for component tests only. */
export function getAiTranscriptEditorForTest(
  ref: AiTranscriptEditorRef | null,
): AiTranscriptEditorInstance {
  if (ref === null) {
    throw new Error("AiTranscriptEditor ref is not attached");
  }
  const editor = editorByRef.get(ref);
  if (editor === undefined) {
    throw new Error("AiTranscriptEditor Slate instance is unavailable");
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

function blockViewToElement(block: TranscriptBlockView): TranscriptBlockElement {
  return {
    type: "transcript-block",
    blockId: block.blockId,
    sequence: block.sequence,
    upstreamText: block.text,
    startTimestampMs: block.startTimestampMs,
    language: block.language,
    children: [{ text: block.displayText }],
  };
}

function blocksFromEditor(editor: Editor): TranscriptBlockView[] {
  return editor.children.filter(isTranscriptBlockElement).map((block) => ({
    blockId: block.blockId,
    sequence: block.sequence,
    text: block.upstreamText,
    displayText: block.children.map((child) => child.text).join(""),
    startTimestampMs: block.startTimestampMs,
    language: block.language,
  }));
}

function hasEmptyPlaceholder(editor: Editor): boolean {
  if (editor.children.length !== 1) {
    return false;
  }
  const node = editor.children[0];
  return node !== undefined && !isTranscriptBlockElement(node) && Node.string(node) === "";
}

function renderTranscriptBlock({ attributes, children, element }: RenderElementProps) {
  if (!isTranscriptBlockElement(element)) {
    return <div {...attributes}>{children}</div>;
  }

  return (
    <div
      {...attributes}
      data-transcript-block
      data-block-id={element.blockId}
      data-start-timestamp-ms={element.startTimestampMs}
    >
      {children}
    </div>
  );
}

export const AiTranscriptEditor = forwardRef<AiTranscriptEditorRef, AiTranscriptEditorProps>(
  function AiTranscriptEditor({ blocks = EMPTY_BLOCKS }, ref) {
    const editor = useMemo(() => createAiTranscriptEditor(), []);
    const initialValue = useMemo(() => createInitialValue(), []);

    const appendBlock = useCallback(
      (block: TranscriptBlockView) => {
        const alreadyPresent = editor.children.some(
          (child) => isTranscriptBlockElement(child) && child.blockId === block.blockId,
        );
        if (alreadyPresent) {
          return;
        }

        const node = blockViewToElement(block);
        Editor.withoutNormalizing(editor, () => {
          if (hasEmptyPlaceholder(editor)) {
            const placeholder = editor.children[0];
            if (placeholder !== undefined) {
              editor.apply({
                type: "remove_node",
                path: [0],
                node: placeholder,
              });
            }
          }
          editor.applyUpstream({
            type: "insert_node",
            path: [editor.children.length],
            node,
          });
        });
        editor.onChange();
      },
      [editor],
    );

    useLayoutEffect(() => {
      for (const block of blocks) {
        appendBlock(block);
      }
    }, [appendBlock, blocks]);

    useImperativeHandle(ref, () => {
      const handle: AiTranscriptEditorRef = {
        getBlocks: () => blocksFromEditor(editor),
        appendBlock,
      };
      editorByRef.set(handle, editor);
      return handle;
    }, [appendBlock, editor]);

    return (
      <div className="ai-transcript-editor-panel" style={{ backgroundColor: "var(--jagged-ice)" }}>
        <div
          className="ai-transcript-editor-scroll"
          data-testid="ai-transcript-editor"
          style={AI_TRANSCRIPT_SCROLL_STYLE}
        >
          <Slate editor={editor} initialValue={initialValue}>
            <Editable
              renderElement={renderTranscriptBlock}
              renderLeaf={renderLockedLeaf}
              spellCheck={false}
            />
          </Slate>
        </div>
      </div>
    );
  },
);
