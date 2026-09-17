//! Thin platform layer over the system clipboard: list flavors, read one, write several
//! while optionally preserving whatever else is there.
use crate::convert::Flavor;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;
#[cfg(target_os = "linux")]
pub use linux::serve;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod other;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use other as imp;

pub mod rich {
    /// RTF -> HTML and HTML -> RTF, where the OS offers a converter (macOS). Elsewhere: None.
    pub fn rtf_to_html(rtf: &[u8]) -> Option<String> { super::imp::rtf_to_html(rtf) }
    pub fn html_to_rtf(html: &str) -> Option<Vec<u8>> { super::imp::html_to_rtf(html) }
}

/// Native names of every format currently on the clipboard.
pub fn types() -> Vec<String> { imp::types() }

/// Flavors we understand that are currently present.
pub fn present() -> Vec<Flavor> { Flavor::ALL.iter().copied().filter(|f| imp::has(*f)).collect() }

/// Raw bytes of one flavor (UTF-8 for text and HTML).
pub fn read(f: Flavor) -> Option<Vec<u8>> { imp::read(f) }

/// Write flavors. With `keep_others`, every format already on the clipboard that is not
/// being replaced is preserved, so rich apps keep pasting the original formatting.
pub fn write(items: &[(Flavor, Vec<u8>)], keep_others: bool) -> Result<(), String> { imp::write(items, keep_others) }

/// A counter that changes whenever the clipboard content changes (daemon only).
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn change_count() -> i64 { imp::change_count() }

#[cfg(target_os = "windows")]
pub use imp::PreviousApp;
/// Bring the process to the front before showing a popup menu and remember who had focus.
#[cfg(target_os = "windows")]
pub fn activate_app_for_popup() -> Option<PreviousApp> { imp::activate_app() }
/// Bundle id of the app that had focus before the popup, when known.
#[cfg(target_os = "windows")]
pub fn previous_app_id(prev: &Option<PreviousApp>) -> Option<String> { imp::previous_app_id(prev) }
/// Return focus to the previously active app once the popup is gone.
#[cfg(target_os = "windows")]
pub fn restore_previous_app(prev: Option<PreviousApp>) { imp::restore_app(prev) }
