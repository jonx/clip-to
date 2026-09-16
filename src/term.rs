//! ANSI colours, enabled only when writing to a terminal and `NO_COLOR` is unset.
use std::io::IsTerminal;
use std::sync::OnceLock;

fn enabled_for(is_tty: bool) -> bool {
    is_tty && std::env::var_os("NO_COLOR").is_none()
}

pub fn stdout_color() -> bool {
    static C: OnceLock<bool> = OnceLock::new();
    *C.get_or_init(|| enabled_for(std::io::stdout().is_terminal()))
}

pub fn stderr_color() -> bool {
    static C: OnceLock<bool> = OnceLock::new();
    *C.get_or_init(|| enabled_for(std::io::stderr().is_terminal()))
}

fn paint(code: &str, s: &str, on: bool) -> String {
    if on { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
}

pub fn bold(s: &str) -> String { paint("1", s, stdout_color()) }
pub fn dim(s: &str) -> String { paint("2", s, stdout_color()) }
pub fn cyan(s: &str) -> String { paint("36", s, stdout_color()) }
pub fn green(s: &str) -> String { paint("32", s, stdout_color()) }
pub fn yellow(s: &str) -> String { paint("33", s, stdout_color()) }
pub fn magenta(s: &str) -> String { paint("35", s, stdout_color()) }

pub fn err_green(s: &str) -> String { paint("32", s, stderr_color()) }
pub fn err_cyan(s: &str) -> String { paint("36", s, stderr_color()) }
pub fn err_red(s: &str) -> String { paint("31", s, stderr_color()) }
pub fn err_dim(s: &str) -> String { paint("2", s, stderr_color()) }
