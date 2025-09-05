use std::process::{Command, Stdio};

pub(crate) fn get_previous_branch() -> Result<String, GetPreviousBranchError> {
    // First verify the placeholder is resolvable
    let status = Command::new("git")
        .args(["rev-parse", "-q", "--verify", "@{-1}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| {
            GetPreviousBranchError::CommandFailed("rev-parse --verify @{-1}".to_string())
        })?;

    if !status.success() {
        return Err(GetPreviousBranchError::NoPreviousCheckout);
    }

    // Get abbreviated branch name
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{-1}"])
        .output()
        .map_err(|_| {
            GetPreviousBranchError::CommandFailed("rev-parse --abbrev-ref @{-1}".to_string())
        })?;

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // If we're not in a detached state, we have a good name
    if name != "HEAD" {
        return Ok(name);
    }

    // Otherwise, try to get a more meaningful name
    match Command::new("git")
        .args(["name-rev", "--name-only", "--no-undefined", "@{-1}"])
        .output()
    {
        Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Err(_) => Ok(name), // Fall back to HEAD if that failed
    }
}

#[derive(Debug)]
pub(crate) enum GetPreviousBranchError {
    CommandFailed(String),
    NoPreviousCheckout,
}

impl std::fmt::Display for GetPreviousBranchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GetPreviousBranchError::CommandFailed(cmd) => {
                write!(f, "unable to execute command: {}", cmd)
            }
            GetPreviousBranchError::NoPreviousCheckout => {
                write!(
                    f,
                    "unable to resolve '-' branch identifier: no previous checkout recorded"
                )
            }
        }
    }
}

impl std::error::Error for GetPreviousBranchError {}
