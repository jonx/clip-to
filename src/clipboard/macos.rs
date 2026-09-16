//! macOS: NSPasteboard for the clipboard, NSAttributedString for RTF <-> HTML.
use crate::convert::Flavor;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSAttributedStringAppKitDocumentFormats, NSAttributedStringDocumentAttributeKey, NSAttributedStringDocumentFormats, NSDocumentTypeDocumentAttribute,
    NSExcludedElementsDocumentAttribute, NSHTMLTextDocumentType, NSPasteboard, NSPasteboardType, NSPasteboardTypeHTML,
    NSPasteboardTypeRTF, NSPasteboardTypeString, NSRTFTextDocumentType,
};
use objc2_foundation::{NSArray, NSAttributedString, NSData, NSDictionary, NSRange, NSString};

fn pb() -> Retained<NSPasteboard> { NSPasteboard::generalPasteboard() }

fn ns_type(f: Flavor) -> &'static NSPasteboardType {
    unsafe {
        match f { Flavor::Text => NSPasteboardTypeString, Flavor::Html => NSPasteboardTypeHTML, Flavor::Rtf => NSPasteboardTypeRTF }
    }
}

pub fn types() -> Vec<String> {
    pb().types().map(|a| a.iter().map(|t| t.to_string()).collect()).unwrap_or_default()
}

pub fn has(f: Flavor) -> bool { pb().dataForType(ns_type(f)).is_some() }

pub fn read(f: Flavor) -> Option<Vec<u8>> {
    let p = pb();
    match f {
        Flavor::Rtf => p.dataForType(ns_type(f)).map(|d| d.to_vec()),
        // Strings go through NSString so the pasteboard normalises encodings for us.
        _ => p.stringForType(ns_type(f)).map(|s| s.to_string().into_bytes()),
    }
}

pub fn write(items: &[(Flavor, Vec<u8>)], keep_others: bool) -> Result<(), String> {
    let p = pb();
    let mut all: Vec<(Retained<NSString>, Retained<NSData>)> = items
        .iter()
        .map(|(f, d)| (NSString::from_str(ns_type(*f).to_string().as_str()), NSData::with_bytes(d)))
        .collect();
    if keep_others {
        let written: Vec<String> = items.iter().map(|(f, _)| ns_type(*f).to_string()).collect();
        if let Some(existing) = p.types() {
            for t in existing.iter() {
                let name = t.to_string();
                if written.contains(&name) { continue; }
                if let Some(d) = p.dataForType(&t) { all.push((NSString::from_str(&name), d)); }
            }
        }
    }
    p.clearContents();
    for (t, d) in &all {
        if !p.setData_forType(Some(d), t) { return Err(format!("could not write {t}")); }
    }
    Ok(())
}

pub fn change_count() -> i64 { pb().changeCount() as i64 }

fn doc_type_dict(kind: &'static objc2_app_kit::NSAttributedStringDocumentType, exclude: Option<&[&str]>)
    -> Retained<NSDictionary<NSAttributedStringDocumentAttributeKey, AnyObject>> {
    let mut keys: Vec<&NSAttributedStringDocumentAttributeKey> = vec![unsafe { NSDocumentTypeDocumentAttribute }];
    let mut values: Vec<&AnyObject> = vec![kind];
    let excluded: Option<Retained<NSArray<NSString>>> = exclude.map(|names| {
        let strs: Vec<Retained<NSString>> = names.iter().map(|n| NSString::from_str(n)).collect();
        NSArray::from_retained_slice(&strs)
    });
    if let Some(arr) = &excluded {
        keys.push(unsafe { NSExcludedElementsDocumentAttribute });
        values.push(arr);
    }
    NSDictionary::from_slices(&keys, &values)
}

/// RTF -> HTML through AppKit. Excluding the style machinery makes AppKit emit
/// `<b>`, `<i>`, `<font>` tags that an HTML-to-Markdown converter understands.
pub fn rtf_to_html(rtf: &[u8]) -> Option<String> {
    unsafe {
        let data = NSData::with_bytes(rtf);
        let attr = NSAttributedString::initWithRTF_documentAttributes(NSAttributedString::alloc(), &data, None)?;
        let dict = doc_type_dict(NSHTMLTextDocumentType, Some(&["XML", "DOCTYPE", "HTML", "HEAD", "META", "TITLE", "STYLE", "SPAN", "Apple-converted-space"]));
        let out = attr.dataFromRange_documentAttributes_error(NSRange::new(0, attr.length()), &dict).ok()?;
        String::from_utf8(out.to_vec()).ok()
    }
}

pub fn html_to_rtf(html: &str) -> Option<Vec<u8>> {
    unsafe {
        let data = NSData::with_bytes(html.as_bytes());
        let attr = NSAttributedString::initWithHTML_documentAttributes(NSAttributedString::alloc(), &data, None)?;
        let dict = doc_type_dict(NSRTFTextDocumentType, None);
        attr.RTFFromRange_documentAttributes(NSRange::new(0, attr.length()), &dict).map(|d| d.to_vec())
    }
}

/// Handle to the app that had focus before we showed a popup.
pub struct PreviousApp(Retained<objc2_app_kit::NSRunningApplication>);

/// Remember the frontmost app, then bring our process to the front so a popup menu can take key focus.
pub fn activate_app() -> Option<PreviousApp> {
    use objc2_app_kit::NSWorkspace;
    let prev = unsafe { NSWorkspace::sharedWorkspace().frontmostApplication() }.map(PreviousApp);
    activate_self();
    prev
}

/// Give focus back to the app the user was in, so their next paste lands in the right field.
pub fn restore_app(prev: Option<PreviousApp>) {
    use objc2_app_kit::NSApplicationActivationOptions;
    if let Some(PreviousApp(app)) = prev {
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
    }
}

fn activate_self() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    if let Some(mtm) = MainThreadMarker::new() {
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}
