//! Selection validation and capture restart coordination.

use gijirec_domain::audio::{AudioDeviceId, AudioDeviceList, CapturePhase, DeviceSelection};

use super::{
    CaptureSelectionPort, DefaultDeviceSelectionService, DeviceEnumeratorPort,
    DeviceSelectionClock, DeviceSelectionError, DeviceSelectionEvents, SpeakerPreflightPort,
};

pub(crate) fn validate_selection<P: SpeakerPreflightPort>(
    preflight: &P,
    selection: &DeviceSelection,
    list: &AudioDeviceList,
) -> Result<(), DeviceSelectionError> {
    if let Some(mic_id) = selection.microphone_id() {
        let exists = list
            .inputs
            .iter()
            .any(|device| device.id().as_str() == mic_id.as_str());
        if !exists {
            return Err(DeviceSelectionError::invalid_device());
        }
    }

    if let Some(speaker_id) = selection.speaker_id() {
        let exists = list
            .outputs
            .iter()
            .any(|device| device.id().as_str() == speaker_id.as_str());
        if !exists {
            return Err(DeviceSelectionError::invalid_device());
        }
    }

    preflight.validate_speaker(selection.speaker_id(), list)
}

pub(crate) fn should_restart_capture(phase: CapturePhase) -> bool {
    matches!(
        phase,
        CapturePhase::Capturing | CapturePhase::Starting | CapturePhase::Error
    )
}

pub(crate) fn selection_id_str(id: Option<&AudioDeviceId>) -> Option<&str> {
    id.map(|device_id| device_id.as_str())
}

pub(crate) fn device_display_names(
    list: &AudioDeviceList,
    selection: &DeviceSelection,
) -> (Option<String>, Option<String>) {
    let microphone_name = selection.microphone_id().and_then(|id| {
        list.inputs
            .iter()
            .find(|device| device.id().as_str() == id.as_str())
            .map(|device| device.name().to_string())
    });
    let speaker_name = selection.speaker_id().and_then(|id| {
        list.outputs
            .iter()
            .find(|device| device.id().as_str() == id.as_str())
            .map(|device| device.name().to_string())
    });
    (microphone_name, speaker_name)
}

/* jscpd:ignore-start */
impl<E, O, P, Ev, C> DefaultDeviceSelectionService<E, O, P, Ev, C>
where
    E: DeviceEnumeratorPort + 'static,
    O: CaptureSelectionPort,
    P: SpeakerPreflightPort,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
    /* jscpd:ignore-end */
{
    pub(crate) fn restart_capture_until_stable(&self) -> Result<(), DeviceSelectionError> {
        loop {
            let selection = self.store.get_selection();
            let microphone_id = selection_id_str(selection.microphone_id());
            let speaker_id = selection_id_str(selection.speaker_id());
            if !self.restart_capture_once(&selection, microphone_id, speaker_id)? {
                return Ok(());
            }
            if self.store.get_selection() == selection {
                break;
            }
        }
        Ok(())
    }

    /// Returns `false` when capture is not in a restart-eligible phase (caller should stop).
    fn restart_capture_once(
        &self,
        selection: &DeviceSelection,
        microphone_id: Option<&str>,
        speaker_id: Option<&str>,
    ) -> Result<bool, DeviceSelectionError> {
        let mut orchestrator = self
            .orchestrator
            .lock()
            .map_err(|_| DeviceSelectionError::internal("orchestrator lock poisoned"))?;
        if !should_restart_capture(orchestrator.capture_phase()) {
            return Ok(false);
        }

        let correlation_id = uuid::Uuid::new_v4().to_string();
        let started_ms = self.shared.clock.now_ms();
        self.observability
            .log_recapture_started(&correlation_id, microphone_id, speaker_id);

        orchestrator
            .restart_with_selection(selection)
            .map_err(|err| DeviceSelectionError::internal(err.to_string()))?;

        let duration_ms = self.shared.clock.now_ms().saturating_sub(started_ms);
        self.observability
            .log_recapture_completed(&correlation_id, duration_ms);
        Ok(true)
    }
}
