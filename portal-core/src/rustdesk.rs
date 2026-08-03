use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use thiserror::Error;

use crate::model::{Action, Device, Launch};
use crate::platform::{ExecutableNotFound, resolve_executable};
use crate::state::record_connection;

#[derive(Debug, Error)]
pub enum RustDeskError {
    #[error(transparent)]
    ExecutableNotFound(#[from] ExecutableNotFound),
    #[error("could not start RustDesk at {path}: {source}")]
    Start {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub struct LaunchOutcome {
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Missing,
    Stopped,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionStep {
    pub description: String,
    pub launch: Launch,
    pub privileged: bool,
    pub persistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionPlan {
    pub platform: String,
    pub service_state: ServiceState,
    pub steps: Vec<ProvisionStep>,
}

#[derive(Debug)]
pub struct ProvisionOutcome {
    pub verified: bool,
}

#[derive(Debug, Error)]
pub enum ProvisionError {
    #[error("local provisioning is implemented only for Windows in Stage 1")]
    UnsupportedPlatform,
    #[error(transparent)]
    ExecutableNotFound(#[from] ExecutableNotFound),
    #[error("could not inspect the RustDesk service with {path}: {source}")]
    InspectService {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not run provisioning command {program}: {source}")]
    Start {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "provisioning command {program} exited with {status}: {message}",
        status = status.map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
    )]
    Exit {
        program: PathBuf,
        status: Option<i32>,
        message: String,
    },
    #[error("administrator elevation is required; rerun Portal from an elevated terminal")]
    ElevationRequired,
    #[error("RustDesk provisioning could not be verified: {message}")]
    Verification { message: String },
}

pub(crate) fn launch_spec(
    rustdesk_override: Option<&Path>,
    action: Action,
    device: &Device,
) -> Result<Launch, RustDeskError> {
    let program = resolve_executable("rustdesk", rustdesk_override)?;
    Ok(build_launch(program, action, device))
}

pub(crate) fn build_launch(program: PathBuf, action: Action, device: &Device) -> Launch {
    let mode = match action {
        Action::Connect => "--connect",
        Action::Transfer => "--file-transfer",
    };
    Launch {
        program,
        args: vec![
            OsString::from(mode),
            OsString::from(device.preferred_ip.to_string()),
        ],
    }
}

pub(crate) fn execute_launch(
    launch: &Launch,
    state_path: &Path,
    device: &Device,
) -> Result<LaunchOutcome, RustDeskError> {
    execute_launch_with(launch, state_path, device, |launch| {
        Command::new(&launch.program)
            .args(&launch.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
    })
}

#[cfg(windows)]
pub(crate) fn local_provision_plan(
    rustdesk_override: Option<&Path>,
) -> Result<ProvisionPlan, ProvisionError> {
    let rustdesk = resolve_executable("rustdesk", rustdesk_override)?;
    let sc = windows_sc_path();
    let service_state = inspect_service_state(&sc)?;
    Ok(build_windows_provision_plan(rustdesk, sc, service_state))
}

#[cfg(not(windows))]
pub(crate) fn local_provision_plan(
    _rustdesk_override: Option<&Path>,
) -> Result<ProvisionPlan, ProvisionError> {
    Err(ProvisionError::UnsupportedPlatform)
}

#[cfg(windows)]
pub(crate) fn apply_local_provision_plan(
    plan: &ProvisionPlan,
) -> Result<ProvisionOutcome, ProvisionError> {
    let rustdesk = plan
        .steps
        .iter()
        .find(|step| {
            step.launch
                .args
                .first()
                .is_some_and(|arg| arg == "--option" || arg == "--install-service")
        })
        .map(|step| step.launch.program.clone())
        .ok_or_else(|| ProvisionError::Verification {
            message: "provisioning plan has no RustDesk command".to_owned(),
        })?;

    ensure_elevated(&rustdesk)?;
    for step in &plan.steps {
        run_checked(&step.launch)?;
    }
    verify_windows_provisioning(&rustdesk, &windows_sc_path())?;
    Ok(ProvisionOutcome { verified: true })
}

#[cfg(not(windows))]
pub(crate) fn apply_local_provision_plan(
    _plan: &ProvisionPlan,
) -> Result<ProvisionOutcome, ProvisionError> {
    Err(ProvisionError::UnsupportedPlatform)
}

#[cfg(any(windows, test))]
fn build_windows_provision_plan(
    rustdesk: PathBuf,
    sc: PathBuf,
    service_state: ServiceState,
) -> ProvisionPlan {
    let mut steps = Vec::new();
    match service_state {
        ServiceState::Missing => steps.push(provision_step(
            "Install the RustDesk service",
            rustdesk.clone(),
            ["--install-service"],
        )),
        ServiceState::Stopped => steps.push(provision_step(
            "Start the RustDesk service",
            sc,
            ["start", "RustDesk"],
        )),
        ServiceState::Running => {}
    }
    steps.push(provision_step(
        "Enable direct IP access",
        rustdesk.clone(),
        ["--option", "direct-server", "Y"],
    ));
    steps.push(provision_step(
        "Restrict incoming access to the Tailscale CGNAT range",
        rustdesk,
        ["--option", "whitelist", "100.64.0.0/10"],
    ));
    ProvisionPlan {
        platform: "windows".to_owned(),
        service_state,
        steps,
    }
}

#[cfg(any(windows, test))]
fn provision_step<const N: usize>(
    description: &str,
    program: PathBuf,
    args: [&str; N],
) -> ProvisionStep {
    ProvisionStep {
        description: description.to_owned(),
        launch: Launch {
            program,
            args: args.into_iter().map(OsString::from).collect(),
        },
        privileged: true,
        persistent: true,
    }
}

#[cfg(windows)]
fn windows_sc_path() -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(
        || PathBuf::from("sc.exe"),
        |root| PathBuf::from(root).join("System32").join("sc.exe"),
    )
}

#[cfg(windows)]
fn inspect_service_state(sc: &Path) -> Result<ServiceState, ProvisionError> {
    let output = Command::new(sc)
        .args(["query", "RustDesk"])
        .output()
        .map_err(|source| ProvisionError::InspectService {
            path: sc.to_owned(),
            source,
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && text.contains("RUNNING") {
        Ok(ServiceState::Running)
    } else if output.status.success() && text.contains("SERVICE_NAME") {
        Ok(ServiceState::Stopped)
    } else {
        Ok(ServiceState::Missing)
    }
}

#[cfg(windows)]
fn ensure_elevated(rustdesk: &Path) -> Result<(), ProvisionError> {
    let output = Command::new(rustdesk)
        .args(["--option", "direct-server"])
        .output()
        .map_err(|source| ProvisionError::Start {
            program: rustdesk.to_owned(),
            source,
        })?;
    let message = combined_output(&output);
    if message
        .to_ascii_lowercase()
        .contains("administrative privileges required")
    {
        return Err(ProvisionError::ElevationRequired);
    }
    if !output.status.success() {
        return Err(ProvisionError::Exit {
            program: rustdesk.to_owned(),
            status: output.status.code(),
            message,
        });
    }
    Ok(())
}

#[cfg(windows)]
fn run_checked(launch: &Launch) -> Result<(), ProvisionError> {
    let output = Command::new(&launch.program)
        .args(&launch.args)
        .output()
        .map_err(|source| ProvisionError::Start {
            program: launch.program.clone(),
            source,
        })?;
    let message = combined_output(&output);
    if message
        .to_ascii_lowercase()
        .contains("administrative privileges required")
    {
        return Err(ProvisionError::ElevationRequired);
    }
    if !output.status.success() {
        return Err(ProvisionError::Exit {
            program: launch.program.clone(),
            status: output.status.code(),
            message,
        });
    }
    Ok(())
}

#[cfg(windows)]
fn verify_windows_provisioning(rustdesk: &Path, sc: &Path) -> Result<(), ProvisionError> {
    if inspect_service_state(sc)? != ServiceState::Running {
        return Err(ProvisionError::Verification {
            message: "the RustDesk service is not running".to_owned(),
        });
    }
    for (option, expected) in [("direct-server", "Y"), ("whitelist", "100.64.0.0/10")] {
        let output = Command::new(rustdesk)
            .args(["--option", option])
            .output()
            .map_err(|source| ProvisionError::Start {
                program: rustdesk.to_owned(),
                source,
            })?;
        let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !output.status.success() || actual != expected {
            return Err(ProvisionError::Verification {
                message: format!("{option} expected {expected:?}, found {actual:?}"),
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn combined_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = format!("{} {}", stdout.trim(), stderr.trim())
        .trim()
        .chars()
        .take(500)
        .collect::<String>();
    if message.is_empty() {
        "(no output)".to_owned()
    } else {
        message
    }
}

fn execute_launch_with(
    launch: &Launch,
    state_path: &Path,
    device: &Device,
    spawn: impl FnOnce(&Launch) -> std::io::Result<()>,
) -> Result<LaunchOutcome, RustDeskError> {
    let mut warnings = Vec::new();
    if !device.online {
        warnings.push(format!(
            "{} is reported offline; attempting RustDesk launch anyway",
            device.display_name()
        ));
    }
    spawn(launch).map_err(|source| RustDeskError::Start {
        path: launch.program.clone(),
        source,
    })?;
    if let Err(error) = record_connection(state_path, &device.id) {
        warnings.push(format!(
            "RustDesk started, but last-connected state could not be saved: {error}"
        ));
    }
    Ok(LaunchOutcome { warnings })
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::model::{Action, Device};
    use crate::state::load_state;

    use super::{
        RustDeskError, ServiceState, build_launch, build_windows_provision_plan,
        execute_launch_with,
    };

    #[test]
    fn windows_provision_plan_is_idempotent_for_running_service() {
        let plan = build_windows_provision_plan(
            PathBuf::from("rustdesk.exe"),
            PathBuf::from("sc.exe"),
            ServiceState::Running,
        );

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].launch.args[0], "--option");
        assert_eq!(plan.steps[0].launch.args[1], "direct-server");
        assert_eq!(plan.steps[0].launch.args[2], "Y");
        assert_eq!(plan.steps[1].launch.args[1], "whitelist");
        assert_eq!(plan.steps[1].launch.args[2], "100.64.0.0/10");
        assert!(plan.steps.iter().all(|step| step.privileged));
        assert!(plan.steps.iter().all(|step| step.persistent));
        assert!(
            !plan
                .steps
                .iter()
                .flat_map(|step| &step.launch.args)
                .any(|argument| argument
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains("password"))
        );
    }

    #[test]
    fn windows_provision_plan_installs_or_starts_service_as_needed() {
        let missing = build_windows_provision_plan(
            PathBuf::from("rustdesk.exe"),
            PathBuf::from("sc.exe"),
            ServiceState::Missing,
        );
        let stopped = build_windows_provision_plan(
            PathBuf::from("rustdesk.exe"),
            PathBuf::from("sc.exe"),
            ServiceState::Stopped,
        );

        assert_eq!(missing.steps[0].launch.args, ["--install-service"]);
        assert_eq!(stopped.steps[0].launch.args, ["start", "RustDesk"]);
    }

    #[test]
    fn configured_rustdesk_path_is_used() {
        let temp = tempdir().unwrap();
        let executable = temp.path().join("custom rustdesk");
        std::fs::write(&executable, "").unwrap();
        let device = device(IpAddr::V4(Ipv4Addr::LOCALHOST), true);

        let launch = super::launch_spec(Some(&executable), Action::Connect, &device).unwrap();

        assert_eq!(launch.program, executable);
    }

    #[test]
    fn connect_launch_has_exact_separate_arguments_for_ipv4() {
        let device = device(IpAddr::V4(Ipv4Addr::new(100, 100, 0, 42)), true);
        let launch = build_launch(
            PathBuf::from("folder with spaces").join("rustdesk"),
            Action::Connect,
            &device,
        );

        assert_eq!(
            launch.program,
            PathBuf::from("folder with spaces").join("rustdesk")
        );
        assert_eq!(
            launch.args,
            [OsString::from("--connect"), OsString::from("100.100.0.42")]
        );
    }

    #[test]
    fn transfer_launch_has_exact_separate_arguments_for_ipv6() {
        let device = device(
            IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 42)),
            true,
        );
        let launch = build_launch(PathBuf::from("rustdesk"), Action::Transfer, &device);

        assert_eq!(
            launch.args,
            [
                OsString::from("--file-transfer"),
                OsString::from("fd7a:115c:a1e0::2a")
            ]
        );
        assert!(
            !launch
                .args
                .iter()
                .any(|arg| arg.to_string_lossy().contains(' '))
        );
    }

    #[test]
    fn successful_spawn_records_state_and_warns_for_offline_device() {
        let temp = tempdir().unwrap();
        let state_path = temp.path().join("state.json");
        let device = device(IpAddr::V4(Ipv4Addr::LOCALHOST), false);
        let launch = build_launch(PathBuf::from("rustdesk"), Action::Connect, &device);

        let outcome = execute_launch_with(&launch, &state_path, &device, |_| Ok(())).unwrap();
        let (state, warning) = load_state(&state_path);

        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("offline"));
        assert!(warning.is_none());
        assert!(state.last_connected.contains_key(&device.id));
    }

    #[test]
    fn failed_spawn_does_not_record_state() {
        let temp = tempdir().unwrap();
        let state_path = temp.path().join("state.json");
        let device = device(IpAddr::V4(Ipv4Addr::LOCALHOST), true);
        let launch = build_launch(PathBuf::from("missing"), Action::Connect, &device);

        let error = execute_launch_with(&launch, &state_path, &device, |_| {
            Err(io::Error::new(io::ErrorKind::NotFound, "missing"))
        })
        .unwrap_err();

        assert!(matches!(error, RustDeskError::Start { .. }));
        assert!(!state_path.exists());
    }

    #[test]
    fn state_failure_does_not_turn_successful_spawn_into_failure() {
        let temp = tempdir().unwrap();
        let blocking_file = temp.path().join("not-a-directory");
        std::fs::write(&blocking_file, "").unwrap();
        let state_path = blocking_file.join("state.json");
        let device = device(IpAddr::V4(Ipv4Addr::LOCALHOST), true);
        let launch = build_launch(PathBuf::from("rustdesk"), Action::Connect, &device);

        let outcome = execute_launch_with(&launch, &state_path, &device, |_| Ok(())).unwrap();

        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("could not be saved"));
    }

    #[test]
    fn arguments_contain_only_mode_and_ip() {
        let mut device = device(IpAddr::V4(Ipv4Addr::LOCALHOST), true);
        device.nickname = Some("safe & harmless; nickname".to_owned());
        let launch = build_launch(PathBuf::from("rustdesk"), Action::Connect, &device);

        assert_eq!(launch.args.len(), 2);
        assert_eq!(launch.args[0], OsStr::new("--connect"));
        assert_eq!(launch.args[1], OsStr::new("127.0.0.1"));
    }

    fn device(preferred_ip: IpAddr, online: bool) -> Device {
        Device {
            id: "node-test".to_owned(),
            hostname: "test-host".to_owned(),
            dns_name: Some("test-host.example.test".to_owned()),
            nickname: Some("test device".to_owned()),
            ips: vec![preferred_ip],
            preferred_ip,
            os: "linux".to_owned(),
            online,
            tags: vec!["safe".to_owned()],
            favorite: false,
            notes: Some("sanitized fixture".to_owned()),
            last_connected_at: None,
        }
    }
}
