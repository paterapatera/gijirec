import { createContext, type ReactNode, useContext } from "react";
import type { CaptureEventListenFn, CaptureStatusState } from "./capture-status";
import { type UseCaptureStatusOptions, useCaptureStatus } from "./useCaptureStatus";

const CaptureStatusContext = createContext<CaptureStatusState | null>(null);

export interface CaptureStatusProviderProps {
  children: ReactNode;
  listenFn?: CaptureEventListenFn;
  invokeFn?: UseCaptureStatusOptions["invokeFn"];
}

/** Single capture status subscription for the App tree. */
export function CaptureStatusProvider({
  children,
  listenFn,
  invokeFn,
}: Readonly<CaptureStatusProviderProps>): ReactNode {
  const status = useCaptureStatus({
    ...(listenFn !== undefined ? { listenFn } : {}),
    ...(invokeFn !== undefined ? { invokeFn } : {}),
  });

  return <CaptureStatusContext.Provider value={status}>{children}</CaptureStatusContext.Provider>;
}

export function useCaptureStatusContext(): CaptureStatusState {
  const context = useContext(CaptureStatusContext);
  if (context === null) {
    throw new Error("useCaptureStatusContext must be used within CaptureStatusProvider");
  }
  return context;
}

export function useOptionalCaptureStatusContext(): CaptureStatusState | null {
  return useContext(CaptureStatusContext);
}
