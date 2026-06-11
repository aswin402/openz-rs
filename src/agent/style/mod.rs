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
    ($($arg:tt)*) => {
        $crate::agent::style::tui_println_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_print {
    ($($arg:tt)*) => {
        $crate::agent::style::tui_print_fn(format!($($arg)*))
    };
}

pub fn tui_println_fn<T: AsRef<str>>(msg: T) {
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

pub fn tui_print_fn<T: AsRef<str>>(msg: T) {
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}", replaced);
    let _ = std::io::stdout().flush();
}
