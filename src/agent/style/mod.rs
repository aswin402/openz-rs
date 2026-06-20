pub mod colors;
pub mod icons;
pub mod spinner;
pub mod menu;

pub use colors::*;
pub use icons::*;
pub use spinner::*;
pub use menu::*;

use std::io::Write;

#[macro_export]
macro_rules! tui_println {
    () => {
        $crate::agent::style::tui_println_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_println_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_print {
    () => {
        $crate::agent::style::tui_print_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_print_fn(format!($($arg)*))
    };
}

pub fn tui_println_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

pub fn tui_print_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}", replaced);
    let _ = std::io::stdout().flush();
}

#[macro_export]
macro_rules! tui_eprintln {
    () => {
        $crate::agent::style::tui_eprintln_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprintln_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_eprint {
    () => {
        $crate::agent::style::tui_eprint_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprint_fn(format!($($arg)*))
    };
}

pub fn tui_eprintln_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}\r\n", replaced);
    let _ = std::io::stderr().flush();
}

pub fn tui_eprint_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}", replaced);
    let _ = std::io::stderr().flush();
}
