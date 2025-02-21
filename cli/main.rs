use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: hide-cli <session_id> <command> [args...]");
        std::process::exit(1);
    }

    let cwd = env::current_dir()?;

    let _session_id = env::var("SESSION_ID").unwrap_or_default();
    let plugin_name = env::var("PLUGIN_NAME").unwrap_or("hide".to_string());
    let command = &args[1];
    let command_args = &args[2..];
    let status = match command.as_str() {
        "run" => run_command(&command_args)?,
        "pipe" => pipe_command(&plugin_name, &command_args)?,
        "new" => {
            let path = command_args
                .get(0)
                .map(|s| PathBuf::from_str(s.as_str()))
                .ok_or("invalid path".to_string())?
                .unwrap_or(cwd);

            let file_name = if path.is_file() {
                path.parent().and_then(|f| f.file_name())
            } else {
                path.file_name()
            };

            let file_name = file_name
                .ok_or(format!("invalid file path {path:?}"))
                .map(|f| f.to_str().map(String::from))?
                .ok_or("invalid path buf {path:?}".to_string())?;

            let path = path.to_string_lossy().to_string();

            pipe_command(&plugin_name, &[
                "new_instance".into(),
                format!("name={file_name}"),
                format!("path={}", &path),
            ])?
        }
        _ => return Err("invalid command: {command}".into()),
    };

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn run_command(args: &[String]) -> Result<ExitStatus> {
    let command = &args[0];
    let command_args = &args[1..];
    let mut cmd = Command::new(command);
    cmd.current_dir(env::current_dir()?)
        .envs(env::vars())
        .args(command_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.status().map_err(Into::into)
}

fn pipe_command(plugin_name: &str, args: &[String]) -> Result<ExitStatus> {
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
    cmd.current_dir(env::current_dir()?)
        .envs(env::vars())
        .arg("action")
        .arg("pipe")
        .arg("--plugin")
        .arg(plugin_name)
        .arg("--")
        .arg(message)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.status().map_err(Into::into)
}
