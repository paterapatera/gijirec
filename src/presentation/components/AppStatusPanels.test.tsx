import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { cleanup, render } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
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
    captureError: { message_ja: string; action_ja: string } | null;
    transcribePhase: string;
    transcribeError: { message_ja: string; action_ja: string } | null;
    modelProgress: { status: string; percent: number | null; bytes_downloaded: number } | null;
  }> = {},
) {
  return render(
    <AppStatusPanels
      capturePhase={overrides.capturePhase ?? "idle"}
      captureError={overrides.captureError ?? null}
      transcribePhase={overrides.transcribePhase ?? "idle"}
      transcribeError={overrides.transcribeError ?? null}
      modelProgress={overrides.modelProgress ?? null}
    />,
  );
}

describe("AppStatusPanels horizontal layout", () => {
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

  test("renders progress and error panels outside phase-panels-row", () => {
    const { container } = renderPanels({
      transcribePhase: "loading_model",
      modelProgress: { status: "downloading", percent: 42, bytes_downloaded: 1000 },
      captureError: { message_ja: "キャプチャエラー", action_ja: "再試行してください" },
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
