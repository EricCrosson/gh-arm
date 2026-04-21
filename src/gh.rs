use std::process::{Command, Stdio};

use crate::pr_ref::PrRef;

#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Ready,
    Merge,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stage::Ready => write!(f, "ready"),
            Stage::Merge => write!(f, "merge"),
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Armed,
    Failed {
        stage: Stage,
        stderr: String,
        #[allow(dead_code)]
        exit_code: i32,
    },
}

#[derive(Debug)]
pub struct RunFailure {
    pub stderr: String,
    pub exit_code: i32,
}

pub trait GhRunner: Send + Sync {
    fn run(&self, args: &[String]) -> Result<(), RunFailure>;
}

pub struct RealGhRunner;

impl GhRunner for RealGhRunner {
    fn run(&self, args: &[String]) -> Result<(), RunFailure> {
        let output = Command::new("gh")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| RunFailure {
                stderr: e.to_string(),
                exit_code: -1,
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(RunFailure {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code().unwrap_or(-1),
            })
        }
    }
}

pub fn build_ready_args(target: Option<&PrRef>) -> Vec<String> {
    let mut args = vec!["pr".to_string(), "ready".to_string()];
    append_target(&mut args, target);
    args
}

pub fn build_merge_args(target: Option<&PrRef>) -> Vec<String> {
    let mut args = vec![
        "pr".to_string(),
        "merge".to_string(),
        "--auto".to_string(),
        "--merge".to_string(),
    ];
    append_target(&mut args, target);
    args
}

fn append_target(args: &mut Vec<String>, target: Option<&PrRef>) {
    match target {
        None => {}
        Some(PrRef::Bare(s)) => args.push(s.clone()),
        Some(PrRef::Qualified {
            owner,
            repo,
            number,
        }) => {
            args.push("--repo".to_string());
            args.push(format!("{owner}/{repo}"));
            args.push(number.clone());
        }
    }
}

pub fn arm(runner: &dyn GhRunner, target: Option<&PrRef>) -> Outcome {
    let ready_args = build_ready_args(target);
    if let Err(failure) = runner.run(&ready_args) {
        return Outcome::Failed {
            stage: Stage::Ready,
            stderr: failure.stderr,
            exit_code: failure.exit_code,
        };
    }

    let merge_args = build_merge_args(target);
    match runner.run(&merge_args) {
        Err(failure) => Outcome::Failed {
            stage: Stage::Merge,
            stderr: failure.stderr,
            exit_code: failure.exit_code,
        },
        Ok(()) => Outcome::Armed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(s: &str) -> PrRef {
        PrRef::Bare(s.into())
    }

    fn qualified(owner: &str, repo: &str, number: &str) -> PrRef {
        PrRef::Qualified {
            owner: owner.into(),
            repo: repo.into(),
            number: number.into(),
        }
    }

    #[test]
    fn build_ready_none() {
        assert_eq!(build_ready_args(None), vec!["pr", "ready"]);
    }

    #[test]
    fn build_ready_bare_number() {
        assert_eq!(
            build_ready_args(Some(&bare("123"))),
            vec!["pr", "ready", "123"]
        );
    }

    #[test]
    fn build_ready_bare_branch() {
        assert_eq!(
            build_ready_args(Some(&bare("feature-x"))),
            vec!["pr", "ready", "feature-x"]
        );
    }

    #[test]
    fn build_ready_qualified() {
        assert_eq!(
            build_ready_args(Some(&qualified("BitGo", "foo", "15"))),
            vec!["pr", "ready", "--repo", "BitGo/foo", "15"]
        );
    }

    #[test]
    fn build_merge_none() {
        assert_eq!(
            build_merge_args(None),
            vec!["pr", "merge", "--auto", "--merge"]
        );
    }

    #[test]
    fn build_merge_bare() {
        assert_eq!(
            build_merge_args(Some(&bare("123"))),
            vec!["pr", "merge", "--auto", "--merge", "123"]
        );
    }

    #[test]
    fn build_merge_qualified() {
        assert_eq!(
            build_merge_args(Some(&qualified("BitGo", "foo", "15"))),
            vec![
                "pr",
                "merge",
                "--auto",
                "--merge",
                "--repo",
                "BitGo/foo",
                "15"
            ]
        );
    }
}
