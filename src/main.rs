#![feature(let_chains)]

use std::{
    collections::{HashMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

use lib::*;

use zellij_tile::prelude::*;

mod lib;

static LAYOUT: &'static str = include_str!("../layouts/default.kdl");

#[derive(Default, Debug)]
struct FocusedSession {
    id: u128,
    tab: usize,
}

#[derive(Default)]
struct State {
    initialized: bool,
    events_backlog: VecDeque<Event>,
    pipe_backlog: VecDeque<PipeMessage>,

    focused_tab: TabInfo,
    instances: HashMap<u128, Vec<InstancePane>>,
    // TODO: should we keep this even if there's no longer a focused pane?
    // maybe keeping it as the last focused session is fine as it will allow external
    // tools to somewhat interact with it through the cli without specifying a session explicitly
    focused_session: FocusedSession,
}

impl State {
    fn handle_event(&mut self, ev: Event) -> bool {
        match ev {
            Event::TabUpdate(tabs) => {
                if let Some(active_tab) = tabs.into_iter().find(|tab| tab.active) {
                    self.focused_tab = active_tab;
                    self.set_focused_session();
                }
            }
            Event::PaneUpdate(manifest) => {
                self.instances.drain();

                for (tab_index, panes) in manifest.panes {
                    for info in panes {
                        if info.is_plugin {
                            continue;
                        }

                        let Some(terminal_command) = &info.terminal_command else {
                            continue;
                        };

                        let Some(session_id) =
                            extract_session_id_from_cmd(terminal_command.as_str())
                        else {
                            continue;
                        };

                        self.instances
                            .entry(session_id)
                            .or_insert_with(Vec::new)
                            .push(InstancePane {
                                typ: PaneType::from(info.title.as_str()),
                                info,
                                tab_index,
                            });
                    }
                }

                self.set_focused_session();
                dbg!(&self.focused_session);
            }
            _ => {}
        }

        false
    }

    // set_focused_session is a best effort to set the focused session based on the
    // selected tab and pane. It's called on both tab and pane updates to not rely on order
    // although generally the order is tab event followed by pane event.
    fn set_focused_session(&mut self) {
        for (session_id, panes) in &self.instances {
            if let Some(pane) = panes
                .iter()
                .find(|pane| pane.tab_index == self.focused_tab.position && pane.info.is_focused)
                .map(Clone::clone)
            {
                self.focused_session = FocusedSession {
                    id: *session_id,
                    tab: pane.tab_index,
                };
                break;
            }
        }
    }

    fn handle_pipe_message(&mut self, msg: PipeMessage) -> bool {
        dbg!("Handle pipe message: ", &msg);
        // let pipe_id = match msg.source {
        //     PipeSource::Cli(pipe_id) => pipe_id,
        //     _ => return false,
        // };

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
                        eprintln!("edit_file error: {e}");
                    }
                }
                V0Message::FocusPane(focus_pane) => {
                    if let Err(e) = self.focus_instance_type(focus_pane.typ.as_str().into()) {
                        eprintln!("error focus pane: {e}");
                    }
                }
                V0Message::WriteToPane(write_to_pane) => {
                    if let Err(e) = self.write_to_pane(write_to_pane.typ, &write_to_pane.data.0) {
                        eprintln!("error write to pane: {e}");
                    }
                }
            },
        }

        false
    }

    fn focus_instance_type(&self, typ: PaneType) -> lib::Result<()> {
        let instance = self.find_instance_by_type(typ)?;
        focus_pane_with_id(PaneId::Terminal(instance.info.id), true);

        Ok(())
    }

    fn new_tab(&self, tab_name: &str, path: &str, session_id: u128) {
        let mut layout = LAYOUT.to_string();
        layout = layout.replace("{tab_name}", tab_name);
        layout = layout.replace("{session_id}", format!("{session_id}").as_str());
        layout = layout.replace("{path}", path);

        new_tabs_with_layout(&layout);
    }

    fn find_instance_by_type(&self, typ: PaneType) -> lib::Result<&InstancePane> {
        let focused_session = &self.focused_session;
        let focused_session_id = &focused_session.id;
        let instances = self
            .instances
            .get(focused_session_id)
            .ok_or_else(|| format!("invalid session id: {focused_session_id}"))?;

        let instance = instances.iter().find(|p| p.typ.eq(&typ)).ok_or_else(|| {
            format!("invalid instance type {typ:?} for session {focused_session_id}")
        })?;

        Ok(instance)
    }

    fn write_to_pane(&self, typ: PaneType, w: &[WriteToPane]) -> lib::Result<()> {
        let instance = self.find_instance_by_type(typ)?;
        let pane_id = PaneId::Terminal(instance.info.id);
        focus_pane_with_id(pane_id, true);
        for w in w {
            match w {
                WriteToPane::Bytes(b) => write_to_pane_id(b.to_vec(), pane_id),
                WriteToPane::String(s) => write_chars_to_pane_id(s.as_str(), pane_id),
                WriteToPane::Enter => write_to_pane_id(vec![13], pane_id),
                WriteToPane::Escape => write_to_pane_id(vec![27], pane_id),
            }
        }

        Ok(())
    }

    fn edit_file(&self, path: &str) -> lib::Result<()> {
        self.write_to_pane(PaneType::Editor, &[
            // Write Esc to go back to normal mode
            WriteToPane::Escape,
            WriteToPane::String(format!(":o {}", path)),
            // Write Enter to confirm command
            WriteToPane::Enter,
        ])
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: std::collections::BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
            PermissionType::WriteToStdin,
            PermissionType::RunCommands,
        ]);

        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
            EventType::TabUpdate,
        ]);
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
