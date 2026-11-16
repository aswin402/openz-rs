//! Native OpenZ Browser Engine (WebKit2GTK + GTK3 + In-Page `PageController`).
//!
//! Provides a unified headless (`GtkOffscreenWindow`) and GUI (`GtkWindow`) browser
//! engine with zero external browser binaries, zero WebDriver/CDP daemons, and hard
//! JavaScriptCore / WebKit memory pressure limits.

pub mod broker;
pub mod client;
pub mod common;
pub mod status;
pub mod tool;
pub mod webkit_ffi;
pub mod worker;

pub use broker::{
    browser_backend_priority, eval_with_browser_broker, render_page_with_browser_broker,
    render_with_browser_broker, BrowserBackendChoice, BrowserBrokerResult, RenderedBrowserPage,
};
pub use client::{
    export_html_or_url_to_pdf, list_active_browser_sessions, render_target_to_png,
    BrowserSessionClient, ScreenshotRequest, SessionLaunchSpec,
};
pub use status::{
    browser_preflight_error_value, collect_browser_status_report, BrowserBackend,
    BrowserBackendStatus, BrowserHealth,
};
pub use tool::NativeBrowserTool;
pub use webkit_ffi::{probe_webkit_runtime, WebKitRuntimeProbe};
pub use worker::{
    maybe_run_browser_cli_from_env_args, run_browser_worker, BrowserWorkerOptions,
};
