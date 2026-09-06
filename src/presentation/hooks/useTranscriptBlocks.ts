import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import {
  appendBlock,
  createTranscriptSessionState,
  type TranscriptSessionState,
} from "../../application/transcript/blockReducer";
import type {
  TranscriptBlockAppended,
  TranscriptBlockContract,
  TranscriptBlockEventListenFn,
} from "./transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "./transcript-blocks";

export interface UseTranscriptBlocksOptions {
  listenFn?: TranscriptBlockEventListenFn;
}

function mapContractBlockToView(block: TranscriptBlockContract) {
  return {
    blockId: block.block_id,
    sequence: block.sequence,
    text: block.text,
    startTimestampMs: block.start_timestamp_ms,
    language: block.language,
    displayText: block.text,
  };
}

function applyBlockAppended(
  payload: TranscriptBlockAppended,
  setState: Dispatch<SetStateAction<TranscriptSessionState>>,
): void {
  setState((prev) => appendBlock(prev, mapContractBlockToView(payload.block)));
}

function asBlockAppended(payload: unknown): TranscriptBlockAppended | null {
  if (payload === null || typeof payload !== "object") {
    return null;
  }
  const record = payload as Record<string, unknown>;
  if (record.block !== null && typeof record.block === "object") {
    return payload as TranscriptBlockAppended;
  }
  const nested = record.payload;
  if (nested !== null && typeof nested === "object" && "block" in nested) {
    return nested as TranscriptBlockAppended;
  }
  return null;
}

async function subscribeBlockAppended(
  listenFn: TranscriptBlockEventListenFn,
  setState: Dispatch<SetStateAction<TranscriptSessionState>>,
  isCancelled: () => boolean,
): Promise<(() => void) | undefined> {
  try {
    const unlisten = await listenFn(BLOCK_APPENDED_EVENT, (event) => {
      const payload = asBlockAppended(event.payload);
      if (payload === null) {
        return;
      }
      applyBlockAppended(payload, setState);
    });
    if (isCancelled()) {
      unlisten();
      return undefined;
    }

    return unlisten;
  } catch {
    return undefined;
  }
}

/**
 * Subscribes to whisper-transcribe block-appended events and mirrors append-only
 * transcript blocks into session memory. v1 does not replay buffered blocks on mount.
 */
export function useTranscriptBlocks(
  options: UseTranscriptBlocksOptions = {},
): TranscriptSessionState {
  const { listenFn = listen } = options;
  const [state, setState] = useState<TranscriptSessionState>(() => createTranscriptSessionState());

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void subscribeBlockAppended(listenFn, setState, () => cancelled).then((handle) => {
      if (handle === undefined) {
        return;
      }
      unlisten = handle;
      if (cancelled) {
        unlisten();
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [listenFn]);

  return state;
}
