//! Interactive terminal frontend for Portal.

mod app;
mod ui;

use std::io;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use app::{
    App, AvailabilityState, ConfigEditorCommand, InputCommand, MessageLevel, ProbeTarget,
    ThemePickerCommand, config_editor_command, input_command, theme_picker_command,
};
use portal_core::{Action, Device, Portal, ProbeMeasurement, Snapshot};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const EVENT_POLL_MAX: Duration = Duration::from_millis(100);

struct ProbeRequest {
    target: ProbeTarget,
    device: Device,
}

struct ProbeResponse {
    target: ProbeTarget,
    result: Result<ProbeMeasurement, String>,
}

/// Runs Portal's interactive terminal interface.
pub fn run(portal: Portal) -> io::Result<()> {
    let portal = Arc::new(portal);
    let configured_theme = portal.theme();
    let (request_tx, request_rx) = mpsc::sync_channel(1);
    let (response_tx, response_rx) = mpsc::channel();
    let worker_portal = Arc::clone(&portal);
    let discovery_worker = thread::Builder::new()
        .name("portal-discovery".to_owned())
        .spawn(move || {
            while request_rx.recv().is_ok() {
                let result = worker_portal
                    .snapshot_with_warnings()
                    .map_err(|error| error.to_string());
                if response_tx.send(result).is_err() {
                    break;
                }
            }
        })?;

    let (probe_request_tx, probe_request_rx) = mpsc::sync_channel::<ProbeRequest>(1);
    let (probe_response_tx, probe_response_rx) = mpsc::channel::<ProbeResponse>();
    let probe_portal = Arc::clone(&portal);
    let probe_worker = thread::Builder::new()
        .name("portal-probe".to_owned())
        .spawn(move || {
            while let Ok(request) = probe_request_rx.recv() {
                let result = probe_portal
                    .probe(&request.device)
                    .map_err(|error| error.to_string());
                if probe_response_tx
                    .send(ProbeResponse {
                        target: request.target,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })?;

    let mut app = App::new(configured_theme);
    request_refresh(&mut app, &request_tx);
    let result = ratatui::run(|terminal| {
        run_loop(
            terminal,
            &portal,
            &request_tx,
            &response_rx,
            &probe_request_tx,
            &probe_response_rx,
            &mut app,
        )
    });

    drop(request_tx);
    drop(probe_request_tx);
    let _ = discovery_worker.join();
    let _ = probe_worker.join();
    result
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    portal: &Portal,
    request_tx: &SyncSender<()>,
    response_rx: &Receiver<Result<Snapshot, String>>,
    probe_request_tx: &SyncSender<ProbeRequest>,
    probe_response_rx: &Receiver<ProbeResponse>,
    app: &mut App,
) -> io::Result<()> {
    let mut needs_redraw = true;
    let mut next_refresh = Instant::now() + REFRESH_INTERVAL;

    loop {
        loop {
            match response_rx.try_recv() {
                Ok(result) => {
                    let follow_up = app.finish_refresh(result);
                    next_refresh = Instant::now() + REFRESH_INTERVAL;
                    if follow_up {
                        send_refresh(app, request_tx);
                    }
                    needs_redraw = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if app.refresh_in_flight {
                        app.refresh_pending = false;
                        let _ = app.finish_refresh(Err("discovery worker stopped".to_owned()));
                        needs_redraw = true;
                    }
                    break;
                }
            }
        }

        loop {
            match probe_response_rx.try_recv() {
                Ok(response) => {
                    if app.finish_probe(&response.target, response.result, Instant::now()) {
                        needs_redraw = true;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if app.probe_in_flight.is_some() {
                        needs_redraw |= app.probe_send_failed();
                    }
                    break;
                }
            }
        }

        if Instant::now() >= next_refresh {
            request_refresh(app, request_tx);
            next_refresh = Instant::now() + REFRESH_INTERVAL;
            needs_redraw = true;
        }

        needs_redraw |= start_due_probe(app, probe_request_tx, Instant::now());

        if needs_redraw {
            let now = unix_seconds();
            let theme = app.visible_theme();
            terminal.draw(|frame| ui::render(frame, app, theme, now))?;
            needs_redraw = false;
        }

        if event::poll(EVENT_POLL_MAX)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.config_editor_open() {
                        if let Some(command) = config_editor_command(key) {
                            needs_redraw = true;
                            match command {
                                ConfigEditorCommand::PreviousField => {
                                    app.select_previous_config_field()
                                }
                                ConfigEditorCommand::NextField => app.select_next_config_field(),
                                ConfigEditorCommand::CursorLeft => app.move_config_cursor_left(),
                                ConfigEditorCommand::CursorRight => app.move_config_cursor_right(),
                                ConfigEditorCommand::CursorHome => app.move_config_cursor_home(),
                                ConfigEditorCommand::CursorEnd => app.move_config_cursor_end(),
                                ConfigEditorCommand::Insert(character) => {
                                    app.insert_config_character(character)
                                }
                                ConfigEditorCommand::Backspace => app.backspace_config_character(),
                                ConfigEditorCommand::Delete => app.delete_config_character(),
                                ConfigEditorCommand::Clear => app.clear_config_field(),
                                ConfigEditorCommand::Save => {
                                    save_device_config(app, portal, request_tx)
                                }
                                ConfigEditorCommand::Cancel => app.cancel_config_editor(),
                            }
                        }
                    } else if app.theme_picker_open() {
                        if let Some(command) = theme_picker_command(key) {
                            needs_redraw = true;
                            match command {
                                ThemePickerCommand::Previous => app.select_previous_theme(),
                                ThemePickerCommand::Next => app.select_next_theme(),
                                ThemePickerCommand::Apply => {
                                    if let Some(theme) = app.pending_theme() {
                                        match portal.set_theme(theme) {
                                            Ok(()) => app.apply_theme_picker(),
                                            Err(error) => {
                                                app.cancel_theme_picker();
                                                app.set_action_message(
                                                    MessageLevel::Error,
                                                    error.to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                ThemePickerCommand::Cancel => app.cancel_theme_picker(),
                            }
                        }
                    } else if let Some(command) = input_command(key) {
                        needs_redraw = true;
                        match command {
                            InputCommand::Previous => app.select_previous(),
                            InputCommand::Next => app.select_next(),
                            InputCommand::Connect => {
                                launch_selected(app, portal, Action::Connect, request_tx)
                            }
                            InputCommand::Transfer => {
                                launch_selected(app, portal, Action::Transfer, request_tx)
                            }
                            InputCommand::Refresh => {
                                app.mark_probes_due(Instant::now());
                                request_refresh(app, request_tx);
                            }
                            InputCommand::CycleSort => app.cycle_sort(),
                            InputCommand::OpenThemePicker => app.open_theme_picker(),
                            InputCommand::OpenConfigEditor => app.open_config_editor(),
                            InputCommand::Quit => return Ok(()),
                        }
                    }
                }
                Event::Resize(_, _) => needs_redraw = true,
                _ => {}
            }
        }
    }
}

fn save_device_config(app: &mut App, portal: &Portal, request_tx: &SyncSender<()>) {
    let Some((device_id, machine_name, hostname, config)) = app.pending_device_config() else {
        return;
    };
    match portal.update_device_config(&machine_name, &hostname, config.clone()) {
        Ok(()) => {
            app.commit_config_editor(&device_id, &config);
            request_refresh(app, request_tx);
        }
        Err(error) => app.set_action_message(MessageLevel::Error, error.to_string()),
    }
}

fn start_due_probe(app: &mut App, request_tx: &SyncSender<ProbeRequest>, now: Instant) -> bool {
    let Some(target) = app.start_next_probe(now) else {
        return false;
    };
    let Some(device) = app
        .devices
        .iter()
        .find(|device| device.id == target.id && device.preferred_ip == target.ip)
        .cloned()
    else {
        app.cancel_probe_start();
        return false;
    };

    match request_tx.try_send(ProbeRequest { target, device }) {
        Ok(()) => false,
        Err(TrySendError::Full(_)) => {
            app.cancel_probe_start();
            false
        }
        Err(TrySendError::Disconnected(_)) => app.probe_send_failed(),
    }
}

fn launch_selected(app: &mut App, portal: &Portal, action: Action, request_tx: &SyncSender<()>) {
    let Some(device) = app.selected_device().cloned() else {
        app.set_action_message(MessageLevel::Warning, "No device selected");
        return;
    };

    match portal.launch_with_warnings(action, &device) {
        Ok(outcome) => {
            let unavailable = app.availability_state(&device) == AvailabilityState::Unavailable;
            if outcome.warnings.is_empty() && !unavailable {
                let operation = match action {
                    Action::Connect => "Opened desktop for",
                    Action::Transfer => "Opened file transfer for",
                };
                app.set_action_message(
                    MessageLevel::Info,
                    format!("{operation} {}", device.display_name()),
                );
            } else {
                let mut warnings = Vec::new();
                if unavailable {
                    warnings
                        .push("No recent Tailscale response; opening RustDesk anyway".to_owned());
                }
                warnings.extend(outcome.warnings);
                app.set_action_message(MessageLevel::Warning, warnings.join("; "));
            }
            request_refresh(app, request_tx);
        }
        Err(error) => app.set_action_message(MessageLevel::Error, error.to_string()),
    }
}

fn request_refresh(app: &mut App, request_tx: &SyncSender<()>) {
    if app.request_refresh() {
        send_refresh(app, request_tx);
    }
}

fn send_refresh(app: &mut App, request_tx: &SyncSender<()>) {
    match request_tx.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => {}
        Err(TrySendError::Disconnected(())) => {
            app.refresh_pending = false;
            let _ = app.finish_refresh(Err("discovery worker stopped".to_owned()));
        }
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
