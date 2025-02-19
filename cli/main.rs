use std::env;
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: hide-cli <session_id> <command> [args...]");
        std::process::exit(1);
    }

    let _session_id = env::var("SESSION_ID")?;
    let command = &args[1];
    let command_args = &args[2..];
    match command.as_str() {
        "run" => run_command(&command_args)?,
        "pipe" => pipe_command(&command_args)?,
        _ => return Err("invalid command: {command}".into()),
    }

    Ok(())
}

fn run_command(args: &[String]) -> Result<()> {
    let command = &args[0];
    let command_args = &args[1..];
    let mut cmd = Command::new(command);
    cmd.current_dir(env::current_dir()?)
        .envs(env::vars())
        .args(command_args)
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

fn pipe_command(args: &[String]) -> Result<()> {
    let mut message = String::new();
    message.push('0'); // protocol version
    message.push_str(&args[0]); // message type, e.g. edit_file
    message.push(';');
    for kv in &args[1..] {
        // message args, e.g. path=/tmp
        let mut split = kv.split('=');
        let k = split.next().ok_or("key: keyvalue pair required")?;
        let v = split.next().ok_or("value: keyvalue pair required")?;

        message.push_str(&format!("{}={};", k, v));
    }

    let mut cmd = Command::new("zellij");
    cmd.arg("action")
        .arg("pipe")
        .arg("--plugin")
        .arg("hide")
        .arg("--")
        .arg(message)
        .envs(env::vars())
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
