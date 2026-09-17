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
use objc2_foundation::{NSArray, NSAttributedString, NSCopying, NSData, NSDictionary, NSRange, NSString};

fn pb() -> Retained<NSPasteboard> { NSPasteboard::generalPasteboard() }

/// Private flavor carrying the Markdown source written by `rich`.
pub const MARKDOWN_TYPE: &str = "me.jkn.clipto.markdown";

fn ns_type(f: Flavor) -> Retained<NSPasteboardType> {
    unsafe {
        match f {
            Flavor::Text => NSPasteboardTypeString.copy(),
            Flavor::Html => NSPasteboardTypeHTML.copy(),
            Flavor::Rtf => NSPasteboardTypeRTF.copy(),
            Flavor::Md => NSString::from_str(MARKDOWN_TYPE),
        }
    }
}

pub fn types() -> Vec<String> {
    pb().types().map(|a| a.iter().map(|t| t.to_string()).collect()).unwrap_or_default()
}

pub fn has(f: Flavor) -> bool { pb().dataForType(&ns_type(f)).is_some() }

pub fn read(f: Flavor) -> Option<Vec<u8>> {
    let p = pb();
    match f {
        Flavor::Rtf | Flavor::Md => p.dataForType(&ns_type(f)).map(|d| d.to_vec()),
        // Strings go through NSString so the pasteboard normalises encodings for us.
        _ => p.stringForType(&ns_type(f)).map(|s| s.to_string().into_bytes()),
    }
}

pub fn write(items: &[(Flavor, Vec<u8>)], keep_others: bool) -> Result<(), String> {
    let p = pb();
    let mut all: Vec<(Retained<NSString>, Retained<NSData>)> = items
        .iter()
        .map(|(f, d)| (ns_type(*f), NSData::with_bytes(d)))
        .collect();
    if keep_others {
        let written: Vec<String> = items.iter().map(|(f, _)| ns_type(*f).to_string()).collect();
        let writing_text = items.iter().any(|(f, _)| *f == Flavor::Text);
        if let Some(existing) = p.types() {
            for t in existing.iter() {
                let name = t.to_string();
                if written.contains(&name) { continue; }
                if !keep_existing_type(&name, writing_text) { continue; }
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

/// Should an existing pasteboard type be carried over when we rewrite the clipboard?
/// Legacy pboard types ("NSStringPboardType", "Apple HTML pasteboard type", ...) are aliases of the
/// modern UTIs: re-adding them would overwrite what we just wrote. Same for the UTF-16 variants of
/// the text flavor when we are writing text.
pub fn keep_existing_type(name: &str, writing_text: bool) -> bool {
    let legacy = !name.contains('.') || name.contains(' ');
    let text_alias = writing_text && (name.starts_with("public.utf16") || name == "public.plain-text");
    !(legacy || text_alias)
}

#[cfg(test)]
mod tests {
    use super::keep_existing_type;

    /// Regression: Chrome's clipboard carries NSStringPboardType, whose old text overwrote the
    /// Markdown written by `ct md` when it was carried over.
    #[test]
    fn legacy_aliases_are_not_carried_over() {
        for legacy in ["NSStringPboardType", "Apple HTML pasteboard type", "NeXT Rich Text Format v1.0 pasteboard type", "CorePasteboardFlavorType 0x75743136"] {
            assert!(!keep_existing_type(legacy, true), "{legacy}");
            assert!(!keep_existing_type(legacy, false), "{legacy}");
        }
        assert!(!keep_existing_type("public.utf16-external-plain-text", true));
        assert!(keep_existing_type("public.utf16-external-plain-text", false));
        for modern in ["public.html", "public.rtf", "org.chromium.source-url", "me.jkn.clipto.markdown", "public.png"] {
            assert!(keep_existing_type(modern, true), "{modern}");
        }
    }
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
