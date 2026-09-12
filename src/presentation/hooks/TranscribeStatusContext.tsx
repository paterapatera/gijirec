import { createContext, type ReactNode, useContext } from "react";
import type { TranscribeEventListenFn, TranscribeStatusState } from "./transcribe-status";
import { type UseTranscribeStatusOptions, useTranscribeStatus } from "./useTranscribeStatus";

const TranscribeStatusContext = createContext<TranscribeStatusState | null>(null);

export interface TranscribeStatusProviderProps {
  children: ReactNode;
  listenFn?: TranscribeEventListenFn;
  invokeFn?: UseTranscribeStatusOptions["invokeFn"];
}

/** Single transcribe status subscription for the App tree. */
export function TranscribeStatusProvider({
  children,
  listenFn,
  invokeFn,
}: Readonly<TranscribeStatusProviderProps>): ReactNode {
  const status = useTranscribeStatus({
    ...(listenFn !== undefined ? { listenFn } : {}),
    ...(invokeFn !== undefined ? { invokeFn } : {}),
  });

  return (
    <TranscribeStatusContext.Provider value={status}>{children}</TranscribeStatusContext.Provider>
  );
}

export function useTranscribeStatusContext(): TranscribeStatusState {
  const context = useContext(TranscribeStatusContext);
  if (context === null) {
    throw new Error("useTranscribeStatusContext must be used within TranscribeStatusProvider");
  }
  return context;
}

export function useOptionalTranscribeStatusContext(): TranscribeStatusState | null {
  return useContext(TranscribeStatusContext);
}
