import type { invoke } from "@tauri-apps/api/core";
import type { ChangeEvent } from "react";
import {
  WHISPER_MODEL_VARIANTS,
  WHISPER_MODEL_VARIANT_LABELS,
  type WhisperModelVariant,
} from "../../infrastructure/tauri/transcribeSettingsCommands";
import type { TranscribeEventListenFn, TranscribePhase } from "../hooks/transcribe-status";
import { useTranscribeSettings } from "../hooks/useTranscribeSettings";
import { useTranscribeStatus } from "../hooks/useTranscribeStatus";

interface ModelVariantSelectorInjectedProps {
  readonly selectedVariant: WhisperModelVariant;
  readonly onVariantChange: (variant: WhisperModelVariant) => void;
  readonly transcribePhase: TranscribePhase;
  readonly isLoading: boolean;
}

interface ModelVariantSelectorRuntimeProps {
  readonly invokeFn?: typeof invoke;
  readonly listenFn?: TranscribeEventListenFn;
}

export type ModelVariantSelectorProps = Partial<ModelVariantSelectorInjectedProps> &
  ModelVariantSelectorRuntimeProps;

interface ModelVariantSelectorViewProps extends ModelVariantSelectorInjectedProps {}

function ModelVariantSelectorView({
  selectedVariant,
  onVariantChange,
  transcribePhase,
  isLoading,
}: ModelVariantSelectorViewProps) {
  const disabled = isLoading || transcribePhase === "loading_model";

  const handleChange = (event: ChangeEvent<HTMLSelectElement>) => {
    onVariantChange(event.target.value as WhisperModelVariant);
  };

  return (
    <section className="model-variant-panel" aria-label="Whisper モデルバリアント">
      <label className="status-label" htmlFor="model-variant-select">
        文字起こしモデル（現在: {WHISPER_MODEL_VARIANT_LABELS[selectedVariant]}）
      </label>
      <select
        id="model-variant-select"
        data-testid="model-variant-select"
        value={selectedVariant}
        onChange={handleChange}
        disabled={disabled}
        aria-disabled={disabled}
      >
        {WHISPER_MODEL_VARIANTS.map((variant) => (
          <option key={variant} value={variant}>
            {WHISPER_MODEL_VARIANT_LABELS[variant]}
          </option>
        ))}
      </select>
    </section>
  );
}

function ModelVariantSelectorConnected(props: ModelVariantSelectorRuntimeProps) {
  const transcribeSettings = useTranscribeSettings(
    props.invokeFn === undefined ? {} : { invokeFn: props.invokeFn },
  );
  const transcribeStatus = useTranscribeStatus(
    props.listenFn === undefined ? {} : { listenFn: props.listenFn },
  );

  return (
    <ModelVariantSelectorView
      selectedVariant={transcribeSettings.settings.model_variant}
      onVariantChange={(variant) => {
        void transcribeSettings.setModelVariant(variant);
      }}
      transcribePhase={transcribeStatus.phase}
      isLoading={transcribeSettings.isLoading}
    />
  );
}

export function ModelVariantSelector(props: ModelVariantSelectorProps = {}) {
  if (
    props.selectedVariant !== undefined &&
    props.onVariantChange !== undefined &&
    props.transcribePhase !== undefined &&
    props.isLoading !== undefined
  ) {
    return (
      <ModelVariantSelectorView
        selectedVariant={props.selectedVariant}
        onVariantChange={props.onVariantChange}
        transcribePhase={props.transcribePhase}
        isLoading={props.isLoading}
      />
    );
  }

  return <ModelVariantSelectorConnected {...props} />;
}
