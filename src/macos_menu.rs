//! macOS-only polish for the popup menu: number badges (pills) on the target items and a
//! small, multi-line, secondary-coloured preview. Works on the NSMenu that muda created.
use muda::{ContextMenu, Menu};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AllocAnyThread;
use objc2_app_kit::{NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSMenu, NSMenuItemBadge};
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSDictionary, NSString};

fn small_secondary(text: &str) -> Retained<NSAttributedString> {
    let font = NSFont::systemFontOfSize(11.0);
    let color = NSColor::secondaryLabelColor();
    let keys: [&NSAttributedStringKey; 2] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [&font, &color];
    let attrs = NSDictionary::from_slices(&keys, &values);
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &NSString::from_str(text), Some(&attrs)) }
}

/// `preview`: (title as appended, text to show). `badges`: (item title, badge text).
pub fn decorate(menu: &Menu, preview: Option<(&str, &str)>, badges: &[(String, String)]) {
    let ptr = menu.ns_menu();
    if ptr.is_null() { return; }
    let ns_menu: &NSMenu = unsafe { &*(ptr as *const NSMenu) };
    for item in ns_menu.itemArray().iter() {
        let title = item.title().to_string();
        if let Some((marker, text)) = preview {
            if title == marker {
                item.setAttributedTitle(Some(&small_secondary(text)));
                continue;
            }
        }
        if let Some((_, badge)) = badges.iter().find(|(t, _)| *t == title) {
            let b = NSMenuItemBadge::initWithString(NSMenuItemBadge::alloc(), &NSString::from_str(badge));
            item.setBadge(Some(&b));
        }
    }
}
