import type { CaptureEventListenFn } from "./hooks/capture-status";
import type { TranscribeEventListenFn } from "./hooks/transcribe-status";
import { useCaptureStatus } from "./hooks/useCaptureStatus";
import { useTranscribeStatus } from "./hooks/useTranscribeStatus";
import "./App.css";

export interface AppProps {
  listenFn?: CaptureEventListenFn & TranscribeEventListenFn;
}

export function App({ listenFn }: AppProps = {}) {
  const captureStatus = useCaptureStatus(listenFn === undefined ? {} : { listenFn });
  const transcribeStatus = useTranscribeStatus(listenFn === undefined ? {} : { listenFn });

  return (
    <main className="app">
      <h1 className="app-title">gijirec Audio Capture & Transcribe</h1>
      <section className="status-panel" aria-live="polite">
        <p className="status-label">キャプチャ状態</p>
        <p className="status-phase" data-testid="capture-phase">
          {captureStatus.phase}
        </p>
      </section>
      <section className="status-panel" aria-live="polite">
        <p className="status-label">文字起こし状態</p>
        <p className="status-phase" data-testid="transcribe-phase">
          {transcribeStatus.phase}
        </p>
      </section>
      {transcribeStatus.phase === "loading_model" && transcribeStatus.modelProgress !== null ? (
        <section className="progress-panel" aria-live="polite">
          <p className="progress-label" data-testid="model-progress-status">
            モデル取得中 ({transcribeStatus.modelProgress.status}):{" "}
            {transcribeStatus.modelProgress.percent !== null
              ? `${String(transcribeStatus.modelProgress.percent)}%`
              : `${String(transcribeStatus.modelProgress.bytes_downloaded)} bytes`}
          </p>
          <progress
            data-testid="model-progress-bar"
            value={transcribeStatus.modelProgress.percent ?? undefined}
            max={100}
          />
        </section>
      ) : null}
      {captureStatus.error !== null ? (
        <section className="error-panel" role="alert">
          <p className="error-message" data-testid="error-message">
            {captureStatus.error.message_ja}
          </p>
          <p className="error-action" data-testid="error-action">
            {captureStatus.error.action_ja}
          </p>
        </section>
      ) : null}
      {transcribeStatus.error !== null ? (
        <section className="error-panel" role="alert">
          <p className="error-message" data-testid="transcribe-error-message">
            {transcribeStatus.error.message_ja}
          </p>
          <p className="error-action" data-testid="transcribe-error-action">
            {transcribeStatus.error.action_ja}
          </p>
        </section>
      ) : null}
    </main>
  );
}
