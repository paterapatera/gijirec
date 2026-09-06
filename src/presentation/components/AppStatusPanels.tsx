import type { CaptureUserError } from "../hooks/capture-status";
import type { ModelDownloadProgress, TranscribeUserError } from "../hooks/transcribe-status";

interface PhaseStatusPanelProps {
  readonly label: string;
  readonly phase: string;
  readonly testId: string;
}

function PhaseStatusPanel({ label, phase, testId }: PhaseStatusPanelProps) {
  return (
    <section className="status-panel" aria-live="polite">
      <p className="status-label">{label}</p>
      <p className="status-phase" data-testid={testId}>
        {phase}
      </p>
    </section>
  );
}

interface ErrorPanelProps {
  readonly error: CaptureUserError | TranscribeUserError;
  readonly messageTestId: string;
  readonly actionTestId: string;
}

function ErrorPanel({ error, messageTestId, actionTestId }: ErrorPanelProps) {
  return (
    <section className="error-panel" role="alert">
      <p className="error-message" data-testid={messageTestId}>
        {error.message_ja}
      </p>
      <p className="error-action" data-testid={actionTestId}>
        {error.action_ja}
      </p>
    </section>
  );
}

interface ModelProgressPanelProps {
  readonly progress: ModelDownloadProgress;
}

function ModelProgressPanel({ progress }: ModelProgressPanelProps) {
  const progressLabel =
    progress.percent !== null
      ? `${String(progress.percent)}%`
      : `${String(progress.bytes_downloaded)} bytes`;

  return (
    <section className="progress-panel" aria-live="polite">
      <p className="progress-label" data-testid="model-progress-status">
        モデル取得中 ({progress.status}): {progressLabel}
      </p>
      <progress data-testid="model-progress-bar" value={progress.percent ?? undefined} max={100} />
    </section>
  );
}

export interface AppStatusPanelsProps {
  readonly capturePhase: string;
  readonly captureError: CaptureUserError | null;
  readonly transcribePhase: string;
  readonly transcribeError: TranscribeUserError | null;
  readonly modelProgress: ModelDownloadProgress | null;
}

export function AppStatusPanels({
  capturePhase,
  captureError,
  transcribePhase,
  transcribeError,
  modelProgress,
}: AppStatusPanelsProps) {
  return (
    <>
      <PhaseStatusPanel label="キャプチャ状態" phase={capturePhase} testId="capture-phase" />
      <PhaseStatusPanel label="文字起こし状態" phase={transcribePhase} testId="transcribe-phase" />
      {transcribePhase === "loading_model" && modelProgress !== null ? (
        <ModelProgressPanel progress={modelProgress} />
      ) : null}
      {captureError !== null ? (
        <ErrorPanel
          error={captureError}
          messageTestId="error-message"
          actionTestId="error-action"
        />
      ) : null}
      {transcribeError !== null ? (
        <ErrorPanel
          error={transcribeError}
          messageTestId="transcribe-error-message"
          actionTestId="transcribe-error-action"
        />
      ) : null}
    </>
  );
}
