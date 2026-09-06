import { createEditor } from "slate";
import { type ReactEditor, withReact } from "slate-react";
import { withAppendOnlyBlocks } from "./plugins/withAppendOnlyBlocks";
import { type LockedRangesEditor, withLockedRanges } from "./plugins/withLockedRanges";
import { type StableSelectionEditor, withStableSelection } from "./plugins/withStableSelection";

export type AiTranscriptEditorInstance = ReactEditor & LockedRangesEditor & StableSelectionEditor;

export function createAiTranscriptEditor(): AiTranscriptEditorInstance {
  return withStableSelection(withLockedRanges(withAppendOnlyBlocks(withReact(createEditor()))));
}
