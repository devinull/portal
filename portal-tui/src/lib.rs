//! Interactive terminal frontend for Portal.

mod app;
mod ui;

use std::io;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use app::{
    App, InputCommand, MessageLevel, ThemePickerCommand, input_command, theme_picker_command,
};
use portal_core::{Action, Portal, Snapshot};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const EVENT_POLL_MAX: Duration = Duration::from_millis(100);

/// Runs Portal's interactive terminal interface.
pub fn run(portal: Portal) -> io::Result<()> {
    let portal = Arc::new(portal);
    let configured_theme = portal.theme();
    let (request_tx, request_rx) = mpsc::sync_channel(1);
    let (response_tx, response_rx) = mpsc::channel();
    let worker_portal = Arc::clone(&portal);
    let worker = thread::Builder::new()
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

    let mut app = App::new(configured_theme);
    request_refresh(&mut app, &request_tx);
    let result =
        ratatui::run(|terminal| run_loop(terminal, &portal, &request_tx, &response_rx, &mut app));

    drop(request_tx);
    let _ = worker.join();
    result
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    portal: &Portal,
    request_tx: &SyncSender<()>,
    response_rx: &Receiver<Result<Snapshot, String>>,
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

        if Instant::now() >= next_refresh {
            request_refresh(app, request_tx);
            next_refresh = Instant::now() + REFRESH_INTERVAL;
            needs_redraw = true;
        }

        if needs_redraw {
            let now = unix_seconds();
            let theme = app.visible_theme();
            terminal.draw(|frame| ui::render(frame, app, theme, now))?;
            needs_redraw = false;
        }

        if event::poll(EVENT_POLL_MAX)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.theme_picker_open() {
                        if let Some(command) = theme_picker_command(key) {
                            needs_redraw = true;
                            match command {
                                ThemePickerCommand::Previous => app.select_previous_theme(),
                                ThemePickerCommand::Next => app.select_next_theme(),
                                ThemePickerCommand::Apply => app.apply_theme_picker(),
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
                            InputCommand::Refresh => request_refresh(app, request_tx),
                            InputCommand::CycleSort => app.cycle_sort(),
                            InputCommand::OpenThemePicker => app.open_theme_picker(),
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

fn launch_selected(app: &mut App, portal: &Portal, action: Action, request_tx: &SyncSender<()>) {
    let Some(device) = app.selected_device().cloned() else {
        app.set_action_message(MessageLevel::Warning, "No device selected");
        return;
    };

    match portal.launch_with_warnings(action, &device) {
        Ok(outcome) => {
            if outcome.warnings.is_empty() {
                let operation = match action {
                    Action::Connect => "Opened desktop for",
                    Action::Transfer => "Opened file transfer for",
                };
                app.set_action_message(
                    MessageLevel::Info,
                    format!("{operation} {}", device.display_name()),
                );
            } else {
                app.set_action_message(MessageLevel::Warning, outcome.warnings.join("; "));
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
