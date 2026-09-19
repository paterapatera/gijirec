import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { cleanup, render } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type { CaptureUserError } from "../hooks/capture-status";
import type { ModelDownloadProgress, TranscribeUserError } from "../hooks/transcribe-status";
import { AppStatusPanels } from "./AppStatusPanels";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

function renderPanels(
  overrides: Partial<{
    capturePhase: string;
    captureError: CaptureUserError | null;
    transcribePhase: string;
    transcribeError: TranscribeUserError | null;
    modelProgress: ModelDownloadProgress | null;
    pcmBacklogSeconds: number;
  }> = {},
) {
  return render(
    <AppStatusPanels
      capturePhase={overrides.capturePhase ?? "idle"}
      captureError={overrides.captureError ?? null}
      transcribePhase={overrides.transcribePhase ?? "idle"}
      transcribeError={overrides.transcribeError ?? null}
      modelProgress={overrides.modelProgress ?? null}
      pcmBacklogSeconds={overrides.pcmBacklogSeconds ?? 0}
    />,
  );
}

describe("AppStatusPanels (task 13.2 / req 5.1–5.2)", () => {
  test("wraps capture and transcribe phase panels in phase-panels-row", () => {
    const { container, getByTestId } = renderPanels();

    const row = container.querySelector(".phase-panels-row");
    expect(row).not.toBeNull();
    expect(row?.contains(getByTestId("capture-phase"))).toBe(true);
    expect(row?.contains(getByTestId("transcribe-phase"))).toBe(true);
  });

  test("keeps data-testid and aria-live on phase panels", () => {
    const { container, getByTestId } = renderPanels({
      capturePhase: "capturing",
      transcribePhase: "transcribing",
    });

    const capturePanel = getByTestId("capture-phase").closest("section");
    const transcribePanel = getByTestId("transcribe-phase").closest("section");

    expect(capturePanel?.getAttribute("aria-live")).toBe("polite");
    expect(transcribePanel?.getAttribute("aria-live")).toBe("polite");
    expect(container.textContent).toContain("キャプチャ状態");
    expect(container.textContent).toContain("文字起こし状態");
    expect(getByTestId("capture-phase").textContent).toBe("capturing");
    expect(getByTestId("transcribe-phase").textContent).toBe("transcribing");
  });

  test("shows inference backlog hint when transcribing and backlog is at least 30s", () => {
    const { getByTestId } = renderPanels({
      transcribePhase: "transcribing",
      pcmBacklogSeconds: 90,
    });

    expect(getByTestId("transcribe-pcm-backlog").textContent).toBe("推論待ち 約 2 分");
  });

  test("hides inference backlog hint below display threshold", () => {
    const { queryByTestId } = renderPanels({
      transcribePhase: "transcribing",
      pcmBacklogSeconds: 29,
    });

    expect(queryByTestId("transcribe-pcm-backlog")).toBeNull();
  });

  test("does not render stop-flush or session flush progress UI (req 5.1)", () => {
    const { container } = renderPanels({
      capturePhase: "capturing",
      transcribePhase: "transcribing",
      pcmBacklogSeconds: 120,
    });

    const text = container.textContent ?? "";
    expect(text).not.toContain("stop_flush");
    expect(text).not.toContain("flush_in_progress");
    expect(container.querySelector("[data-testid='stop-flush']")).toBeNull();
  });

  test("shows capture and transcribe phases together while session is active (req 5.2)", () => {
    const { getByTestId, queryByTestId } = renderPanels({
      capturePhase: "capturing",
      transcribePhase: "transcribing",
      pcmBacklogSeconds: 60,
    });

    expect(getByTestId("capture-phase").textContent).toBe("capturing");
    expect(getByTestId("transcribe-phase").textContent).toBe("transcribing");
    expect(queryByTestId("transcribe-pcm-backlog")?.textContent).toBe("推論待ち 約 1 分");
  });

  test("renders progress and error panels outside phase-panels-row", () => {
    const { container } = renderPanels({
      transcribePhase: "loading_model",
      modelProgress: {
        status: "downloading",
        percent: 42,
        bytes_downloaded: 1000,
        bytes_total: null,
      },
      captureError: {
        code: "INTERNAL",
        message_ja: "キャプチャエラー",
        action_ja: "再試行してください",
        recoverable: true,
      },
    });

    const row = container.querySelector(".phase-panels-row");
    const progressPanel = container.querySelector(".progress-panel");
    const errorPanel = container.querySelector(".error-panel");

    expect(row?.contains(progressPanel)).toBe(false);
    expect(row?.contains(errorPanel)).toBe(false);
    expect(progressPanel).not.toBeNull();
    expect(errorPanel).not.toBeNull();
  });
});
