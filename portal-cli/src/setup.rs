use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use portal_core::{DependencyStatus, Portal};

#[derive(Clone, Copy)]
enum Dependency {
    Tailscale,
    RustDesk,
}

impl Dependency {
    const fn name(self) -> &'static str {
        match self {
            Self::Tailscale => "Tailscale",
            Self::RustDesk => "RustDesk",
        }
    }

    const fn url(self) -> &'static str {
        match self {
            Self::Tailscale => "https://tailscale.com/download",
            Self::RustDesk => "https://github.com/rustdesk/rustdesk/releases/latest",
        }
    }
}

struct InstallAction {
    label: String,
    program: OsString,
    args: Vec<OsString>,
}

enum Choice {
    Install(InstallAction),
    OpenPage,
    Skip,
}

pub(super) fn run(portal: &Portal, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let status = portal.dependency_status()?;
    println!("Portal dependency setup\n");
    println!(
        "{} Tailscale\n{} RustDesk",
        marker(status.tailscale),
        marker(status.rustdesk)
    );
    for dependency in missing_dependencies(status) {
        let choices = choices_for(dependency);
        println!("\n{} is not installed.", dependency.name());
        for (index, choice) in choices.iter().enumerate() {
            println!("{}. {}", index + 1, choice_label(choice, dependency));
        }
        if dry_run {
            continue;
        }
        let selected = prompt_choice(choices.len())?;
        match &choices[selected] {
            Choice::Install(action) => execute_install(action)?,
            Choice::OpenPage => open_page(dependency.url())?,
            Choice::Skip => println!("Skipped {}.", dependency.name()),
        }
    }
    if status.tailscale && status.rustdesk {
        println!("\nPortal dependencies are installed.");
    } else if dry_run {
        println!("\nDry run only; no installer or browser was launched.");
    }
    Ok(())
}

fn marker(installed: bool) -> &'static str {
    if installed { "✓" } else { "×" }
}

fn missing_dependencies(status: DependencyStatus) -> Vec<Dependency> {
    let mut dependencies = Vec::new();
    if !status.tailscale {
        dependencies.push(Dependency::Tailscale);
    }
    if !status.rustdesk {
        dependencies.push(Dependency::RustDesk);
    }
    dependencies
}

fn choices_for(dependency: Dependency) -> Vec<Choice> {
    let mut choices = Vec::new();
    if let Some(action) = install_action(dependency) {
        choices.push(Choice::Install(action));
    }
    choices.push(Choice::OpenPage);
    choices.push(Choice::Skip);
    choices
}

fn install_action(dependency: Dependency) -> Option<InstallAction> {
    if cfg!(windows) {
        let package = match dependency {
            Dependency::Tailscale => "Tailscale.Tailscale",
            Dependency::RustDesk => "RustDesk.RustDesk",
        };
        if command_succeeds(
            "winget",
            &[
                "show",
                "--id",
                package,
                "--exact",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        ) {
            return Some(InstallAction {
                label: "Install with WinGet".to_owned(),
                program: "winget".into(),
                args: [
                    "install",
                    "--id",
                    package,
                    "--exact",
                    "--source",
                    "winget",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                    "--disable-interactivity",
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
            });
        }
    } else if cfg!(target_os = "macos") && command_succeeds("brew", &["--version"]) {
        let cask = match dependency {
            Dependency::Tailscale => "tailscale-app",
            Dependency::RustDesk => "rustdesk",
        };
        return Some(InstallAction {
            label: "Install with Homebrew".to_owned(),
            program: "brew".into(),
            args: ["install", "--cask", cask]
                .into_iter()
                .map(OsString::from)
                .collect(),
        });
    } else if cfg!(target_os = "linux") {
        match dependency {
            Dependency::Tailscale => {
                if apt_package_available("tailscale") {
                    return Some(InstallAction {
                        label: "Install with APT".to_owned(),
                        program: "sudo".into(),
                        args: ["apt-get", "install", "tailscale"]
                            .into_iter()
                            .map(OsString::from)
                            .collect(),
                    });
                }
                if command_succeeds("dnf", &["info", "tailscale"]) {
                    return Some(InstallAction {
                        label: "Install with DNF".to_owned(),
                        program: "sudo".into(),
                        args: ["dnf", "install", "tailscale"]
                            .into_iter()
                            .map(OsString::from)
                            .collect(),
                    });
                }
            }
            Dependency::RustDesk => {
                if command_succeeds(
                    "flatpak",
                    &["remote-info", "flathub", "com.rustdesk.RustDesk"],
                ) {
                    return Some(InstallAction {
                        label: "Install from Flathub".to_owned(),
                        program: "flatpak".into(),
                        args: ["install", "--user", "flathub", "com.rustdesk.RustDesk"]
                            .into_iter()
                            .map(OsString::from)
                            .collect(),
                    });
                }
            }
        }
    }
    None
}

fn apt_package_available(package: &str) -> bool {
    let Ok(output) = Command::new("apt-cache")
        .args(["policy", package])
        .stderr(Stdio::null())
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.trim_start().starts_with("Candidate:") && !line.contains("(none)"))
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn choice_label(choice: &Choice, dependency: Dependency) -> String {
    match choice {
        Choice::Install(action) => action.label.clone(),
        Choice::OpenPage => format!("Open the official {} download page", dependency.name()),
        Choice::Skip => "Skip for now".to_owned(),
    }
}

fn prompt_choice(count: usize) -> io::Result<usize> {
    loop {
        print!("Choose [1-{count}]: ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if let Ok(selected) = answer.trim().parse::<usize>()
            && (1..=count).contains(&selected)
        {
            return Ok(selected - 1);
        }
        println!("Enter a number from 1 to {count}.");
    }
}

fn execute_install(action: &InstallAction) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running: {:?} {:?}", action.program, action.args);
    let status = Command::new(&action.program).args(&action.args).status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "installer exited with {}",
            status
                .code()
                .map_or_else(|| "no status".to_owned(), |code| code.to_string())
        ))
        .into());
    }
    Ok(())
}

fn open_page(url: &str) -> io::Result<()> {
    let (program, args): (&str, Vec<&str>) = if cfg!(windows) {
        ("explorer.exe", vec![url])
    } else if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else {
        ("xdg-open", vec![url])
    };
    Command::new(program).args(args).spawn().map(|_| ())
}
