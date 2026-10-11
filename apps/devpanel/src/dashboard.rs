//! The bones side of the app: one native module that owns the web panel,
//! carries page JSON to the [`Controller`], and carries events back.
//!
//! It is the only file that speaks the bones bus. Desktop capabilities the
//! page asks for — a folder picker, the browser, the clipboard — are
//! forwarded to the engine's `os` module rather than reimplemented.

use std::collections::HashMap;
use std::time::Instant;

use bones_engine::bus::{Bus, Envelope, Handler, Module, ModuleContext, Registry};
use bones_engine::logging::Logger;
use bones_engine::messages::os::{Action as OsAction, Request as OsRequest, Result as OsResult};
use bones_engine::messages::tick::Tick;
use bones_engine::messages::web::{Command, OpenPanel, PageMessage, PanelFailed, PanelSource, SendJson};
use bones_engine::messages::{DecodeMessage, EncodeMessage, Message};

use crate::controller::Controller;
use crate::protocol::{Action, Event};

/// Bus endpoint name; the web module also uses it as the panel owner.
pub const NAME: &str = "devpanel";
const PANEL: &str = "main";

pub struct Dashboard {
    controller: Controller,
    logger: Logger,
    page: String,
    bus: Option<Bus>,
    registry: Option<Registry>,
    opened: bool,
    /// `os` request id → the page request waiting for its answer.
    pending: HashMap<u32, u64>,
    next_os_request: u32,
}

impl Dashboard {
    pub fn new(controller: Controller, logger: Logger, page: String) -> Self {
        Self {
            controller,
            logger,
            page,
            bus: None,
            registry: None,
            opened: false,
            pending: HashMap::new(),
            next_os_request: 0,
        }
    }

    /// Opened on the first tick rather than in `init`, so the window and
    /// every module are fully up before the webview is created.
    fn open_panel(&mut self) {
        self.opened = true;
        let command = Command::Open(OpenPanel { panel: PANEL, source: PanelSource::Html(&self.page) });
        self.call_web(&command.encode());
    }

    fn call_web(&self, payload: &[u8]) {
        let Some(registry) = &self.registry else { return };
        if let Err(error) = registry.call(NAME, bones_engine::messages::web::ENDPOINT, payload) {
            self.logger.error(NAME, &format!("web panel unavailable: {error:?}"));
        }
    }

    fn send_events(&self, events: &[Event]) {
        if events.is_empty() {
            return;
        }
        match serde_json::to_string(events) {
            Ok(json) => self.call_web(&Command::SendJson(SendJson { panel: PANEL, json: &json }).encode()),
            Err(error) => self.logger.error(NAME, &format!("encoding events: {error}")),
        }
    }

    fn on_page_message(&mut self, json: &str) {
        let action: Action = match serde_json::from_str(json) {
            Ok(action) => action,
            Err(error) => {
                self.logger.warn(NAME, &format!("ignoring page message {json}: {error}"));
                return;
            }
        };
        match action {
            Action::PickFolder { request } => self.ask_os(OsAction::PickFolder, "", Some(request)),
            Action::OpenUrl { url } => self.ask_os(OsAction::OpenUrl, &url, None),
            Action::Copy { text } => self.ask_os(OsAction::WriteClipboard, &text, None),
            action => self.controller.handle(action),
        }
    }

    fn ask_os(&mut self, action: OsAction, value: &str, page_request: Option<u64>) {
        let Some(bus) = &self.bus else { return };
        self.next_os_request = self.next_os_request.wrapping_add(1);
        let request_id = self.next_os_request;
        if let Some(page_request) = page_request {
            self.pending.insert(request_id, page_request);
        }
        bus.publish(Envelope {
            topic: OsRequest::TOPIC.into(),
            sender: NAME.into(),
            correlation: None,
            payload: OsRequest { request_id, action, value }.encode(),
        });
    }

    fn on_os_result(&mut self, result: OsResult<'_>) {
        // Results are broadcast; only ids this module issued are its own.
        let Some(page_request) = self.pending.remove(&result.request_id) else { return };
        // Not accepted with no error is a cancelled dialog: an empty answer
        // the page ignores, not a failure to report.
        let reply = if result.accepted || result.error.is_empty() {
            Ok(serde_json::Value::String(result.value.to_string()))
        } else {
            Err(devpanel_core::HubError::Command(result.error.to_string()))
        };
        self.controller.reply(page_request, reply);
    }
}

impl Handler for Dashboard {
    fn handle(&mut self, envelope: &Envelope) {
        match envelope.topic.as_str() {
            Tick::TOPIC => {
                if !self.opened {
                    self.open_panel();
                }
                self.controller.tick(Instant::now());
                let events = self.controller.drain();
                self.send_events(&events);
            }
            PageMessage::TOPIC => {
                if let Ok(message) = PageMessage::decode(&envelope.payload) {
                    if message.owner == NAME {
                        self.on_page_message(message.json);
                    }
                }
            }
            OsResult::TOPIC => {
                if let Ok(result) = OsResult::decode(&envelope.payload) {
                    self.on_os_result(result);
                }
            }
            PanelFailed::TOPIC => {
                if let Ok(failed) = PanelFailed::decode(&envelope.payload) {
                    self.logger.error(NAME, &format!("panel failed: {}", failed.reason));
                }
            }
            _ => {}
        }
    }
}

impl Module for Dashboard {
    fn name(&self) -> &str {
        NAME
    }

    fn init(&mut self, ctx: &mut ModuleContext) -> Result<(), String> {
        self.bus = ctx.get_service::<Bus>().cloned();
        self.registry = ctx.get_service::<Registry>().cloned();
        if self.registry.is_none() {
            return Err("the engine did not provide the call registry".into());
        }
        for topic in [Tick::TOPIC, PageMessage::TOPIC, OsResult::TOPIC, PanelFailed::TOPIC] {
            ctx.subscribe(topic);
        }
        Ok(())
    }
}
