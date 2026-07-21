//! FFmpeg command builder for HLS video transcoding.
//!
//! Pure logic module — builds argument lists but never spawns processes.
//! The calling server is responsible for executing commands and managing
//! the transcoding worker lifecycle.

/// Configuration for multi-resolution HLS transcoding.
pub struct TranscodeConfig {
    pub input_path: String,
    pub output_dir: String,
    /// Target resolutions in pixels (vertical), e.g. `[240, 480, 720, 1080]`.
    pub resolutions: Vec<u32>,
}

/// A single FFmpeg invocation for one resolution.
pub struct TranscodeCommand {
    pub resolution: u32,
    /// Full argument list (excluding the `ffmpeg` binary name).
    pub args: Vec<String>,
    pub output_path: String,
}

/// Standard video bitrates (kbps) per resolution.
/// Values match PeerTube defaults for x264 HLS output.
pub fn bitrate_for_resolution(resolution: u32) -> u32 {
    match resolution {
        0..=240 => 600,
        241..=360 => 1000,
        361..=480 => 1500,
        481..=720 => 2500,
        721..=1080 => 5000,
        1081..=1440 => 10000,
        _ => 15000,
    }
}

/// Audio bitrate (kbps) paired with a video resolution.
fn audio_bitrate_for_resolution(resolution: u32) -> u32 {
    if resolution <= 480 {
        96
    } else {
        128
    }
}

/// Build FFmpeg commands for HLS transcoding at multiple resolutions.
///
/// Each command produces a single-resolution HLS stream (`.m3u8` + segments)
/// inside `config.output_dir/<resolution>p/`. The caller should also generate
/// a master playlist pointing at each resolution's playlist.
pub fn build_transcode_commands(config: &TranscodeConfig) -> Vec<TranscodeCommand> {
    config
        .resolutions
        .iter()
        .map(|&res| {
            let out_dir = format!("{}/{}p", config.output_dir, res);
            let playlist = format!("{}/index.m3u8", out_dir);
            let segment_pattern = format!("{}/segment_%03d.ts", out_dir);
            let video_bitrate = format!("{}k", bitrate_for_resolution(res));
            let audio_bitrate = format!("{}k", audio_bitrate_for_resolution(res));
            let scale = format!("scale=-2:{res}");

            let args = vec![
                "-i".into(),
                config.input_path.clone(),
                "-vf".into(),
                scale,
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "medium".into(),
                "-b:v".into(),
                video_bitrate,
                "-maxrate".into(),
                format!("{}k", bitrate_for_resolution(res) * 12 / 10),
                "-bufsize".into(),
                format!("{}k", bitrate_for_resolution(res) * 2),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                audio_bitrate,
                "-f".into(),
                "hls".into(),
                "-hls_time".into(),
                "4".into(),
                "-hls_playlist_type".into(),
                "vod".into(),
                "-hls_segment_filename".into(),
                segment_pattern,
                "-progress".into(),
                "pipe:1".into(),
                playlist.clone(),
            ];

            TranscodeCommand {
                resolution: res,
                args,
                output_path: playlist,
            }
        })
        .collect()
}

/// Build FFmpeg argument list for thumbnail extraction.
///
/// Extracts a single frame at `timestamp_secs` and writes it as a JPEG.
pub fn build_thumbnail_command(input: &str, output: &str, timestamp_secs: u32) -> Vec<String> {
    vec![
        "-ss".into(),
        timestamp_secs.to_string(),
        "-i".into(),
        input.into(),
        "-vframes".into(),
        "1".into(),
        "-q:v".into(),
        "2".into(),
        "-y".into(),
        output.into(),
    ]
}

/// Build FFmpeg argument list for an animated preview GIF.
///
/// Extracts `duration` seconds starting at `start` and produces a 320px-wide GIF.
pub fn build_preview_command(input: &str, output: &str, start: u32, duration: u32) -> Vec<String> {
    vec![
        "-ss".into(),
        start.to_string(),
        "-t".into(),
        duration.to_string(),
        "-i".into(),
        input.into(),
        "-vf".into(),
        "fps=10,scale=320:-1:flags=lanczos".into(),
        "-y".into(),
        output.into(),
    ]
}

/// Estimate output file size in bytes from duration and resolution.
///
/// Uses the standard bitrate for the resolution plus audio overhead.
/// Returns bytes.
pub fn estimate_file_size(duration_secs: u32, resolution: u32) -> u64 {
    let video_kbps = bitrate_for_resolution(resolution) as u64;
    let audio_kbps = audio_bitrate_for_resolution(resolution) as u64;
    let total_kbps = video_kbps + audio_kbps;
    // kbps * seconds / 8 = kilobytes, * 1000 = bytes
    total_kbps * duration_secs as u64 * 1000 / 8
}

/// Parse FFmpeg progress output to extract completion percentage.
///
/// FFmpeg writes `out_time_us=<microseconds>` lines to the progress pipe.
/// Returns a percentage (0..=100) or `None` if the line doesn't contain
/// timing information.
pub fn parse_progress(output: &str, total_duration_secs: u32) -> Option<u32> {
    if total_duration_secs == 0 {
        return None;
    }

    for line in output.lines().rev() {
        let line = line.trim();
        if let Some(us_str) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = us_str.trim().parse::<u64>() {
                let elapsed_secs = us / 1_000_000;
                let pct = (elapsed_secs * 100 / total_duration_secs as u64).min(100) as u32;
                return Some(pct);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrates_are_sane() {
        assert_eq!(bitrate_for_resolution(240), 600);
        assert_eq!(bitrate_for_resolution(480), 1500);
        assert_eq!(bitrate_for_resolution(720), 2500);
        assert_eq!(bitrate_for_resolution(1080), 5000);
        assert_eq!(bitrate_for_resolution(2160), 15000);
    }

    #[test]
    fn transcode_commands_count_matches_resolutions() {
        let config = TranscodeConfig {
            input_path: "/tmp/input.mp4".into(),
            output_dir: "/tmp/out".into(),
            resolutions: vec![240, 720, 1080],
        };
        let cmds = build_transcode_commands(&config);
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0].resolution, 240);
        assert_eq!(cmds[1].resolution, 720);
        assert_eq!(cmds[2].resolution, 1080);

        // Each output path is under the output_dir with resolution subdir
        assert_eq!(cmds[0].output_path, "/tmp/out/240p/index.m3u8");
        assert_eq!(cmds[2].output_path, "/tmp/out/1080p/index.m3u8");
    }

    #[test]
    fn transcode_args_contain_required_flags() {
        let config = TranscodeConfig {
            input_path: "/tmp/test.mp4".into(),
            output_dir: "/tmp/out".into(),
            resolutions: vec![720],
        };
        let cmds = build_transcode_commands(&config);
        let args = &cmds[0].args;

        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/tmp/test.mp4".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(args.contains(&"hls".to_string()));
        assert!(args.contains(&"vod".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn thumbnail_command_uses_timestamp() {
        let args = build_thumbnail_command("/input.mp4", "/thumb.jpg", 5);
        assert_eq!(args[1], "5");
        assert!(args.contains(&"-vframes".to_string()));
        assert!(args.contains(&"1".to_string()));
    }

    #[test]
    fn preview_command_uses_start_and_duration() {
        let args = build_preview_command("/input.mp4", "/preview.gif", 10, 3);
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"3".to_string()));
        assert!(args.iter().any(|a| a.contains("fps=10")));
    }

    #[test]
    fn estimate_file_size_is_reasonable() {
        // 60s of 720p: (2500+128) kbps * 60s / 8 * 1000 bytes
        let size = estimate_file_size(60, 720);
        let expected = (2500u64 + 128) * 60 * 1000 / 8;
        assert_eq!(size, expected);
    }

    #[test]
    fn parse_progress_extracts_percentage() {
        let output = "frame=100\nfps=30\nout_time_us=30000000\n";
        assert_eq!(parse_progress(output, 60), Some(50));
    }

    #[test]
    fn parse_progress_clamps_to_100() {
        let output = "out_time_us=120000000\n";
        assert_eq!(parse_progress(output, 60), Some(100));
    }

    #[test]
    fn parse_progress_returns_none_for_no_timing() {
        assert_eq!(parse_progress("frame=100\n", 60), None);
    }

    #[test]
    fn parse_progress_returns_none_for_zero_duration() {
        let output = "out_time_us=5000000\n";
        assert_eq!(parse_progress(output, 0), None);
    }
}
