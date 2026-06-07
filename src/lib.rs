pub mod app;
pub mod core;

pub use app::run_worker;
pub use core::{AutomationClient, IncomingMessage, MessageEv, Settings, StartedEv, TargetMessage};
