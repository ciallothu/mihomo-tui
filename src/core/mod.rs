//! Core business logic modules for mihomo-tui.
//!
//! These modules handle the main functional domains:
//!
//! - [`kernel`] – Download, manage, and switch mihomo kernel binaries.
//! - [`subscription`] – Manage subscription URLs and fetch proxy lists.
//! - [`config_file`] – Read, write, validate, and switch mihomo config files.
//! - [`provider`] – Interact with proxy/rule providers through the API.

pub mod config_file;
pub mod kernel;
pub mod provider;
pub mod subscription;
