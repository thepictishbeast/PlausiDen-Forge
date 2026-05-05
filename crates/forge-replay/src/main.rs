//! Forge — log replay tool (T38).
//!
//! Walks `reports/build-*.json` + `/tmp/skillshots-server-slow.log`,
//! producing a terminal-friendly summary that surfaces:
//!
//! 1. **Recent build trend** — last N builds: strict / warn / duration.
//! 2. **Findings churn** — what was added/removed since the previous
//!    build (per-phase). Highlights flapping detectors.
//! 3. **Slow-URL hotspots** — top URLs by mean ms + max ms,
//!    aggregated from the dev-server slow log.
//!
//! Exit code is informational; non-zero only if file I/O fails.
//! The replay tool is a triage helper, not a build gate.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use forge_core::{BuildReport, Finding};

#[derive(Parser, Debug)]
#[command(
    name = "forge-replay",
    version,
    about = "Forge log replay — surface trends, churn, slow URLs."
)]
struct Args {
    /// Project root containing reports/ and /tmp/skillshots-server-slow.log.
    /// Defaults to CWD.
    #[arg(long)]
    root: Option<PathBuf>,

    /// Number of recent builds to show in the trend table.
    #[arg(long, default_value_t = 10)]
    last: usize,

    /// Path to dev-server slow log (forge-serve writes here when
    /// SLOW_REQUEST_MS_THRESHOLD is exceeded).
    #[arg(long, default_value = "/tmp/skillshots-server-slow.log")]
    slow_log: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = match args.root {
        Some(p) => p,
        None => std::env::current_dir().context("CWD unreadable")?,
    };
    let reports_dir = root.join("reports");

    println!(
        "forge-replay {} root={}",
        env!("CARGO_PKG_VERSION"),
        root.display()
    );

    // --- Load build history (Rust forge JSON only; bash format
    //     skipped on parse error). ---
    let mut history = load_reports(&reports_dir)?;
    history.sort_by(|a, b| a.path.cmp(&b.path)); // chronological by filename
    let total = history.len();
    if total == 0 {
        println!(
            "\nno parseable build reports found in {}",
            reports_dir.display()
        );
    } else {
        render_trend_table(&history, args.last);
        render_churn(&history);
    }

    // --- Slow log ---
    render_slow_hotspots(&args.slow_log)?;

    Ok(())
}

/// One report entry with its filesystem path for chronological sort.
struct StoredReport {
    path: PathBuf,
    report: BuildReport,
}

/// Walk reports/build-*.json. Skips files we can't parse (bash-era
/// reports have a different shape — they survive as historical
/// data but aren't replayable here).
fn load_reports(reports_dir: &Path) -> Result<Vec<StoredReport>> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(reports_dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(anyhow::anyhow!("read_dir {}: {e}", reports_dir.display()));
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("build-") || !name.ends_with(".json") {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<BuildReport>(&text) {
                Ok(report) => out.push(StoredReport { path, report }),
                Err(_) => { /* skip bash-era / unparseable */ }
            },
            Err(_) => { /* skip unreadable */ }
        }
    }
    Ok(out)
}

/// Render the "last N builds" trend table.
fn render_trend_table(history: &[StoredReport], last: usize) {
    let n = history.len().min(last);
    let slice = &history[history.len() - n..];
    println!(
        "\n=== build trend (last {} of {} reports) ===",
        n,
        history.len()
    );
    println!(
        "  {:<32} {:>6} {:>6} {:>10}",
        "report", "strict", "warn", "duration"
    );
    println!(
        "  {} {} {} {}",
        "-".repeat(32),
        "-".repeat(6),
        "-".repeat(6),
        "-".repeat(10)
    );
    for s in slice {
        let stem = s.path.file_stem().and_then(|x| x.to_str()).unwrap_or("?");
        println!(
            "  {:<32} {:>6} {:>6} {:>9}ms",
            stem, s.report.strict_count, s.report.warn_count, s.report.duration_ms
        );
    }
}

/// Diff consecutive reports — surface flapping findings.
fn render_churn(history: &[StoredReport]) {
    if history.len() < 2 {
        return;
    }
    let prev = &history[history.len() - 2].report;
    let curr = &history[history.len() - 1].report;
    let prev_set = finding_set(prev);
    let curr_set = finding_set(curr);

    let added: Vec<&String> = curr_set.iter().filter(|k| !prev_set.contains(*k)).collect();
    let removed: Vec<&String> = prev_set.iter().filter(|k| !curr_set.contains(*k)).collect();

    if added.is_empty() && removed.is_empty() {
        println!("\n=== findings churn (latest vs previous) ===");
        println!("  ✓ no churn — stable build");
        return;
    }
    println!("\n=== findings churn (latest vs previous) ===");
    if !added.is_empty() {
        println!("  + {} new finding(s):", added.len());
        for k in &added {
            println!("      {k}");
        }
    }
    if !removed.is_empty() {
        println!("  - {} resolved finding(s):", removed.len());
        for k in &removed {
            println!("      {k}");
        }
    }
}

/// Hash a finding into a stable string for set membership.
fn finding_set(r: &BuildReport) -> std::collections::HashSet<String> {
    r.findings.iter().map(finding_key).collect()
}

fn finding_key(f: &Finding) -> String {
    format!("{}|{}|{}", f.phase, f.path, f.message)
}

/// Read the dev-server slow log + render top hot URLs.
fn render_slow_hotspots(slow_log: &Path) -> Result<()> {
    if !slow_log.exists() {
        println!(
            "\n=== slow URL hotspots ===\n  no slow log at {} — nothing to replay",
            slow_log.display()
        );
        return Ok(());
    }
    let text =
        fs::read_to_string(slow_log).with_context(|| format!("read {}", slow_log.display()))?;

    // Parse format: `<ts> METHOD path STATUS BYTESb MSms`
    // (forge-serve uses time-prefix bracketed; tolerate both).
    struct Hit {
        ms: u128,
    }
    let mut by_url: BTreeMap<String, Vec<Hit>> = BTreeMap::new();
    for line in text.lines() {
        // Find " METHOD path STATUS ... MSms"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        // Find the last token ending in "ms"
        let Some(ms_tok) = parts.iter().rev().find(|t| t.ends_with("ms")) else {
            continue;
        };
        let ms_str = ms_tok.trim_end_matches("ms");
        let Ok(ms) = ms_str.parse::<u128>() else {
            continue;
        };
        // Heuristic: find the GET/POST/HEAD/etc token, the path is the next.
        let method_idx = parts
            .iter()
            .position(|t| matches!(*t, "GET" | "POST" | "HEAD" | "PUT" | "DELETE" | "PATCH"));
        let path_token = match method_idx {
            Some(i) if i + 1 < parts.len() => parts[i + 1].to_owned(),
            _ => continue,
        };
        by_url.entry(path_token).or_default().push(Hit { ms });
    }

    if by_url.is_empty() {
        println!("\n=== slow URL hotspots ===\n  log present but no parseable lines");
        return Ok(());
    }

    // Compute mean / max / count per URL.
    let mut summarized: Vec<(String, f64, u128, usize)> = by_url
        .into_iter()
        .map(|(url, hits)| {
            let count = hits.len();
            let sum: u128 = hits.iter().map(|h| h.ms).sum();
            let mean = sum as f64 / count as f64;
            let max = hits.iter().map(|h| h.ms).max().unwrap_or(0);
            (url, mean, max, count)
        })
        .collect();

    println!("\n=== slow URL hotspots ===");
    summarized.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("  top 5 by mean ms:");
    println!(
        "  {:<40} {:>8} {:>8} {:>6}",
        "url", "mean ms", "max ms", "count"
    );
    println!(
        "  {} {} {} {}",
        "-".repeat(40),
        "-".repeat(8),
        "-".repeat(8),
        "-".repeat(6)
    );
    for (url, mean, max, count) in summarized.iter().take(5) {
        println!("  {:<40} {:>8.0} {:>8} {:>6}", url, mean, max, count);
    }
    summarized.sort_by_key(|s| std::cmp::Reverse(s.2));
    println!("\n  top 5 by max ms:");
    println!(
        "  {:<40} {:>8} {:>8} {:>6}",
        "url", "mean ms", "max ms", "count"
    );
    println!(
        "  {} {} {} {}",
        "-".repeat(40),
        "-".repeat(8),
        "-".repeat(8),
        "-".repeat(6)
    );
    for (url, mean, max, count) in summarized.iter().take(5) {
        println!("  {:<40} {:>8.0} {:>8} {:>6}", url, mean, max, count);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finding_key_stable() {
        let f = Finding::warn("p", "x.html", "msg");
        assert_eq!(finding_key(&f), "p|x.html|msg");
    }
}
