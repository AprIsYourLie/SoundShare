use std::collections::BTreeMap;
use std::error::Error;

use wasapi::{DeviceEnumerator, Direction, SessionState};

mod capture;
mod process;

pub use capture::{
    capture_application_to_wav, start_application_capture, ApplicationCapture, CaptureSummary,
    STREAM_SAMPLES_PER_CHANNEL,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSession {
    pub process_id: u32,
    pub process_name: Option<String>,
    pub peak: f32,
}

impl AudioSession {
    pub fn display_name(&self) -> &str {
        self.process_name.as_deref().unwrap_or("Unknown process")
    }
}

pub fn active_render_sessions() -> Result<Vec<AudioSession>, Box<dyn Error>> {
    wasapi::initialize_mta().ok()?;

    let enumerator = DeviceEnumerator::new()?;
    let mut sessions_by_pid = BTreeMap::<u32, AudioSession>::new();

    for device in &enumerator.get_device_collection(&Direction::Render)? {
        let device = device?;
        let manager = device.get_iaudiosessionmanager()?;
        let sessions = manager.get_audiosessionenumerator()?;

        for index in 0..sessions.get_count()? {
            let control = sessions.get_session(index)?;
            if control.get_state()? != SessionState::Active {
                continue;
            }

            let process_id = control.get_process_id()?;
            let peak = control.get_audiometerinformation()?.get_peak_value()?;

            sessions_by_pid
                .entry(process_id)
                .and_modify(|session| session.peak = session.peak.max(peak))
                .or_insert_with(|| AudioSession {
                    process_id,
                    process_name: process::query_process_name(process_id),
                    peak,
                });
        }
    }

    let mut sessions: Vec<_> = sessions_by_pid.into_values().collect();
    sessions.sort_by(|left, right| {
        left.display_name()
            .to_ascii_lowercase()
            .cmp(&right.display_name().to_ascii_lowercase())
            .then(left.process_id.cmp(&right.process_id))
    });
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_uses_a_readable_fallback_name() {
        let session = AudioSession {
            process_id: 42,
            process_name: None,
            peak: 0.0,
        };

        assert_eq!(session.display_name(), "Unknown process");
    }
}
