//! Browser automation and inspection tools, unified broker, and common utilities.

pub mod broker;
pub mod common;
pub mod firefox;
pub mod gsd;
pub mod obscura;
pub mod status;

pub use broker::{
    browser_backend_priority, eval_with_browser_broker, render_with_browser_broker,
    BrowserBackendChoice, BrowserBrokerResult,
};
pub use common::{
    connect_to_tab, ensure_browser_running, kill_browser_on_port_9222, send_cdp_cmd, WsSink,
    WsStream,
};
pub use firefox::FirefoxBrowserTool;
pub use gsd::{stop_gsd_browser_daemon, GsdBrowserTool};
pub use obscura::ObscuraBrowserTool;
pub use status::{
    browser_preflight_error_value, BrowserBackendStatus, BrowserHealth, InspectBrowsersTool,
};
