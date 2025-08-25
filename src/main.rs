use std::env;
use std::process::{Command, exit};

fn print_usage() {
    println!("\nUsage:");
    println!("  gh arm [<number> | <url> | <branch>]...");
    println!("\nOptions:");
    println!("  <number>    PR number to merge");
    println!("  <url>       PR URL to merge");
    println!("  <branch>    Branch name to merge");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Check for help flag anywhere in the arguments
    for arg in &args[1..] {
        if arg == "-h" || arg == "--help" {
            print_usage();
            exit(0);
        }
    }

    // If no additional arguments are provided, process the current branch
    if args.len() == 1 {
        process_pull_request(None);
        return;
    }

    // Process each argument sequentially
    for pull_request_identifier in args.iter().skip(1) {
        process_pull_request(Some(pull_request_identifier));
    }
}

fn process_pull_request(pull_request_identifier: Option<&str>) {
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
        .status();

    match ready_status {
        Err(e) => {
            eprintln!("Failed to execute ready command: {}", e);
            exit(1);
        }
        Ok(status) if !status.success() => {
            exit(status.code().unwrap_or(1));
        }
        _ => {}
    }

    // Then enable auto-merge
    let merge_status = automerge_command
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match merge_status {
        Err(e) => {
            eprintln!("Failed to execute merge command: {}", e);
            exit(1);
        }
        Ok(status) if !status.success() => {
            exit(status.code().unwrap_or(1));
        }
        _ => {}
    }
}
