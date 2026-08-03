use std::cmp::Ordering;

use portal_core::{Device, Snapshot};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::TableState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SortMode {
    Recent,
    Name,
    Status,
}

impl SortMode {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Recent => Self::Name,
            Self::Name => Self::Status,
            Self::Status => Self::Recent,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Name => "name",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageSource {
    Refresh,
    Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatusMessage {
    pub(crate) level: MessageLevel,
    pub(crate) text: String,
    pub(crate) source: MessageSource,
}

#[derive(Debug)]
pub(crate) struct App {
    pub(crate) devices: Vec<Device>,
    pub(crate) table_state: TableState,
    pub(crate) sort_mode: SortMode,
    pub(crate) initial_load_complete: bool,
    pub(crate) refresh_in_flight: bool,
    pub(crate) refresh_pending: bool,
    pub(crate) message: Option<StatusMessage>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            table_state: TableState::default(),
            sort_mode: SortMode::Recent,
            initial_load_complete: false,
            refresh_in_flight: false,
            refresh_pending: false,
            message: None,
        }
    }
}

impl App {
    pub(crate) fn selected_device(&self) -> Option<&Device> {
        self.table_state
            .selected()
            .and_then(|index| self.devices.get(index))
    }

    pub(crate) fn select_previous(&mut self) {
        let selected = match (self.table_state.selected(), self.devices.is_empty()) {
            (_, true) => None,
            (Some(index), false) => Some(index.saturating_sub(1)),
            (None, false) => Some(0),
        };
        self.table_state.select(selected);
    }

    pub(crate) fn select_next(&mut self) {
        let selected = match (self.table_state.selected(), self.devices.len()) {
            (_, 0) => None,
            (Some(index), len) => Some(index.saturating_add(1).min(len - 1)),
            (None, _) => Some(0),
        };
        self.table_state.select(selected);
    }

    pub(crate) fn cycle_sort(&mut self) {
        let selected_id = self.selected_device().map(|device| device.id.clone());
        self.sort_mode = self.sort_mode.next();
        sort_devices(&mut self.devices, self.sort_mode);
        self.restore_selection(selected_id.as_deref(), self.table_state.selected());
    }

    pub(crate) fn request_refresh(&mut self) -> bool {
        if self.refresh_in_flight {
            self.refresh_pending = true;
            false
        } else {
            self.refresh_in_flight = true;
            true
        }
    }

    pub(crate) fn finish_refresh(&mut self, result: Result<Snapshot, String>) -> bool {
        self.refresh_in_flight = false;
        self.initial_load_complete = true;

        match result {
            Ok(snapshot) => self.apply_snapshot(snapshot),
            Err(error) => {
                self.message = Some(StatusMessage {
                    level: MessageLevel::Error,
                    text: error,
                    source: MessageSource::Refresh,
                });
            }
        }

        if self.refresh_pending {
            self.refresh_pending = false;
            self.refresh_in_flight = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn set_action_message(&mut self, level: MessageLevel, text: impl Into<String>) {
        self.message = Some(StatusMessage {
            level,
            text: text.into(),
            source: MessageSource::Action,
        });
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let old_index = self.table_state.selected();
        let selected_id = self.selected_device().map(|device| device.id.clone());
        self.devices = snapshot.devices;
        sort_devices(&mut self.devices, self.sort_mode);
        self.restore_selection(selected_id.as_deref(), old_index);

        if snapshot.warnings.is_empty() {
            if self
                .message
                .as_ref()
                .is_some_and(|message| message.source == MessageSource::Refresh)
            {
                self.message = None;
            }
        } else {
            self.message = Some(StatusMessage {
                level: MessageLevel::Warning,
                text: snapshot.warnings.join("; "),
                source: MessageSource::Refresh,
            });
        }
    }

    fn restore_selection(&mut self, selected_id: Option<&str>, old_index: Option<usize>) {
        let selected = if self.devices.is_empty() {
            None
        } else if let Some(id) = selected_id {
            self.devices
                .iter()
                .position(|device| device.id == id)
                .or_else(|| old_index.map(|index| index.min(self.devices.len() - 1)))
                .or(Some(0))
        } else {
            Some(old_index.unwrap_or(0).min(self.devices.len() - 1))
        };
        self.table_state = TableState::default().with_selected(selected);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputCommand {
    Previous,
    Next,
    Connect,
    Transfer,
    Refresh,
    CycleSort,
    Quit,
}

pub(crate) fn input_command(event: KeyEvent) -> Option<InputCommand> {
    if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }

    match event.code {
        KeyCode::Up | KeyCode::Char('k') => Some(InputCommand::Previous),
        KeyCode::Down | KeyCode::Char('j') => Some(InputCommand::Next),
        KeyCode::Enter => Some(InputCommand::Connect),
        KeyCode::Char('s') => Some(InputCommand::Transfer),
        KeyCode::Char('r') => Some(InputCommand::Refresh),
        KeyCode::Tab => Some(InputCommand::CycleSort),
        KeyCode::Char('q') | KeyCode::Esc => Some(InputCommand::Quit),
        _ => None,
    }
}

pub(crate) fn format_last_connected(timestamp: Option<u64>, now: u64) -> String {
    let Some(timestamp) = timestamp else {
        return "never".to_owned();
    };
    let elapsed = now.saturating_sub(timestamp);
    if elapsed < 60 {
        "just now".to_owned()
    } else if elapsed < 60 * 60 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 24 * 60 * 60 {
        format!("{}h ago", elapsed / (60 * 60))
    } else {
        format!("{}d ago", elapsed / (24 * 60 * 60))
    }
}

fn sort_devices(devices: &mut [Device], mode: SortMode) {
    devices.sort_by(|left, right| match mode {
        SortMode::Recent => recent_order(left, right).then_with(|| name_order(left, right)),
        SortMode::Name => name_order(left, right),
        SortMode::Status => right
            .online
            .cmp(&left.online)
            .then_with(|| name_order(left, right)),
    });
}

fn recent_order(left: &Device, right: &Device) -> Ordering {
    match (left.last_connected_at, right.last_connected_at) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn name_order(left: &Device, right: &Device) -> Ordering {
    left.display_name()
        .to_lowercase()
        .cmp(&right.display_name().to_lowercase())
        .then_with(|| left.hostname.cmp(&right.hostname))
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use portal_core::{Device, Snapshot};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::{
        App, InputCommand, MessageLevel, MessageSource, SortMode, format_last_connected,
        input_command,
    };

    #[test]
    fn recent_sort_uses_timestamp_then_deterministic_names() {
        let mut app = App::default();
        app.finish_refresh(Ok(snapshot(vec![
            device("never", "Zulu", true, None),
            device("old", "beta", false, Some(10)),
            device("new-b", "Alpha", true, Some(20)),
            device("new-a", "alpha", true, Some(20)),
        ])));

        assert_eq!(ids(&app), ["new-b", "new-a", "old", "never"]);
    }

    #[test]
    fn name_and_status_sort_are_deterministic() {
        let mut app = app_with(vec![
            device("offline-b", "Beta", false, None),
            device("online-z", "zeta", true, None),
            device("offline-a", "alpha", false, None),
        ]);

        app.cycle_sort();
        assert_eq!(app.sort_mode, SortMode::Name);
        assert_eq!(app.sort_mode.label(), "name");
        assert_eq!(ids(&app), ["offline-a", "offline-b", "online-z"]);

        app.cycle_sort();
        assert_eq!(app.sort_mode, SortMode::Status);
        assert_eq!(ids(&app), ["online-z", "offline-a", "offline-b"]);

        app.cycle_sort();
        assert_eq!(app.sort_mode, SortMode::Recent);
    }

    #[test]
    fn stable_id_selection_survives_reorder_and_rename() {
        let mut app = app_with(vec![
            device("one", "alpha", true, Some(1)),
            device("two", "beta", true, Some(2)),
        ]);
        app.table_state.select(Some(1));
        assert_eq!(app.selected_device().unwrap().id, "one");

        let mut renamed = device("one", "alpha", true, Some(3));
        renamed.nickname = Some("primary".to_owned());
        app.finish_refresh(Ok(snapshot(vec![
            device("two", "beta", true, Some(2)),
            renamed,
        ])));

        assert_eq!(app.selected_device().unwrap().id, "one");
    }

    #[test]
    fn removed_selection_clamps_and_empty_snapshot_clears() {
        let mut app = app_with(vec![
            device("one", "a", true, None),
            device("two", "b", true, None),
            device("three", "c", true, None),
        ]);
        app.table_state.select(Some(2));

        app.finish_refresh(Ok(snapshot(vec![device("one", "a", true, None)])));
        assert_eq!(app.selected_device().unwrap().id, "one");

        app.finish_refresh(Ok(snapshot(Vec::new())));
        assert!(app.selected_device().is_none());
    }

    #[test]
    fn navigation_clamps_without_wrapping() {
        let mut app = app_with(vec![
            device("one", "a", true, None),
            device("two", "b", true, None),
        ]);

        app.select_previous();
        assert_eq!(app.table_state.selected(), Some(0));
        app.select_next();
        app.select_next();
        assert_eq!(app.table_state.selected(), Some(1));

        let mut empty = App::default();
        empty.select_previous();
        empty.select_next();
        assert_eq!(empty.table_state.selected(), None);
    }

    #[test]
    fn refresh_requests_coalesce_and_errors_preserve_devices() {
        let mut app = app_with(vec![device("one", "a", true, None)]);

        assert!(app.request_refresh());
        assert!(!app.request_refresh());
        assert!(app.refresh_pending);
        assert!(app.finish_refresh(Err("temporarily unavailable".to_owned())));
        assert_eq!(app.devices.len(), 1);
        assert_eq!(app.message.as_ref().unwrap().source, MessageSource::Refresh);
        assert!(app.refresh_in_flight);
    }

    #[test]
    fn refresh_success_clears_only_refresh_messages() {
        let mut app = app_with(Vec::new());
        app.finish_refresh(Err("bad status".to_owned()));
        app.finish_refresh(Ok(snapshot(Vec::new())));
        assert!(app.message.is_none());

        app.set_action_message(MessageLevel::Info, "opened alpha");
        app.finish_refresh(Ok(snapshot(Vec::new())));
        assert_eq!(app.message.as_ref().unwrap().text, "opened alpha");
    }

    #[test]
    fn key_mapping_accepts_press_and_repeat_but_not_release() {
        assert_eq!(
            command(KeyCode::Up, KeyEventKind::Press),
            Some(InputCommand::Previous)
        );
        assert_eq!(
            command(KeyCode::Char('j'), KeyEventKind::Repeat),
            Some(InputCommand::Next)
        );
        assert_eq!(
            command(KeyCode::Enter, KeyEventKind::Press),
            Some(InputCommand::Connect)
        );
        assert_eq!(
            command(KeyCode::Char('s'), KeyEventKind::Press),
            Some(InputCommand::Transfer)
        );
        assert_eq!(
            command(KeyCode::Char('r'), KeyEventKind::Press),
            Some(InputCommand::Refresh)
        );
        assert_eq!(
            command(KeyCode::Tab, KeyEventKind::Press),
            Some(InputCommand::CycleSort)
        );
        assert_eq!(
            command(KeyCode::Esc, KeyEventKind::Press),
            Some(InputCommand::Quit)
        );
        assert_eq!(command(KeyCode::Down, KeyEventKind::Release), None);
        assert_eq!(command(KeyCode::Char('x'), KeyEventKind::Press), None);
    }

    #[test]
    fn relative_time_labels_cover_boundaries_and_future_values() {
        assert_eq!(format_last_connected(None, 100), "never");
        assert_eq!(format_last_connected(Some(100), 100), "just now");
        assert_eq!(format_last_connected(Some(110), 100), "just now");
        assert_eq!(format_last_connected(Some(40), 100), "1m ago");
        assert_eq!(format_last_connected(Some(100), 3_700), "1h ago");
        assert_eq!(format_last_connected(Some(100), 86_500), "1d ago");
    }

    fn command(code: KeyCode, kind: KeyEventKind) -> Option<InputCommand> {
        input_command(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind))
    }

    fn app_with(devices: Vec<Device>) -> App {
        let mut app = App::default();
        app.finish_refresh(Ok(snapshot(devices)));
        app
    }

    fn snapshot(devices: Vec<Device>) -> Snapshot {
        Snapshot {
            devices,
            warnings: Vec::new(),
        }
    }

    fn ids(app: &App) -> Vec<&str> {
        app.devices
            .iter()
            .map(|device| device.id.as_str())
            .collect()
    }

    fn device(id: &str, name: &str, online: bool, last_connected_at: Option<u64>) -> Device {
        Device {
            id: id.to_owned(),
            hostname: name.to_owned(),
            dns_name: Some(format!("{name}.example.test")),
            nickname: None,
            ips: vec![IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1))],
            preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1)),
            os: "linux".to_owned(),
            online,
            tags: Vec::new(),
            favorite: false,
            notes: None,
            last_connected_at,
        }
    }
}
