import { type Ref, useCallback, useMemo, useRef } from "react";
import type { SaveTranscriptSessionResult } from "../infrastructure/tauri/editorCommands";
import { defaultInvoke } from "../infrastructure/tauri/injectableInvoke";
import type { AiTranscriptEditorRef } from "./components/AiTranscriptEditor";
import { AppStatusPanels } from "./components/AppStatusPanels";
import { DeviceSelectorPanel } from "./components/DeviceSelectorPanel";
import type { HandwritingEditorRef } from "./components/HandwritingEditor";
import { ModelVariantSelector } from "./components/ModelVariantSelector";
import { TranscriptEditorView } from "./components/TranscriptEditorView";
import { Toaster } from "./components/ui/sonner";
import { AppStatusProviders } from "./hooks/AppStatusProviders";
import { useCaptureStatusContext } from "./hooks/CaptureStatusContext";
import type { CaptureEventListenFn } from "./hooks/capture-status";
import { useTranscribeStatusContext } from "./hooks/TranscribeStatusContext";
import type { TranscribeEventListenFn } from "./hooks/transcribe-status";
import type { TranscriptBlockEventListenFn } from "./hooks/transcript-blocks";
import type { UseEditorSettingsOptions } from "./hooks/useEditorSettings";
import { useEditorSettings } from "./hooks/useEditorSettings";
import { useSaveTranscript } from "./hooks/useSaveTranscript";
import "./App.css";

const SESSION_ID = "gijirec-session";

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (ref === undefined || ref === null) {
    return;
  }
  if (typeof ref === "function") {
    ref(value);
  } else {
    ref.current = value;
  }
}

export interface AppProps {
  listenFn?: CaptureEventListenFn & TranscribeEventListenFn & TranscriptBlockEventListenFn;
  invokeFn?: UseEditorSettingsOptions["invokeFn"];
  handwritingEditorRef?: Ref<HandwritingEditorRef>;
  aiTranscriptEditorRef?: Ref<AiTranscriptEditorRef>;
  showSaveResultFn?: (result: SaveTranscriptSessionResult) => void;
}

function AppContent({
  listenFn,
  invokeFn = defaultInvoke,
  handwritingEditorRef: externalHandwritingRef,
  aiTranscriptEditorRef: externalAiRef,
  showSaveResultFn,
}: Readonly<AppProps>) {
  const captureStatus = useCaptureStatusContext();
  const transcribeStatus = useTranscribeStatusContext();

  const settingsHook = useEditorSettings({ invokeFn });

  const handwritingEditorRef = useRef<HandwritingEditorRef>(null);
  const aiTranscriptEditorRef = useRef<AiTranscriptEditorRef>(null);

  const mergedHandwritingRef = useCallback(
    (value: HandwritingEditorRef | null) => {
      handwritingEditorRef.current = value;
      assignRef(externalHandwritingRef, value);
    },
    [externalHandwritingRef],
  );
  const mergedAiRef = useCallback(
    (value: AiTranscriptEditorRef | null) => {
      aiTranscriptEditorRef.current = value;
      assignRef(externalAiRef, value);
    },
    [externalAiRef],
  );

  const handwritingEditorPort = useMemo(
    () => ({
      getPlainText: () => handwritingEditorRef.current?.getPlainText() ?? "",
    }),
    [],
  );

  const aiEditorPort = useMemo(
    () => ({
      getBlocks: () => aiTranscriptEditorRef.current?.getBlocks() ?? [],
    }),
    [],
  );

  const { onSave, isSaving } = useSaveTranscript({
    handwritingEditor: handwritingEditorPort,
    aiEditor: aiEditorPort,
    settings: settingsHook.settings,
    sessionId: SESSION_ID,
    invokeFn,
    ...(showSaveResultFn !== undefined ? { showSaveResultFn } : {}),
  });

  return (
    <main className="app">
      <h1 className="app-title">gijirec Audio Capture & Transcribe</h1>
      <AppStatusPanels
        capturePhase={captureStatus.phase}
        captureError={captureStatus.error}
        transcribePhase={transcribeStatus.phase}
        transcribeError={transcribeStatus.error}
        modelProgress={transcribeStatus.modelProgress}
      />
      <DeviceSelectorPanel
        invokeFn={invokeFn}
        capturePhase={captureStatus.phase}
        captureError={captureStatus.error}
        {...(listenFn !== undefined ? { listenFn } : {})}
      />
      <ModelVariantSelector invokeFn={invokeFn} transcribePhase={transcribeStatus.phase} />
      <TranscriptEditorView
        onSave={onSave}
        isSaving={isSaving}
        settings={settingsHook.settings}
        isLoading={settingsHook.isLoading}
        pickSaveDirectory={settingsHook.pickSaveDirectory}
        setExportJsonlEnabled={settingsHook.setExportJsonlEnabled}
        {...(listenFn !== undefined ? { listenFn } : {})}
        handwritingEditorRef={mergedHandwritingRef}
        aiTranscriptEditorRef={mergedAiRef}
      />
      <Toaster />
    </main>
  );
}

export function App(props: Readonly<AppProps> = {}) {
  return (
    <AppStatusProviders
      {...(props.listenFn !== undefined ? { listenFn: props.listenFn } : {})}
      {...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {})}
    >
      <AppContent {...props} />
    </AppStatusProviders>
  );
}
