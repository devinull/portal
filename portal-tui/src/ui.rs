use portal_core::{Device, Theme, ThemeColor};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table, Wrap,
};

use crate::app::{
    App, Availability, AvailabilityState, ConfigField, DiscoveryHealth, MessageLevel,
    MessageSource, ThemeOption, format_last_connected,
};

const MINIMUM_WIDTH: u16 = 48;
const MINIMUM_HEIGHT: u16 = 12;

pub(crate) fn render(frame: &mut Frame<'_>, app: &mut App, theme: Theme, now: u64) {
    let area = frame.area();
    frame.render_widget(Block::new().style(base_style(theme)), area);

    if area.width < MINIMUM_WIDTH || area.height < MINIMUM_HEIGHT {
        render_too_small(frame, area, theme);
        return;
    }

    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    render_header(frame, vertical[0], app, theme);
    render_body(frame, vertical[2], app, theme, now);
    render_footer(frame, vertical[4], app, theme);
    if app.theme_picker_open() {
        render_theme_picker(frame, app, theme);
    } else if app.config_editor_open() {
        render_config_editor(frame, app, theme);
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let rows = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .split(area);
    let message =
        Paragraph::new("Portal needs at least 48 × 12.\nResize the terminal or press q to quit.")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(color(theme.foreground))
                    .bg(color(theme.background)),
            );
    frame.render_widget(message, rows[1]);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let reachable = app
        .devices
        .iter()
        .filter(|device| app.availability_state(device) == AvailabilityState::Reachable)
        .count();
    let summary = format!("{reachable} reachable · {} devices", app.devices.len());
    let summary_width = summary
        .chars()
        .count()
        .saturating_add(2)
        .min(usize::from(u16::MAX)) as u16;
    let columns = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(summary_width.min(area.width)),
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new("PORTAL").style(
            Style::default()
                .fg(color(theme.accent))
                .bg(color(theme.background))
                .add_modifier(Modifier::BOLD),
        ),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                match app.discovery_health {
                    DiscoveryHealth::Healthy => "●",
                    DiscoveryHealth::Unknown | DiscoveryHealth::Unavailable => "○",
                },
                Style::default().fg(color(match app.discovery_health {
                    DiscoveryHealth::Healthy => theme.signal,
                    DiscoveryHealth::Unknown | DiscoveryHealth::Unavailable => theme.muted,
                })),
            ),
            Span::styled(
                format!(" {summary}"),
                Style::default().fg(color(theme.muted)),
            ),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(color(theme.background))),
        columns[1],
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: Theme, now: u64) {
    let discovery_failed = app.devices.is_empty()
        && app.initial_load_complete
        && app.message.as_ref().is_some_and(|message| {
            message.source == MessageSource::Refresh && message.level == MessageLevel::Error
        });
    if area.width >= 100 && area.height >= 14 {
        let columns = Layout::horizontal([
            Constraint::Percentage(60),
            Constraint::Length(2),
            Constraint::Percentage(40),
        ])
        .split(area);
        render_device_table(frame, columns[0], app, discovery_failed, false, theme);
        render_details(
            frame,
            columns[2],
            app.selected_device(),
            app.selected_device()
                .and_then(|device| app.availability(device)),
            theme,
            now,
        );
    } else if area.width < 100 && area.height >= 18 {
        let rows = Layout::vertical([
            Constraint::Percentage(55),
            Constraint::Length(1),
            Constraint::Percentage(45),
        ])
        .split(area);
        render_device_table(frame, rows[0], app, discovery_failed, false, theme);
        render_details(
            frame,
            rows[2],
            app.selected_device(),
            app.selected_device()
                .and_then(|device| app.availability(device)),
            theme,
            now,
        );
    } else {
        render_device_table(frame, area, app, discovery_failed, true, theme);
    }
}

fn render_device_table(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    discovery_failed: bool,
    show_latency: bool,
    theme: Theme,
) {
    let block = panel(" devices ", theme);
    if app.devices.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let message = if discovery_failed {
            "Discovery failed — press r to retry"
        } else if app.initial_load_complete {
            "No reachable desktop peers"
        } else {
            "Discovering devices..."
        };
        frame.render_widget(
            Paragraph::new(message).alignment(Alignment::Center).style(
                Style::default()
                    .fg(color(theme.muted))
                    .bg(color(theme.background)),
            ),
            vertically_centered(inner),
        );
        return;
    }

    let column_mode = if show_latency {
        list_only_column_mode(area.width)
    } else {
        table_column_mode(area.width)
    };
    let selected_index = app.table_state.selected();
    let selected_style = Style::default()
        .fg(color(theme.background))
        .bg(color(theme.accent))
        .add_modifier(Modifier::BOLD);
    let rows = app.devices.iter().enumerate().map(|(index, device)| {
        let selected = selected_index == Some(index);
        let device_availability = app.availability.get(&device.id);
        let state = device_availability.map_or(AvailabilityState::Unknown, |value| value.state);
        let reachable = state == AvailabilityState::Reachable;
        let mut status_style =
            Style::default().fg(color(if reachable { theme.signal } else { theme.muted }));
        if selected {
            status_style = status_style.bg(color(theme.accent));
        }
        let mut cells = vec![Cell::from(Span::styled(
            if reachable { "●" } else { "○" },
            status_style,
        ))];
        cells.push(Cell::from(device.display_name().to_owned()));
        if column_mode != ColumnMode::Narrow {
            cells.push(Cell::from(device.os.clone()));
        }
        if column_mode == ColumnMode::Wide {
            cells.push(Cell::from(device.preferred_ip.to_string()));
        }
        if show_latency {
            cells.push(Cell::from(format_latency(
                device_availability.and_then(|value| value.latency),
            )));
        }
        Row::new(cells).style(if selected {
            selected_style
        } else {
            base_style(theme)
        })
    });
    let (mut headings, mut widths): (Vec<&str>, Vec<Constraint>) = match column_mode {
        ColumnMode::Wide => (
            vec!["", "NAME", "OS", "ADDRESS"],
            vec![
                Constraint::Length(2),
                Constraint::Min(12),
                Constraint::Length(10),
                Constraint::Length(20),
            ],
        ),
        ColumnMode::Medium => (
            vec!["", "NAME", "OS"],
            vec![
                Constraint::Length(2),
                Constraint::Min(12),
                Constraint::Length(10),
            ],
        ),
        ColumnMode::Narrow => (
            vec!["", "NAME"],
            vec![Constraint::Length(2), Constraint::Min(12)],
        ),
    };
    if show_latency {
        headings.push("LATENCY");
        widths.push(Constraint::Length(9));
    }
    let header = Row::new(headings)
        .style(
            Style::default()
                .fg(color(theme.muted))
                .bg(color(theme.background)),
        )
        .bottom_margin(1);
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1)
        .style(base_style(theme));
    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_details(
    frame: &mut Frame<'_>,
    area: Rect,
    device: Option<&Device>,
    availability: Option<&Availability>,
    theme: Theme,
    now: u64,
) {
    let block = panel(" details ", theme);
    let Some(device) = device else {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new("No device selected")
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(color(theme.muted))
                        .bg(color(theme.background)),
                ),
            vertically_centered(inner),
        );
        return;
    };

    let state = availability.map_or(AvailabilityState::Unknown, |value| value.state);
    let latency = format_latency(availability.and_then(|value| value.latency));
    let inner = block.inner(area);
    let name = device.display_name();
    let name_width = name.chars().count();
    let latency_width = latency.chars().count();
    let first_line = if name_width + latency_width < usize::from(inner.width) {
        Line::from(vec![
            Span::styled(
                name.to_owned(),
                Style::default()
                    .fg(color(theme.foreground))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(usize::from(inner.width) - name_width - latency_width)),
            Span::styled(latency, Style::default().fg(color(theme.muted))),
        ])
    } else {
        Line::from(Span::styled(
            name.to_owned(),
            Style::default()
                .fg(color(theme.foreground))
                .add_modifier(Modifier::BOLD),
        ))
    };
    let mut lines = vec![
        first_line,
        Line::from(Span::styled(
            match state {
                AvailabilityState::Reachable => "reachable",
                AvailabilityState::Unknown => "checking",
                AvailabilityState::Unavailable => "unavailable",
            },
            Style::default().fg(color(if state == AvailabilityState::Reachable {
                theme.signal
            } else {
                theme.muted
            })),
        )),
        Line::from(""),
        detail_line("machine", &device.machine_name, theme),
        detail_line("hostname", &device.hostname, theme),
    ];
    if let Some(dns_name) = &device.dns_name {
        lines.push(detail_line("dns", dns_name, theme));
    }
    lines.extend([
        detail_line("address", &device.preferred_ip.to_string(), theme),
        detail_line("os", &device.os, theme),
    ]);
    lines.push(detail_line(
        "nickname",
        device.nickname.as_deref().unwrap_or("—"),
        theme,
    ));
    let tags = if device.tags.is_empty() {
        "—".to_owned()
    } else {
        device.tags.join(" · ")
    };
    lines.push(detail_line("tags", &tags, theme));
    lines.push(detail_line(
        "favorite",
        if device.favorite { "yes" } else { "no" },
        theme,
    ));
    lines.push(detail_line(
        "notes",
        device.notes.as_deref().unwrap_or("—"),
        theme,
    ));
    lines.push(detail_line(
        "last used",
        &format_last_connected(device.last_connected_at, now),
        theme,
    ));

    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(base_style(theme)),
        inner,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let local = app
        .local_device
        .as_ref()
        .map(|device| format!("{} · {}", device.machine_name, device.preferred_ip));
    let local_width = local.as_ref().map_or(0, |value| {
        value.chars().count().min(usize::from(u16::MAX)) as u16
    });
    let columns = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(local_width.min(area.width)),
    ])
    .split(area);

    if let Some(message) = &app.message {
        let (prefix, foreground) = match message.level {
            MessageLevel::Info => ("", theme.signal),
            MessageLevel::Warning => ("warning: ", theme.foreground),
            MessageLevel::Error => ("error: ", theme.foreground),
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color(theme.muted))),
                Span::styled(&message.text, Style::default().fg(color(foreground))),
            ]))
            .style(Style::default().bg(color(theme.background))),
            columns[0],
        );
    } else {
        let accent = Style::default().fg(color(theme.accent));
        let muted = Style::default().fg(color(theme.muted));
        let line = Line::from(vec![
            Span::styled("↑/k ↓/j", accent),
            Span::styled(" move  ", muted),
            Span::styled("enter", accent),
            Span::styled(" connect  ", muted),
            Span::styled("s", accent),
            Span::styled(" share  ", muted),
            Span::styled("tab", accent),
            Span::styled(format!(" sort: {}  ", app.sort_mode.label()), muted),
            Span::styled("r", accent),
            Span::styled(" refresh  ", muted),
            Span::styled("t", accent),
            Span::styled(" theme  ", muted),
            Span::styled("e", accent),
            Span::styled(" edit  ", muted),
            Span::styled("q", accent),
            Span::styled(" quit", muted),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(color(theme.background))),
            columns[0],
        );
    }

    if let Some(local) = local {
        frame.render_widget(
            Paragraph::new(local).alignment(Alignment::Right).style(
                Style::default()
                    .fg(color(theme.muted))
                    .bg(color(theme.background)),
            ),
            columns[1],
        );
    }
}

fn render_config_editor(frame: &mut Frame<'_>, app: &App, theme: Theme) {
    let Some(editor) = app.config_editor() else {
        return;
    };
    let area = centered_popup(frame.area(), 68, 13);
    frame.render_widget(Clear, area);
    let title = format!(" configure {} ", editor.machine_name);
    let block = panel(&title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::vertical([Constraint::Length(6), Constraint::Min(3)]).split(inner);
    let active = editor.field;
    let items = [
        config_item(
            "nickname",
            &editor.nickname,
            active == ConfigField::Nickname,
            editor.cursor,
            theme,
        ),
        config_item(
            "tags",
            &editor.tags,
            active == ConfigField::Tags,
            editor.cursor,
            theme,
        ),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("{:<10}", "favorite"),
                Style::default().fg(color(theme.muted)),
            ),
            Span::styled(
                if editor.favorite { "[x] yes" } else { "[ ] no" },
                Style::default().fg(color(theme.foreground)),
            ),
        ])),
        config_item(
            "notes",
            &editor.notes,
            active == ConfigField::Notes,
            editor.cursor,
            theme,
        ),
    ];
    let selected = match active {
        ConfigField::Nickname => 0,
        ConfigField::Tags => 1,
        ConfigField::Favorite => 2,
        ConfigField::Notes => 3,
    };
    let mut state = ListState::default().with_selected(Some(selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(color(theme.background))
                .bg(color(theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .style(base_style(theme));
    frame.render_stateful_widget(list, rows[0], &mut state);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("tab/↑/↓", Style::default().fg(color(theme.accent))),
                Span::styled(" field  ", Style::default().fg(color(theme.muted))),
                Span::styled("space", Style::default().fg(color(theme.accent))),
                Span::styled(" toggle favorite", Style::default().fg(color(theme.muted))),
            ]),
            Line::from(vec![
                Span::styled("enter", Style::default().fg(color(theme.accent))),
                Span::styled(" save  ", Style::default().fg(color(theme.muted))),
                Span::styled("ctrl+u", Style::default().fg(color(theme.accent))),
                Span::styled(" clear  ", Style::default().fg(color(theme.muted))),
                Span::styled("esc", Style::default().fg(color(theme.accent))),
                Span::styled(" cancel", Style::default().fg(color(theme.muted))),
            ]),
            Line::from(Span::styled(
                "Separate tags with commas",
                Style::default().fg(color(theme.muted)),
            )),
        ])
        .style(base_style(theme)),
        rows[1],
    );
}

fn config_item(
    label: &str,
    value: &str,
    active: bool,
    cursor: usize,
    theme: Theme,
) -> ListItem<'static> {
    let value = if active {
        with_cursor(value, cursor)
    } else if value.is_empty() {
        "—".to_owned()
    } else {
        value.to_owned()
    };
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{label:<10}"),
            Style::default().fg(color(theme.muted)),
        ),
        Span::styled(value, Style::default().fg(color(theme.foreground))),
    ]))
}

fn with_cursor(value: &str, cursor: usize) -> String {
    let mut characters: Vec<char> = value.chars().collect();
    characters.insert(cursor.min(characters.len()), '│');
    characters.into_iter().collect()
}

fn render_theme_picker(frame: &mut Frame<'_>, app: &App, theme: Theme) {
    let area = centered_popup(frame.area(), 44, 11);
    frame.render_widget(Clear, area);
    let block = panel(" themes ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(inner);
    let items = app.themes.iter().map(|option| theme_item(option, theme));
    let mut state = ListState::default().with_selected(app.theme_picker_selected());
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(color(theme.background))
                .bg(color(theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .style(base_style(theme));
    frame.render_stateful_widget(list, rows[0], &mut state);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("↑/k ↓/j", Style::default().fg(color(theme.accent))),
                Span::styled(" preview  ", Style::default().fg(color(theme.muted))),
                Span::styled("enter", Style::default().fg(color(theme.accent))),
                Span::styled(" apply", Style::default().fg(color(theme.muted))),
            ]),
            Line::from(vec![
                Span::styled("esc/q", Style::default().fg(color(theme.accent))),
                Span::styled(" cancel", Style::default().fg(color(theme.muted))),
            ]),
        ])
        .style(base_style(theme)),
        rows[1],
    );
}

fn theme_item(option: &ThemeOption, theme: Theme) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        format!("  {}", option.name),
        Style::default().fg(color(theme.foreground)),
    )))
}

fn detail_line(label: &str, value: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<10}"),
            Style::default().fg(color(theme.muted)),
        ),
        Span::styled(
            value.to_owned(),
            Style::default().fg(color(theme.foreground)),
        ),
    ])
}

fn panel<'a>(title: &'a str, theme: Theme) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .padding(Padding::uniform(1))
        .border_style(Style::default().fg(color(theme.muted)))
        .style(base_style(theme))
}

fn vertically_centered(area: Rect) -> Rect {
    Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(area)[1]
}

fn centered_popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .split(vertical[1])[1]
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ColumnMode {
    Wide,
    Medium,
    Narrow,
}

const fn table_column_mode(width: u16) -> ColumnMode {
    if width >= 64 {
        ColumnMode::Wide
    } else if width >= 44 {
        ColumnMode::Medium
    } else {
        ColumnMode::Narrow
    }
}

const fn list_only_column_mode(width: u16) -> ColumnMode {
    if width >= 72 {
        ColumnMode::Wide
    } else if width >= 54 {
        ColumnMode::Medium
    } else {
        ColumnMode::Narrow
    }
}

pub(crate) fn format_latency(latency: Option<std::time::Duration>) -> String {
    let Some(latency) = latency else {
        return "—".to_owned();
    };
    if latency < std::time::Duration::from_millis(1) {
        "<1 ms".to_owned()
    } else if latency < std::time::Duration::from_secs(1) {
        format!("{} ms", (latency.as_secs_f64() * 1_000.0).round() as u64)
    } else {
        format!("{:.1} s", latency.as_secs_f64())
    }
}

fn base_style(theme: Theme) -> Style {
    Style::default()
        .fg(color(theme.foreground))
        .bg(color(theme.background))
}

const fn color(value: ThemeColor) -> Color {
    Color::Rgb(value.red, value.green, value.blue)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    use portal_core::{Device, LocalDevice, Snapshot, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use crate::app::{App, AvailabilityState, MessageLevel};

    use super::{color, format_latency, render};

    #[test]
    fn wide_layout_contains_table_details_and_footer() {
        let mut app = populated_app();
        let text = render_text(&mut app, 120, 30);

        assert!(text.contains("PORTAL"));
        assert!(text.contains("devices"));
        assert!(text.contains("details"));
        assert!(text.contains("studio"));
        assert!(text.contains("studio-pc"));
        assert!(text.contains("100.100.0.42"));
        assert!(text.contains("desktop · primary"));
        assert!(text.contains("Sanitized workstation"));
        assert!(text.contains("24 ms"));
        assert!(text.contains("reachable"));
        assert!(!text.contains("LATENCY"));
        assert!(text.contains("sort: recent"));
        assert!(text.contains("t theme"));
        assert!(!text.contains("refreshing"));
    }

    #[test]
    fn header_uses_a_stable_discovery_health_dot() {
        let mut pending = App::default();
        let pending_text = render_text(&mut pending, 80, 24);
        assert!(
            pending_text
                .lines()
                .next()
                .unwrap()
                .contains("○ 0 reachable")
        );

        pending.finish_refresh(Ok(snapshot(Vec::new())));
        let healthy_text = render_text(&mut pending, 80, 24);
        assert!(
            healthy_text
                .lines()
                .next()
                .unwrap()
                .contains("● 0 reachable")
        );

        pending.finish_refresh(Err("connection lost".to_owned()));
        let failed_text = render_text(&mut pending, 80, 24);
        assert!(
            failed_text
                .lines()
                .next()
                .unwrap()
                .contains("○ 0 reachable")
        );
    }

    #[test]
    fn stacked_and_list_only_layouts_follow_size_thresholds() {
        let mut app = populated_app();
        let stacked = render_text(&mut app, 80, 24);
        assert!(stacked.contains("details"));
        assert!(stacked.contains("hostname"));
        assert!(stacked.contains("24 ms"));
        assert!(!stacked.contains("LATENCY"));

        let list_only = render_text(&mut app, 60, 16);
        assert!(list_only.contains("devices"));
        assert!(!list_only.contains("details"));
        assert!(list_only.contains("LATENCY"));
        assert!(list_only.contains("24 ms"));
    }

    #[test]
    fn list_only_latency_columns_follow_exact_width_thresholds() {
        let mut app = populated_app();
        let wide = render_text(&mut app, 72, 16);
        assert!(wide.contains("OS"));
        assert!(wide.contains("ADDRESS"));
        assert!(wide.contains("LATENCY"));

        let medium = render_text(&mut app, 54, 16);
        assert!(medium.contains("OS"));
        assert!(!medium.contains("ADDRESS"));
        assert!(medium.contains("LATENCY"));

        let narrow = render_text(&mut app, 48, 16);
        assert!(!narrow.contains(" OS "));
        assert!(!narrow.contains("ADDRESS"));
        assert!(narrow.contains("LATENCY"));
    }

    #[test]
    fn latency_formatter_uses_operational_precision() {
        assert_eq!(format_latency(None), "—");
        assert_eq!(format_latency(Some(Duration::from_nanos(800))), "<1 ms");
        assert_eq!(format_latency(Some(Duration::from_micros(24_499))), "24 ms");
        assert_eq!(format_latency(Some(Duration::from_micros(24_500))), "25 ms");
        assert_eq!(format_latency(Some(Duration::from_secs_f64(1.24))), "1.2 s");
    }

    #[test]
    fn undersized_layout_is_explicit() {
        let mut app = App::default();
        let text = render_text(&mut app, 40, 10);

        assert!(text.contains("Portal needs at least 48 × 12"));
        assert!(text.contains("press q to quit"));
    }

    #[test]
    fn loading_empty_and_error_states_are_distinct() {
        let mut loading = App::default();
        assert!(render_text(&mut loading, 60, 16).contains("Discovering devices..."));

        loading.finish_refresh(Ok(snapshot(Vec::new())));
        assert!(render_text(&mut loading, 60, 16).contains("No reachable desktop peers"));

        loading.finish_refresh(Err("Tailscale is unavailable".to_owned()));
        let error = render_text(&mut loading, 60, 16);
        assert!(error.contains("Discovery failed — press r to retry"));
        assert!(error.contains("error:"));
        assert!(error.contains("Tailscale is unavailable"));
        assert!(!error.contains("sort: recent"));
    }

    #[test]
    fn status_dots_and_selected_row_use_theme_colors() {
        for theme in [
            Theme::portal(),
            Theme::hacker(),
            Theme::terminator(),
            Theme::cyber_cyan(),
        ] {
            let mut app = populated_app();
            let backend = TestBackend::new(120, 30);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, theme, 200))
                .unwrap();
            let buffer = terminal.backend().buffer();

            let filled_dot_colors: Vec<Color> = buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == "●")
                .map(|cell| cell.fg)
                .collect();
            let empty_dot_colors: Vec<Color> = buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == "○")
                .map(|cell| cell.fg)
                .collect();
            assert!(filled_dot_colors.contains(&color(theme.signal)));
            assert!(empty_dot_colors.contains(&color(theme.muted)));
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| cell.bg == color(theme.accent))
            );
        }
    }

    #[test]
    fn narrow_details_omit_latency_instead_of_truncating_the_name_for_it() {
        let mut app = populated_app();
        app.devices[0].nickname =
            Some("a-device-name-that-nearly-fills-the-details-panel".to_owned());
        let text = render_text(&mut app, 48, 24);

        assert!(text.contains("a-device-name"));
        assert!(!text.contains("24 ms"));
    }

    #[test]
    fn action_message_replaces_normal_hints() {
        let mut app = populated_app();
        app.set_action_message(MessageLevel::Error, "RustDesk was not found");
        let text = render_text(&mut app, 80, 24);

        assert!(text.contains("error: RustDesk was not found"));
        assert!(!text.contains("sort: recent"));
    }

    #[test]
    fn footer_shows_local_tailscale_identity_on_the_right() {
        let mut app = populated_app();
        app.local_device = Some(LocalDevice {
            machine_name: "local-machine".to_owned(),
            preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 9)),
        });
        let text = render_text(&mut app, 120, 30);

        assert!(
            text.lines()
                .last()
                .unwrap()
                .ends_with("local-machine · 100.100.0.9")
        );
    }

    #[test]
    fn panels_have_uniform_inner_padding() {
        let mut app = populated_app();
        let text = render_text(&mut app, 120, 30);
        let lines: Vec<&str> = text.lines().collect();
        let device_row = text
            .lines()
            .find(|line| line.contains("studio") && line.contains('●'))
            .unwrap();

        assert!(!lines[3].contains("NAME"));
        assert!(lines[4].contains("NAME"));
        assert_eq!(
            device_row.chars().position(|character| character == '●'),
            Some(2)
        );
    }

    #[test]
    fn theme_picker_matches_the_previewed_theme() {
        let mut app = populated_app();
        app.open_theme_picker();
        app.select_next_theme();
        let theme = app.visible_theme();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, theme, 200))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer_text(buffer, 80, 24);

        assert!(text.contains("themes"));
        assert!(text.contains("Portal"));
        assert!(text.contains("Hacker"));
        assert!(text.contains("Terminator"));
        assert!(text.contains("Cyber Cyan"));
        assert!(text.contains("preview"));
        assert!(text.contains("enter apply"));
        assert_eq!(theme, Theme::hacker());
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.bg == color(theme.accent))
        );
    }

    #[test]
    fn config_editor_renders_machine_fields_and_help() {
        let mut app = populated_app();
        app.open_config_editor();
        let text = render_text(&mut app, 100, 30);

        assert!(text.contains("configure studio-pc"));
        assert!(text.contains("nickname"));
        assert!(text.contains("favorite"));
        assert!(text.contains("Separate tags with commas"));
        assert!(text.contains("enter save"));
    }

    fn render_text(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = app.visible_theme();
        terminal
            .draw(|frame| render(frame, app, theme, 200))
            .unwrap();
        buffer_text(terminal.backend().buffer(), width, height)
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn populated_app() -> App {
        let mut first = device("node-studio", "studio-pc", true, Some(100));
        first.nickname = Some("studio".to_owned());
        first.tags = vec!["desktop".to_owned(), "primary".to_owned()];
        first.favorite = true;
        first.notes = Some("Sanitized workstation".to_owned());
        let mut app = App::default();
        app.finish_refresh(Ok(snapshot(vec![
            first,
            device("node-laptop", "laptop", true, None),
            device("node-server", "server", false, None),
        ])));
        let studio = app.availability.get_mut("node-studio").unwrap();
        studio.state = AvailabilityState::Reachable;
        studio.latency = Some(Duration::from_millis(24));
        app.availability.get_mut("node-laptop").unwrap().state = AvailabilityState::Reachable;
        app.availability.get_mut("node-server").unwrap().state = AvailabilityState::Unavailable;
        app
    }

    fn snapshot(devices: Vec<Device>) -> Snapshot {
        Snapshot {
            local_device: None,
            devices,
            warnings: Vec::new(),
        }
    }

    fn device(id: &str, hostname: &str, online: bool, last_connected_at: Option<u64>) -> Device {
        Device {
            id: id.to_owned(),
            machine_name: hostname.to_owned(),
            hostname: hostname.to_owned(),
            dns_name: Some(format!("{hostname}.example.test")),
            nickname: None,
            ips: vec![IpAddr::V4(Ipv4Addr::new(100, 100, 0, 42))],
            preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 42)),
            os: "windows".to_owned(),
            online,
            tags: Vec::new(),
            favorite: false,
            notes: None,
            last_connected_at,
        }
    }
}
