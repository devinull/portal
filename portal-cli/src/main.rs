use std::error::Error;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use portal_core::{Action, Device, Launch, Portal, ProvisionPlan, resolve};

#[derive(Debug, Parser)]
#[command(
    name = "portal",
    version,
    about = "Reach Tailscale devices through RustDesk"
)]
struct Cli {
    /// Read address-book configuration from this path.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Resolve and display the launch argument vector without starting RustDesk.
    #[arg(long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<Command>,

    /// Device identifier; shorthand for `portal connect <name>`.
    #[arg(value_name = "NAME")]
    target: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List discovered devices.
    List {
        /// Emit only scriptable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Start a remote desktop session.
    Connect { target: String },
    /// Open RustDesk file transfer.
    #[command(alias = "transfer")]
    Share { target: String },
    /// Prepare RustDesk provisioning.
    #[command(alias = "provision")]
    Init { target: String },
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("portal: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let (action, target) = match (cli.command, cli.target) {
        (Some(Command::List { json }), None) => {
            let portal = Portal::load(cli.config.as_deref())?;
            let snapshot = portal.snapshot_with_warnings()?;
            for warning in snapshot.warnings {
                eprintln!("warning: {warning}");
            }
            if json {
                println!("{}", render_json(&snapshot.devices)?);
            } else {
                print!("{}", render_human(&snapshot.devices));
            }
            return Ok(());
        }
        (Some(Command::Connect { target }), None) | (None, Some(target)) => {
            (Action::Connect, target)
        }
        (Some(Command::Share { target }), None) => (Action::Transfer, target),
        (Some(Command::Init { target }), None) => {
            let portal = Portal::load(cli.config.as_deref())?;
            let snapshot = portal.snapshot_with_warnings()?;
            for warning in snapshot.warnings {
                eprintln!("warning: {warning}");
            }
            let device = resolve(&snapshot.devices, &target)?;
            let plan = portal.local_provision_plan()?;
            println!(
                "Preparing this machine for connections from {} and other tailnet devices.",
                device.display_name()
            );
            print!("{}", render_provision_plan(&plan));
            if cli.dry_run {
                return Ok(());
            }
            if !confirm_provisioning()? {
                println!("Provisioning cancelled; no changes were made.");
                return Ok(());
            }
            let outcome = portal.apply_local_provision_plan(&plan)?;
            if outcome.verified {
                println!("RustDesk service, direct access, and tailnet whitelist verified.");
            }
            return Ok(());
        }
        (None, None) => {
            if cli.dry_run {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--dry-run requires an action command or device target",
                )
                .into());
            }
            let portal = Portal::load(cli.config.as_deref())?;
            portal_tui::run(portal)?;
            return Ok(());
        }
        (Some(_), Some(_)) => unreachable!("Clap prevents a subcommand and shorthand target"),
    };

    let portal = Portal::load(cli.config.as_deref())?;
    let snapshot = portal.snapshot_with_warnings()?;
    for warning in snapshot.warnings {
        eprintln!("warning: {warning}");
    }
    let device = resolve(&snapshot.devices, &target)?;
    if cli.dry_run {
        print!("{}", render_launch(&portal.launch_spec(action, device)?));
        return Ok(());
    }
    let outcome = portal.launch_with_warnings(action, device)?;
    for warning in outcome.warnings {
        eprintln!("warning: {warning}");
    }
    Ok(())
}

fn render_json(devices: &[Device]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(devices)
}

fn render_human(devices: &[Device]) -> String {
    if devices.is_empty() {
        return "No Tailscale peers found.\n".to_owned();
    }

    let rows: Vec<[String; 4]> = devices
        .iter()
        .map(|device| {
            [
                device.display_name().to_owned(),
                device.hostname.clone(),
                device.os.clone(),
                device.preferred_ip.to_string(),
            ]
        })
        .collect();
    let headers = ["NAME", "HOSTNAME", "OS", "IP"];
    let mut widths = headers.map(str::len);
    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut output = String::new();
    write_row(&mut output, headers, widths);
    for row in rows {
        write_row(&mut output, row, widths);
    }
    output
}

fn render_launch(launch: &Launch) -> String {
    let mut output = String::from("Diagnostic argument vector (not a shell command):\n");
    writeln!(output, "program: {:?}", launch.program.as_os_str())
        .expect("writing to a String cannot fail");
    for (index, argument) in launch.args.iter().enumerate() {
        writeln!(output, "arg[{index}]: {argument:?}").expect("writing to a String cannot fail");
    }
    output
}

fn render_provision_plan(plan: &ProvisionPlan) -> String {
    let mut output = format!(
        "Local provisioning preview ({}, service: {:?}):\n",
        plan.platform, plan.service_state
    );
    for (index, step) in plan.steps.iter().enumerate() {
        writeln!(
            output,
            "{}. {} [administrator, persistent]",
            index + 1,
            step.description
        )
        .expect("writing to a String cannot fail");
        writeln!(output, "   program: {:?}", step.launch.program.as_os_str())
            .expect("writing to a String cannot fail");
        for (argument_index, argument) in step.launch.args.iter().enumerate() {
            writeln!(output, "   arg[{argument_index}]: {argument:?}")
                .expect("writing to a String cannot fail");
        }
    }
    output
}

fn confirm_provisioning() -> Result<bool, io::Error> {
    print!("Apply these privileged changes to the current machine? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn write_row(
    output: &mut String,
    values: impl IntoIterator<Item = impl AsRef<str>>,
    widths: [usize; 4],
) {
    let values: Vec<_> = values.into_iter().collect();
    writeln!(
        output,
        "{:<name_width$}  {:<hostname_width$}  {:<os_width$}  {}",
        values[0].as_ref(),
        values[1].as_ref(),
        values[2].as_ref(),
        values[3].as_ref(),
        name_width = widths[0],
        hostname_width = widths[1],
        os_width = widths[2],
    )
    .expect("writing to a String cannot fail");
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;

    use clap::{CommandFactory, Parser};
    use portal_core::{Device, Launch, ProvisionPlan, ProvisionStep, ServiceState};

    use super::{
        Cli, Command, render_human, render_json, render_launch, render_provision_plan, run,
    };

    #[test]
    fn parses_bare_invocation() {
        let cli = Cli::try_parse_from(["portal"]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.target.is_none());
    }

    #[test]
    fn rejects_bare_dry_run_before_starting_the_tui() {
        let cli = Cli::try_parse_from(["portal", "--dry-run"]).unwrap();

        let error = run(cli).unwrap_err();

        assert!(error.to_string().contains("requires an action"));
    }

    #[test]
    fn parses_shorthand_and_explicit_connect() {
        let shorthand = Cli::try_parse_from(["portal", "rig"]).unwrap();
        assert_eq!(shorthand.target.as_deref(), Some("rig"));

        let explicit = Cli::try_parse_from(["portal", "connect", "rig"]).unwrap();
        assert!(matches!(
            explicit.command,
            Some(Command::Connect { target }) if target == "rig"
        ));
    }

    #[test]
    fn parses_share_and_transfer_alias() {
        for spelling in ["share", "transfer"] {
            let cli = Cli::try_parse_from(["portal", spelling, "rig"]).unwrap();
            assert!(matches!(
                cli.command,
                Some(Command::Share { target }) if target == "rig"
            ));
        }
    }

    #[test]
    fn parses_init_and_provision_alias() {
        for spelling in ["init", "provision"] {
            let cli = Cli::try_parse_from(["portal", spelling, "rig"]).unwrap();
            assert!(matches!(
                cli.command,
                Some(Command::Init { target }) if target == "rig"
            ));
        }
    }

    #[test]
    fn parses_list_json_and_global_options() {
        let cli = Cli::try_parse_from([
            "portal",
            "--config",
            "custom.toml",
            "list",
            "--json",
            "--dry-run",
        ])
        .unwrap();

        assert_eq!(cli.config.unwrap(), PathBuf::from("custom.toml"));
        assert!(cli.dry_run);
        assert!(matches!(cli.command, Some(Command::List { json: true })));
    }

    #[test]
    fn rejects_missing_targets_and_unknown_flags() {
        assert!(Cli::try_parse_from(["portal", "connect"]).is_err());
        assert!(Cli::try_parse_from(["portal", "--unknown"]).is_err());
    }

    #[test]
    fn reserved_command_names_are_not_shorthand_targets() {
        assert!(Cli::try_parse_from(["portal", "list"]).is_ok());
        assert!(Cli::try_parse_from(["portal", "share"]).is_err());
    }

    #[test]
    fn command_definition_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn renders_human_list_with_and_without_nicknames() {
        let mut devices = vec![device("alpha", true), device("beta", false)];
        devices[0].nickname = Some("rig".to_owned());

        let output = render_human(&devices);

        assert!(output.starts_with("NAME"));
        assert!(!output.contains("STATUS"));
        assert!(!output.contains("online"));
        assert!(output.contains("rig"));
        assert!(output.contains("alpha"));
        assert!(output.contains("beta"));
    }

    #[test]
    fn renders_useful_empty_list() {
        assert_eq!(render_human(&[]), "No Tailscale peers found.\n");
    }

    #[test]
    fn json_render_is_valid_and_uses_stable_fields() {
        let output = render_json(&[device("alpha", true)]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value[0]["hostname"], "alpha");
        assert_eq!(value[0]["online"], true);
        assert!(value[0].get("preferred_ip").is_some());
        assert!(value[0].get("last_connected_at").is_some());
        assert!(value[0].get("config_path").is_none());
    }

    #[test]
    fn dry_run_render_shows_separate_program_and_arguments() {
        let launch = Launch {
            program: PathBuf::from("folder with spaces").join("rustdesk"),
            args: vec!["--connect".into(), "100.100.0.1".into()],
        };

        let output = render_launch(&launch);

        assert!(output.starts_with("Diagnostic argument vector"));
        assert!(output.contains("program:"));
        assert!(output.contains("arg[0]: \"--connect\""));
        assert!(output.contains("arg[1]: \"100.100.0.1\""));
        assert!(!output.contains("rustdesk --connect"));
    }

    #[test]
    fn provisioning_preview_is_explicit_and_contains_no_password() {
        let plan = ProvisionPlan {
            platform: "windows".to_owned(),
            service_state: ServiceState::Running,
            steps: vec![ProvisionStep {
                description: "Enable direct IP access".to_owned(),
                launch: Launch {
                    program: PathBuf::from("rustdesk.exe"),
                    args: vec!["--option".into(), "direct-server".into(), "Y".into()],
                },
                privileged: true,
                persistent: true,
            }],
        };

        let output = render_provision_plan(&plan);

        assert!(output.contains("administrator, persistent"));
        assert!(output.contains("arg[0]: \"--option\""));
        assert!(!output.to_ascii_lowercase().contains("password"));
    }

    fn device(hostname: &str, online: bool) -> Device {
        Device {
            id: format!("node-{hostname}"),
            machine_name: hostname.to_owned(),
            hostname: hostname.to_owned(),
            dns_name: Some(format!("{hostname}.example.test")),
            nickname: None,
            ips: vec![IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1))],
            preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1)),
            os: "linux".to_owned(),
            online,
            tags: Vec::new(),
            favorite: false,
            notes: None,
            last_connected_at: None,
        }
    }
}
