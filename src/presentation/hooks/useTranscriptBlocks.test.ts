import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { createTranscriptSessionState } from "../../application/transcript/blockReducer";
import { setupTestDom } from "../../test-setup";
import type { TranscriptBlockAppended, TranscriptBlockEventListenFn } from "./transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "./transcript-blocks";
import { useTranscriptBlocks } from "./useTranscriptBlocks";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();
  const unlistenEvents: string[] = [];

  const listenFn: TranscriptBlockEventListenFn = async (event, handler) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler as EventHandler);
    listeners.set(event, handlers);
    return () => {
      unlistenEvents.push(event);
      const list = listeners.get(event) ?? [];
      const index = list.indexOf(handler as EventHandler);
      if (index >= 0) {
        list.splice(index, 1);
      }
    };
  };

  const emit = (event: string, payload: unknown) => {
    for (const handler of listeners.get(event) ?? []) {
      handler({ payload });
    }
  };

  return { listenFn, emit, unlistenEvents, listeners };
}

function makeBlockAppended(
  overrides: Partial<TranscriptBlockAppended["block"]> &
    Pick<TranscriptBlockAppended["block"], "block_id">,
): TranscriptBlockAppended {
  return {
    block: {
      sequence: 1,
      text: "hello",
      start_timestamp_ms: 100,
      language: "ja",
      ...overrides,
    },
    timestamp_ms: 200,
  };
}

describe("useTranscriptBlocks", () => {
  test("starts with empty session state (no mount replay)", () => {
    const { listenFn } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    expect(result.current).toEqual(createTranscriptSessionState());
  });

  test("subscribes to block-appended on mount", async () => {
    const { listenFn, listeners } = createMockListen();
    renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });
  });

  test("appends block via reducer when block-appended event arrives", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-1",
          sequence: 1,
          text: "first block",
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.blocks).toHaveLength(1);
    });
    expect(result.current.blocks[0]).toEqual({
      blockId: "block-1",
      sequence: 1,
      text: "first block",
      startTimestampMs: 100,
      language: "ja",
      displayText: "first block",
    });
    expect(result.current.sequenceGapCount).toBe(0);
  });

  test("appends multiple blocks in arrival order", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(BLOCK_APPENDED_EVENT, makeBlockAppended({ block_id: "a", sequence: 1, text: "one" }));
      emit(BLOCK_APPENDED_EVENT, makeBlockAppended({ block_id: "b", sequence: 2, text: "two" }));
    });

    await waitFor(() => {
      expect(result.current.blocks).toHaveLength(2);
    });
    expect(result.current.blocks.map((b) => b.displayText)).toEqual(["one", "two"]);
  });

  test("does not mutate existing block when duplicate block_id is re-emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "a", sequence: 1, text: "original" }),
      );
    });

    await waitFor(() => {
      expect(result.current.blocks).toHaveLength(1);
    });

    const firstBlockRef = result.current.blocks[0];

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "a", sequence: 1, text: "mutated" }),
      );
    });

    await waitFor(() => {
      expect(result.current.blocks).toHaveLength(1);
    });
    expect(result.current.blocks[0]).toBe(firstBlockRef);
    expect(result.current.blocks[0]?.text).toBe("original");
  });

  test("tracks sequence gaps via reducer", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(BLOCK_APPENDED_EVENT, makeBlockAppended({ block_id: "a", sequence: 1 }));
      emit(BLOCK_APPENDED_EVENT, makeBlockAppended({ block_id: "b", sequence: 3 }));
    });

    await waitFor(() => {
      expect(result.current.sequenceGapCount).toBe(1);
    });
  });

  test("cleans up event listener on unmount", async () => {
    const { listenFn, unlistenEvents } = createMockListen();
    const { unmount } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(unlistenEvents.length).toBe(0);
    });

    unmount();

    expect(unlistenEvents).toContain(BLOCK_APPENDED_EVENT);
  });

  test("unwraps nested payload envelopes from the event body", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(BLOCK_APPENDED_EVENT, {
        payload: makeBlockAppended({ block_id: "nested", sequence: 1, text: "wrapped" }),
      });
    });

    await waitFor(() => {
      expect(result.current.blocks).toHaveLength(1);
    });
    expect(result.current.blocks[0]?.displayText).toBe("wrapped");
  });

  test("keeps empty session when listenFn rejects", async () => {
    const listenFn: TranscriptBlockEventListenFn = async () => {
      throw new Error("listen denied");
    };
    const { result } = renderHook(() => useTranscriptBlocks({ listenFn }));

    await waitFor(() => {
      expect(result.current).toEqual(createTranscriptSessionState());
    });
  });
});
