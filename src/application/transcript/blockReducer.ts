import type { TranscriptBlockView } from "../../domain/transcript/types";

export interface TranscriptSessionState {
  blocks: TranscriptBlockView[];
  sequenceGapCount: number;
}

export function createTranscriptSessionState(): TranscriptSessionState {
  return {
    blocks: [],
    sequenceGapCount: 0,
  };
}

export function appendBlock(
  state: TranscriptSessionState,
  block: TranscriptBlockView,
): TranscriptSessionState {
  const alreadyPresent = state.blocks.some((existing) => existing.blockId === block.blockId);
  if (alreadyPresent) {
    return state;
  }

  let sequenceGapCount = state.sequenceGapCount;
  const lastBlock = state.blocks.at(-1);
  if (lastBlock !== undefined) {
    const expectedSequence = lastBlock.sequence + 1;
    if (block.sequence > expectedSequence) {
      sequenceGapCount += block.sequence - expectedSequence;
    }
  }

  return {
    blocks: [...state.blocks, block],
    sequenceGapCount,
  };
}
