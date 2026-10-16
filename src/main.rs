use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use soundshare::{audio, config, teamspeak};

const APP_NAME: &str = "SoundShare";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(error) = run() {
        eprintln!("SoundShare error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);

    match command {
        Some("list") => print_active_audio_sessions(),
        Some("record") => record_application(&arguments[1..]),
        Some("connect-test") => connect_test(&arguments[1..]),
        Some("share") => share_application(&arguments[1..]),
        Some("--version" | "-V") => {
            println!("{APP_NAME} {APP_VERSION}");
            Ok(())
        }
        Some("--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(unknown) => Err(format!("unknown command: {unknown}").into()),
    }
}

fn share_application(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if arguments.len() < 2 || arguments.len() > 4 {
        return Err("usage: soundshare share <PID> <server[:port]> [channel-id] [nickname]".into());
    }

    let process_id = arguments[0]
        .parse::<u32>()
        .map_err(|_| format!("invalid process ID: {}", arguments[0]))?;
    let server_address = arguments[1].clone();
    let channel_id = arguments
        .get(2)
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| "channel ID must be a positive whole number")?;
    let nickname = arguments
        .get(3)
        .cloned()
        .unwrap_or_else(|| "SoundShare · 伴奏".to_string());
    let identity_path = config::identity_path()?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(teamspeak::share_application(
        teamspeak::ConnectionSettings {
            server_address,
            nickname,
            channel_id,
            identity_path,
        },
        process_id,
    ))?;
    Ok(())
}

fn connect_test(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if arguments.is_empty() || arguments.len() > 3 {
        return Err(
            "usage: soundshare connect-test <server[:port]> [channel-id] [nickname]".into(),
        );
    }

    let server_address = arguments[0].clone();
    let channel_id = arguments
        .get(1)
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| "channel ID must be a positive whole number")?;
    let nickname = arguments
        .get(2)
        .cloned()
        .unwrap_or_else(|| "SoundShare Dev Test".to_string());
    let identity_path = config::identity_path()?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(teamspeak::test_connection(teamspeak::ConnectionSettings {
        server_address,
        nickname,
        channel_id,
        identity_path,
    }))?;
    Ok(())
}

fn print_active_audio_sessions() -> Result<(), Box<dyn Error>> {
    let sessions = audio::active_render_sessions()?;

    if sessions.is_empty() {
        println!("No active application audio sessions were found.");
        println!("Start playback in QQ Music or another application, then run this command again.");
        return Ok(());
    }

    println!("Active application audio sessions:");
    for session in sessions {
        println!(
            "  PID {:>6}  peak {:>5.1}%  {}",
            session.process_id,
            session.peak * 100.0,
            session.display_name()
        );
    }

    Ok(())
}

fn record_application(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if arguments.is_empty() || arguments.len() > 3 {
        return Err("usage: soundshare record <PID> [seconds] [output.wav]".into());
    }

    let process_id: u32 = arguments[0]
        .parse()
        .map_err(|_| format!("invalid process ID: {}", arguments[0]))?;
    let seconds: u64 = arguments
        .get(1)
        .map(|value| value.parse())
        .transpose()
        .map_err(|_| "recording duration must be a whole number of seconds")?
        .unwrap_or(10);

    if !(1..=300).contains(&seconds) {
        return Err("recording duration must be between 1 and 300 seconds".into());
    }

    let output_path = arguments
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("soundshare-capture.wav"));

    println!("Capturing PID {process_id} for {seconds} seconds...");
    let summary =
        audio::capture_application_to_wav(process_id, Duration::from_secs(seconds), &output_path)?;
    println!("Saved: {}", output_path.display());
    println!(
        "Captured {} frames ({:.2} seconds of audio data)",
        summary.frames,
        summary.frames as f64 / summary.sample_rate as f64
    );
    if summary.frames == 0 {
        println!("No audio frames were received. Make sure the selected application is playing.");
    }

    Ok(())
}

fn print_help() {
    println!("{APP_NAME} {APP_VERSION}");
    println!();
    println!("Usage:");
    println!("  soundshare list       List applications with active audio sessions");
    println!("  soundshare record <PID> [seconds] [output.wav]");
    println!("                        Capture one application to a WAV test file");
    println!("  soundshare connect-test <server[:port]> [channel-id] [nickname]");
    println!("                        Connect briefly without sending audio");
    println!("  soundshare share <PID> <server[:port]> [channel-id] [nickname]");
    println!("                        Share one application's audio until Ctrl+C");
    println!("  soundshare --version  Print the application version");
}
