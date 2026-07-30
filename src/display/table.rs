//! Pretty table output for one-shot and watch modes.

use crate::metrics::{filter_and_rank, format_bps_pair, Snapshot, SortBy, TargetKind};
use bytesize::ByteSize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};

pub fn print_snapshot(
    snapshot: &Snapshot,
    name_filter: Option<&str>,
    sort_by: SortBy,
    top: Option<usize>,
) {
    print_host(snapshot);
    println!();

    let ranked = filter_and_rank(&snapshot.targets, name_filter, sort_by, top);
    if ranked.is_empty() {
        println!("(no matching targets)");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("KIND").add_attribute(Attribute::Bold),
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("NAME").add_attribute(Attribute::Bold),
            Cell::new("CPU%").add_attribute(Attribute::Bold),
            Cell::new("MEM").add_attribute(Attribute::Bold),
            Cell::new("MEM%").add_attribute(Attribute::Bold),
            // Values include "/s" when interval rates are known; else cumulative.
            Cell::new("DISK R/W").add_attribute(Attribute::Bold),
            Cell::new("NET RX/TX").add_attribute(Attribute::Bold),
            Cell::new("STATUS").add_attribute(Attribute::Bold),
        ]);

    for s in &ranked {
        let kind = match s.kind {
            TargetKind::Process => "proc",
            TargetKind::Container => "ctr",
            TargetKind::Host => "host",
        };
        let mem_pct = s
            .memory_percent()
            .map(|p| format!("{p:.1}"))
            .unwrap_or_else(|| "-".into());
        // Prefer interval rates; fall back to cumulative totals on first sample.
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

        let cpu_cell = colorize_cpu(s.cpu_percent);

        table.add_row(vec![
            Cell::new(kind),
            Cell::new(truncate(&s.id, 12)),
            Cell::new(truncate(&s.name, 28)),
            cpu_cell,
            Cell::new(ByteSize::b(s.memory_bytes).to_string()),
            Cell::new(mem_pct),
            Cell::new(disk),
            Cell::new(net),
            Cell::new(s.status.as_deref().unwrap_or("-")),
        ]);
    }

    println!("{table}");
    println!(
        "sampled at {}  ·  {} targets  ·  sorted by {:?}",
        snapshot.sampled_at.format("%Y-%m-%d %H:%M:%S UTC"),
        ranked.len(),
        sort_by
    );
}

fn print_host(snapshot: &Snapshot) {
    let h = &snapshot.host;
    let mem_pct = h.memory_percent();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!(
        "║  JENTAYU  ·  host CPU {:>5.1}%  ·  MEM {:>5.1}% ({}/{})  ║",
        h.cpu_percent,
        mem_pct,
        ByteSize::b(h.memory_used_bytes),
        ByteSize::b(h.memory_total_bytes),
    );
    let net = if h.net_rx_bps.is_some() || h.net_tx_bps.is_some() {
        format_bps_pair(h.net_rx_bps, h.net_tx_bps)
    } else {
        format!(
            "{}/{}",
            ByteSize::b(h.net_rx_bytes),
            ByteSize::b(h.net_tx_bytes)
        )
    };
    println!(
        "║  load {:.2} {:.2} {:.2}  ·  disk free {} / {}  ·  net {}  ║",
        h.load_avg_1,
        h.load_avg_5,
        h.load_avg_15,
        ByteSize::b(h.disk_available_bytes),
        ByteSize::b(h.disk_total_bytes),
        net,
    );
    println!("╚══════════════════════════════════════════════════════════════════╝");
}

fn colorize_cpu(cpu: f32) -> Cell {
    let text = format!("{cpu:.1}");
    let cell = Cell::new(text);
    if cpu >= 80.0 {
        cell.fg(Color::Red)
    } else if cpu >= 50.0 {
        cell.fg(Color::Yellow)
    } else {
        cell.fg(Color::Green)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
