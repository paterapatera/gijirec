import type { ReactNode } from "react";
import { CaptureStatusProvider } from "./CaptureStatusContext";
import type { CaptureEventListenFn } from "./capture-status";
import { TranscribeStatusProvider } from "./TranscribeStatusContext";
import type { TranscribeEventListenFn } from "./transcribe-status";
import type { UseEditorSettingsOptions } from "./useEditorSettings";

export interface AppStatusProvidersProps {
  children: ReactNode;
  listenFn?: CaptureEventListenFn & TranscribeEventListenFn;
  invokeFn?: UseEditorSettingsOptions["invokeFn"];
}

export function AppStatusProviders({
  children,
  listenFn,
  invokeFn,
}: Readonly<AppStatusProvidersProps>): ReactNode {
  const captureProps = listenFn === undefined ? {} : { listenFn };
  const transcribeProps = {
    ...captureProps,
    ...(invokeFn !== undefined ? { invokeFn } : {}),
  };

  return (
    <CaptureStatusProvider {...captureProps} {...(invokeFn !== undefined ? { invokeFn } : {})}>
      <TranscribeStatusProvider {...transcribeProps}>{children}</TranscribeStatusProvider>
    </CaptureStatusProvider>
  );
}
