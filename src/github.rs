use std::process::Command;

pub(crate) fn process_pull_request(
    pull_request_identifier: Option<&str>,
) -> Result<(), ProcessPullRequestError> {
    // Create base commands
    let mut ready_command = Command::new("gh");
    ready_command.arg("pr").arg("ready");

    let mut automerge_command = Command::new("gh");
    automerge_command
        .arg("pr")
        .arg("merge")
        .arg("--auto")
        .arg("--merge");

    // Add PR identifier if provided
    if let Some(id) = pull_request_identifier {
        ready_command.arg(id);
        automerge_command.arg(id);
    }

    // First mark the PR as ready
    let ready_status = ready_command
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| ProcessPullRequestError::CommandFailed(format!("ready command: {}", e)))?;

    if !ready_status.success() {
        return Err(ProcessPullRequestError::ExecutionFailed(format!(
            "ready command exited with code {}",
            ready_status.code().unwrap_or(-1)
        )));
    }

    // Then enable auto-merge
    let merge_status = automerge_command
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| ProcessPullRequestError::CommandFailed(format!("merge command: {}", e)))?;

    if !merge_status.success() {
        return Err(ProcessPullRequestError::ExecutionFailed(format!(
            "merge command exited with code {}",
            merge_status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

#[derive(Debug)]
pub(crate) enum ProcessPullRequestError {
    CommandFailed(String),
    ExecutionFailed(String),
}

impl std::fmt::Display for ProcessPullRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessPullRequestError::CommandFailed(cmd) => {
                write!(f, "command exited unsuccessfully: {}", cmd)
            }
            ProcessPullRequestError::ExecutionFailed(msg) => {
                write!(f, "unable to execute command: {}", msg)
            }
        }
    }
}

impl std::error::Error for ProcessPullRequestError {}
