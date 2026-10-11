//! The heart of DevPanel: what a tracked repository is, how its condition is
//! judged, and the use cases the dashboard offers.
//!
//! Nothing here performs I/O. Git, the filesystem, process launching and
//! persistence are reached through the traits in [`ports`], which
//! `devpanel-infra` implements. That keeps every rule in this crate testable
//! with plain fakes and lets the presentation layer change without touching it.

pub mod domain;
pub mod error;
pub mod hub;
pub mod ports;

pub use error::{HubError, HubResult};
pub use hub::Hub;
