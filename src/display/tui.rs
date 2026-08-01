//! Interactive terminal UI powered by ratatui.

use crate::collector::Collector;
use crate::metrics::{filter_and_rank, format_bps_pair, Snapshot, SortBy, TargetKind};
use crate::util::sanitize_control_chars;
use anyhow::Result;
use bytesize::ByteSize;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, ExecutableCommand};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

pub struct TuiConfig {
    pub interval: Duration,
    pub name_filter: Option<String>,
    pub sort_by: SortBy,
    pub top: Option<usize>,
}

struct App {
    snapshot: Snapshot,
    filter: Option<String>,
    sort_by: SortBy,
    top: Option<usize>,
    table_state: TableState,
    last_error: Option<String>,
    tick: u64,
}

impl App {
    fn ranked(&self) -> Vec<crate::metrics::ResourceSample> {
        filter_and_rank(
            &self.snapshot.targets,
            self.filter.as_deref(),
            self.sort_by,
            self.top,
        )
    }

    fn cycle_sort(&mut self) {
        self.sort_by = match self.sort_by {
            SortBy::Cpu => SortBy::Memory,
            SortBy::Memory => SortBy::Disk,
            SortBy::Disk => SortBy::Network,
            SortBy::Network => SortBy::Cpu,
        };
    }
}

pub async fn run_tui(mut collector: Collector, config: TuiConfig) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = ratatui::init();

    let result = run_loop(&mut terminal, &mut collector, config).await;

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    result
}

async fn run_loop(
    terminal: &mut DefaultTerminal,
    collector: &mut Collector,
    config: TuiConfig,
) -> Result<()> {
    let snapshot = collector.snapshot().await;
    let mut app = App {
        snapshot,
        filter: config.name_filter,
        sort_by: config.sort_by,
        top: config.top,
        table_state: TableState::default().with_selected(0),
        last_error: None,
        tick: 0,
    };

    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|f| draw(f, &mut app))?;

        let timeout = config
            .interval
            .checked_sub(last_refresh.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,

                    KeyCode::Char('s') => app.cycle_sort(),

                    KeyCode::Char('r') => {
                        app.snapshot = collector.snapshot().await;
                        app.tick = app.tick.wrapping_add(1);
                        last_refresh = Instant::now();
                    }

                    KeyCode::Down | KeyCode::Char('j') => {
                        let len = app.ranked().len();
                        if len > 0 {
                            let i = app.table_state.selected().unwrap_or(0);
                            app.table_state.select(Some((i + 1) % len));
                        }
                    }

                    KeyCode::Up | KeyCode::Char('k') => {
                        let len = app.ranked().len();
                        if len > 0 {
                            let i = app.table_state.selected().unwrap_or(0);
                            app.table_state
                                .select(Some(if i == 0 { len - 1 } else { i - 1 }));
                        }
                    }
                    _ => {}
                }
            }
        }

        if last_refresh.elapsed() >= config.interval {
            app.snapshot = collector.snapshot().await;
            app.tick = app.tick.wrapping_add(1);
            app.last_error = None;
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_table(frame, chunks[1], app);
    draw_footer(frame, chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let h = &app.snapshot.host;
    let docker = if app
        .snapshot
        .targets
        .iter()
        .any(|t| t.kind == TargetKind::Container)
    {
        "docker: yes"
    } else {
        "docker: n/a"
    };

    let text = vec![
        Line::from(vec![
            Span::styled(
                " JENTAYU ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  mythical eye over your containers  "),
            Span::styled(docker, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(format!(
            " CPU {:>5.1}%   MEM {:>5.1}% ({}/{})   load {:.2} {:.2} {:.2}",
            h.cpu_percent,
            h.memory_percent(),
            ByteSize::b(h.memory_used_bytes),
            ByteSize::b(h.memory_total_bytes),
            h.load_avg_1,
            h.load_avg_5,
            h.load_avg_15,
        )),
        Line::from(format!(
            " disk free {} / {}   net rx/tx {}   tick #{}",
            ByteSize::b(h.disk_available_bytes),
            ByteSize::b(h.disk_total_bytes),
            if h.net_rx_bps.is_some() || h.net_tx_bps.is_some() {
                format_bps_pair(h.net_rx_bps, h.net_tx_bps)
            } else {
                format!(
                    "{}/{}",
                    ByteSize::b(h.net_rx_bytes),
                    ByteSize::b(h.net_tx_bytes)
                )
            },
            app.tick,
        )),
    ];

    let block = Block::default().borders(Borders::ALL).title(" host ");
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let ranked = app.ranked();
    let header = Row::new([
        "KIND",
        "ID",
        "NAME",
        "CPU%",
        "MEM",
        "MEM%",
        "DISK R/W",
        "NET RX/TX",
        "STATUS",
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .bottom_margin(0);

    let rows = ranked.iter().map(|s| {
        let kind = match s.kind {
            TargetKind::Process => "proc",
            TargetKind::Container => "ctr",
            TargetKind::Host => "host",
        };
        let cpu_style = if s.cpu_percent >= 80.0 {
            Style::default().fg(Color::Red)
        } else if s.cpu_percent >= 50.0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        };
        let mem_pct = s
            .memory_percent()
            .map(|p| format!("{p:.1}"))
            .unwrap_or_else(|| "-".into());
        let disk = if s.disk_read_bps.is_some() || s.disk_write_bps.is_some() {
            format_bps_pair(s.disk_read_bps, s.disk_write_bps)
        } else {
            format!(
                "{}/{}",
                ByteSize::b(s.disk_read_bytes),
                ByteSize::b(s.disk_write_bytes)
            )
        };
        let net = if s.net_rx_bps.is_some() || s.net_tx_bps.is_some() {
            format_bps_pair(s.net_rx_bps, s.net_tx_bps)
        } else {
            format!(
                "{}/{}",
                ByteSize::b(s.net_rx_bytes),
                ByteSize::b(s.net_tx_bytes)
            )
        };

        // Sanitize free-text before char-truncate so BEL/CSI never reach the TTY.
        let id = sanitize_control_chars(&s.id)
            .chars()
            .take(12)
            .collect::<String>();
        let name = sanitize_control_chars(&s.name)
            .chars()
            .take(24)
            .collect::<String>();
        let status: String = s
            .status
            .as_deref()
            .map(|st| sanitize_control_chars(st).chars().take(16).collect())
            .unwrap_or_else(|| "-".into());

        Row::new(vec![
            Cell::from(kind),
            Cell::from(id),
            Cell::from(name),
            Cell::from(format!("{:.1}", s.cpu_percent)).style(cpu_style),
            Cell::from(ByteSize::b(s.memory_bytes).to_string()),
            Cell::from(mem_pct),
            Cell::from(disk),
            Cell::from(net),
            Cell::from(status),
        ])
    });

    let widths = [
        Constraint::Length(5),
        Constraint::Length(12),
        Constraint::Percentage(22),
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Percentage(14),
    ];

    let title = format!(
        " targets  ·  sort={:?}  ·  filter={}  ·  n={} ",
        app.sort_by,
        app.filter.as_deref().unwrap_or("*"),
        ranked.len()
    );

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help = " [q]uit  [r]efresh  [s]ort  [j/k] move  ·  Jentayu watches from above ";
    let msg = app
        .last_error
        .as_deref()
        .map(|e| format!(" error: {e} "))
        .unwrap_or_else(|| help.into());
    let style = if app.last_error.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(Paragraph::new(msg).style(style), area);
}
