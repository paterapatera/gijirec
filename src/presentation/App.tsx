import type { CaptureEventListenFn } from "./hooks/capture-status";
import { useCaptureStatus } from "./hooks/useCaptureStatus";
import "./App.css";

export interface AppProps {
  listenFn?: CaptureEventListenFn;
}

export function App({ listenFn }: AppProps = {}) {
  const status = useCaptureStatus(listenFn === undefined ? {} : { listenFn });

  return (
    <main className="app">
      <h1 className="app-title">gijirec Audio Capture</h1>
      <section className="status-panel" aria-live="polite">
        <p className="status-label">状態</p>
        <p className="status-phase" data-testid="capture-phase">
          {status.phase}
        </p>
      </section>
      {status.error !== null ? (
        <section className="error-panel" role="alert">
          <p className="error-message" data-testid="error-message">
            {status.error.message_ja}
          </p>
          <p className="error-action" data-testid="error-action">
            {status.error.action_ja}
          </p>
        </section>
      ) : null}
    </main>
  );
}
