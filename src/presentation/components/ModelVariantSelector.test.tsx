import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import { ModelVariantSelector } from "./ModelVariantSelector";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

describe("ModelVariantSelector", () => {
  test("shows three variant options and current selection label", () => {
    const onVariantChange = () => {};

    const { getByTestId, getByText } = render(
      <ModelVariantSelector
        selectedVariant="fp16"
        transcribePhase="ready"
        isLoading={false}
        onVariantChange={onVariantChange}
      />,
    );

    const select = getByTestId("model-variant-select") as HTMLSelectElement;
    expect(select.options.length).toBe(3);
    expect(select.value).toBe("fp16");
    expect(getByText(/現在: FP16/)).toBeTruthy();
  });

  test("disables selection while loading_model", () => {
    const { getByTestId } = render(
      <ModelVariantSelector
        selectedVariant="q8_0"
        transcribePhase="loading_model"
        isLoading={false}
        onVariantChange={() => {}}
      />,
    );

    const select = getByTestId("model-variant-select") as HTMLSelectElement;
    expect(select.disabled).toBe(true);
  });

  test("calls onVariantChange when user selects a variant", () => {
    let selected = "fp16";
    const onVariantChange = (variant: typeof selected) => {
      selected = variant;
    };

    const { getByTestId } = render(
      <ModelVariantSelector
        selectedVariant="fp16"
        transcribePhase="ready"
        isLoading={false}
        onVariantChange={onVariantChange}
      />,
    );

    fireEvent.change(getByTestId("model-variant-select"), { target: { value: "q5_0" } });
    expect(selected).toBe("q5_0");
  });
});
