import { describe, expect, test } from "bun:test";
import { asInjectableInvokeFn } from "./injectableInvoke";
import {
  DEFAULT_TRANSCRIBE_SETTINGS,
  getTranscribeSettings,
  setTranscribeModelVariant,
  WHISPER_MODEL_VARIANT_LABELS,
} from "./transcribeSettingsCommands";

describe("transcribeSettingsCommands", () => {
  test("getTranscribeSettings invokes contract command", async () => {
    const invokeFn = asInjectableInvokeFn(async (cmd: string) => {
      expect(cmd).toBe("get_transcribe_settings");
      return {
        settings: { model_variant: "q8_0" },
        local_availability: { q5_0: false, q8_0: true, fp16: true },
      };
    });

    const response = await getTranscribeSettings({ invokeFn });
    expect(response.settings.model_variant).toBe("q8_0");
    expect(response.local_availability.q8_0).toBe(true);
  });

  test("setTranscribeModelVariant invokes contract command", async () => {
    const invokeFn = asInjectableInvokeFn(async (cmd: string, args?: Record<string, unknown>) => {
      expect(cmd).toBe("set_transcribe_model_variant");
      expect(args).toEqual({ model_variant: "q5_0" });
      return { settings: { model_variant: "q5_0" } };
    });

    const response = await setTranscribeModelVariant({ model_variant: "q5_0" }, { invokeFn });
    expect(response.settings.model_variant).toBe("q5_0");
  });

  test("variant labels match UI contract", () => {
    expect(WHISPER_MODEL_VARIANT_LABELS).toEqual({
      q5_0: "Q5_0",
      q8_0: "Q8_0",
      fp16: "FP16",
    });
    expect(DEFAULT_TRANSCRIBE_SETTINGS.model_variant).toBe("fp16");
  });
});
