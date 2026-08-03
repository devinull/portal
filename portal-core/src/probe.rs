use std::ffi::OsString;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::Duration;

use thiserror::Error;

use crate::platform::{ExecutableNotFound, resolve_executable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeMeasurement {
    pub latency: Option<Duration>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error(transparent)]
    ExecutableNotFound(#[from] ExecutableNotFound),
    #[error("could not start Tailscale probe at {path}: {source}")]
    Start {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "Tailscale probe at {path} exited with {status}",
        status = status.map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
    )]
    Exit { path: PathBuf, status: Option<i32> },
}

pub(crate) fn probe(
    tailscale_override: Option<&Path>,
    target: IpAddr,
) -> Result<ProbeMeasurement, ProbeError> {
    let executable = resolve_executable("tailscale", tailscale_override)?;
    let args = probe_args(target);
    let output = Command::new(&executable)
        .args(&args)
        .output()
        .map_err(|source| ProbeError::Start {
            path: executable.clone(),
            source,
        })?;

    measurement_from_output(&executable, output.status, &output.stdout)
}

fn probe_args(target: IpAddr) -> [OsString; 7] {
    [
        OsString::from("ping"),
        OsString::from("--tsmp"),
        OsString::from("--c"),
        OsString::from("1"),
        OsString::from("--timeout"),
        OsString::from("2s"),
        OsString::from(target.to_string()),
    ]
}

fn measurement_from_output(
    executable: &Path,
    status: ExitStatus,
    stdout: &[u8],
) -> Result<ProbeMeasurement, ProbeError> {
    if !status.success() {
        return Err(ProbeError::Exit {
            path: executable.to_owned(),
            status: status.code(),
        });
    }

    Ok(ProbeMeasurement {
        latency: std::str::from_utf8(stdout).ok().and_then(parse_latency),
    })
}

fn parse_latency(output: &str) -> Option<Duration> {
    let (_, remainder) = output.rsplit_once(" in ")?;
    parse_duration(remainder.lines().next()?.trim())
}

fn parse_duration(value: &str) -> Option<Duration> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ns") {
        (number, 1.0)
    } else if let Some(number) = value
        .strip_suffix("us")
        .or_else(|| value.strip_suffix("µs"))
    {
        (number, 1_000.0)
    } else if let Some(number) = value.strip_suffix("ms") {
        (number, 1_000_000.0)
    } else {
        (value.strip_suffix('s')?, 1_000_000_000.0)
    };

    let number = number.parse::<f64>().ok()?;
    if !number.is_finite() || number.is_sign_negative() {
        return None;
    }
    let nanos = number * multiplier;
    if nanos > u64::MAX as f64 {
        return None;
    }
    Some(Duration::from_nanos(nanos.round() as u64))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::Path;
    use std::process::ExitStatus;
    use std::time::Duration;

    use super::{measurement_from_output, parse_latency, probe_args};

    #[test]
    fn command_has_exact_separate_arguments() {
        assert_eq!(
            probe_args(IpAddr::V4(Ipv4Addr::new(100, 100, 0, 20))),
            [
                OsString::from("ping"),
                OsString::from("--tsmp"),
                OsString::from("--c"),
                OsString::from("1"),
                OsString::from("--timeout"),
                OsString::from("2s"),
                OsString::from("100.100.0.20"),
            ]
        );
    }

    #[test]
    fn parses_all_supported_units_and_surrounding_text() {
        assert_eq!(
            parse_latency("pong from node via TSMP in 800ns"),
            Some(Duration::from_nanos(800))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 800us"),
            Some(Duration::from_micros(800))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 800µs"),
            Some(Duration::from_micros(800))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 20ms"),
            Some(Duration::from_millis(20))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 20.5ms"),
            Some(Duration::from_micros(20_500))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 1s"),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            parse_latency("pong from node via TSMP in 1.2s"),
            Some(Duration::from_millis(1_200))
        );
    }

    #[test]
    fn parses_the_final_duration_and_trailing_newline() {
        let output = "setup in invalid\npong from sanitized-node via TSMP in 24ms\n";
        assert_eq!(parse_latency(output), Some(Duration::from_millis(24)));
    }

    #[test]
    fn missing_and_malformed_latency_are_ignored() {
        for output in [
            "pong from sanitized-node",
            "pong from sanitized-node in ",
            "pong from sanitized-node in fast",
            "pong from sanitized-node in -1ms",
            "pong from sanitized-node in NaNms",
            "pong from sanitized-node in 20 ms",
        ] {
            assert_eq!(parse_latency(output), None, "{output}");
        }
        assert_eq!(
            parse_latency("pong from node in 20ms\nnew final format in unknown"),
            None
        );
    }

    #[test]
    fn successful_exit_remains_reachable_when_output_changes() {
        let result = measurement_from_output(
            Path::new("tailscale"),
            successful_status(),
            b"new output without latency",
        )
        .unwrap();
        assert_eq!(result.latency, None);
    }

    #[test]
    fn failure_does_not_include_private_process_output() {
        let error = measurement_from_output(
            Path::new("tailscale"),
            failed_status(),
            b"private-node.example.test 100.100.0.20",
        )
        .unwrap_err()
        .to_string();
        assert!(!error.contains("private-node"));
        assert!(!error.contains("100.100.0.20"));
    }

    #[cfg(unix)]
    fn successful_status() -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn successful_status() -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    #[cfg(unix)]
    fn failed_status() -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(1 << 8)
    }

    #[cfg(windows)]
    fn failed_status() -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(1)
    }
}
