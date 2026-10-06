//! Windows: Win32 clipboard through clipboard-win. HTML uses the registered "HTML Format"
//! (CF_HTML, header handled by clipboard-win), RTF the registered "Rich Text Format".
use crate::convert::Flavor;
use clipboard_win::{formats, options::NoClear, raw, Clipboard};
use std::num::NonZeroU32;

fn format_id(f: Flavor) -> Option<u32> {
    match f {
        Flavor::Text => Some(formats::CF_UNICODETEXT),
        Flavor::Html => raw::register_format("HTML Format").map(NonZeroU32::get),
        Flavor::Rtf => raw::register_format("Rich Text Format").map(NonZeroU32::get),
        Flavor::Md => raw::register_format("ClipTo Markdown").map(NonZeroU32::get),
    }
}

fn open() -> Option<Clipboard> { Clipboard::new_attempts(10).ok() }

pub fn types() -> Vec<String> {
    let Some(_c) = open() else { return vec![] };
    raw::EnumFormats::new()
        .map(|id| raw::format_name_big(id).unwrap_or_else(|| format!("format {id}")))
        .collect()
}

pub fn has(f: Flavor) -> bool {
    match format_id(f) { Some(id) => raw::is_format_avail(id), None => false }
}

pub fn read(f: Flavor) -> Option<Vec<u8>> {
    let id = format_id(f)?;
    let _c = open()?;
    if !raw::is_format_avail(id) { return None; }
    let mut buf = Vec::new();
    match f {
        Flavor::Text => raw::get_string(&mut buf).ok()?,
        Flavor::Html => raw::get_html(id, &mut buf).ok()?,
        Flavor::Rtf | Flavor::Md => raw::get_vec(id, &mut buf).ok()?,
    };
    Some(buf)
}

pub fn write(items: &[(Flavor, Vec<u8>)], keep_others: bool) -> Result<(), String> {
    // Validate before emptying: errors must not destroy the source clipboard.
    for (f, d) in items {
        format_id(*f).ok_or("could not register clipboard format")?;
        if matches!(f, Flavor::Text | Flavor::Html) { std::str::from_utf8(d).map_err(|e| e.to_string())?; }
    }
    let _c = open().ok_or("could not open the clipboard")?;
    let mut keep: Vec<(u32, Vec<u8>)> = Vec::new();
    if keep_others {
        let written: Vec<u32> = items.iter().filter_map(|(f, _)| format_id(*f)).collect();
        for id in raw::EnumFormats::new() {
            // Synthesized formats (CF_TEXT/CF_OEMTEXT/locale) are re-derived by Windows from CF_UNICODETEXT.
            if written.contains(&id) || matches!(id, 1 | 7 | 16) { continue; }
            let mut buf = Vec::new();
            raw::get_vec(id, &mut buf).map_err(|e| format!("cannot preserve clipboard format {id}: {e}"))?;
            keep.push((id, buf));
        }
    }
    raw::empty().map_err(|e| e.to_string())?;
    for (f, d) in items {
        let id = format_id(*f).ok_or("could not register clipboard format")?;
        let r = match f {
            Flavor::Text => raw::set_string_with(std::str::from_utf8(d).map_err(|e| e.to_string())?, NoClear),
            Flavor::Html => raw::set_html(id, std::str::from_utf8(d).map_err(|e| e.to_string())?),
            Flavor::Rtf | Flavor::Md => raw::set_without_clear(id, d),
        };
        r.map_err(|e| e.to_string())?;
    }
    for (id, d) in &keep { raw::set_without_clear(*id, d).map_err(|e| e.to_string())?; }
    Ok(())
}

pub fn change_count() -> i64 {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() as i64 }
}

pub fn rtf_to_html(_rtf: &[u8]) -> Option<String> { None }
pub fn html_to_rtf(_html: &str) -> Option<Vec<u8>> { None }
#[derive(Clone, Copy)]
pub struct PreviousApp(windows_sys::Win32::Foundation::HWND);
pub fn activate_app() -> Option<PreviousApp> {
    let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    (!hwnd.is_null()).then_some(PreviousApp(hwnd))
}
pub fn restore_app(p: Option<PreviousApp>) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow};
    let Some(PreviousApp(hwnd)) = p else { return false };
    unsafe {
        if IsWindow(hwnd) == 0 { return false; }
        SetForegroundWindow(hwnd);
        GetForegroundWindow() == hwnd
    }
}
