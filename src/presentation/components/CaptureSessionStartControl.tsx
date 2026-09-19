import type { InjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import type {
  CaptureSessionEventListenFn,
  CaptureSessionPhase,
  CaptureSessionState,
} from "../hooks/capture-session-types";
import {
  type UseCaptureSessionOptions,
  type UseCaptureSessionResult,
  useCaptureSession,
} from "../hooks/useCaptureSession";

interface CaptureSessionStartControlInjectedProps {
  readonly session_phase: CaptureSessionPhase;
  readonly disabled: boolean;
  readonly busy: boolean;
  readonly startCaptureSession: () => Promise<CaptureSessionState>;
}

interface CaptureSessionStartControlRuntimeProps {
  readonly invokeFn?: InjectableInvokeFn;
  readonly listenFn?: CaptureSessionEventListenFn;
  readonly enabled?: boolean;
}

export type CaptureSessionStartControlProps = Partial<CaptureSessionStartControlInjectedProps> &
  CaptureSessionStartControlRuntimeProps;

function resolveStartLabel(sessionPhase: CaptureSessionPhase): string {
  if (sessionPhase === "starting") {
    return "キャプチャセッションを開始中";
  }
  return "キャプチャセッションを開始";
}

type CaptureSessionStartControlViewProps = CaptureSessionStartControlInjectedProps;

function CaptureSessionStartControlView({
  session_phase,
  disabled,
  busy,
  startCaptureSession,
}: CaptureSessionStartControlViewProps) {
  if (session_phase === "active") {
    return null;
  }

  const handleClick = (): void => {
    if (disabled || session_phase !== "idle") {
      return;
    }
    void startCaptureSession();
  };

  return (
    <section
      className="capture-session-toggle-row"
      aria-label="キャプチャセッション"
      data-testid="capture-session-toggle-section"
    >
      <button
        type="button"
        className="capture-session-toggle"
        data-testid="capture-session-toggle"
        aria-label={resolveStartLabel(session_phase)}
        disabled={disabled || session_phase !== "idle"}
        aria-busy={busy ? "true" : undefined}
        onClick={handleClick}
      >
        <span
          className="capture-session-toggle-icon capture-session-toggle-icon--start"
          data-testid="capture-session-toggle-icon-start"
          aria-hidden="true"
        />
      </button>
    </section>
  );
}

function isInjectedProps(
  props: CaptureSessionStartControlProps,
): props is CaptureSessionStartControlInjectedProps & CaptureSessionStartControlRuntimeProps {
  return (
    props.session_phase !== undefined &&
    props.disabled !== undefined &&
    props.busy !== undefined &&
    props.startCaptureSession !== undefined
  );
}

function CaptureSessionStartControlConnected(props: CaptureSessionStartControlRuntimeProps) {
  const hookOptions: UseCaptureSessionOptions = {
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    ...(props.listenFn !== undefined ? { listenFn: props.listenFn } : {}),
    ...(props.enabled !== undefined ? { enabled: props.enabled } : {}),
  };
  const session: UseCaptureSessionResult = useCaptureSession(hookOptions);

  const viewProps: CaptureSessionStartControlViewProps = {
    session_phase: session.session_phase,
    disabled: session.disabled,
    busy: session.busy,
    startCaptureSession: session.startCaptureSession,
  };

  return <CaptureSessionStartControlView {...viewProps} />;
}

export function CaptureSessionStartControl(props: CaptureSessionStartControlProps = {}) {
  if (isInjectedProps(props)) {
    const viewProps: CaptureSessionStartControlViewProps = {
      session_phase: props.session_phase,
      disabled: props.disabled,
      busy: props.busy,
      startCaptureSession: props.startCaptureSession,
    };
    return <CaptureSessionStartControlView {...viewProps} />;
  }
  return <CaptureSessionStartControlConnected {...props} />;
}
