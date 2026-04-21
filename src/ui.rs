use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::gh::{Outcome, Stage};
use crate::row::Row;

// ANSI helpers — produce styled strings when color is enabled, plain otherwise.
fn ansi(code: &str, s: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn dim(s: &str, color: bool) -> String {
    ansi("2", s, color)
}

fn red(s: &str, color: bool) -> String {
    ansi("31", s, color)
}

fn bold_green(s: &str, color: bool) -> String {
    ansi("1;32", s, color)
}

fn bold_red(s: &str, color: bool) -> String {
    ansi("1;31", s, color)
}

const TICK_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ";
const TICK_INTERVAL: Duration = Duration::from_millis(100);

pub trait ProgressReporter: Send + Sync {
    /// Called by `arm_all` before dispatching `arm()`; starts the per-row
    /// timer and (in TTY mode) begins spinning the row's ProgressBar.
    fn mark_started(&self, idx: usize);

    /// Called by `arm_all` after `arm()` returns, with the elapsed time
    /// measured by `arm_all` from `mark_started` to `arm()` return.
    fn mark_finished(&self, idx: usize, outcome: &Outcome, elapsed: Duration);
}

#[allow(dead_code)]
pub struct NopReporter;

impl ProgressReporter for NopReporter {
    fn mark_started(&self, _idx: usize) {}
    fn mark_finished(&self, _idx: usize, _outcome: &Outcome, _elapsed: Duration) {}
}

pub struct Panel {
    tty: bool,
    multi: MultiProgress,
    bars: Vec<ProgressBar>,
    labels: Vec<String>,
}

/// Wraps a cloned `MultiProgress` (cheap Arc ref-count bump) for use in the
/// SIGINT handler. Shares the same underlying state as the live `Panel`.
pub struct PanelHandle(MultiProgress);

impl PanelHandle {
    pub fn abort(&self) {
        self.0.clear().ok();
    }
}

impl Panel {
    pub fn new(rows: &[Row], color: bool) -> Self {
        let multi = if color {
            MultiProgress::new()
        } else {
            MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
        };

        let running_style = ProgressStyle::with_template("  {spinner:.green} {msg}")
            .expect("valid template")
            .tick_chars(TICK_CHARS);

        let bars = rows
            .iter()
            .map(|row| {
                let bar = multi.add(ProgressBar::new_spinner());
                bar.set_style(running_style.clone());
                bar.set_message(row.label.clone());
                bar
            })
            .collect();

        Panel {
            tty: color,
            multi,
            bars,
            labels: rows.iter().map(|r| r.label.clone()).collect(),
        }
    }

    pub fn handle(&self) -> PanelHandle {
        PanelHandle(self.multi.clone())
    }

    /// Print a line above the panel (visible in both TTY and non-TTY modes).
    pub fn println(&self, line: &str) {
        self.multi.println(line).ok();
    }

    /// Normal completion — all bars are already finished; just clean up.
    pub fn finish(&self) {
        self.multi.clear().ok();
    }
}

impl ProgressReporter for Panel {
    fn mark_started(&self, idx: usize) {
        if self.tty {
            self.bars[idx].enable_steady_tick(TICK_INTERVAL);
        }
    }

    fn mark_finished(&self, idx: usize, outcome: &Outcome, elapsed: Duration) {
        let label = &self.labels[idx];
        let elapsed_s = format!("{:.1}s", elapsed.as_secs_f64());

        if self.tty {
            let bar = &self.bars[idx];
            bar.disable_steady_tick();
            match outcome {
                Outcome::Armed => {
                    let style = ProgressStyle::with_template("{msg}").expect("valid template");
                    bar.set_style(style);
                    bar.finish_with_message(format!(
                        "  {} {}  {}",
                        bold_green("✓", true),
                        label,
                        dim(&elapsed_s, true),
                    ));
                }
                Outcome::Failed { stage, stderr, .. } => {
                    let style = ProgressStyle::with_template("{msg}").expect("valid template");
                    bar.set_style(style);
                    bar.finish_with_message(format!(
                        "  {} {}  {}",
                        red("✗", true),
                        label,
                        red(&format!("{stage} failed"), true),
                    ));
                    for line in stderr.lines() {
                        self.multi.println(format!("      {line}")).ok();
                    }
                }
            }
        } else {
            // Non-TTY: one plain line per PR, no animation.
            match outcome {
                Outcome::Armed => eprintln!("{}", format_row_success(label, elapsed, false)),
                Outcome::Failed { stage, stderr, .. } => {
                    eprintln!("{}", format_row_failure(label, stage, false));
                    for line in stderr.lines() {
                        eprintln!("      {line}");
                    }
                }
            }
        }
    }
}

// ── Pure formatters ───────────────────────────────────────────────────────────

pub fn format_row_success(label: &str, elapsed: Duration, color: bool) -> String {
    let elapsed_s = format!("{:.1}s", elapsed.as_secs_f64());
    format!(
        "  {} {}  {}",
        bold_green("✓", color),
        label,
        dim(&elapsed_s, color),
    )
}

pub fn format_row_failure(label: &str, stage: &Stage, color: bool) -> String {
    format!(
        "  {} {}  {}",
        red("✗", color),
        label,
        red(&format!("{stage} failed"), color),
    )
}

pub fn format_row_dry_run(label: &str, _color: bool) -> String {
    format!("  → {label}  (dry-run)")
}

/// Always returns a non-empty string. Callers gate invocation on
/// `rows.len() > 1` so this function has a clean, always-valid contract.
pub fn format_summary(outcomes: &[Outcome], elapsed: Duration, color: bool) -> String {
    let armed = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Armed))
        .count();
    let failed = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Failed { .. }))
        .count();
    let elapsed_s = format!("{:.1}s", elapsed.as_secs_f64());
    let n = outcomes.len();

    if failed == 0 {
        bold_green(&format!("Armed {n} PRs in {elapsed_s}"), color)
    } else if armed == 0 {
        bold_red(&format!("{n} failed in {elapsed_s}"), color)
    } else {
        let failed_part = red(&format!("{failed} failed"), color);
        format!("Armed {armed} PRs, {failed_part} in {elapsed_s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armed() -> Outcome {
        Outcome::Armed
    }

    fn failed() -> Outcome {
        Outcome::Failed {
            stage: Stage::Ready,
            stderr: String::new(),
            exit_code: 1,
        }
    }

    #[test]
    fn success_contains() {
        let s = format_row_success("BitGo/foo#15", Duration::from_millis(1234), false);
        assert!(s.contains("BitGo/foo#15"), "label");
        assert!(s.contains('✓'), "checkmark");
        assert!(s.contains("1.2s"), "elapsed");
    }

    #[test]
    fn failure_contains() {
        let s = format_row_failure("x", &Stage::Merge, false);
        assert!(s.contains('x'), "label");
        assert!(s.contains('✗'), "cross");
        assert!(s.contains("merge"), "stage");
    }

    #[test]
    fn dry_run_contains() {
        let s = format_row_dry_run("x", false);
        assert!(s.contains('x'), "label");
        assert!(s.contains("dry-run"), "dry-run");
    }

    #[test]
    fn summary_all_success() {
        let outcomes = vec![armed(), armed()];
        let s = format_summary(&outcomes, Duration::from_secs_f64(5.0), false);
        assert!(s.contains('2'), "count");
        assert!(s.contains("Armed"), "armed");
        assert!(s.contains("5.0s"), "elapsed");
    }

    #[test]
    fn summary_mixed() {
        let outcomes = vec![armed(), failed()];
        let s = format_summary(&outcomes, Duration::from_secs_f64(3.0), false);
        assert!(s.contains('1'), "success count");
        assert!(s.contains("failed"), "failed");
        assert!(s.contains("3.0s"), "elapsed");
    }

    #[test]
    fn summary_all_failed() {
        let outcomes = vec![failed(), failed()];
        let s = format_summary(&outcomes, Duration::from_secs_f64(1.0), false);
        assert!(s.contains('2'), "count");
        assert!(s.contains("failed"), "failed");
        assert!(s.contains("1.0s"), "elapsed");
    }
}
