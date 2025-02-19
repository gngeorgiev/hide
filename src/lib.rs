use std::collections::HashMap;
use zellij_tile::prelude::PaneInfo;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
pub struct NewInstanceMessage {
    pub name: String,
    pub path: String,
}

#[derive(Debug)]
pub enum V0Message {
    NewInstance(NewInstanceMessage),
}

#[derive(Debug)]
pub enum Message {
    V0(V0Message),
}

#[derive(Debug)]
pub struct Instance {
    pub pane: PaneInfo,
    pub tab_index: usize,
    pub typ: InstanceType,
}

#[derive(Debug)]
pub enum InstanceType {
    Helix,
    Yazi,
}

pub fn parse_pipe_message(payload: &str) -> Result<Message> {
    // versioning will become more enforced with v1
    let version = payload.chars().next().ok_or("invalid payload len 0")?;
    if !version.is_numeric() {
        return Err("first character must be a protocol version".into());
    }

    let mut idx_start = 1 as usize;
    let mut parts = vec![];
    for (mut idx, ch) in payload.chars().skip(1).enumerate() {
        idx += 1;
        if ch == ';' {
            parts.push(&payload[idx_start..idx]);
            idx_start = idx + 1;
        }
    }

    let command = parts
        .first()
        .ok_or("command is required after protocol version")?;

    let mut kvs = HashMap::new();
    for part in parts.iter().skip(1) {
        let idx = part.find("=").ok_or("no kv pair in command args")?;

        let k = &part[..idx];
        let v = &part[idx + 1..];

        kvs.insert(k, v);
    }

    let message = match *command {
        "new_instance" => Message::V0(V0Message::NewInstance(NewInstanceMessage {
            name: (*kvs.get("name").ok_or("name is required")?).into(),
            path: (*kvs.get("path").ok_or("path is required")?).into(),
        })),
        _ => return Err(format!("invalid protocol message {command}")),
    };

    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pipe_message_new_instance() {
        let payload = "0new_instance;name=test_instance;";
        let message = parse_pipe_message(payload).unwrap();

        match message {
            Message::V0(V0Message::NewInstance(instance)) => {
                assert_eq!(instance.name, "test_instance");
            }
        }
    }

    #[test]
    fn test_parse_pipe_message_invalid_version() {
        let payload = "xnew_instance;name=test_instance;";
        let message = parse_pipe_message(payload);

        assert!(message.is_err());
        assert_eq!(
            message.err().unwrap(),
            "first character must be a protocol version".to_string()
        );
    }

    #[test]
    fn test_parse_pipe_message_invalid_message() {
        let payload = "0invalid_message;name=test_instance;";
        let message = parse_pipe_message(payload);

        assert!(message.is_err());
        assert_eq!(
            message.err().unwrap(),
            "invalid protocol message invalid_message".to_string()
        );
    }

    #[test]
    fn test_parse_pipe_message_missing_instance_name() {
        let payload = "0new_instance;";
        let message = parse_pipe_message(payload);

        assert!(message.is_err());
        assert_eq!(message.err().unwrap(), "name is required".to_string());
    }

    #[test]
    fn test_parse_pipe_message_empty_payload() {
        let payload = "";
        let message = parse_pipe_message(payload);

        assert!(message.is_err());
        assert_eq!(message.err().unwrap(), "invalid payload len 0".to_string());
    }
}
