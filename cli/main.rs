use std::env;
use std::process::{Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: hide-cli <session_id> <command> [args...]");
        std::process::exit(1);
    }

    let _session_id = &args[1];
    let command = &args[2];
    let command_args = &args[3..];

    let mut cmd = Command::new(command);
    cmd.args(command_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    let status = child.wait()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
