mod git;
mod github;
mod little_anyhow;

fn print_usage() {
    println!("\nUsage:");
    println!("  gh arm [<number> | <url> | <branch> | -]...");
    println!("\nOptions:");
    println!("  <number>    PR number to merge");
    println!("  <url>       PR URL to merge");
    println!("  <branch>    Branch name to merge");
    println!("  -           Previous branch (like git checkout -)");
}

fn main() -> Result<(), little_anyhow::Error> {
    use git::get_previous_branch;
    use github::process_pull_request;

    let args: Vec<String> = std::env::args().collect();

    // Check for help flag anywhere in the arguments
    for arg in &args[1..] {
        if arg == "-h" || arg == "--help" {
            print_usage();
            return Ok(());
        }
    }

    // If no additional arguments are provided, process the current branch
    if args.len() == 1 {
        process_pull_request(None)?;
        return Ok(());
    }

    // Process each argument sequentially
    for pull_request_identifier in args.iter().skip(1) {
        // Handle the special case for `-` (previous branch)
        if *pull_request_identifier == "-" {
            let branch = get_previous_branch()?;
            process_pull_request(Some(&branch))?;
        } else {
            process_pull_request(Some(pull_request_identifier))?;
        }
    }

    Ok(())
}
