//! Dynamic runtime FFI bindings for GTK3, Cairo, GLib/GObject, JavaScriptCore, and WebKit2GTK.
//!
//! Loaded on demand inside the isolated `openz browser --worker` process via `libc::dlopen`
//! against versioned SONAMEs (`4.1` with automatic `4.0` fallback). Keeps zero WebKit/GTK
//! shared library overhead in the main `openz` agent runtime and requires zero `-dev` headers
//! or external Cargo `-sys` crates at compile time.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::ffi::{c_char, c_double, c_int, c_uint, c_void, CStr, CString};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebKitRuntimeProbe {
    pub available: bool,
    pub platform_supported: bool,
    pub webkit_soname: Option<String>,
    pub jsc_soname: Option<String>,
    pub gtk_soname: Option<String>,
    pub cairo_soname: Option<String>,
    pub display_backend: Option<String>,
    pub error: Option<String>,
}

pub fn find_broadwayd_binary() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|val| std::env::split_paths(&val).collect())
            .unwrap_or_default();
        for name in ["broadwayd", "gtk3-broadwayd"] {
            for dir in &path_dirs {
                let candidate = dir.join(name);
                match candidate.is_file() {
                    true => return Some(candidate),
                    false => {}
                }
            }
            for fixed in ["/usr/bin", "/usr/local/bin", "/bin"] {
                let candidate = PathBuf::from(fixed).join(name);
                match candidate.is_file() {
                    true => return Some(candidate),
                    false => {}
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn wayland_socket_reachable(sock: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let p = std::path::Path::new(sock);
        match p.is_absolute() {
            true => p.exists(),
            false => {
                let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
                    .ok()
                    .filter(|val| !val.trim().is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        let uid = unsafe { libc::getuid() };
                        PathBuf::from(format!("/run/user/{uid}"))
                    });
                runtime_dir.join(sock).exists()
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        !sock.is_empty()
    }
}

fn x11_display_reachable(disp: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        match disp.strip_prefix(':') {
            Some(rest) => {
                let num_part = rest.split('.').next().unwrap_or("");
                (!num_part.is_empty())
                    && PathBuf::from(format!("/tmp/.X11-unix/X{num_part}")).exists()
            }
            None => !disp.trim().is_empty(),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        !disp.is_empty()
    }
}

pub fn detect_display_environment() -> Option<(&'static str, String)> {
    let wayland_env = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|val| !val.trim().is_empty() && wayland_socket_reachable(val.trim()));
    match wayland_env {
        Some(sock) => Some(("wayland", sock)),
        None => {
            let x11_env = std::env::var("DISPLAY")
                .ok()
                .filter(|val| !val.trim().is_empty() && x11_display_reachable(val.trim()));
            match x11_env {
                Some(disp) => Some(("x11", disp)),
                None => discover_desktop_display_socket()
                    .or_else(|| find_broadwayd_binary().map(|_| ("broadway", ":5".to_string()))),
            }
        }
    }
}

fn discover_desktop_display_socket() -> Option<(&'static str, String)> {
    #[cfg(target_os = "linux")]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .ok()
            .filter(|val| !val.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let uid = unsafe { libc::getuid() };
                PathBuf::from(format!("/run/user/{uid}"))
            });

        for candidate in ["wayland-0", "wayland-1"] {
            let sock_path = runtime_dir.join(candidate);
            match sock_path.exists() {
                true => return Some(("wayland", candidate.to_string())),
                false => {}
            }
        }

        for display_num in [0u8, 1u8] {
            let x_sock = PathBuf::from(format!("/tmp/.X11-unix/X{display_num}"));
            match x_sock.exists() {
                true => return Some(("x11", format!(":{display_num}"))),
                false => {}
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Fast zero-RSS probe of WebKit2GTK + GTK3 + Cairo shared libraries on the host.
pub fn probe_webkit_runtime() -> WebKitRuntimeProbe {
    #[cfg(target_os = "linux")]
    {
        let search_dirs = candidate_library_dirs();
        let webkit = find_soname_in_dirs(
            &search_dirs,
            &["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37"],
        )
        .or_else(|| dlopen_check_any(&["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37"]));
        let jsc = find_soname_in_dirs(
            &search_dirs,
            &[
                "libjavascriptcoregtk-4.1.so.0",
                "libjavascriptcoregtk-4.0.so.18",
            ],
        )
        .or_else(|| {
            dlopen_check_any(&[
                "libjavascriptcoregtk-4.1.so.0",
                "libjavascriptcoregtk-4.0.so.18",
            ])
        });
        let gtk = find_soname_in_dirs(&search_dirs, &["libgtk-3.so.0"])
            .or_else(|| dlopen_check_any(&["libgtk-3.so.0"]));
        let cairo = find_soname_in_dirs(&search_dirs, &["libcairo.so.2"])
            .or_else(|| dlopen_check_any(&["libcairo.so.2"]));

        let display_backend =
            detect_display_environment().map(|(kind, target)| format!("{kind}:{target}"));
        let all_libs_present =
            webkit.is_some() && jsc.is_some() && gtk.is_some() && cairo.is_some();
        let available = all_libs_present && display_backend.is_some();
        let error = match (all_libs_present, display_backend.is_some()) {
            (true, true) => None,
            (false, _) => Some(
                "Missing runtime libraries (install libwebkit2gtk-4.1-0 and libgtk-3-0)".to_string(),
            ),
            (true, false) => Some(
                "No active Wayland or X11 display socket detected (set WAYLAND_DISPLAY or DISPLAY, or run under xvfb-run on bare servers)"
                    .to_string(),
            ),
        };

        WebKitRuntimeProbe {
            available,
            platform_supported: true,
            webkit_soname: webkit,
            jsc_soname: jsc,
            gtk_soname: gtk,
            cairo_soname: cairo,
            display_backend,
            error,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        WebKitRuntimeProbe {
            available: false,
            platform_supported: false,
            webkit_soname: None,
            jsc_soname: None,
            gtk_soname: None,
            cairo_soname: None,
            display_backend: None,
            error: Some(
                "Native WebKit2GTK runtime is currently implemented for Linux hosts".to_string(),
            ),
        }
    }
}

#[cfg(target_os = "linux")]
fn candidate_library_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var("LD_LIBRARY_PATH") {
        Ok(ld_path) => {
            for part in ld_path.split(':').filter(|s| !s.trim().is_empty()) {
                dirs.push(PathBuf::from(part));
            }
        }
        Err(_) => {}
    }
    for fixed in [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/lib64",
        "/usr/lib",
        "/lib",
        "/usr/local/lib",
    ] {
        dirs.push(PathBuf::from(fixed));
    }
    dirs
}

#[cfg(target_os = "linux")]
fn find_soname_in_dirs(dirs: &[PathBuf], sonames: &[&str]) -> Option<String> {
    for &soname in sonames {
        let found = dirs.iter().any(|dir| dir.join(soname).exists());
        match found {
            true => return Some(soname.to_string()),
            false => {}
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn dlopen_check_any(sonames: &[&str]) -> Option<String> {
    for &soname in sonames {
        let c_name = CString::new(soname).ok()?;
        let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
        match handle.is_null() {
            true => {}
            false => {
                unsafe {
                    libc::dlclose(handle);
                }
                return Some(soname.to_string());
            }
        }
    }
    None
}

// ─── C ABI Constants & Types ────────────────────────────────────────────────

pub const G_PRIORITY_DEFAULT_IDLE: c_int = 200;
pub const GTK_WINDOW_TOPLEVEL: c_int = 0;
pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
pub const GTK_ORIENTATION_VERTICAL: c_int = 1;
pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: c_uint = 600;

pub const CAIRO_FORMAT_ARGB32: c_int = 0;
pub const CAIRO_STATUS_SUCCESS: c_int = 0;

pub const WEBKIT_CACHE_MODEL_DOCUMENT_VIEWER: c_int = 0;
pub const WEBKIT_INJECT_ALL_FRAMES: c_int = 0;
pub const WEBKIT_INJECT_AT_DOCUMENT_START: c_int = 0;
pub const WEBKIT_HARDWARE_ACCELERATION_POLICY_NEVER: c_int = 2;

pub const WEBKIT_LOAD_STARTED: c_int = 0;
pub const WEBKIT_LOAD_REDIRECTED: c_int = 1;
pub const WEBKIT_LOAD_COMMITTED: c_int = 2;
pub const WEBKIT_LOAD_FINISHED: c_int = 3;

pub const WEBKIT_SNAPSHOT_REGION_VISIBLE: c_int = 0;
pub const WEBKIT_SNAPSHOT_REGION_FULL_DOCUMENT: c_int = 1;
pub const WEBKIT_SNAPSHOT_OPTIONS_NONE: c_int = 0;
pub const WEBKIT_SNAPSHOT_OPTIONS_TRANSPARENT_BACKGROUND: c_int = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GdkRGBA {
    pub red: c_double,
    pub green: c_double,
    pub blue: c_double,
    pub alpha: c_double,
}

#[repr(C)]
pub struct GError {
    pub domain: u32,
    pub code: c_int,
    pub message: *const c_char,
}

pub type GSourceFunc = unsafe extern "C" fn(*mut c_void) -> c_int;
pub type GDestroyNotify = Option<unsafe extern "C" fn(*mut c_void)>;
pub type GCallback = unsafe extern "C" fn();
pub type GClosureNotify = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;
pub type GAsyncReadyCallback =
    Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>;

#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub struct WebKitApi {
    pub webkit_soname: String,
    // GLib / GObject
    pub g_idle_add_full:
        unsafe extern "C" fn(c_int, GSourceFunc, *mut c_void, GDestroyNotify) -> c_uint,
    pub g_timeout_add_full:
        unsafe extern "C" fn(c_int, c_uint, GSourceFunc, *mut c_void, GDestroyNotify) -> c_uint,
    pub g_free: unsafe extern "C" fn(*mut c_void),
    pub g_error_free: unsafe extern "C" fn(*mut GError),
    pub g_object_new: unsafe extern "C" fn(usize, *const c_char, ...) -> *mut c_void,
    pub g_object_set: unsafe extern "C" fn(*mut c_void, *const c_char, ...),
    pub g_object_ref_sink: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub g_object_unref: unsafe extern "C" fn(*mut c_void),
    pub g_signal_connect_data: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        GCallback,
        *mut c_void,
        GClosureNotify,
        c_int,
    ) -> u64,

    // Cairo
    pub cairo_image_surface_create: unsafe extern "C" fn(c_int, c_int, c_int) -> *mut c_void,
    pub cairo_image_surface_get_width: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub cairo_image_surface_get_height: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub cairo_pdf_surface_create:
        unsafe extern "C" fn(*const c_char, c_double, c_double) -> *mut c_void,
    pub cairo_surface_flush: unsafe extern "C" fn(*mut c_void),
    pub cairo_surface_destroy: unsafe extern "C" fn(*mut c_void),
    pub cairo_surface_write_to_png: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    pub cairo_create: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub cairo_destroy: unsafe extern "C" fn(*mut c_void),
    pub cairo_status: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub cairo_set_source_rgb: unsafe extern "C" fn(*mut c_void, c_double, c_double, c_double),
    pub cairo_set_source_surface:
        unsafe extern "C" fn(*mut c_void, *mut c_void, c_double, c_double),
    pub cairo_scale: unsafe extern "C" fn(*mut c_void, c_double, c_double),
    pub cairo_translate: unsafe extern "C" fn(*mut c_void, c_double, c_double),
    pub cairo_rectangle:
        unsafe extern "C" fn(*mut c_void, c_double, c_double, c_double, c_double),
    pub cairo_clip: unsafe extern "C" fn(*mut c_void),
    pub cairo_paint: unsafe extern "C" fn(*mut c_void),
    pub cairo_show_page: unsafe extern "C" fn(*mut c_void),

    // GDK / GTK3
    pub gdk_screen_get_default: unsafe extern "C" fn() -> *mut c_void,
    pub gdk_screen_get_resolution: unsafe extern "C" fn(*mut c_void) -> c_double,
    pub gdk_screen_set_resolution: unsafe extern "C" fn(*mut c_void, c_double),
    pub gtk_settings_get_default: unsafe extern "C" fn() -> *mut c_void,
    pub gtk_init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
    pub gtk_main: unsafe extern "C" fn(),
    pub gtk_main_quit: unsafe extern "C" fn(),
    pub gtk_events_pending: unsafe extern "C" fn() -> c_int,
    pub gtk_main_iteration_do: unsafe extern "C" fn(c_int) -> c_int,
    pub gtk_offscreen_window_new: unsafe extern "C" fn() -> *mut c_void,
    pub gtk_window_new: unsafe extern "C" fn(c_int) -> *mut c_void,
    pub gtk_window_set_title: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub gtk_window_set_default_size: unsafe extern "C" fn(*mut c_void, c_int, c_int),
    pub gtk_window_resize: unsafe extern "C" fn(*mut c_void, c_int, c_int),
    pub gtk_window_set_titlebar: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_header_bar_new: unsafe extern "C" fn() -> *mut c_void,
    pub gtk_header_bar_set_show_close_button: unsafe extern "C" fn(*mut c_void, c_int),
    pub gtk_header_bar_set_title: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub gtk_header_bar_set_subtitle: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub gtk_header_bar_pack_start: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_header_bar_pack_end: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_box_new: unsafe extern "C" fn(c_int, c_int) -> *mut c_void,
    pub gtk_box_pack_start:
        unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int, c_uint),
    pub gtk_button_new_with_label: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    pub gtk_entry_new: unsafe extern "C" fn() -> *mut c_void,
    pub gtk_entry_set_text: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub gtk_entry_get_text: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub gtk_entry_set_placeholder_text: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub gtk_container_add: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_container_remove: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_widget_show_all: unsafe extern "C" fn(*mut c_void),
    pub gtk_widget_hide: unsafe extern "C" fn(*mut c_void),
    pub gtk_widget_destroy: unsafe extern "C" fn(*mut c_void),
    pub gtk_widget_set_size_request: unsafe extern "C" fn(*mut c_void, c_int, c_int),
    pub gtk_widget_get_allocated_width: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub gtk_widget_get_allocated_height: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub gtk_widget_draw: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub gtk_css_provider_new: unsafe extern "C" fn() -> *mut c_void,
    pub gtk_css_provider_load_from_data:
        unsafe extern "C" fn(*mut c_void, *const c_char, isize, *mut *mut GError) -> c_int,
    pub gtk_style_context_add_provider_for_screen:
        unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint),

    // JavaScriptCore
    pub jsc_value_to_string: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
    pub jsc_value_is_undefined: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub jsc_value_is_null: unsafe extern "C" fn(*mut c_void) -> c_int,

    // WebKit2GTK
    pub webkit_memory_pressure_settings_new: unsafe extern "C" fn() -> *mut c_void,
    pub webkit_memory_pressure_settings_free: unsafe extern "C" fn(*mut c_void),
    pub webkit_memory_pressure_settings_set_memory_limit:
        unsafe extern "C" fn(*mut c_void, c_uint),
    pub webkit_memory_pressure_settings_set_conservative_threshold:
        unsafe extern "C" fn(*mut c_void, c_double),
    pub webkit_memory_pressure_settings_set_strict_threshold:
        unsafe extern "C" fn(*mut c_void, c_double),
    pub webkit_memory_pressure_settings_set_kill_threshold:
        unsafe extern "C" fn(*mut c_void, c_double),
    pub webkit_website_data_manager_set_memory_pressure_settings:
        unsafe extern "C" fn(*mut c_void),
    pub webkit_website_data_manager_new:
        unsafe extern "C" fn(*const c_char, ...) -> *mut c_void,
    pub webkit_web_context_new_ephemeral: unsafe extern "C" fn() -> *mut c_void,
    pub webkit_web_context_new_with_website_data_manager:
        unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub webkit_web_context_set_cache_model: unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_web_context_set_web_process_count_limit:
        unsafe extern "C" fn(*mut c_void, c_uint),
    pub webkit_user_content_manager_new: unsafe extern "C" fn() -> *mut c_void,
    pub webkit_user_content_manager_add_script:
        unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub webkit_user_content_manager_register_script_message_handler:
        unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    pub webkit_user_script_new: unsafe extern "C" fn(
        *const c_char,
        c_int,
        c_int,
        *const *const c_char,
        *const *const c_char,
    ) -> *mut c_void,
    pub webkit_user_script_unref: unsafe extern "C" fn(*mut c_void),
    pub webkit_settings_new: unsafe extern "C" fn() -> *mut c_void,
    pub webkit_settings_set_enable_javascript: unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_settings_set_enable_developer_extras:
        unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_settings_set_enable_webgl: unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_settings_set_enable_media_stream: unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_settings_set_hardware_acceleration_policy:
        unsafe extern "C" fn(*mut c_void, c_int),
    pub webkit_settings_set_user_agent: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub webkit_web_view_get_type: unsafe extern "C" fn() -> usize,
    pub webkit_web_view_set_settings: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub webkit_web_view_set_background_color:
        unsafe extern "C" fn(*mut c_void, *const GdkRGBA),
    pub webkit_web_view_set_zoom_level: unsafe extern "C" fn(*mut c_void, c_double),
    pub webkit_web_view_get_zoom_level: unsafe extern "C" fn(*mut c_void) -> c_double,
    pub webkit_web_view_load_uri: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub webkit_web_view_load_html:
        unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char),
    pub webkit_web_view_reload: unsafe extern "C" fn(*mut c_void),
    pub webkit_web_view_go_back: unsafe extern "C" fn(*mut c_void),
    pub webkit_web_view_go_forward: unsafe extern "C" fn(*mut c_void),
    pub webkit_web_view_can_go_back: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub webkit_web_view_can_go_forward: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub webkit_web_view_get_uri: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub webkit_web_view_get_title: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub webkit_web_view_is_loading: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub webkit_web_view_get_estimated_load_progress:
        unsafe extern "C" fn(*mut c_void) -> c_double,
    pub webkit_web_view_run_javascript: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *mut c_void,
        GAsyncReadyCallback,
        *mut c_void,
    ),
    pub webkit_web_view_get_snapshot: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        c_int,
        *mut c_void,
        GAsyncReadyCallback,
        *mut c_void,
    ),
    pub webkit_web_view_get_snapshot_finish:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut GError) -> *mut c_void,
    pub webkit_javascript_result_get_js_value:
        unsafe extern "C" fn(*mut c_void) -> *mut c_void,
}

#[cfg(target_os = "linux")]
unsafe impl Send for WebKitApi {}
#[cfg(target_os = "linux")]
unsafe impl Sync for WebKitApi {}

#[cfg(target_os = "linux")]
fn open_shared_lib(candidates: &[&str]) -> Result<(*mut c_void, String)> {
    let mut errors = Vec::new();
    for &soname in candidates {
        let c_soname = CString::new(soname)?;
        let handle =
            unsafe { libc::dlopen(c_soname.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
        match handle.is_null() {
            false => return Ok((handle, soname.to_string())),
            true => {
                let err_ptr = unsafe { libc::dlerror() };
                let err_msg = match err_ptr.is_null() {
                    true => "unknown dlopen error".to_string(),
                    false => unsafe { CStr::from_ptr(err_ptr).to_string_lossy().to_string() },
                };
                errors.push(format!("{soname}: {err_msg}"));
            }
        }
    }
    Err(anyhow!(
        "Failed to dlopen runtime library ({})",
        errors.join("; ")
    ))
}

#[cfg(target_os = "linux")]
unsafe fn load_sym<T: Copy>(handle: *mut c_void, name: &str) -> Result<T> {
    let c_name = CString::new(name)?;
    let raw = libc::dlsym(handle, c_name.as_ptr());
    match raw.is_null() {
        true => Err(anyhow!("Missing C symbol '{name}' in shared library")),
        false => Ok(std::mem::transmute_copy::<*mut c_void, T>(&raw)),
    }
}

#[cfg(target_os = "linux")]
impl WebKitApi {
    pub fn get() -> Result<&'static Self> {
        static API: std::sync::OnceLock<std::result::Result<WebKitApi, String>> =
            std::sync::OnceLock::new();
        let res = API.get_or_init(|| Self::load().map_err(|err| err.to_string()));
        match res {
            Ok(api) => Ok(api),
            Err(msg) => Err(anyhow!("{msg}")),
        }
    }

    fn load() -> Result<Self> {
        let (glib, _) = open_shared_lib(&["libglib-2.0.so.0"])?;
        let (gobject, _) = open_shared_lib(&["libgobject-2.0.so.0"])?;
        let (cairo, _) = open_shared_lib(&["libcairo.so.2"])?;
        let (gdk, _) = open_shared_lib(&["libgdk-3.so.0"])?;
        let (gtk, _) = open_shared_lib(&["libgtk-3.so.0"])?;
        let (jsc, _) = open_shared_lib(&[
            "libjavascriptcoregtk-4.1.so.0",
            "libjavascriptcoregtk-4.0.so.18",
        ])?;
        let (webkit, webkit_soname) =
            open_shared_lib(&["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37"])?;

        unsafe {
            Ok(Self {
                webkit_soname,
                g_idle_add_full: load_sym(glib, "g_idle_add_full")?,
                g_timeout_add_full: load_sym(glib, "g_timeout_add_full")?,
                g_free: load_sym(glib, "g_free")?,
                g_error_free: load_sym(glib, "g_error_free")?,
                g_object_new: load_sym(gobject, "g_object_new")?,
                g_object_set: load_sym(gobject, "g_object_set")?,
                g_object_ref_sink: load_sym(gobject, "g_object_ref_sink")?,
                g_object_unref: load_sym(gobject, "g_object_unref")?,
                g_signal_connect_data: load_sym(gobject, "g_signal_connect_data")?,

                cairo_image_surface_create: load_sym(cairo, "cairo_image_surface_create")?,
                cairo_image_surface_get_width: load_sym(cairo, "cairo_image_surface_get_width")?,
                cairo_image_surface_get_height: load_sym(cairo, "cairo_image_surface_get_height")?,
                cairo_pdf_surface_create: load_sym(cairo, "cairo_pdf_surface_create")?,
                cairo_surface_flush: load_sym(cairo, "cairo_surface_flush")?,
                cairo_surface_destroy: load_sym(cairo, "cairo_surface_destroy")?,
                cairo_surface_write_to_png: load_sym(cairo, "cairo_surface_write_to_png")?,
                cairo_create: load_sym(cairo, "cairo_create")?,
                cairo_destroy: load_sym(cairo, "cairo_destroy")?,
                cairo_status: load_sym(cairo, "cairo_status")?,
                cairo_set_source_rgb: load_sym(cairo, "cairo_set_source_rgb")?,
                cairo_set_source_surface: load_sym(cairo, "cairo_set_source_surface")?,
                cairo_scale: load_sym(cairo, "cairo_scale")?,
                cairo_translate: load_sym(cairo, "cairo_translate")?,
                cairo_rectangle: load_sym(cairo, "cairo_rectangle")?,
                cairo_clip: load_sym(cairo, "cairo_clip")?,
                cairo_paint: load_sym(cairo, "cairo_paint")?,
                cairo_show_page: load_sym(cairo, "cairo_show_page")?,

                gdk_screen_get_default: load_sym(gdk, "gdk_screen_get_default")?,
                gdk_screen_get_resolution: load_sym(gdk, "gdk_screen_get_resolution")?,
                gdk_screen_set_resolution: load_sym(gdk, "gdk_screen_set_resolution")?,
                gtk_settings_get_default: load_sym(gtk, "gtk_settings_get_default")?,
                gtk_init_check: load_sym(gtk, "gtk_init_check")?,
                gtk_main: load_sym(gtk, "gtk_main")?,
                gtk_main_quit: load_sym(gtk, "gtk_main_quit")?,
                gtk_events_pending: load_sym(gtk, "gtk_events_pending")?,
                gtk_main_iteration_do: load_sym(gtk, "gtk_main_iteration_do")?,
                gtk_offscreen_window_new: load_sym(gtk, "gtk_offscreen_window_new")?,
                gtk_window_new: load_sym(gtk, "gtk_window_new")?,
                gtk_window_set_title: load_sym(gtk, "gtk_window_set_title")?,
                gtk_window_set_default_size: load_sym(gtk, "gtk_window_set_default_size")?,
                gtk_window_resize: load_sym(gtk, "gtk_window_resize")?,
                gtk_window_set_titlebar: load_sym(gtk, "gtk_window_set_titlebar")?,
                gtk_header_bar_new: load_sym(gtk, "gtk_header_bar_new")?,
                gtk_header_bar_set_show_close_button: load_sym(
                    gtk,
                    "gtk_header_bar_set_show_close_button",
                )?,
                gtk_header_bar_set_title: load_sym(gtk, "gtk_header_bar_set_title")?,
                gtk_header_bar_set_subtitle: load_sym(gtk, "gtk_header_bar_set_subtitle")?,
                gtk_header_bar_pack_start: load_sym(gtk, "gtk_header_bar_pack_start")?,
                gtk_header_bar_pack_end: load_sym(gtk, "gtk_header_bar_pack_end")?,
                gtk_box_new: load_sym(gtk, "gtk_box_new")?,
                gtk_box_pack_start: load_sym(gtk, "gtk_box_pack_start")?,
                gtk_button_new_with_label: load_sym(gtk, "gtk_button_new_with_label")?,
                gtk_entry_new: load_sym(gtk, "gtk_entry_new")?,
                gtk_entry_set_text: load_sym(gtk, "gtk_entry_set_text")?,
                gtk_entry_get_text: load_sym(gtk, "gtk_entry_get_text")?,
                gtk_entry_set_placeholder_text: load_sym(gtk, "gtk_entry_set_placeholder_text")?,
                gtk_container_add: load_sym(gtk, "gtk_container_add")?,
                gtk_container_remove: load_sym(gtk, "gtk_container_remove")?,
                gtk_widget_show_all: load_sym(gtk, "gtk_widget_show_all")?,
                gtk_widget_hide: load_sym(gtk, "gtk_widget_hide")?,
                gtk_widget_destroy: load_sym(gtk, "gtk_widget_destroy")?,
                gtk_widget_set_size_request: load_sym(gtk, "gtk_widget_set_size_request")?,
                gtk_widget_get_allocated_width: load_sym(gtk, "gtk_widget_get_allocated_width")?,
                gtk_widget_get_allocated_height: load_sym(gtk, "gtk_widget_get_allocated_height")?,
                gtk_widget_draw: load_sym(gtk, "gtk_widget_draw")?,
                gtk_css_provider_new: load_sym(gtk, "gtk_css_provider_new")?,
                gtk_css_provider_load_from_data: load_sym(gtk, "gtk_css_provider_load_from_data")?,
                gtk_style_context_add_provider_for_screen: load_sym(
                    gtk,
                    "gtk_style_context_add_provider_for_screen",
                )?,

                jsc_value_to_string: load_sym(jsc, "jsc_value_to_string")?,
                jsc_value_is_undefined: load_sym(jsc, "jsc_value_is_undefined")?,
                jsc_value_is_null: load_sym(jsc, "jsc_value_is_null")?,

                webkit_memory_pressure_settings_new: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_new",
                )?,
                webkit_memory_pressure_settings_free: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_free",
                )?,
                webkit_memory_pressure_settings_set_memory_limit: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_set_memory_limit",
                )?,
                webkit_memory_pressure_settings_set_conservative_threshold: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_set_conservative_threshold",
                )?,
                webkit_memory_pressure_settings_set_strict_threshold: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_set_strict_threshold",
                )?,
                webkit_memory_pressure_settings_set_kill_threshold: load_sym(
                    webkit,
                    "webkit_memory_pressure_settings_set_kill_threshold",
                )?,
                webkit_website_data_manager_set_memory_pressure_settings: load_sym(
                    webkit,
                    "webkit_website_data_manager_set_memory_pressure_settings",
                )?,
                webkit_website_data_manager_new: load_sym(
                    webkit,
                    "webkit_website_data_manager_new",
                )?,
                webkit_web_context_new_ephemeral: load_sym(
                    webkit,
                    "webkit_web_context_new_ephemeral",
                )?,
                webkit_web_context_new_with_website_data_manager: load_sym(
                    webkit,
                    "webkit_web_context_new_with_website_data_manager",
                )?,
                webkit_web_context_set_cache_model: load_sym(
                    webkit,
                    "webkit_web_context_set_cache_model",
                )?,
                webkit_web_context_set_web_process_count_limit: load_sym(
                    webkit,
                    "webkit_web_context_set_web_process_count_limit",
                )?,
                webkit_user_content_manager_new: load_sym(
                    webkit,
                    "webkit_user_content_manager_new",
                )?,
                webkit_user_content_manager_add_script: load_sym(
                    webkit,
                    "webkit_user_content_manager_add_script",
                )?,
                webkit_user_content_manager_register_script_message_handler: load_sym(
                    webkit,
                    "webkit_user_content_manager_register_script_message_handler",
                )?,
                webkit_user_script_new: load_sym(webkit, "webkit_user_script_new")?,
                webkit_user_script_unref: load_sym(webkit, "webkit_user_script_unref")?,
                webkit_settings_new: load_sym(webkit, "webkit_settings_new")?,
                webkit_settings_set_enable_javascript: load_sym(
                    webkit,
                    "webkit_settings_set_enable_javascript",
                )?,
                webkit_settings_set_enable_developer_extras: load_sym(
                    webkit,
                    "webkit_settings_set_enable_developer_extras",
                )?,
                webkit_settings_set_enable_webgl: load_sym(
                    webkit,
                    "webkit_settings_set_enable_webgl",
                )?,
                webkit_settings_set_enable_media_stream: load_sym(
                    webkit,
                    "webkit_settings_set_enable_media_stream",
                )?,
                webkit_settings_set_hardware_acceleration_policy: load_sym(
                    webkit,
                    "webkit_settings_set_hardware_acceleration_policy",
                )?,
                webkit_settings_set_user_agent: load_sym(
                    webkit,
                    "webkit_settings_set_user_agent",
                )?,
                webkit_web_view_get_type: load_sym(webkit, "webkit_web_view_get_type")?,
                webkit_web_view_set_settings: load_sym(webkit, "webkit_web_view_set_settings")?,
                webkit_web_view_set_background_color: load_sym(
                    webkit,
                    "webkit_web_view_set_background_color",
                )?,
                webkit_web_view_set_zoom_level: load_sym(
                    webkit,
                    "webkit_web_view_set_zoom_level",
                )?,
                webkit_web_view_get_zoom_level: load_sym(
                    webkit,
                    "webkit_web_view_get_zoom_level",
                )?,
                webkit_web_view_load_uri: load_sym(webkit, "webkit_web_view_load_uri")?,
                webkit_web_view_load_html: load_sym(webkit, "webkit_web_view_load_html")?,
                webkit_web_view_reload: load_sym(webkit, "webkit_web_view_reload")?,
                webkit_web_view_go_back: load_sym(webkit, "webkit_web_view_go_back")?,
                webkit_web_view_go_forward: load_sym(webkit, "webkit_web_view_go_forward")?,
                webkit_web_view_can_go_back: load_sym(webkit, "webkit_web_view_can_go_back")?,
                webkit_web_view_can_go_forward: load_sym(
                    webkit,
                    "webkit_web_view_can_go_forward",
                )?,
                webkit_web_view_get_uri: load_sym(webkit, "webkit_web_view_get_uri")?,
                webkit_web_view_get_title: load_sym(webkit, "webkit_web_view_get_title")?,
                webkit_web_view_is_loading: load_sym(webkit, "webkit_web_view_is_loading")?,
                webkit_web_view_get_estimated_load_progress: load_sym(
                    webkit,
                    "webkit_web_view_get_estimated_load_progress",
                )?,
                webkit_web_view_run_javascript: load_sym(
                    webkit,
                    "webkit_web_view_run_javascript",
                )?,
                webkit_web_view_get_snapshot: load_sym(webkit, "webkit_web_view_get_snapshot")?,
                webkit_web_view_get_snapshot_finish: load_sym(
                    webkit,
                    "webkit_web_view_get_snapshot_finish",
                )?,
                webkit_javascript_result_get_js_value: load_sym(
                    webkit,
                    "webkit_javascript_result_get_js_value",
                )?,
            })
        }
    }
}
