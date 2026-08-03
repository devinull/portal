use std::cmp::Ordering;
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use portal_core::{Device, LocalDevice, ProbeMeasurement, Snapshot, Theme};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::TableState;

pub(crate) const REACHABLE_PROBE_INTERVAL: Duration = Duration::from_secs(15);
pub(crate) const RETRY_PROBE_INTERVAL: Duration = Duration::from_secs(5);
pub(crate) const FAILURES_BEFORE_UNAVAILABLE: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AvailabilityState {
    Unknown,
    Reachable,
    Unavailable,
}

#[derive(Clone, Debug)]
pub(crate) struct Availability {
    pub(crate) state: AvailabilityState,
    pub(crate) latency: Option<Duration>,
    consecutive_failures: u8,
    next_probe_at: Instant,
    ip: IpAddr,
}

impl Availability {
    fn unknown(ip: IpAddr, now: Instant) -> Self {
        Self {
            state: AvailabilityState::Unknown,
            latency: None,
            consecutive_failures: 0,
            next_probe_at: now,
            ip,
        }
    }

    fn apply_result(&mut self, result: Result<ProbeMeasurement, String>, now: Instant) {
        match result {
            Ok(measurement) => {
                self.state = AvailabilityState::Reachable;
                self.latency = measurement.latency;
                self.consecutive_failures = 0;
                self.next_probe_at = now + REACHABLE_PROBE_INTERVAL;
            }
            Err(_) => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                self.latency = None;
                self.next_probe_at = now + RETRY_PROBE_INTERVAL;
                if self.consecutive_failures >= FAILURES_BEFORE_UNAVAILABLE {
                    self.state = AvailabilityState::Unavailable;
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProbeTarget {
    pub(crate) id: String,
    pub(crate) ip: IpAddr,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryHealth {
    Unknown,
    Healthy,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ThemeOption {
    pub(crate) name: &'static str,
    pub(crate) theme: Theme,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThemePicker {
    selected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatusMessage {
    pub(crate) level: MessageLevel,
    pub(crate) text: String,
    pub(crate) source: MessageSource,
}

#[derive(Debug)]
pub(crate) struct App {
    pub(crate) local_device: Option<LocalDevice>,
    pub(crate) devices: Vec<Device>,
    pub(crate) table_state: TableState,
    pub(crate) sort_mode: SortMode,
    pub(crate) initial_load_complete: bool,
    pub(crate) refresh_in_flight: bool,
    pub(crate) refresh_pending: bool,
    pub(crate) message: Option<StatusMessage>,
    pub(crate) discovery_health: DiscoveryHealth,
    pub(crate) availability: HashMap<String, Availability>,
    pub(crate) probe_in_flight: Option<ProbeTarget>,
    pub(crate) themes: Vec<ThemeOption>,
    probe_worker_disconnected_reported: bool,
    active_theme: usize,
    theme_picker: Option<ThemePicker>,
}

impl Default for App {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl App {
    pub(crate) fn new(configured_theme: Theme) -> Self {
        let configured_name = if configured_theme == Theme::portal() {
            "Portal"
        } else {
            "Configured"
        };
        Self {
            local_device: None,
            devices: Vec::new(),
            table_state: TableState::default(),
            sort_mode: SortMode::Recent,
            initial_load_complete: false,
            refresh_in_flight: false,
            refresh_pending: false,
            message: None,
            discovery_health: DiscoveryHealth::Unknown,
            availability: HashMap::new(),
            probe_in_flight: None,
            themes: vec![
                ThemeOption {
                    name: configured_name,
                    theme: configured_theme,
                },
                ThemeOption {
                    name: "Hacker",
                    theme: Theme::hacker(),
                },
                ThemeOption {
                    name: "Cyber Cyan",
                    theme: Theme::cyber_cyan(),
                },
            ],
            probe_worker_disconnected_reported: false,
            active_theme: 0,
            theme_picker: None,
        }
    }

    pub(crate) fn visible_theme(&self) -> Theme {
        self.themes[self
            .theme_picker
            .map_or(self.active_theme, |picker| picker.selected)]
        .theme
    }

    pub(crate) fn theme_picker_open(&self) -> bool {
        self.theme_picker.is_some()
    }

    pub(crate) fn theme_picker_selected(&self) -> Option<usize> {
        self.theme_picker.map(|picker| picker.selected)
    }

    pub(crate) fn open_theme_picker(&mut self) {
        self.theme_picker = Some(ThemePicker {
            selected: self.active_theme,
        });
    }

    pub(crate) fn select_previous_theme(&mut self) {
        if let Some(picker) = &mut self.theme_picker {
            picker.selected = picker.selected.saturating_sub(1);
        }
    }

    pub(crate) fn select_next_theme(&mut self) {
        if let Some(picker) = &mut self.theme_picker {
            picker.selected = picker.selected.saturating_add(1).min(self.themes.len() - 1);
        }
    }

    pub(crate) fn apply_theme_picker(&mut self) {
        if let Some(picker) = self.theme_picker.take() {
            self.active_theme = picker.selected;
            let name = self.themes[self.active_theme].name;
            self.set_action_message(MessageLevel::Info, format!("Applied {name} theme"));
        }
    }

    pub(crate) fn cancel_theme_picker(&mut self) {
        self.theme_picker = None;
    }
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
        sort_devices(&mut self.devices, self.sort_mode, &self.availability);
        self.restore_selection(selected_id.as_deref(), self.table_state.selected());
    }

    pub(crate) fn availability(&self, device: &Device) -> Option<&Availability> {
        self.availability.get(&device.id)
    }

    pub(crate) fn availability_state(&self, device: &Device) -> AvailabilityState {
        self.availability(device)
            .map_or(AvailabilityState::Unknown, |availability| {
                availability.state
            })
    }

    pub(crate) fn mark_probes_due(&mut self, now: Instant) {
        for availability in self.availability.values_mut() {
            availability.next_probe_at = now;
        }
    }

    pub(crate) fn start_next_probe(&mut self, now: Instant) -> Option<ProbeTarget> {
        if self.probe_in_flight.is_some() {
            return None;
        }

        let selected_id = self.selected_device().map(|device| device.id.as_str());
        let selected_due = selected_id.and_then(|id| {
            self.devices
                .iter()
                .find(|device| device.id == id)
                .and_then(|_device| {
                    self.availability
                        .get(id)
                        .filter(|availability| availability.next_probe_at <= now)
                        .map(|availability| ProbeTarget {
                            id: id.to_owned(),
                            ip: availability.ip,
                        })
                })
        });

        let target = selected_due.or_else(|| {
            self.devices
                .iter()
                .enumerate()
                .filter_map(|(index, device)| {
                    let availability = self.availability.get(&device.id)?;
                    (availability.next_probe_at <= now).then_some((
                        availability.next_probe_at,
                        index,
                        ProbeTarget {
                            id: device.id.clone(),
                            ip: availability.ip,
                        },
                    ))
                })
                .min_by_key(|(next_probe_at, index, _)| (*next_probe_at, *index))
                .map(|(_, _, target)| target)
        })?;

        self.probe_in_flight = Some(target.clone());
        Some(target)
    }

    pub(crate) fn probe_send_failed(&mut self) -> bool {
        self.probe_in_flight = None;
        if !self.probe_worker_disconnected_reported {
            self.probe_worker_disconnected_reported = true;
            self.set_action_message(MessageLevel::Warning, "Probe worker stopped");
            true
        } else {
            false
        }
    }

    pub(crate) fn cancel_probe_start(&mut self) {
        self.probe_in_flight = None;
    }

    pub(crate) fn finish_probe(
        &mut self,
        target: &ProbeTarget,
        result: Result<ProbeMeasurement, String>,
        now: Instant,
    ) -> bool {
        if self.probe_in_flight.as_ref() != Some(target) {
            return false;
        }
        self.probe_in_flight = None;

        let Some(device) = self
            .devices
            .iter()
            .find(|device| device.id == target.id && device.preferred_ip == target.ip)
        else {
            return false;
        };
        let Some(availability) = self.availability.get_mut(&device.id) else {
            return false;
        };
        if availability.ip != target.ip {
            return false;
        }
        availability.apply_result(result, now);

        if self.sort_mode == SortMode::Status {
            let selected_id = self.selected_device().map(|device| device.id.clone());
            let old_index = self.table_state.selected();
            sort_devices(&mut self.devices, self.sort_mode, &self.availability);
            self.restore_selection(selected_id.as_deref(), old_index);
        }
        true
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
        self.finish_refresh_at(result, Instant::now())
    }

    fn finish_refresh_at(&mut self, result: Result<Snapshot, String>, now: Instant) -> bool {
        self.refresh_in_flight = false;
        self.initial_load_complete = true;

        match result {
            Ok(snapshot) => {
                self.discovery_health = DiscoveryHealth::Healthy;
                self.apply_snapshot(snapshot, now);
            }
            Err(error) => {
                self.discovery_health = DiscoveryHealth::Unavailable;
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

    fn apply_snapshot(&mut self, snapshot: Snapshot, now: Instant) {
        let old_index = self.table_state.selected();
        let selected_id = self.selected_device().map(|device| device.id.clone());
        self.local_device = snapshot.local_device;
        self.devices = snapshot.devices;
        self.reconcile_availability(now);
        sort_devices(&mut self.devices, self.sort_mode, &self.availability);
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

    fn reconcile_availability(&mut self, now: Instant) {
        self.availability
            .retain(|id, _| self.devices.iter().any(|device| device.id == *id));
        for device in &self.devices {
            self.availability
                .entry(device.id.clone())
                .and_modify(|availability| {
                    if availability.ip != device.preferred_ip {
                        *availability = Availability::unknown(device.preferred_ip, now);
                    }
                })
                .or_insert_with(|| Availability::unknown(device.preferred_ip, now));
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
    OpenThemePicker,
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
        KeyCode::Char('t') => Some(InputCommand::OpenThemePicker),
        KeyCode::Char('q') | KeyCode::Esc => Some(InputCommand::Quit),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThemePickerCommand {
    Previous,
    Next,
    Apply,
    Cancel,
}

pub(crate) fn theme_picker_command(event: KeyEvent) -> Option<ThemePickerCommand> {
    if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }

    match event.code {
        KeyCode::Up | KeyCode::Char('k') => Some(ThemePickerCommand::Previous),
        KeyCode::Down | KeyCode::Char('j') => Some(ThemePickerCommand::Next),
        KeyCode::Enter => Some(ThemePickerCommand::Apply),
        KeyCode::Esc | KeyCode::Char('q' | 't') => Some(ThemePickerCommand::Cancel),
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

fn sort_devices(
    devices: &mut [Device],
    mode: SortMode,
    availability: &HashMap<String, Availability>,
) {
    devices.sort_by(|left, right| match mode {
        SortMode::Recent => recent_order(left, right).then_with(|| name_order(left, right)),
        SortMode::Name => name_order(left, right),
        SortMode::Status => availability_rank(availability.get(&left.id))
            .cmp(&availability_rank(availability.get(&right.id)))
            .then_with(|| name_order(left, right)),
    });
}

fn availability_rank(availability: Option<&Availability>) -> u8 {
    match availability.map(|availability| availability.state) {
        Some(AvailabilityState::Reachable) => 0,
        Some(AvailabilityState::Unavailable) => 2,
        Some(AvailabilityState::Unknown) | None => 1,
    }
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
    use std::time::{Duration, Instant};

    use portal_core::{Device, ProbeMeasurement, Snapshot, Theme};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::{
        App, AvailabilityState, DiscoveryHealth, InputCommand, MessageLevel, MessageSource,
        REACHABLE_PROBE_INTERVAL, RETRY_PROBE_INTERVAL, SortMode, ThemePickerCommand,
        format_last_connected, input_command, theme_picker_command,
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

        app.availability.get_mut("online-z").unwrap().state = AvailabilityState::Reachable;
        app.availability.get_mut("offline-b").unwrap().state = AvailabilityState::Unavailable;
        app.cycle_sort();
        assert_eq!(app.sort_mode, SortMode::Status);
        assert_eq!(ids(&app), ["online-z", "offline-a", "offline-b"]);

        app.cycle_sort();
        assert_eq!(app.sort_mode, SortMode::Recent);
    }

    #[test]
    fn probe_transitions_use_exact_hysteresis_and_intervals() {
        let base = Instant::now();
        let mut app = app_with_at(vec![device("one", "alpha", false, None)], base);

        let first = app.start_next_probe(base).unwrap();
        assert!(app.finish_probe(&first, Err("timeout".to_owned()), base));
        let availability = app.availability.get("one").unwrap();
        assert_eq!(availability.state, AvailabilityState::Unknown);
        assert_eq!(availability.latency, None);
        assert_eq!(availability.next_probe_at, base + RETRY_PROBE_INTERVAL);

        let second = app.start_next_probe(base + RETRY_PROBE_INTERVAL).unwrap();
        assert!(app.finish_probe(
            &second,
            Err("timeout".to_owned()),
            base + RETRY_PROBE_INTERVAL,
        ));
        assert_eq!(
            app.availability.get("one").unwrap().state,
            AvailabilityState::Unavailable
        );

        let recovery_at = base + RETRY_PROBE_INTERVAL * 2;
        let recovery = app.start_next_probe(recovery_at).unwrap();
        assert!(app.finish_probe(
            &recovery,
            Ok(ProbeMeasurement {
                latency: Some(Duration::from_millis(24)),
            }),
            recovery_at,
        ));
        let availability = app.availability.get("one").unwrap();
        assert_eq!(availability.state, AvailabilityState::Reachable);
        assert_eq!(availability.latency, Some(Duration::from_millis(24)));
        assert_eq!(
            availability.next_probe_at,
            recovery_at + REACHABLE_PROBE_INTERVAL
        );

        let failure_at = recovery_at + REACHABLE_PROBE_INTERVAL;
        let transient = app.start_next_probe(failure_at).unwrap();
        assert!(app.finish_probe(&transient, Err("timeout".to_owned()), failure_at));
        let availability = app.availability.get("one").unwrap();
        assert_eq!(availability.state, AvailabilityState::Reachable);
        assert_eq!(availability.latency, None);
    }

    #[test]
    fn reconciliation_preserves_unchanged_nodes_resets_changed_ips_and_removes_old_nodes() {
        let base = Instant::now();
        let one = device_with_ip("one", "alpha", Ipv4Addr::new(100, 100, 0, 1));
        let two = device_with_ip("two", "beta", Ipv4Addr::new(100, 100, 0, 2));
        let mut app = app_with_at(vec![one.clone(), two], base);

        let target = app.start_next_probe(base).unwrap();
        app.finish_probe(
            &target,
            Ok(ProbeMeasurement {
                latency: Some(Duration::from_millis(10)),
            }),
            base,
        );
        app.finish_refresh_at(
            Ok(snapshot(vec![one.clone()])),
            base + Duration::from_secs(1),
        );
        assert!(!app.availability.contains_key("two"));
        assert_eq!(
            app.availability.get("one").unwrap().next_probe_at,
            base + REACHABLE_PROBE_INTERVAL
        );

        let changed = device_with_ip("one", "alpha", Ipv4Addr::new(100, 100, 0, 9));
        app.finish_refresh_at(Ok(snapshot(vec![changed])), base + Duration::from_secs(2));
        let availability = app.availability.get("one").unwrap();
        assert_eq!(availability.state, AvailabilityState::Unknown);
        assert_eq!(availability.ip, IpAddr::V4(Ipv4Addr::new(100, 100, 0, 9)));
        assert_eq!(availability.next_probe_at, base + Duration::from_secs(2));
    }

    #[test]
    fn scheduler_prioritizes_selected_due_device_and_allows_only_one_in_flight() {
        let base = Instant::now();
        let mut app = app_with_at(
            vec![
                device("one", "alpha", false, None),
                device("two", "beta", false, None),
                device("three", "gamma", false, None),
            ],
            base,
        );
        app.table_state.select(Some(2));

        let target = app.start_next_probe(base).unwrap();
        assert_eq!(target.id, "three");
        assert!(app.start_next_probe(base).is_none());

        app.finish_probe(&target, Err("timeout".to_owned()), base);
        assert_eq!(app.start_next_probe(base).unwrap().id, "one");
    }

    #[test]
    fn stale_probe_results_are_ignored_after_ip_change() {
        let base = Instant::now();
        let mut app = app_with_at(
            vec![device_with_ip(
                "one",
                "alpha",
                Ipv4Addr::new(100, 100, 0, 1),
            )],
            base,
        );
        let target = app.start_next_probe(base).unwrap();
        app.finish_refresh_at(
            Ok(snapshot(vec![device_with_ip(
                "one",
                "alpha",
                Ipv4Addr::new(100, 100, 0, 9),
            )])),
            base,
        );

        assert!(!app.finish_probe(
            &target,
            Ok(ProbeMeasurement {
                latency: Some(Duration::from_millis(1)),
            }),
            base,
        ));
        assert_eq!(
            app.availability.get("one").unwrap().state,
            AvailabilityState::Unknown
        );
    }

    #[test]
    fn status_result_reorders_without_changing_selected_stable_id() {
        let base = Instant::now();
        let mut app = app_with_at(
            vec![
                device("one", "alpha", false, None),
                device("two", "zeta", false, None),
            ],
            base,
        );
        app.cycle_sort();
        app.cycle_sort();
        app.table_state.select(Some(1));
        assert_eq!(app.selected_device().unwrap().id, "two");

        let target = app.start_next_probe(base).unwrap();
        app.finish_probe(
            &target,
            Ok(ProbeMeasurement {
                latency: Some(Duration::from_millis(5)),
            }),
            base,
        );

        assert_eq!(ids(&app), ["two", "one"]);
        assert_eq!(app.selected_device().unwrap().id, "two");
    }

    #[test]
    fn disconnected_probe_worker_warning_is_reported_once() {
        let mut app = App::default();
        assert!(app.probe_send_failed());
        assert_eq!(app.message.as_ref().unwrap().text, "Probe worker stopped");

        app.set_action_message(MessageLevel::Info, "newer message");
        assert!(!app.probe_send_failed());
        assert_eq!(app.message.as_ref().unwrap().text, "newer message");
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
    fn discovery_health_tracks_valid_responses_and_failures() {
        let mut app = App::default();
        assert_eq!(app.discovery_health, DiscoveryHealth::Unknown);

        app.finish_refresh(Ok(snapshot(Vec::new())));
        assert_eq!(app.discovery_health, DiscoveryHealth::Healthy);

        app.finish_refresh(Err("connection lost".to_owned()));
        assert_eq!(app.discovery_health, DiscoveryHealth::Unavailable);
    }

    #[test]
    fn theme_picker_previews_applies_and_cancels_presets() {
        let configured = Theme::portal();
        let mut app = App::new(configured);
        assert_eq!(app.visible_theme(), configured);
        assert_eq!(
            app.themes
                .iter()
                .map(|theme| theme.name)
                .collect::<Vec<_>>(),
            ["Portal", "Hacker", "Cyber Cyan"]
        );

        app.open_theme_picker();
        app.select_next_theme();
        assert_eq!(app.visible_theme(), Theme::hacker());
        app.cancel_theme_picker();
        assert_eq!(app.visible_theme(), configured);

        app.open_theme_picker();
        app.select_next_theme();
        app.select_next_theme();
        app.select_next_theme();
        assert_eq!(app.visible_theme(), Theme::cyber_cyan());
        app.apply_theme_picker();
        assert_eq!(app.visible_theme(), Theme::cyber_cyan());
        assert!(!app.theme_picker_open());
        assert!(app.message.as_ref().unwrap().text.contains("Cyber Cyan"));
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
            command(KeyCode::Char('t'), KeyEventKind::Press),
            Some(InputCommand::OpenThemePicker)
        );
        assert_eq!(
            command(KeyCode::Esc, KeyEventKind::Press),
            Some(InputCommand::Quit)
        );
        assert_eq!(command(KeyCode::Down, KeyEventKind::Release), None);
        assert_eq!(command(KeyCode::Char('x'), KeyEventKind::Press), None);
    }

    #[test]
    fn theme_picker_key_mapping_is_contextual() {
        assert_eq!(
            picker_command(KeyCode::Up),
            Some(ThemePickerCommand::Previous)
        );
        assert_eq!(
            picker_command(KeyCode::Char('j')),
            Some(ThemePickerCommand::Next)
        );
        assert_eq!(
            picker_command(KeyCode::Enter),
            Some(ThemePickerCommand::Apply)
        );
        assert_eq!(
            picker_command(KeyCode::Esc),
            Some(ThemePickerCommand::Cancel)
        );
        assert_eq!(picker_command(KeyCode::Char('x')), None);
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

    fn picker_command(code: KeyCode) -> Option<ThemePickerCommand> {
        theme_picker_command(KeyEvent::new_with_kind(
            code,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))
    }

    fn app_with(devices: Vec<Device>) -> App {
        app_with_at(devices, Instant::now())
    }

    fn app_with_at(devices: Vec<Device>, now: Instant) -> App {
        let mut app = App::default();
        app.finish_refresh_at(Ok(snapshot(devices)), now);
        app
    }

    fn snapshot(devices: Vec<Device>) -> Snapshot {
        Snapshot {
            local_device: None,
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
            machine_name: name.to_owned(),
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

    fn device_with_ip(id: &str, name: &str, ip: Ipv4Addr) -> Device {
        let mut device = device(id, name, false, None);
        device.ips = vec![IpAddr::V4(ip)];
        device.preferred_ip = IpAddr::V4(ip);
        device
    }
}
