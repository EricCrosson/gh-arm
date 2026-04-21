mod gh;
mod git;
mod little_anyhow;
mod pr_ref;
mod row;
mod ui;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use gh::{GhRunner, Outcome, RealGhRunner, build_merge_args, build_ready_args};
use row::Row;
use ui::ProgressReporter;

fn print_usage() {
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  gh arm [<pr>...]");
    eprintln!();
    eprintln!("Each <pr> is one of:");
    eprintln!("  <number>             PR number               (cwd repo)");
    eprintln!("  <branch>             Branch name             (cwd repo)");
    eprintln!("  -                    Previous branch         (cwd repo + git)");
    eprintln!("  OWNER/REPO#NUMBER    Qualified atom          (any repo, no cwd needed)");
    eprintln!();
    eprintln!("Flags:");
    eprintln!("  --dry-run            Print the `gh` invocations that would run; execute nothing.");
    eprintln!("  -j, --jobs <N>       Maximum concurrent PRs (default: 1; clamp [1, 8]).");
    eprintln!("  -h, --help           Print this message.");
    eprintln!();
    eprintln!("No args → arm the PR for the current branch.");
    eprintln!("All args must be bare refs OR qualified atoms; mixing is rejected.");
}

struct Args {
    help: bool,
    dry_run: bool,
    jobs: usize,
    positional: Vec<String>,
}

#[derive(Debug)]
enum ArgParseError {
    MissingJobsValue,
    InvalidJobsValue(String),
}

impl std::fmt::Display for ArgParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgParseError::MissingJobsValue => write!(f, "--jobs requires a value"),
            ArgParseError::InvalidJobsValue(v) => {
                write!(f, "--jobs value must be a positive integer, got {v:?}")
            }
        }
    }
}

impl std::error::Error for ArgParseError {}

fn parse_argv() -> Result<Args, ArgParseError> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        help: false,
        dry_run: false,
        jobs: 1,
        positional: Vec::new(),
    };

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "-h" | "--help" => args.help = true,
            "--dry-run" => args.dry_run = true,
            "-j" | "--jobs" => {
                i += 1;
                if i >= raw.len() {
                    return Err(ArgParseError::MissingJobsValue);
                }
                let n: usize = raw[i]
                    .parse()
                    .map_err(|_| ArgParseError::InvalidJobsValue(raw[i].clone()))?;
                args.jobs = n.clamp(1, 8);
            }
            s if s.starts_with("--jobs=") => {
                let val = &s["--jobs=".len()..];
                let n: usize = val
                    .parse()
                    .map_err(|_| ArgParseError::InvalidJobsValue(val.to_string()))?;
                args.jobs = n.clamp(1, 8);
            }
            _ => args.positional.push(raw[i].clone()),
        }
        i += 1;
    }
    Ok(args)
}

/// Resolve raw positional argv into `Vec<Row>`, substituting `-` with the
/// previous branch via `branch_resolver`. Injectable for tests.
fn resolve_args(
    positional: &[String],
    branch_resolver: impl Fn() -> Result<String, git::GetPreviousBranchError>,
) -> Result<Vec<Row>, little_anyhow::Error> {
    let resolved: Vec<pr_ref::PrRef> = positional
        .iter()
        .map(|a| {
            if a == "-" {
                branch_resolver()
                    .map(pr_ref::PrRef::Bare)
                    .map_err(little_anyhow::Error::from)
            } else {
                pr_ref::parse_arg(a).map_err(little_anyhow::Error::from)
            }
        })
        .collect::<Result<_, _>>()?;

    pr_ref::validate_homogeneous(&resolved)?;

    let rows = if resolved.is_empty() {
        vec![Row::current_branch()]
    } else {
        resolved.into_iter().map(Row::from).collect()
    };

    Ok(rows)
}

fn effective_jobs(requested: usize, is_tty: bool) -> usize {
    // In non-TTY mode, force sequential to keep output in-order.
    // No warning emitted; this is a silent, documented constraint.
    if is_tty { requested } else { 1 }
}

fn arm_all(
    runner: Arc<dyn GhRunner>,
    rows: &[Row],
    reporter: &dyn ProgressReporter,
    jobs: usize,
) -> Vec<Outcome> {
    if jobs == 1 {
        rows.iter()
            .enumerate()
            .map(|(idx, row)| {
                reporter.mark_started(idx);
                let t0 = Instant::now();
                let outcome = gh::arm(&*runner, row.target.as_pref());
                reporter.mark_finished(idx, &outcome, t0.elapsed());
                outcome
            })
            .collect()
    } else {
        use std::sync::mpsc;

        let (result_tx, result_rx) = mpsc::channel::<(usize, Outcome, std::time::Duration)>();
        // Semaphore: pre-fill with `jobs` permits.
        let (permit_tx, permit_rx) = mpsc::sync_channel::<()>(jobs);
        for _ in 0..jobs {
            permit_tx.send(()).ok();
        }

        for (idx, row) in rows.iter().enumerate() {
            // Block until a permit is available (bounded concurrency).
            permit_rx
                .recv()
                .expect("permit channel unexpectedly closed");
            reporter.mark_started(idx);

            let tx = result_tx.clone();
            let permit_tx = permit_tx.clone();
            let runner = Arc::clone(&runner);
            let target = row.target.as_pref().cloned();

            std::thread::spawn(move || {
                let t0 = Instant::now();
                let outcome = gh::arm(&*runner, target.as_ref());
                let elapsed = t0.elapsed();
                tx.send((idx, outcome, elapsed)).ok();
                permit_tx.send(()).ok();
            });
        }
        // Drop the main sender so result_rx terminates when all workers finish.
        drop(result_tx);

        let mut pairs = Vec::with_capacity(rows.len());
        for (idx, outcome, elapsed) in result_rx {
            reporter.mark_finished(idx, &outcome, elapsed);
            pairs.push((idx, outcome));
        }
        order_by_input(pairs)
    }
}

/// Sort `(idx, Outcome)` pairs by index so results always match input order.
fn order_by_input(mut pairs: Vec<(usize, Outcome)>) -> Vec<Outcome> {
    pairs.sort_by_key(|(idx, _)| *idx);
    pairs.into_iter().map(|(_, o)| o).collect()
}

fn exit_code(outcomes: &[Outcome]) -> ExitCode {
    if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let args = match parse_argv() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    if args.help {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let rows = match resolve_args(&args.positional, git::get_previous_branch) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e:?}");
            return ExitCode::from(1);
        }
    };

    let color = std::io::IsTerminal::is_terminal(&std::io::stderr());

    // --dry-run: no panel, no threads, plain output.
    if args.dry_run {
        for row in &rows {
            let target = row.target.as_pref();
            eprintln!("{}", ui::format_row_dry_run(&row.label, color));
            eprintln!("    gh {}", build_ready_args(target).join(" "));
            eprintln!("    gh {}", build_merge_args(target).join(" "));
        }
        return ExitCode::SUCCESS;
    }

    // Panel::new does not modify the terminal (no cursor hide, no draw yet).
    // The SIGINT handler is registered as soon as a PanelHandle exists,
    // immediately after Panel::new returns.
    let panel = ui::Panel::new(&rows, color);
    if let Err(e) = ctrlc::set_handler({
        let handle = panel.handle();
        move || {
            handle.abort();
            std::process::exit(130);
        }
    }) {
        eprintln!("warning: could not install SIGINT handler: {e}");
    }

    let jobs = effective_jobs(args.jobs, color);
    let runner: Arc<dyn GhRunner> = Arc::new(RealGhRunner);
    let run_start = Instant::now();
    let outcomes = arm_all(Arc::clone(&runner), &rows, &panel, jobs);
    let total_elapsed = run_start.elapsed();

    if rows.len() > 1 {
        panel.println(&ui::format_summary(&outcomes, total_elapsed, color));
    }
    panel.finish();

    exit_code(&outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    use pr_ref::PrRef;
    use row::RowTarget;
    use ui::NopReporter;

    // ── MockRunner ────────────────────────────────────────────────────────────

    struct MockRunner {
        results: std::sync::Mutex<VecDeque<Result<(), gh::RunFailure>>>,
        calls: std::sync::Mutex<Vec<Vec<String>>>,
    }

    impl MockRunner {
        fn new(results: Vec<Result<(), gh::RunFailure>>) -> Self {
            MockRunner {
                results: std::sync::Mutex::new(results.into_iter().collect()),
                calls: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl gh::GhRunner for MockRunner {
        fn run(&self, args: &[String]) -> Result<(), gh::RunFailure> {
            self.calls.lock().unwrap().push(args.to_vec());
            self.results.lock().unwrap().pop_front().unwrap_or(Ok(()))
        }
    }

    fn failure() -> Result<(), gh::RunFailure> {
        Err(gh::RunFailure {
            stderr: "error from gh".into(),
            exit_code: 1,
        })
    }

    fn bare_row(n: &str) -> Row {
        Row::from(PrRef::Bare(n.into()))
    }

    // ── resolve_args tests ────────────────────────────────────────────────────

    #[test]
    fn resolve_dash_substitutes_branch() {
        let rows = resolve_args(&["-".to_string()], || Ok("my-branch".to_string())).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].target,
            RowTarget::Pr(PrRef::Bare("my-branch".into()))
        );
    }

    #[test]
    fn resolve_empty_produces_current_branch_row() {
        let rows = resolve_args(&[], || unreachable!()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].target, RowTarget::CurrentBranch);
    }

    #[test]
    fn resolve_single_positional() {
        let rows = resolve_args(&["123".to_string()], || unreachable!()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].target, RowTarget::Pr(PrRef::Bare("123".into())));
    }

    // ── arm_all / try-all tests ───────────────────────────────────────────────

    #[test]
    fn try_all_continues_after_ready_failure() {
        // PR 1 ready fails; PR 2 and PR 3 should still be fully processed.
        let mock = Arc::new(MockRunner::new(vec![
            failure(), // PR 1 ready → fail (merge skipped)
            Ok(()),    // PR 2 ready
            Ok(()),    // PR 2 merge
            Ok(()),    // PR 3 ready
            Ok(()),    // PR 3 merge
        ]));
        let runner: Arc<dyn gh::GhRunner> = mock.clone();
        let rows = vec![bare_row("1"), bare_row("2"), bare_row("3")];
        let outcomes = arm_all(runner, &rows, &NopReporter, 1);

        assert!(matches!(
            outcomes[0],
            Outcome::Failed {
                stage: gh::Stage::Ready,
                ..
            }
        ));
        assert!(matches!(outcomes[1], Outcome::Armed));
        assert!(matches!(outcomes[2], Outcome::Armed));
        assert_eq!(
            mock.calls().len(),
            5,
            "5 calls: ready(1), ready(2), merge(2), ready(3), merge(3)"
        );
    }

    #[test]
    fn exit_code_all_armed() {
        let outcomes = vec![Outcome::Armed, Outcome::Armed];
        assert_eq!(exit_code(&outcomes), ExitCode::SUCCESS);
    }

    #[test]
    fn exit_code_any_failed() {
        let outcomes = vec![
            Outcome::Armed,
            Outcome::Failed {
                stage: gh::Stage::Ready,
                stderr: String::new(),
                exit_code: 1,
            },
        ];
        assert_eq!(exit_code(&outcomes), ExitCode::from(1));
    }

    #[test]
    fn exit_code_all_failed() {
        let outcomes = vec![
            Outcome::Failed {
                stage: gh::Stage::Ready,
                stderr: String::new(),
                exit_code: 1,
            },
            Outcome::Failed {
                stage: gh::Stage::Merge,
                stderr: String::new(),
                exit_code: 1,
            },
        ];
        assert_eq!(exit_code(&outcomes), ExitCode::from(1));
    }

    #[test]
    fn single_row_current_branch_no_identifier() {
        let mock = Arc::new(MockRunner::new(vec![Ok(()), Ok(())]));
        let runner: Arc<dyn gh::GhRunner> = mock.clone();
        let rows = vec![Row::current_branch()];
        let outcomes = arm_all(runner, &rows, &NopReporter, 1);

        assert!(matches!(outcomes[0], Outcome::Armed));
        let calls = mock.calls();
        assert_eq!(calls[0], vec!["pr", "ready"], "ready with no identifier");
        assert_eq!(
            calls[1],
            vec!["pr", "merge", "--auto", "--merge"],
            "merge with no identifier"
        );
    }

    #[test]
    fn single_row_bare_number_passes_identifier() {
        let mock = Arc::new(MockRunner::new(vec![Ok(()), Ok(())]));
        let runner: Arc<dyn gh::GhRunner> = mock.clone();
        let rows = vec![bare_row("123")];
        let _ = arm_all(runner, &rows, &NopReporter, 1);

        let calls = mock.calls();
        assert_eq!(calls[0], vec!["pr", "ready", "123"]);
        assert_eq!(calls[1], vec!["pr", "merge", "--auto", "--merge", "123"]);
    }

    #[test]
    fn order_by_input_sorts_by_index() {
        let pairs = vec![
            (2, Outcome::Armed),
            (0, Outcome::Armed),
            (
                1,
                Outcome::Failed {
                    stage: gh::Stage::Ready,
                    stderr: String::new(),
                    exit_code: 1,
                },
            ),
        ];
        let ordered = order_by_input(pairs);
        assert!(matches!(ordered[0], Outcome::Armed));
        assert!(matches!(ordered[1], Outcome::Failed { .. }));
        assert!(matches!(ordered[2], Outcome::Armed));
    }
}
