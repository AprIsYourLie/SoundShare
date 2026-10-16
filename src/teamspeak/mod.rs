mod version;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use audiopus::coder::Encoder;
use audiopus::{Application, Bitrate, Channels, SampleRate};
use futures::{FutureExt, StreamExt};
use tsclientlib::{ChannelId, Connection, DisconnectOptions, Identity, StreamItem};
use tsproto_packets::packets::{AudioData, CodecType, OutAudio};

use crate::audio;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    pub server_address: String,
    pub nickname: String,
    pub channel_id: Option<u64>,
    pub identity_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedChannel {
    pub id: u64,
    pub name: String,
}

pub type ConnectedCallback = Box<dyn FnOnce(ConnectedChannel) + Send>;

pub async fn test_connection(settings: ConnectionSettings) -> Result<(), Box<dyn Error>> {
    let mut connection = connect(settings).await?;
    println!("Connected successfully. No audio was sent.");

    connection.disconnect(DisconnectOptions::new())?;
    let drain = connection.events().for_each(|_| futures::future::ready(()));
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    Ok(())
}

pub async fn share_application(
    settings: ConnectionSettings,
    process_id: u32,
) -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_stop.store(true, Ordering::Release);
        }
    });
    share_application_controlled(settings, process_id, stop, None).await
}

pub async fn share_application_controlled(
    settings: ConnectionSettings,
    process_id: u32,
    stop: Arc<AtomicBool>,
    on_connected: Option<ConnectedCallback>,
) -> Result<(), Box<dyn Error>> {
    let mut connection = connect(settings).await?;
    if let (Some(callback), Some(channel)) = (on_connected, current_channel(&connection)) {
        callback(channel);
    }
    let capture = audio::start_application_capture(process_id)?;
    let mut encoder = Encoder::new(SampleRate::Hz48000, Channels::Stereo, Application::Audio)?;
    encoder.set_bitrate(Bitrate::BitsPerSecond(96_000))?;
    let mut opus_output = [0_u8; 1275];
    let mut frame_queue = std::collections::VecDeque::<Vec<f32>>::new();
    let mut sent_packets = 0_u64;
    let mut last_cannot_send_warning = Instant::now() - Duration::from_secs(60);

    let mut event_tick = tokio::time::interval(Duration::from_millis(50));
    event_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut send_tick = tokio::time::interval(Duration::from_millis(20));
    send_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    println!("Sharing audio from PID {process_id}. Press Ctrl+C to stop.");

    let run_result: Result<(), Box<dyn Error>> = 'stream: loop {
        tokio::select! {
            _ = event_tick.tick() => {
                if stop.load(Ordering::Acquire) {
                    break 'stream Ok(());
                }
                loop {
                    let next_item = {
                        let mut events = connection.events();
                        events.next().now_or_never()
                    };
                    match next_item {
                        Some(Some(Ok(_))) => {}
                        Some(Some(Err(error))) => break 'stream Err(error.into()),
                        Some(None) => break 'stream Err("TeamSpeak connection closed".into()),
                        None => break,
                    }
                }
            }
            _ = send_tick.tick() => {
                loop {
                    match capture.try_frame() {
                        Ok(Some(frame)) => frame_queue.push_back(frame),
                        Ok(None) => break,
                        Err(error) => break 'stream Err(error.into()),
                    }
                }

                while frame_queue.len() > 10 {
                    frame_queue.pop_front();
                }

                let Some(frame) = frame_queue.pop_front() else {
                    continue;
                };
                if frame.len() != audio::STREAM_SAMPLES_PER_CHANNEL * 2 {
                    break 'stream Err(format!("unexpected audio frame size: {} samples", frame.len()).into());
                }

                if !connection.can_send_audio() {
                    if last_cannot_send_warning.elapsed() >= Duration::from_secs(3) {
                        eprintln!("Cannot send audio: the client may be muted or lack talk permission.");
                        last_cannot_send_warning = Instant::now();
                    }
                    continue;
                }

                let encoded_length = encoder.encode_float(&frame, &mut opus_output)?;
                let packet = OutAudio::new(&AudioData::C2S {
                    id: 0,
                    codec: CodecType::OpusMusic,
                    data: &opus_output[..encoded_length],
                });
                connection.send_audio(packet)?;
                sent_packets += 1;
            }
        }
    };

    if sent_packets > 0 && connection.can_send_audio() {
        let end_packet = OutAudio::new(&AudioData::C2S {
            id: 0,
            codec: CodecType::OpusMusic,
            data: &[],
        });
        let _ = connection.send_audio(end_packet);
    }

    capture.stop();
    let _ = connection.disconnect(DisconnectOptions::new());
    let drain = connection.events().for_each(|_| futures::future::ready(()));
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    println!("Stopped after sending {sent_packets} audio packets.");
    run_result
}

fn current_channel(connection: &Connection) -> Option<ConnectedChannel> {
    let state = connection.get_state().ok()?;
    let own_client = state.clients.get(&state.own_client)?;
    let channel = state.channels.get(&own_client.channel)?;
    Some(ConnectedChannel {
        id: channel.id.0,
        name: channel.name.clone(),
    })
}

async fn connect(settings: ConnectionSettings) -> Result<Connection, Box<dyn Error>> {
    let identity = load_or_create_identity(&settings.identity_path)?;
    let address = normalize_server_address(&settings.server_address);

    let mut builder = Connection::build(address.clone())
        .name(settings.nickname.clone())
        .version(version::ts6_compatible_client_version())
        .identity(identity)
        .input_muted(false)
        .output_muted(false)
        .input_hardware_enabled(true)
        .output_hardware_enabled(true)
        .log_commands(false);
    if std::env::var_os("SOUNDSHARE_TS_LOG").is_none() {
        builder = builder.logger(slog::Logger::root(slog::Discard, slog::o!()));
    }
    if let Some(channel_id) = settings.channel_id {
        builder = builder.channel_id(ChannelId(channel_id));
    }

    println!("Connecting to {address} as {}...", settings.nickname);
    let mut connection =
        tokio::task::spawn_blocking(move || builder.connect().map_err(Box::new)).await??;

    let connected = tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(item) = connection.events().next().await {
            if let StreamItem::BookEvents(_) = item? {
                return Ok::<(), tsclientlib::Error>(());
            }
        }
        Err(tsclientlib::Error::ConnectionGone)
    })
    .await;

    match connected {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(error.into()),
        Err(_) => return Err("connection timed out while waiting for the server state".into()),
    }
    Ok(connection)
}

fn normalize_server_address(address: &str) -> String {
    let address = address.trim();
    if address.is_empty() || address.contains(':') {
        address.to_string()
    } else {
        format!("{address}:9987")
    }
}

fn load_or_create_identity(path: &Path) -> Result<Identity, Box<dyn Error>> {
    if let Ok(serialized) = fs::read_to_string(path) {
        let serialized = serialized.trim();
        if !serialized.is_empty() {
            return Ok(serde_json::from_str(serialized)?);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let identity = Identity::create();
    let serialized = serde_json::to_vec(&identity)?;
    let temporary_path = path.with_extension("json.tmp");
    fs::write(&temporary_path, serialized)?;
    fs::rename(&temporary_path, path)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_voice_port_is_added() {
        assert_eq!(
            normalize_server_address("voice.example.test"),
            "voice.example.test:9987"
        );
        assert_eq!(
            normalize_server_address("voice.example.test:1234"),
            "voice.example.test:1234"
        );
    }

    #[test]
    fn identity_is_created_once_and_reused() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("soundshare-identity-{unique}"));
        let path = directory.join("identity.json");

        let first = load_or_create_identity(&path).expect("identity should be created");
        let first_serialized = serde_json::to_string(&first).expect("identity should serialize");
        let second = load_or_create_identity(&path).expect("identity should be loaded");
        let second_serialized = serde_json::to_string(&second).expect("identity should serialize");
        let _ = fs::remove_dir_all(&directory);

        assert_eq!(first_serialized, second_serialized);
    }
}
