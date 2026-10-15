//! The bones side of the app: one native module that owns the web panel,
//! carries page JSON to the [`Controller`], and carries events back.
//!
//! It is the only file that speaks the bones bus. Desktop capabilities the
//! page asks for — a folder picker, the browser, the clipboard, desktop
//! registration, updates — are forwarded to the engine's `os`, `desktop` and
//! `upgrader` modules rather than reimplemented. A later `devpanel <path>`
//! arrives from the `instance` module and is handed to the controller like
//! the startup argument.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use bones_kernel::bus::{Bus, Envelope, Handler, Module, ModuleContext, Registry};
use bones_kernel::logging::Logger;
use bones_kernel::messages::desktop::{
    Action as DesktopRequestAction, Outcome as DesktopOutcome, Request as DesktopRequest, Result as DesktopResult,
};
use bones_kernel::messages::instance::Launched;
use bones_kernel::messages::os::{Action as OsAction, Request as OsRequest, Result as OsResult};
use bones_kernel::messages::tick::Tick;
use bones_kernel::messages::upgrader::{
    Action as UpgraderAction, Outcome as UpgraderOutcome, Request as UpgraderRequest, Result as UpgraderResult,
};
use bones_kernel::messages::web::{Command, OpenPanel, PageMessage, PanelFailed, PanelSource, SendJson};
use bones_kernel::messages::{DecodeMessage, EncodeMessage, Message};

use crate::controller::Controller;
use crate::protocol::{Action, DesktopAction, Event, UpdateAction};

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
    /// Request id sent to an engine module → the page request waiting for
    /// its answer. One counter serves every module, so ids never collide.
    pending: HashMap<u32, u64>,
    next_request: u32,
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
            next_request: 0,
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
        if let Err(error) = registry.call(NAME, bones_kernel::messages::web::ENDPOINT, payload) {
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
            Action::Desktop { request, action } => self.ask_desktop(action, request),
            Action::Update { request, action } => self.ask_upgrader(action, request),
            action => self.controller.handle(action),
        }
    }

    /// A fresh id for a request to an engine module, remembering the page
    /// request to answer when one waits for it.
    fn request_id(&mut self, page_request: Option<u64>) -> u32 {
        self.next_request = self.next_request.wrapping_add(1);
        if let Some(page_request) = page_request {
            self.pending.insert(self.next_request, page_request);
        }
        self.next_request
    }

    fn publish(&self, topic: &str, payload: Vec<u8>) {
        let Some(bus) = &self.bus else { return };
        bus.publish(Envelope { topic: topic.into(), sender: NAME.into(), correlation: None, payload });
    }

    fn ask_os(&mut self, action: OsAction, value: &str, page_request: Option<u64>) {
        let request_id = self.request_id(page_request);
        self.publish(OsRequest::TOPIC, OsRequest { request_id, action, value }.encode());
    }

    fn ask_desktop(&mut self, action: DesktopAction, page_request: u64) {
        let request_id = self.request_id(Some(page_request));
        let action = match action {
            DesktopAction::Register => DesktopRequestAction::Register,
            DesktopAction::Unregister => DesktopRequestAction::Unregister,
            DesktopAction::Status => DesktopRequestAction::Status,
        };
        self.publish(DesktopRequest::TOPIC, DesktopRequest { request_id, action }.encode());
    }

    fn ask_upgrader(&mut self, action: UpdateAction, page_request: u64) {
        let request_id = self.request_id(Some(page_request));
        let action = match action {
            UpdateAction::Check => UpgraderAction::Check,
            UpdateAction::Stage => UpgraderAction::Stage,
        };
        self.publish(UpgraderRequest::TOPIC, UpgraderRequest { request_id, action }.encode());
    }

    fn on_upgrader_result(&mut self, result: UpgraderResult<'_>) {
        let Some(page_request) = self.pending.remove(&result.request_id) else { return };
        let reply = match result.outcome {
            UpgraderOutcome::Available { version } => Ok(serde_json::json!({ "available": version })),
            UpgraderOutcome::Staged { version } => Ok(serde_json::json!({ "staged": version })),
            UpgraderOutcome::Failed { reason } => Err(devpanel_core::HubError::Command(reason.to_string())),
            // Up to date, or an answer to an action DevPanel never asks for.
            _ => Ok(serde_json::json!({})),
        };
        self.controller.reply(page_request, reply);
    }

    fn on_desktop_result(&mut self, result: DesktopResult<'_>) {
        let Some(page_request) = self.pending.remove(&result.request_id) else { return };
        let reply = match result.outcome {
            DesktopOutcome::Registered(registered) => Ok(serde_json::Value::Bool(registered)),
            DesktopOutcome::Failed(reason) => Err(devpanel_core::HubError::Command(reason.to_string())),
        };
        self.controller.reply(page_request, reply);
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
            UpgraderResult::TOPIC => {
                if let Ok(result) = UpgraderResult::decode(&envelope.payload) {
                    self.on_upgrader_result(result);
                }
            }
            DesktopResult::TOPIC => {
                if let Ok(result) = DesktopResult::decode(&envelope.payload) {
                    self.on_desktop_result(result);
                }
            }
            Launched::TOPIC => {
                // bones raises the window; a launch with a folder also opens it.
                if let Ok(launched) = Launched::decode(&envelope.payload) {
                    if let Some(arg) = launched.args.first() {
                        self.controller.open_path(Path::new(launched.cwd).join(arg));
                    }
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
        for topic in [Tick::TOPIC, PageMessage::TOPIC, OsResult::TOPIC, PanelFailed::TOPIC, Launched::TOPIC, DesktopResult::TOPIC, UpgraderResult::TOPIC] {
            ctx.subscribe(topic);
        }
        Ok(())
    }
}
