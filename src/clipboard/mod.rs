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
/// Return focus to the previously active app once the popup is gone.
#[cfg(target_os = "windows")]
pub fn restore_previous_app(prev: Option<PreviousApp>) -> bool { imp::restore_app(prev) }

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::*;
    use crate::convert::{self, Source, Target};

    #[test]
    #[cfg_attr(target_os = "windows", ignore = "uses the system clipboard; run on an isolated Windows runner")]
    fn native_clipboard_preserves_all_flavors_and_honors_text_choices() {
        let rich = convert::convert_for(&Source::Markdown("# Café ☕\n\n**important**\n".into()), Target::Rich, false);
        write(&rich.items, false).unwrap();
        for (f, expected) in &rich.items {
            assert!(imp::has(*f), "missing {f:?}");
            assert_eq!(read(*f).as_deref(), Some(expected.as_slice()), "{f:?}");
        }
        let original_html = read(Flavor::Html).unwrap();
        write(&[(Flavor::Text, b"replacement".to_vec())], true).unwrap();
        assert_eq!(read(Flavor::Text).unwrap(), b"replacement");
        assert_eq!(read(Flavor::Html).unwrap(), original_html);
        assert!(read(Flavor::Md).is_some());

        for target in [Target::Md, Target::Plain, Target::Html, Target::Text] {
            write(&rich.items, false).unwrap();
            let plan = convert::prepare_from(&convert::capture(), target, false, false).unwrap();
            write(&plan.output.items, false).unwrap();
            assert_eq!(present(), vec![Flavor::Text], "{target:?} must paste as text into rich editors");
            assert_eq!(read(Flavor::Text).unwrap(), plan.output.result.as_bytes());
        }
    }
}
