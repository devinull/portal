use portal_core::{Device, Theme, ThemeColor};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table,
    TableState, Wrap,
};

use crate::app::{
    App, DiscoveryHealth, MessageLevel, MessageSource, ThemeOption, format_last_connected,
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
    let online = app.devices.iter().filter(|device| device.online).count();
    let summary = format!("{online} online · {} devices", app.devices.len());
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
        render_device_table(
            frame,
            columns[0],
            &app.devices,
            &mut app.table_state,
            app.initial_load_complete,
            discovery_failed,
            theme,
        );
        render_details(frame, columns[2], app.selected_device(), theme, now);
    } else if area.width < 100 && area.height >= 18 {
        let rows = Layout::vertical([
            Constraint::Percentage(55),
            Constraint::Length(1),
            Constraint::Percentage(45),
        ])
        .split(area);
        render_device_table(
            frame,
            rows[0],
            &app.devices,
            &mut app.table_state,
            app.initial_load_complete,
            discovery_failed,
            theme,
        );
        render_details(frame, rows[2], app.selected_device(), theme, now);
    } else {
        render_device_table(
            frame,
            area,
            &app.devices,
            &mut app.table_state,
            app.initial_load_complete,
            discovery_failed,
            theme,
        );
    }
}

fn render_device_table(
    frame: &mut Frame<'_>,
    area: Rect,
    devices: &[Device],
    table_state: &mut TableState,
    initial_load_complete: bool,
    discovery_failed: bool,
    theme: Theme,
) {
    let block = panel(" devices ", theme);
    if devices.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let message = if discovery_failed {
            "Discovery failed — press r to retry"
        } else if initial_load_complete {
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

    let column_mode = table_column_mode(area.width);
    let selected_index = table_state.selected();
    let selected_style = Style::default()
        .fg(color(theme.background))
        .bg(color(theme.accent))
        .add_modifier(Modifier::BOLD);
    let rows = devices.iter().enumerate().map(|(index, device)| {
        let selected = selected_index == Some(index);
        let mut status_style = Style::default().fg(color(if device.online {
            theme.signal
        } else {
            theme.muted
        }));
        if selected {
            status_style = status_style.bg(color(theme.accent));
        }
        let mut cells = vec![Cell::from(Span::styled("●", status_style))];
        cells.push(Cell::from(device.display_name().to_owned()));
        if column_mode != ColumnMode::Narrow {
            cells.push(Cell::from(device.os.clone()));
        }
        if column_mode == ColumnMode::Wide {
            cells.push(Cell::from(device.preferred_ip.to_string()));
        }
        Row::new(cells).style(if selected {
            selected_style
        } else {
            base_style(theme)
        })
    });
    let (headings, widths): (Vec<&str>, Vec<Constraint>) = match column_mode {
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
    frame.render_stateful_widget(table, area, table_state);
}

fn render_details(
    frame: &mut Frame<'_>,
    area: Rect,
    device: Option<&Device>,
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

    let mut lines = vec![
        Line::from(Span::styled(
            device.display_name().to_owned(),
            Style::default()
                .fg(color(theme.foreground))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if device.online { "online" } else { "offline" },
            Style::default().fg(color(if device.online {
                theme.signal
            } else {
                theme.muted
            })),
        )),
        Line::from(""),
        detail_line("machine", &device.machine_name, theme),
        detail_line("host OS", &device.hostname, theme),
    ];
    if let Some(dns_name) = &device.dns_name {
        lines.push(detail_line("dns", dns_name, theme));
    }
    lines.extend([
        detail_line("address", &device.preferred_ip.to_string(), theme),
        detail_line("os", &device.os, theme),
    ]);
    if !device.tags.is_empty() {
        lines.push(detail_line("tags", &device.tags.join(" · "), theme));
    }
    if device.favorite {
        lines.push(detail_line("favorite", "yes", theme));
    }
    if let Some(notes) = &device.notes {
        lines.push(detail_line("notes", notes, theme));
    }
    lines.push(detail_line(
        "last used",
        &format_last_connected(device.last_connected_at, now),
        theme,
    ));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .style(base_style(theme)),
        area,
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

fn render_theme_picker(frame: &mut Frame<'_>, app: &App, theme: Theme) {
    let area = centered_popup(frame.area(), 44, 9);
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

    use portal_core::{Device, LocalDevice, Snapshot, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use crate::app::{App, MessageLevel};

    use super::{color, render};

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
        assert!(text.contains("sort: recent"));
        assert!(text.contains("t theme"));
        assert!(!text.contains("refreshing"));
    }

    #[test]
    fn header_uses_a_stable_discovery_health_dot() {
        let mut pending = App::default();
        let pending_text = render_text(&mut pending, 80, 24);
        assert!(pending_text.lines().next().unwrap().contains("○ 0 online"));

        pending.finish_refresh(Ok(snapshot(Vec::new())));
        let healthy_text = render_text(&mut pending, 80, 24);
        assert!(healthy_text.lines().next().unwrap().contains("● 0 online"));

        pending.finish_refresh(Err("connection lost".to_owned()));
        let failed_text = render_text(&mut pending, 80, 24);
        assert!(failed_text.lines().next().unwrap().contains("○ 0 online"));
    }

    #[test]
    fn stacked_and_list_only_layouts_follow_size_thresholds() {
        let mut app = populated_app();
        let stacked = render_text(&mut app, 80, 24);
        assert!(stacked.contains("details"));
        assert!(stacked.contains("host OS"));

        let list_only = render_text(&mut app, 60, 16);
        assert!(list_only.contains("devices"));
        assert!(!list_only.contains("details"));
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
        let mut app = populated_app();
        let theme = Theme::default();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, theme, 200))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let dot_colors: Vec<Color> = buffer
            .content
            .iter()
            .filter(|cell| cell.symbol() == "●")
            .map(|cell| cell.fg)
            .collect();
        assert!(dot_colors.contains(&color(theme.signal)));
        assert!(dot_colors.contains(&color(theme.muted)));
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.bg == color(theme.accent))
        );
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
