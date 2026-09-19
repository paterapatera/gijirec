import type { CaptureSessionState } from "../../presentation/hooks/capture-session-types";
import { defaultInvoke, type InjectableInvokeFn } from "./injectableInvoke";

export interface CaptureSessionCommandsOptions {
  invokeFn?: InjectableInvokeFn;
}

export async function getCaptureSessionState(
  options: CaptureSessionCommandsOptions = {},
): Promise<CaptureSessionState> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<CaptureSessionState>("get_capture_session_state");
}

export async function startCaptureSession(
  options: CaptureSessionCommandsOptions = {},
): Promise<CaptureSessionState> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<CaptureSessionState>("start_capture_session");
}
