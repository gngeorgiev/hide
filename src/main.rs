#![feature(let_chains)]

use std::{
    collections::{HashMap, VecDeque},
    ops::{Deref, Index},
    time::{SystemTime, UNIX_EPOCH},
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
                        let mut split = terminal_command.split_whitespace();

                        if let Some(cmd) = split.next()
                            && cmd != "hide-cli"
                        {
                            continue;
                        }

                        let Some(session_id) = split.next() else {
                            continue;
                        };

                        let session_id = session_id.parse::<u128>().expect("invalid session id");

                        let typ = match split.next() {
                            Some(typ) => match typ {
                                "hx" => InstanceType::Helix,
                                "yazi" => InstanceType::Yazi,
                                _ => continue,
                            },
                            None => continue,
                        };

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
        // dbg!("Handle pipe message: ", &msg);
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
