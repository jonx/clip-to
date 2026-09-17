//! macOS chooser: a floating, non-activating panel shown at the mouse pointer instead of a
//! context menu. Left: the target list (mouse, keyboard, hover), a "force" checkbox that
//! follows ⌥, and hints. Right: a native NSTextView with the full, scrollable result, rendered
//! by the same engine TextEdit and Notes use. The app that had focus keeps it.
use crate::clipboard;
use crate::config;
use crate::convert::{self, Flavor, Target};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAttributedStringAppKitDocumentFormats, NSBackgroundColorAttributeName, NSBackingStoreType, NSBezierPath, NSButton, NSColor,
    NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightMedium, NSFontWeightRegular,
    NSFontWeightSemibold, NSForegroundColorAttributeName, NSPanel, NSScreen, NSScrollView, NSStringDrawing, NSTextView,
    NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSData, NSDictionary, NSPoint, NSRange, NSRect, NSSize, NSString};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

pub struct Choice { pub target: Target, pub force: bool }

thread_local! {
    static CHOICE: RefCell<Option<Choice>> = const { RefCell::new(None) };
    static PANEL: RefCell<Option<(Retained<ChooserPanel>, Retained<ChooserView>)>> = const { RefCell::new(None) };
}

pub fn take_choice() -> Option<Choice> { CHOICE.with(|c| c.borrow_mut().take()) }
pub fn is_open() -> bool { PANEL.with(|p| p.borrow().as_ref().map(|(w, _)| w.isVisible()).unwrap_or(false)) }

const PANEL_W: f64 = 560.0;
const PANEL_H: f64 = 380.0;
const LIST_W: f64 = 214.0;
const ROW_H: f64 = 32.0;
const ROWS_TOP: f64 = 40.0;
const PAD: f64 = 12.0;

struct Row { target: Target, title: String }

enum Preview { Rich(Retained<NSAttributedString>), Plain(String), Empty(String) }

#[derive(Default)]
struct State {
    rows: Vec<Row>,
    selected: usize,
    hover: Option<usize>,
    previews: HashMap<&'static str, Preview>,
}

struct ViewIvars {
    state: RefCell<State>,
    text: RefCell<Option<Retained<NSTextView>>>,
    checkbox: RefCell<Option<Retained<NSButton>>>,
    option_held: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "CTChooserPanel"]
    #[ivars = ()]
    struct ChooserPanel;

    impl ChooserPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool { true }
        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool { false }
    }
);

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "CTChooserView"]
    #[ivars = ViewIvars]
    struct ChooserView;

    impl ChooserView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool { true }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool { true }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) { self.draw(); }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            let hit = self.row_at(p);
            let changed = { let mut s = self.ivars().state.borrow_mut(); let c = s.hover != hit; s.hover = hit; if let Some(i) = hit { s.selected = i; } c };
            if changed { self.refresh_preview(); self.setNeedsDisplay(true); }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().state.borrow_mut().hover = None;
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            if let Some(i) = self.row_at(p) { self.choose(i); }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            if let Some(i) = self.row_at(p) { self.hide_row(i); }
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            let held = event.modifierFlags().contains(NSEventModifierFlags::Option);
            let was = self.ivars().option_held.get();
            if held && !was {
                // A fresh ⌥ press after the panel opened: arm "force" (the checkbox stays on until chosen or toggled).
                self.ivars().option_held.set(true);
                if let Some(cb) = &*self.ivars().checkbox.borrow() { cb.setState(NSControlStateValueOn); }
            } else if !held && was {
                self.ivars().option_held.set(false);
                if let Some(cb) = &*self.ivars().checkbox.borrow() { cb.setState(0); }
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let code = event.keyCode();
            let chars = event.charactersIgnoringModifiers().map(|s| s.to_string()).unwrap_or_default();
            let cmd = event.modifierFlags().contains(NSEventModifierFlags::Command);
            match code {
                53 => close_panel(),                                   // esc
                36 | 76 => { let i = self.ivars().state.borrow().selected; self.choose(i); } // return, enter
                126 => self.move_selection(-1),                        // up
                125 => self.move_selection(1),                         // down
                _ => {
                    if cmd && chars == "r" { self.restore_rows(); return; }
                    if let Some(d) = chars.chars().next().and_then(|c| c.to_digit(10)) {
                        let n = self.ivars().state.borrow().rows.len();
                        if d >= 1 && (d as usize) <= n { self.choose(d as usize - 1); }
                    } else if chars == " " {
                        if let Some(cb) = &*self.ivars().checkbox.borrow() {
                            cb.setState(if cb.state() == NSControlStateValueOn { 0 } else { NSControlStateValueOn });
                        }
                    }
                }
            }
        }

        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) { close_panel(); }
    }
);

impl ChooserView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars {
            state: RefCell::new(State::default()),
            text: RefCell::new(None),
            checkbox: RefCell::new(None),
            option_held: Cell::new(false),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn row_rect(&self, i: usize) -> NSRect {
        NSRect::new(NSPoint::new(PAD, ROWS_TOP + i as f64 * ROW_H), NSSize::new(LIST_W - 2.0 * PAD, ROW_H - 4.0))
    }

    fn row_at(&self, p: NSPoint) -> Option<usize> {
        let n = self.ivars().state.borrow().rows.len();
        (0..n).find(|&i| { let r = self.row_rect(i); p.x >= r.origin.x && p.x <= r.origin.x + r.size.width && p.y >= r.origin.y && p.y <= r.origin.y + r.size.height })
    }

    fn move_selection(&self, delta: i32) {
        { let mut s = self.ivars().state.borrow_mut(); let n = s.rows.len() as i32; if n == 0 { return; } s.selected = ((s.selected as i32 + delta).rem_euclid(n)) as usize; s.hover = None; }
        self.refresh_preview();
        self.setNeedsDisplay(true);
    }

    fn force(&self) -> bool {
        self.ivars().checkbox.borrow().as_ref().map(|cb| cb.state() == NSControlStateValueOn).unwrap_or(false) || self.ivars().option_held.get()
    }

    fn choose(&self, i: usize) {
        let target = { let s = self.ivars().state.borrow(); match s.rows.get(i) { Some(r) => r.target, None => return } };
        let force = self.force();
        CHOICE.with(|c| *c.borrow_mut() = Some(Choice { target, force }));
        close_panel();
    }

    fn hide_row(&self, i: usize) {
        let mut hidden = config::hidden_targets();
        { let mut s = self.ivars().state.borrow_mut(); if s.rows.len() <= 1 { return; } let t = s.rows.remove(i).target; hidden.push(t); if s.selected >= s.rows.len() { s.selected = s.rows.len() - 1; } s.hover = None; }
        config::set_hidden_targets(&hidden);
        self.refresh_preview();
        self.setNeedsDisplay(true);
    }

    fn restore_rows(&self) {
        config::set_hidden_targets(&[]);
        self.load_rows();
        self.refresh_preview();
        self.setNeedsDisplay(true);
    }

    fn load_rows(&self) {
        let hidden = config::hidden_targets();
        let mut s = self.ivars().state.borrow_mut();
        s.rows = Target::ALL.iter().filter(|t| !hidden.contains(t)).map(|t| Row { target: *t, title: t.title().to_string() }).collect();
        if s.rows.is_empty() { s.rows = Target::ALL.iter().map(|t| Row { target: *t, title: t.title().to_string() }).collect(); }
        s.selected = s.selected.min(s.rows.len().saturating_sub(1));
        s.hover = None;
        s.previews.clear();
    }

    /// Compute (and cache) what the selected target would put on the clipboard, then show it.
    fn refresh_preview(&self) {
        let target = { let s = self.ivars().state.borrow(); match s.rows.get(s.selected) { Some(r) => r.target, None => return } };
        let key = target.name();
        if !self.ivars().state.borrow().previews.contains_key(key) {
            let preview = build_preview(target);
            self.ivars().state.borrow_mut().previews.insert(key, preview);
        }
        let Some(text) = self.ivars().text.borrow().clone() else { return };
        let s = self.ivars().state.borrow();
        match s.previews.get(key) {
            Some(Preview::Rich(attr)) => {
                text.setString(&NSString::from_str(""));
                if let Some(storage) = unsafe { text.textStorage() } {
                    storage.beginEditing();
                    storage.setAttributedString(attr);
                    // Preview only: drop background colours (dark-theme HTML copied from editors), so the
                    // text stays readable. The clipboard keeps them; the target app decides.
                    storage.removeAttribute_range(unsafe { NSBackgroundColorAttributeName }, NSRange::new(0, storage.length()));
                    storage.endEditing();
                }
            }
            Some(Preview::Plain(t)) => {
                text.setString(&NSString::from_str(t));
                text.setFont(Some(&mono(12.0, Weight::Regular)));
                text.setTextColor(Some(&NSColor::textColor()));
            }
            Some(Preview::Empty(t)) => {
                text.setString(&NSString::from_str(t));
                text.setFont(Some(&NSFont::systemFontOfSize(12.0)));
                text.setTextColor(Some(&NSColor::secondaryLabelColor()));
            }
            None => {}
        }
    }

    fn draw(&self) {
        let s = self.ivars().state.borrow();
        let bounds = self.bounds();
        // Header.
        draw_text("Convert to", NSPoint::new(PAD + 2.0, 14.0), &sys(11.0, Weight::Semibold), &NSColor::secondaryLabelColor());
        for (i, row) in s.rows.iter().enumerate() {
            let r = self.row_rect(i);
            let selected = i == s.selected;
            if selected {
                NSColor::selectedContentBackgroundColor().setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, 6.0, 6.0).fill();
            } else if s.hover == Some(i) {
                NSColor::quaternaryLabelColor().setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, 6.0, 6.0).fill();
            }
            let fg = if selected { NSColor::alternateSelectedControlTextColor() } else { NSColor::labelColor() };
            draw_text(&row.title, NSPoint::new(r.origin.x + 10.0, r.origin.y + 6.0), &sys(13.0, Weight::Medium), &fg);
            // Number badge on the right.
            let badge = NSRect::new(NSPoint::new(r.origin.x + r.size.width - 30.0, r.origin.y + 5.0), NSSize::new(22.0, 18.0));
            if selected { NSColor::alternateSelectedControlTextColor().setFill() } else { NSColor::quaternaryLabelColor().setFill() }
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(badge, 9.0, 9.0).fill();
            let nfg = if selected { NSColor::selectedContentBackgroundColor() } else { NSColor::secondaryLabelColor() };
            draw_text(&(i + 1).to_string(), NSPoint::new(badge.origin.x + 7.5, badge.origin.y + 1.5), &mono(11.0, Weight::Semibold), &nfg);
        }
        // Footer hints.
        let hints_y = bounds.size.height - 44.0;
        draw_text("1-5 · ↑↓ · ⏎ choose · esc", NSPoint::new(PAD + 2.0, hints_y), &NSFont::systemFontOfSize(10.0), &NSColor::tertiaryLabelColor());
        draw_text("right-click hides a choice · ⌘R restores", NSPoint::new(PAD + 2.0, hints_y + 14.0), &NSFont::systemFontOfSize(10.0), &NSColor::tertiaryLabelColor());
        // Preview title.
        let title = match s.rows.get(s.selected) {
            Some(r) => format!("Preview — {} ({})", r.title, preview_flavor_label(r.target)),
            None => "Preview".to_string(),
        };
        draw_text(&title, NSPoint::new(LIST_W + 4.0, 14.0), &sys(11.0, Weight::Semibold), &NSColor::secondaryLabelColor());
        let clip = clipboard::present();
        let src = if clip.is_empty() { "clipboard: empty".to_string() } else { format!("clipboard: {}", clip.iter().map(|f| f.label()).collect::<Vec<_>>().join(", ")) };
        draw_text(&src, NSPoint::new(LIST_W + 4.0, bounds.size.height - 30.0), &NSFont::systemFontOfSize(10.0), &NSColor::tertiaryLabelColor());
    }
}

#[derive(Clone, Copy)]
enum Weight { Regular, Medium, Semibold }
fn weight(w: Weight) -> objc2_app_kit::NSFontWeight {
    unsafe { match w { Weight::Regular => NSFontWeightRegular, Weight::Medium => NSFontWeightMedium, Weight::Semibold => NSFontWeightSemibold } }
}
fn sys(size: f64, w: Weight) -> Retained<NSFont> { NSFont::systemFontOfSize_weight(size, weight(w)) }
fn mono(size: f64, w: Weight) -> Retained<NSFont> { NSFont::monospacedSystemFontOfSize_weight(size, weight(w)) }

fn preview_flavor_label(t: Target) -> &'static str {
    match t { Target::Rich => "as rich apps see it", Target::Md | Target::Plain | Target::Html => "text flavor", Target::Text => "plain text only" }
}

fn draw_text(text: &str, at: NSPoint, font: &NSFont, color: &NSColor) {
    let keys: [&NSAttributedStringKey; 2] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [font, color];
    let attrs = NSDictionary::from_slices(&keys, &values);
    unsafe { NSString::from_str(text).drawAtPoint_withAttributes(at, Some(&attrs)) };
}

fn build_preview(target: Target) -> Preview {
    if let Some(reason) = convert::already_satisfied(target) {
        // Show what is there already: the conversion would not run.
        let current = match target {
            Target::Rich => clipboard::read(Flavor::Rtf).and_then(|d| attributed_from_rtf(&d))
                .or_else(|| clipboard::read(Flavor::Html).and_then(|d| attributed_from_html(&d))).map(Preview::Rich),
            _ => clipboard::read(Flavor::Text).map(|b| Preview::Plain(String::from_utf8_lossy(&b).into_owned())),
        };
        return match current { Some(p) => p, None => Preview::Empty(format!("Unchanged: {reason}")) };
    }
    let Some(src) = convert::read_source(&target.prefer()) else { return Preview::Empty("Nothing usable on the clipboard".into()) };
    let out = convert::convert_for(&src, target, false);
    match target {
        Target::Rich => {
            let rtf = out.items.iter().find(|(f, _)| *f == Flavor::Rtf).and_then(|(_, d)| attributed_from_rtf(d));
            let html = || out.items.iter().find(|(f, _)| *f == Flavor::Html).and_then(|(_, d)| attributed_from_html(d));
            match rtf.or_else(html) { Some(a) => Preview::Rich(a), None => Preview::Plain(out.result) }
        }
        _ => Preview::Plain(out.result),
    }
}

fn attributed_from_rtf(rtf: &[u8]) -> Option<Retained<NSAttributedString>> {
    unsafe { NSAttributedString::initWithRTF_documentAttributes(NSAttributedString::alloc(), &NSData::with_bytes(rtf), None) }
}
fn attributed_from_html(html: &[u8]) -> Option<Retained<NSAttributedString>> {
    unsafe { NSAttributedString::initWithHTML_documentAttributes(NSAttributedString::alloc(), &NSData::with_bytes(html), None) }
}

fn close_panel() {
    PANEL.with(|p| if let Some((w, _)) = &*p.borrow() { w.orderOut(None); });
}

fn build(mtm: MainThreadMarker) -> (Retained<ChooserPanel>, Retained<ChooserView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PANEL_W, PANEL_H));
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView | NSWindowStyleMask::NonactivatingPanel | NSWindowStyleMask::UtilityWindow;
    let panel: Retained<ChooserPanel> = unsafe {
        let this = ChooserPanel::alloc(mtm).set_ivars(());
        msg_send![super(this), initWithContentRect: frame, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false]
    };
    panel.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    panel.setTitlebarAppearsTransparent(true);
    panel.setLevel(objc2_app_kit::NSPopUpMenuWindowLevel);
    panel.setFloatingPanel(true);
    panel.setHidesOnDeactivate(false);
    panel.setAcceptsMouseMovedEvents(true);
    unsafe { panel.setReleasedWhenClosed(false) };

    let view = ChooserView::new(mtm, frame);
    let tracking = unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(), frame,
            NSTrackingAreaOptions::MouseMoved | NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::InVisibleRect,
            Some(&view), None)
    };
    view.addTrackingArea(&tracking);

    // Force checkbox under the rows.
    let cb = unsafe { NSButton::checkboxWithTitle_target_action(&NSString::from_str("Force conversion (⌥)"), None, None, mtm) };
    let cb_y = ROWS_TOP + Target::ALL.len() as f64 * ROW_H + 8.0;
    cb.setFrame(NSRect::new(NSPoint::new(PAD + 4.0, cb_y), NSSize::new(LIST_W - 2.0 * PAD, 20.0)));
    view.addSubview(&cb);
    *view.ivars().checkbox.borrow_mut() = Some(cb);

    // Preview: scroll view + text view following the system theme; document colours (black text
    // from RTF/HTML) are remapped for dark mode by AppKit.
    let preview_rect = NSRect::new(NSPoint::new(LIST_W, 30.0), NSSize::new(PANEL_W - LIST_W - PAD, PANEL_H - 30.0 - 34.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), preview_rect);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::LineBorder);
    let text = NSTextView::initWithFrame(NSTextView::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(preview_rect.size.width, preview_rect.size.height)));
    text.setEditable(false);
    text.setSelectable(true);
    text.setRichText(true);
    text.setVerticallyResizable(true);
    text.setHorizontallyResizable(false);
    text.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
    text.setTextContainerInset(NSSize::new(10.0, 10.0));
    if let Some(container) = unsafe { text.textContainer() } {
        container.setWidthTracksTextView(true);
        container.setContainerSize(NSSize::new(preview_rect.size.width, f64::MAX));
    }
    text.setUsesAdaptiveColorMappingForDarkAppearance(true);
    scroll.setDocumentView(Some(&text));
    view.addSubview(&scroll);
    *view.ivars().text.borrow_mut() = Some(text);

    panel.setContentView(Some(&view));
    (panel, view)
}

/// Show the chooser near the mouse pointer. The choice is delivered through `take_choice`.
pub fn show() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    PANEL.with(|slot| {
        if slot.borrow().is_none() { *slot.borrow_mut() = Some(build(mtm)); }
        let guard = slot.borrow();
        let (panel, view) = guard.as_ref().unwrap();
        view.load_rows();
        // Start unchecked: the hotkey itself usually contains ⌥, so the current modifier state means nothing yet.
        view.ivars().option_held.set(false);
        if let Some(cb) = &*view.ivars().checkbox.borrow() { cb.setState(0); }
        view.refresh_preview();
        // Position: just under the pointer, kept inside the screen.
        let mouse = NSEvent::mouseLocation();
        let mut x = mouse.x - 24.0;
        let mut y = mouse.y - PANEL_H + 24.0;
        if let Some(screen) = NSScreen::screens(mtm).iter().find(|s| { let f = s.frame(); mouse.x >= f.origin.x && mouse.x <= f.origin.x + f.size.width && mouse.y >= f.origin.y && mouse.y <= f.origin.y + f.size.height }) {
            let f = screen.visibleFrame();
            x = x.max(f.origin.x).min(f.origin.x + f.size.width - PANEL_W);
            y = y.max(f.origin.y).min(f.origin.y + f.size.height - PANEL_H);
        }
        panel.setFrameOrigin(NSPoint::new(x, y));
        panel.makeKeyAndOrderFront(None);
        panel.makeFirstResponder(Some(view));
        view.setNeedsDisplay(true);
    });
}
