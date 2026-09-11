import { act } from "@testing-library/react";
import type { TranscribeEventListenFn } from "../hooks/transcribe-status";
import type {
  TranscriptBlockAppended,
  TranscriptBlockEventListenFn,
} from "../hooks/transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "../hooks/transcript-blocks";

type EventHandler = (event: { payload: unknown }) => void;

export type MockTranscriptEditorListenFn = TranscriptBlockEventListenFn & TranscribeEventListenFn;

export interface MockListenHandle {
  listenFn: MockTranscriptEditorListenFn;
  emit: (event: string, payload: unknown) => void;
  listeners: Map<string, EventHandler[]>;
}

/** Creates a mock Tauri listen function for block-appended and transcribe events. */
export function createMockListen(): MockListenHandle {
  const listeners = new Map<string, EventHandler[]>();

  const listenFn: MockTranscriptEditorListenFn = (event, handler): Promise<() => void> => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler as EventHandler);
    listeners.set(event, handlers);
    return Promise.resolve(() => {
      const list = listeners.get(event) ?? [];
      listeners.set(
        event,
        list.filter((stored) => stored !== handler),
      );
    });
  };

  const emit = (event: string, payload: unknown) => {
    for (const handler of listeners.get(event) ?? []) {
      handler({ payload });
    }
  };

  return { listenFn, emit, listeners };
}

export function makeBlockAppended(
  overrides: Partial<TranscriptBlockAppended["block"]> &
    Pick<TranscriptBlockAppended["block"], "block_id">,
): TranscriptBlockAppended {
  return {
    block: {
      sequence: 1,
      text: "転写テキスト",
      start_timestamp_ms: 500,
      language: "ja",
      ...overrides,
    },
    timestamp_ms: 1_000,
  };
}

/** Reads visible text from the handwriting editor DOM element. */
export function getHandwritingEditorDomText(container: HTMLElement): string {
  const el = container.querySelector('[data-testid="handwriting-editor"]');
  return el?.textContent ?? "";
}

/**
 * Emits block-appended events and returns handwriting DOM text before and after.
 */
export function emitBlockAppendedAndGetHandwritingDomText(
  emit: MockListenHandle["emit"],
  container: HTMLElement,
  blocks: Array<ReturnType<typeof makeBlockAppended>>,
): { before: string; after: string } {
  const before = getHandwritingEditorDomText(container);

  for (const payload of blocks) {
    act(() => {
      emit(BLOCK_APPENDED_EVENT, payload);
    });
  }

  const after = getHandwritingEditorDomText(container);
  return { before, after };
}
