//! Tiny persistent settings: which targets the user hid from the chooser.
use crate::convert::Target;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("clipto").join("hidden"))
}

pub fn hidden_targets() -> Vec<Target> {
    let Some(p) = path() else { return vec![] };
    std::fs::read_to_string(p).unwrap_or_default().lines().filter_map(|l| Target::parse(l.trim())).collect()
}

pub fn set_hidden_targets(hidden: &[Target]) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
    let body: String = hidden.iter().map(|t| format!("{}\n", t.name())).collect();
    let _ = std::fs::write(p, body);
}
