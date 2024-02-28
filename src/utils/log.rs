//! Helper functions and macros for logging (console printing).
//!
//! Author: Guanzhou Hu
//! Source: https://github.com/josehu07/summerset/blob/main/src/utils/print.rs

use env_logger::fmt::{Color, Style, StyledValue};
use log::Level;

pub fn colored_level(style: &mut Style, level: Level) -> StyledValue<&'static str> {
    match level {
        Level::Trace => style.set_color(Color::Magenta).value("TRACE"),
        Level::Debug => style.set_color(Color::Blue).value("DEBUG"),
        Level::Info => style.set_color(Color::Green).value("INFO "),
        Level::Warn => style.set_color(Color::Yellow).value("WARN "),
        Level::Error => style.set_color(Color::Red).value("ERROR"),
    }
}

/// Log TRACE message with parenthesized prefix.
///
/// Example:
/// ```no_compile
/// pf_trace!(id; "got {} to print", msg);
/// ```
#[macro_export]
macro_rules! pf_trace {
    ($prefix:expr; $fmt_str:literal) => {
        log::trace!(concat!("({}) ", $fmt_str), $prefix)
    };

    ($prefix:expr; $fmt_str:literal, $($fmt_arg:tt)*) => {
        log::trace!(concat!("({}) ", $fmt_str), $prefix, $($fmt_arg)*)
    };
}

/// Log DEBUG message with parenthesized prefix.
///
/// Example:
/// ```no_compile
/// pf_debug!(id; "got {} to print", msg);
/// ```
#[macro_export]
macro_rules! pf_debug {
    ($prefix:expr; $fmt_str:literal) => {
        log::debug!(concat!("({}) ", $fmt_str), $prefix)
    };

    ($prefix:expr; $fmt_str:literal, $($fmt_arg:tt)*) => {
        log::debug!(concat!("({}) ", $fmt_str), $prefix, $($fmt_arg)*)
    };
}

/// Log INFO message with parenthesized prefix.
///
/// Example:
/// ```no_compile
/// pf_info!(id; "got {} to print", msg);
/// ```
#[macro_export]
macro_rules! pf_info {
    ($prefix:expr; $fmt_str:literal) => {
        log::info!(concat!("({}) ", $fmt_str), $prefix)
    };

    ($prefix:expr; $fmt_str:literal, $($fmt_arg:tt)*) => {
        log::info!(concat!("({}) ", $fmt_str), $prefix, $($fmt_arg)*)
    };
}

/// Log WARN message with parenthesized prefix.
///
/// Example:
/// ```no_compile
/// pf_warn!(id; "got {} to print", msg);
/// ```
#[macro_export]
macro_rules! pf_warn {
    ($prefix:expr; $fmt_str:literal) => {
        log::warn!(concat!("({}) ", $fmt_str), $prefix)
    };

    ($prefix:expr; $fmt_str:literal, $($fmt_arg:tt)*) => {
        log::warn!(concat!("({}) ", $fmt_str), $prefix, $($fmt_arg)*)
    };
}

/// Log ERROR message with parenthesized prefix.
///
/// Example:
/// ```no_compile
/// pf_error!(id; "got {} to print", msg);
/// ```
#[macro_export]
macro_rules! pf_error {
    ($prefix:expr; $fmt_str:literal) => {
        log::error!(concat!("({}) ", $fmt_str), $prefix)
    };

    ($prefix:expr; $fmt_str:literal, $($fmt_arg:tt)*) => {
        log::error!(concat!("({}) ", $fmt_str), $prefix, $($fmt_arg)*)
    };
}
