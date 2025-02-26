use std::collections::HashMap;
use zellij_tile::prelude::PaneInfo;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
pub struct NewInstanceMessage {
    pub name: String,
    pub path: String,
}

#[derive(Debug)]
pub struct EditFileMessage {
    pub path: String,
}

#[derive(Debug)]
pub struct FocusPaneMessage {
    pub typ: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WriteToPane {
    Bytes(Vec<u8>),
    String(String),
    Enter,
    Escape,
}

#[derive(Debug)]
pub struct WritesToPane(pub Vec<WriteToPane>);

impl TryFrom<&str> for WritesToPane {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let mut v = vec![];
        let mut chars = value.chars().enumerate();
        let mut str = String::new();

        while let Some((idx, ch)) = chars.next() {
            match ch {
                '<' => {
                    if !str.is_empty() {
                        v.push(WriteToPane::String(str.clone()));
                        str.truncate(0);
                    }

                    let Some(end) = chars.position(|(_, ch)| ch == '>') else {
                        return Err("< not terminated properly with a >".into());
                    };

                    let symbol = &value[idx + 1..idx + end + 1];
                    let w = match symbol {
                        "enter" => WriteToPane::Enter,
                        "esc" => WriteToPane::Escape,
                        _ => {
                            return Err(format!(
                                "invalid <symbol>: {symbol}, valid: <enter>, <esc>"
                            ));
                        }
                    };

                    v.push(w);
                }
                _ => {
                    str.push(ch);
                }
            }
        }

        if !str.is_empty() {
            v.push(WriteToPane::String(str));
        }

        Ok(WritesToPane(v))
    }
}

#[derive(Debug)]
pub struct WriteToPaneMessage {
    pub typ: PaneType,
    pub data: WritesToPane,
}

#[derive(Debug)]
pub enum V0Message {
    NewInstance(NewInstanceMessage),
    EditFile(EditFileMessage),
    FocusPane(FocusPaneMessage),
    WriteToPane(WriteToPaneMessage),
}

#[derive(Debug)]
pub enum Message {
    V0(V0Message),
}

#[derive(Debug, Default, Clone)]
pub struct InstancePane {
    pub info: PaneInfo,
    pub tab_index: usize,
    pub typ: PaneType,
}

#[derive(Debug, Eq, Clone)]
pub enum PaneType {
    Unknown,
    Editor,
    FileExplorer,
    Terminal,
    Custom(String),
}

impl PartialEq for PaneType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PaneType::Custom(a), PaneType::Custom(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }
}

impl Default for PaneType {
    fn default() -> Self {
        PaneType::Unknown
    }
}

impl From<&str> for PaneType {
    fn from(value: &str) -> Self {
        if ["editor", "helix", "hx"]
            .iter()
            .any(|e| value.eq_ignore_ascii_case(e))
        {
            PaneType::Editor
        } else if ["file explorer", "file_explorer", "yazi"]
            .iter()
            .any(|f| value.eq_ignore_ascii_case(f))
        {
            PaneType::FileExplorer
        } else if ["terminal", "shell", "term"]
            .iter()
            .any(|t| t.eq_ignore_ascii_case(value))
        {
            PaneType::Terminal
        } else {
            PaneType::Custom(value.into())
        }
    }
}

pub fn parse_pipe_message(payload: &str) -> Result<Message> {
    // versioning will become more enforced with v1
    let version = payload.chars().next().ok_or("invalid payload len 0")?;
    if !version.is_numeric() {
        return Err("first character must be a protocol version".into());
    }

    match version {
        '0' => parse_v0_message(&payload[1..]),
        _ => return Err("invalid protocol version {version}, only '0' is supported".into()),
    }
}

macro_rules! extract_message_key {
    ($kvs:expr, $key:expr) => {
        (*$kvs.get($key).ok_or(format!("{} is required", $key))?).into()
    };
}

fn parse_v0_message(payload: &str) -> Result<Message> {
    let mut idx_start = 0 as usize;
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

    // TODO: move this to the message to reduce amount of places we need to change
    let message = match *command {
        "new_instance" => V0Message::NewInstance(NewInstanceMessage {
            name: extract_message_key!(kvs, "name"),
            path: extract_message_key!(kvs, "path"),
        }),
        "edit_file" => V0Message::EditFile(EditFileMessage {
            path: extract_message_key!(kvs, "path"),
        }),
        "focus_pane" => V0Message::FocusPane(FocusPaneMessage {
            typ: extract_message_key!(kvs, "type"),
        }),
        "write_to_pane" => V0Message::WriteToPane(WriteToPaneMessage {
            typ: extract_message_key!(kvs, "type"),
            data: {
                let data_str: String = extract_message_key!(kvs, "data");
                data_str.as_str().try_into()?
            },
        }),
        _ => return Err(format!("invalid protocol message {command}")),
    };

    Ok(Message::V0(message))
}

pub fn extract_session_id_from_cmd(terminal_command: &str) -> Option<u128> {
    const SESSION_ID_MARKER: &str = "SESSION_ID=";

    let session_id_idx = terminal_command.find(SESSION_ID_MARKER)? + SESSION_ID_MARKER.len();
    let session_id_end_idx = terminal_command[session_id_idx..].find(' ')?;
    let session_id = terminal_command[session_id_idx..session_id_idx + session_id_end_idx]
        .parse::<u128>()
        .ok()?;

    return Some(session_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writes_to_pane_empty() {
        let input = "";
        let result = WritesToPane::try_from(input).unwrap();
        assert_eq!(result.0.len(), 0);
    }

    #[test]
    fn test_writes_to_pane_string_only() {
        let input = "hello world";
        let result = WritesToPane::try_from(input).unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0[0], WriteToPane::String("hello world".into()));
    }

    #[test]
    fn test_writes_to_pane_with_symbols() {
        let input = "<esc>:rla<enter>";
        let result = WritesToPane::try_from(input).unwrap();
        assert_eq!(result.0.len(), 3);
        assert_eq!(result.0[0], WriteToPane::Escape);
        assert_eq!(result.0[1], WriteToPane::String(":rla".into()));
        assert_eq!(result.0[2], WriteToPane::Enter);
    }

    #[test]
    fn test_writes_to_pane_invalid_symbol() {
        let input = "<invalid>";
        let result = WritesToPane::try_from(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "invalid <symbol>: invalid, valid: <enter>, <esc>".to_string()
        );
    }

    #[test]
    fn test_writes_to_pane_unclosed_symbol() {
        let input = "data=<esc";
        let result = WritesToPane::try_from(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "< not terminated properly with a >".to_string()
        );
    }

    #[test]
    fn test_parse_pipe_message_new_instance_with_path() {
        let payload = "0new_instance;name=test_instance;path=/tmp;";
        let message = parse_pipe_message(payload).unwrap();

        match message {
            Message::V0(V0Message::NewInstance(instance)) => {
                assert_eq!(instance.name, "test_instance");
                assert_eq!(instance.path, "/tmp");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_parse_pipe_message_edit_file() {
        let payload = "0edit_file;path=/tmp/foo.txt;";
        let message = parse_pipe_message(payload).unwrap();

        match message {
            Message::V0(V0Message::EditFile(instance)) => {
                assert_eq!(instance.path, "/tmp/foo.txt");
            }
            _ => unreachable!(),
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

    #[test]
    fn test_parse_pipe_message_protocol_version_1() {
        let payload = "1new_instance;name=test_instance;path=/tmp;";
        let message = parse_pipe_message(payload);

        assert!(message.is_err());
        assert_eq!(
            message.err().unwrap(),
            "invalid protocol version {version}, only \'0\' is supported".to_string()
        );
    }

    #[test]
    fn test_extract_session_id_helix_with_shell() {
        let terminal_command = "fish -c SESSION_ID=1234 hide-cli run hx .";
        let session_id = extract_session_id_from_cmd(terminal_command).unwrap();
        assert_eq!(session_id, 1234);
    }

    #[test]
    fn test_extract_session_id_helix() {
        let terminal_command = "fish -c SESSION_ID=1234 hide-cli run hx";
        let session_id = extract_session_id_from_cmd(terminal_command).unwrap();
        assert_eq!(session_id, 1234);
    }

    #[test]
    fn test_extract_session_id_yazi() {
        let terminal_command = "fish -c SESSION_ID=5678 hide-cli run yazi";
        let session_id = extract_session_id_from_cmd(terminal_command).unwrap();
        assert_eq!(session_id, 5678);
    }

    #[test]
    fn test_extract_session_id_missing_session_id() {
        let terminal_command = "fish -c hide-cli run hx";
        let result = extract_session_id_from_cmd(terminal_command);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_session_id_invalid_session_id() {
        let terminal_command = "fish -c SESSION_ID=abc hide-cli run hx";
        let result = extract_session_id_from_cmd(terminal_command);
        assert!(result.is_none());
    }
}
