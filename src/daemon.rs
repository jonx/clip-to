//! Resident mode: tray/menu bar icon plus a global hotkey. Pressing the hotkey pops up a
//! menu at the mouse pointer listing the flavors on the clipboard and the target formats.
//! Built on tao (event loop), tray-icon, muda (menus) and global-hotkey, which call the
//! native APIs on each platform (NSStatusItem/NSMenu/Carbon hotkeys on macOS, Shell_NotifyIcon/
//! TrackPopupMenu/RegisterHotKey on Windows).
use crate::convert::{self, Target};
use crate::{clipboard, ABOUT_URL};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use muda::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::time::{Duration, Instant};
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::{Window, WindowBuilder};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub const DEFAULT_HOTKEY: &str = "ctrl+alt+super+v";

/// Human-readable form of a hotkey spec ("ctrl+alt+super+v" -> "⌃⌥⌘V" on macOS).
pub fn describe(spec: &str) -> String {
    let mac = cfg!(target_os = "macos");
    spec.split('+')
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => if mac { "⌃" } else { "Ctrl+" }.to_string(),
            "alt" | "option" => if mac { "⌥" } else { "Alt+" }.to_string(),
            "super" | "cmd" | "command" | "meta" => if mac { "⌘" } else { "Win+" }.to_string(),
            "shift" => if mac { "⇧" } else { "Shift+" }.to_string(),
            k => k.to_uppercase(),
        })
        .collect()
}

pub fn parse_hotkey(spec: &str) -> Result<HotKey, String> {
    spec.parse::<HotKey>().map_err(|e| format!("invalid hotkey '{spec}': {e}"))
}

/// A tiny clipboard glyph drawn in code, so no image file has to ship.
fn icon() -> Icon {
    const W: u32 = 22;
    const H: u32 = 22;
    let mut px = vec![0u8; (W * H * 4) as usize];
    let mut set = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        px[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
    };
    for x in 4..18 { for y in [3, 4, 19, 20] { set(x, y); } }          // top and bottom edges
    for y in 3..21 { for x in [4, 5, 16, 17] { set(x, y); } }          // sides
    for x in 8..14 { for y in 1..6 { set(x, y); } }                    // clip
    for y in [9, 12, 15] { for x in 7..15 { set(x, y); } }             // text lines
    Icon::from_rgba(px, W, H).expect("icon")
}

struct Ids {
    targets: Vec<(String, Target)>,
}

/// Wrap a preview into at most `lines` lines of about `width` characters.
fn wrap_preview(text: &str, width: usize, lines: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in flat.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut cur));
            if out.len() == lines { break; }
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(word);
    }
    if out.len() < lines && !cur.is_empty() { out.push(cur); }
    let truncated = out.iter().map(|l| l.chars().count() + 1).sum::<usize>() < flat.chars().count() + 1;
    if truncated {
        if let Some(last) = out.last_mut() {
            *last = last.chars().take(width.saturating_sub(1)).collect::<String>() + "…";
        }
    }
    out.join("\n")
}

const PREVIEW_MARKER: &str = "\u{200b}preview";

fn build_menu(hotkey_desc: &str, resident: bool) -> (Menu, Ids) {
    let menu = Menu::new();
    let present = clipboard::present();
    let preview = clipboard::read(crate::convert::Flavor::Text)
        .and_then(|b| String::from_utf8(b).ok())
        .map(|t| wrap_preview(&t, 44, 3))
        .filter(|p| !p.is_empty());
    if let Some(p) = &preview {
        // On macOS the marker title is replaced by a small attributed title; elsewhere show one line.
        let title = if cfg!(target_os = "macos") { PREVIEW_MARKER.to_string() } else { p.lines().next().unwrap_or("").to_string() };
        let _ = menu.append(&MenuItem::new(title, false, None));
    }
    let header = if present.is_empty() {
        "Clipboard: empty".to_string()
    } else {
        format!("Clipboard: {}", present.iter().map(|f| f.label()).collect::<Vec<_>>().join(", "))
    };
    let _ = menu.append(&MenuItem::new(header, false, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let mut ids = Ids { targets: Vec::new() };
    let mut badges: Vec<(String, String)> = Vec::new();
    for (i, t) in Target::ALL.iter().enumerate() {
        let n = (i + 1).to_string();
        // Windows menus right-align text after a tab; macOS gets a badge pill instead.
        let title = if cfg!(target_os = "macos") { t.title().to_string() } else { format!("{}\t{n}", t.title()) };
        let item = MenuItem::with_id(format!("target-{}", t.name()), &title, !present.is_empty(), None);
        ids.targets.push((item.id().0.clone(), *t));
        badges.push((title, n));
        let _ = menu.append(&item);
    }
    if resident {
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::new(format!("Hotkey: {hotkey_desc}"), false, None));
        let _ = menu.append(&MenuItem::with_id("about", "ClipTo — John Knipper, jkn.me", true, None));
        let _ = menu.append(&MenuItem::with_id("quit", "Quit ct", true, None));
    }
    #[cfg(target_os = "macos")]
    crate::macos_menu::decorate(&menu, preview.as_deref().map(|p| (PREVIEW_MARKER, p)), &badges);
    #[cfg(not(target_os = "macos"))]
    let _ = badges;
    (menu, ids)
}

fn show_popup(menu: &Menu, window: &Window) {
    let previous = clipboard::activate_app_for_popup();
    #[cfg(target_os = "macos")]
    unsafe {
        use tao::platform::macos::WindowExtMacOS;
        menu.show_context_menu_for_nsview(window.ns_view() as *const _, None);
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use tao::platform::windows::WindowExtWindows;
        let hwnd = window.hwnd();
        windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd as _);
        menu.show_context_menu_for_hwnd(hwnd, None);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { let _ = (menu, window); }
    // The menu blocks until dismissed; hand focus back so the user's ⌘V lands where they were.
    clipboard::restore_previous_app(previous);
}

pub fn run(hotkey_spec: &str, auto_paste: bool) -> ! {
    let hotkey = match parse_hotkey(hotkey_spec) {
        Ok(h) => h,
        Err(e) => { eprintln!("ct: {e}"); std::process::exit(2); }
    };
    let desc = describe(hotkey_spec);

    #[cfg(target_os = "windows")]
    unsafe { windows_sys::Win32::System::Console::FreeConsole(); }

    #[allow(unused_mut)]
    let mut event_loop: EventLoop<()> = EventLoop::new();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }
    // A hidden window gives the context menu something to attach to.
    let window = WindowBuilder::new().with_visible(false).with_decorations(false).with_title("ClipTo").build(&event_loop).expect("window");

    let manager = GlobalHotKeyManager::new().expect("hotkey manager");
    if let Err(e) = manager.register(hotkey) {
        eprintln!("ct: could not register hotkey {desc} ({e}); it may be taken by another app.");
    }

    let (menu, mut ids) = build_menu(&desc, true);
    let tray: TrayIcon = TrayIconBuilder::new()
        .with_icon(icon())
        .with_icon_as_template(true)
        .with_tooltip("ClipTo")
        .with_menu(Box::new(menu))
        .build()
        .expect("tray icon");

    eprintln!("ct: resident, press {desc} to convert the clipboard{}. Ctrl-C or Quit in the menu to stop.", if auto_paste { " and paste the result" } else { "" });
    if auto_paste && !crate::paste::trusted(false) {
        eprintln!("ct: pasting needs the Accessibility permission; macOS will ask on first use (System Settings > Privacy & Security > Accessibility).");
    }

    let mut last_change = clipboard::change_count();
    let mut flash_until: Option<Instant> = None;
    let mut popup: Option<(Menu, Ids)> = None;
    let mut from_popup = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(250));
        if let Event::LoopDestroyed = event { return; }

        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state == HotKeyState::Pressed && ev.id == hotkey.id() {
                let built = build_menu(&desc, false);
                show_popup(&built.0, &window);
                popup = Some(built);
                from_popup = true;
            }
        }

        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let id = ev.id.0.as_str();
            let target = ids.targets.iter().chain(popup.iter().flat_map(|p| p.1.targets.iter()))
                .find(|(i, _)| i == id).map(|(_, t)| *t);
            if let Some(t) = target {
                match convert::read_source(&t.prefer()) {
                    None => { tray.set_title(Some(" ✗ empty")); }
                    Some(src) => {
                        let out = convert::convert(&src, t);
                        match clipboard::write(&out.items, !t.replaces_all()) {
                            Ok(()) => {
                                tray.set_title(Some(format!(" ✓ {}", t.title())));
                                if auto_paste && from_popup {
                                    // Focus went back to the previous app when the menu closed; give it a beat.
                                    std::thread::sleep(Duration::from_millis(120));
                                    if let Err(e) = crate::paste::paste() { eprintln!("ct: {e}"); tray.set_title(Some(" ✗ paste")); }
                                }
                            }
                            Err(e) => tray.set_title(Some(format!(" ✗ {e}"))),
                        }
                    }
                }
                flash_until = Some(Instant::now() + Duration::from_millis(1500));
                from_popup = false;
            } else if id == "about" {
                let _ = open_url(ABOUT_URL);
            } else if id == "quit" {
                *control_flow = ControlFlow::Exit;
            }
        }

        if let Some(t) = flash_until { if Instant::now() >= t { tray.set_title(None::<&str>); flash_until = None; } }

        let now = clipboard::change_count();
        if now != last_change {
            last_change = now;
            let (menu, new_ids) = build_menu(&desc, true);
            tray.set_menu(Some(Box::new(menu)));
            ids = new_ids;
        }
    })
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(url).spawn().map(|_| ()) }
    #[cfg(target_os = "windows")]
    { std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map(|_| ()) }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ()) }
}
