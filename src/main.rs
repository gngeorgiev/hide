#![feature(let_chains)]

use std::{
    collections::{HashMap, VecDeque},
    ops::{Deref, Index},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lib::*;

use zellij_tile::prelude::*;

mod lib;

static LAYOUT: &'static str = include_str!("../layouts/default.kdl");

#[derive(Default)]
struct State {
    initialized: bool,
    events_backlog: VecDeque<Event>,
    pipe_backlog: VecDeque<PipeMessage>,

    instances: HashMap<u128, Vec<Instance>>,
    // TODO: should we keep this even if there's no longer a focused pane?
    // maybe keeping it as the last focused session is fine as it will allow external
    // tools to somewhat interact with it through the cli without specifying a session explicitly
    focused_session_id: u128,
}

impl State {
    fn handle_event(&mut self, ev: Event) -> bool {
        match ev {
            Event::PaneUpdate(manifest) => {
                let mut update_instances = vec![];
                self.instances.drain();

                for (tab_index, panes) in manifest.panes {
                    for pane in panes {
                        if pane.is_plugin || pane.terminal_command.is_none() {
                            continue;
                        }

                        let terminal_command = pane.terminal_command.clone().unwrap();
                        let Some((typ, session_id)) = parse_terminal_command(&terminal_command)
                        else {
                            continue;
                        };

                        if pane.is_focused {
                            self.focused_session_id = session_id;
                        }

                        update_instances.push(session_id);
                        self.instances
                            .entry(session_id)
                            .or_insert_with(Vec::new)
                            .push(Instance {
                                pane,
                                tab_index,
                                typ,
                            });
                    }
                }

                dbg!(&self.instances);
            }
            _ => {}
        }

        false
    }

    fn handle_pipe_message(&mut self, msg: PipeMessage) -> bool {
        dbg!("Handle pipe message: ", &msg);
        let pipe_id = match msg.source {
            PipeSource::Cli(pipe_id) => pipe_id,
            _ => return false,
        };

        let payload = match msg.payload {
            Some(payload) => payload,
            _ => return false,
        };

        let message = match lib::parse_pipe_message(&payload) {
            Ok(message) => message,
            Err(err) => {
                dbg!("handle_pipe_message err:", err);
                return false;
            }
        };

        match message {
            lib::Message::V0(v0) => match v0 {
                V0Message::NewInstance(new_instance) => {
                    let session_id = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("get timestamp")
                        .as_millis();
                    self.new_tab(&new_instance.name, &new_instance.path, session_id);
                }
                V0Message::EditFile(edit_file) => {
                    if let Err(e) = self.edit_file(&edit_file.path) {
                        dbg!("edit_file error:", e);
                    }
                }
                V0Message::NavigateFileExplorer(navigate) => {
                    if let Err(e) = self.navigate_file_explorer(&navigate.path) {
                        eprintln!("navigate file explorer error: {e}");
                    }
                }
            },
        }

        false
    }

    fn new_tab(&self, tab_name: &str, path: &str, session_id: u128) {
        let mut layout = LAYOUT.to_string();
        layout = layout.replace("{tab_name}", tab_name);
        layout = layout.replace("{session_id}", format!("{session_id}").as_str());
        layout = layout.replace("{path}", path);

        new_tabs_with_layout(&layout);
    }

    fn find_instance_type(&self, typ: InstanceType) -> lib::Result<&Instance> {
        let session_id = &self.focused_session_id;
        let instances = self
            .instances
            .get(session_id)
            .ok_or_else(|| format!("invalid session id: {session_id}"))?;

        let instance = instances
            .iter()
            .find(|p| p.typ == typ)
            .ok_or_else(|| format!("invalid instance type {typ:?} for session {session_id}"))?;

        Ok(instance)
    }

    fn write_to_instance(&self, instance: &Instance, w: &[WriteToPane]) {
        let pane_id = PaneId::Terminal(instance.pane.id);
        focus_pane_with_id(pane_id, true);
        for w in w {
            thread::sleep(Duration::from_millis(50));

            match w {
                WriteToPane::Bytes(b) => write_to_pane_id(b.to_vec(), pane_id),
                WriteToPane::String(s) => write_chars_to_pane_id(s.as_str(), pane_id),
                WriteToPane::Enter => write_to_pane_id(vec![13], pane_id),
                WriteToPane::Escape => write_to_pane_id(vec![27], pane_id),
            }
        }
    }

    fn edit_file(&self, path: &str) -> lib::Result<()> {
        let helix = self.find_instance_type(InstanceType::Helix)?;
        self.write_to_instance(helix, &[
            // Write Esc to go back to normal mode
            WriteToPane::Escape,
            WriteToPane::String(format!(":o {}", path)),
            // Write Enter to confirm command
            WriteToPane::Enter,
        ]);

        Ok(())
    }

    fn navigate_file_explorer(&self, path: &str) -> lib::Result<()> {
        let explorer = self.find_instance_type(InstanceType::Yazi)?;
        self.write_to_instance(explorer, &[
            WriteToPane::String(format!(":ya emit cd {path}")),
            WriteToPane::Enter,
        ]);
        Ok(())
    }
}

enum WriteToPane {
    Bytes(Vec<u8>),
    String(String),
    Enter,
    Escape,
}

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: std::collections::BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
            PermissionType::WriteToStdin,
            PermissionType::RunCommands,
        ]);

        subscribe(&[EventType::PermissionRequestResult, EventType::PaneUpdate]);
    }

    fn update(&mut self, ev: Event) -> bool {
        // dbg!("Event update: ", &ev);
        if let Event::PermissionRequestResult(_) = ev {
            self.initialized = true;
        }

        self.events_backlog.push_back(ev);
        if !self.initialized {
            return false;
        }

        let mut render = false;
        while let Some(ev) = self.events_backlog.pop_front() {
            if self.handle_event(ev) {
                render = true
            }
        }

        render
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        // dbg!("Pipe message: ", &msg);
        self.pipe_backlog.push_back(msg);
        if !self.initialized {
            return false;
        }

        let mut render = false;
        while let Some(msg) = self.pipe_backlog.pop_front() {
            if self.handle_pipe_message(msg) {
                render = true
            }
        }

        render
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

register_plugin!(State);
