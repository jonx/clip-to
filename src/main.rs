//! ct — ClipTo: convert the clipboard between plain text, Markdown and rich text.
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod clipboard;
mod convert;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod daemon;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_panel;
#[cfg(target_os = "macos")]
mod config;
mod markdown;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod paste;
mod term;

use convert::{Flavor, Target};
use std::io::Write;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ABOUT_URL: &str = "http://jkn.me";

fn print_help() {
    let pad = |s: &str, n: usize| format!("{s:<n$}");
    println!("{} — ClipTo {VERSION}: convert the clipboard between plain text, Markdown and rich text", term::bold(&term::cyan("ct")));
    println!("{}", term::dim("John Knipper · http://jkn.me"));
    println!();
    println!("{}", term::bold("Commands"));
    for t in Target::ALL {
        let name = format!("{}/{}", t.name(), &t.name()[..1]);
        println!("  {}  {}", term::green(&pad(&name, 11)), t.help());
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        println!("  {}  tray icon + hotkey ({}) with a format chooser that pastes the result", term::green(&pad("daemon/d", 11)), daemon::describe(daemon::DEFAULT_HOTKEY));
        println!("  {}  run the daemon now and at login; {} removes it", term::green(&pad("install/i", 11)), term::green("uninstall/u"));
    }
    println!();
    println!("{}", term::bold("Options"));
    for (k, v) in [
        ("-i FILE", "read FILE ('-' = stdin) instead of the clipboard"),
        ("-o", "print the result instead of writing the clipboard"),
        ("-p", "print the result after writing the clipboard"),
        ("-x", "drop the other flavors (md, plain, html keep HTML/RTF)"),
        ("-f", "convert even if the clipboard already holds the requested format"),
        ("--rtf", "rich: RTF without HTML, for Notes, TextEdit, Pages (the popup does it automatically)"),
    ] { println!("  {}  {}", term::yellow(&pad(k, 10)), v); }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    for (k, v) in [
        ("--hotkey K", "daemon/install: e.g. ctrl+alt+super+v, ctrl+shift+f9"),
        ("--no-paste", "daemon/install: convert only, do not paste"),
    ] { println!("  {}  {}", term::yellow(&pad(k, 10)), v); }
}

fn print_clipboard() {
    let types = clipboard::types();
    let list = if types.is_empty() { term::dim("(empty)") } else { types.join(&term::dim(", ")) };
    println!("{} — {list}", term::bold("Clipboard"));
    for f in Flavor::ALL {
        let Some(bytes) = clipboard::read(f) else { continue };
        let s = String::from_utf8_lossy(&bytes);
        let n = s.chars().count();
        println!();
        println!("{}{}{}  {}", term::magenta(&format!("--- {}", f.label())), term::dim(&format!(" ({n} chars) ")), term::magenta("---"), term::dim(&format!("pasted by {}", f.pasted_by())));
        if n > 600 { println!("{}{}", s.chars().take(600).collect::<String>(), term::dim("…")); } else { println!("{s}"); }
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("{} {msg}", term::err_red("ct:"));
    std::process::exit(1)
}

fn main() {
    // Behave like other CLI tools when the reader goes away (`ct | head`): exit quietly instead of panicking.
    #[cfg(unix)]
    unsafe {
        extern "C" { fn signal(sig: i32, handler: usize) -> usize; }
        const SIGPIPE: i32 = 13;
        signal(SIGPIPE, 0);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut infile: Option<String> = None;
    let mut to_stdout = false;
    let mut also_print = false;
    let mut exclusive = false;
    let mut force = false;
    let mut rtf_only = false;
    #[allow(unused_variables, unused_assignments)]
    let mut hotkey: Option<String> = None;
    #[allow(unused_variables, unused_assignments)]
    let mut no_paste = false;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" => { i += 1; infile = Some(args.get(i).cloned().unwrap_or_else(|| "-".into())); }
            "-o" => to_stdout = true,
            "-p" => also_print = true,
            "-x" => exclusive = true,
            "-f" => force = true,
            "--rtf" => rtf_only = true,
            "--hotkey" => { i += 1; hotkey = args.get(i).cloned(); let _ = &hotkey; }
            "--no-paste" => { no_paste = true; let _ = no_paste; }
            "-h" | "--help" | "help" => { print_help(); return; }
            "-v" | "--version" => { println!("ct {VERSION}"); return; }
            other => positional.push(other.to_string()),
        }
        i += 1;
    }
    let cmd = positional.first().map(String::as_str).unwrap_or("");

    match cmd {
        "" => { print_help(); println!(); print_clipboard(); }
        #[cfg(target_os = "linux")]
        "__serve" => clipboard::serve(),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "daemon" | "d" => daemon::run(&hotkey.clone().unwrap_or_else(|| daemon::DEFAULT_HOTKEY.to_string()), !no_paste),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "install" | "i" => {
            if let Some(h) = &hotkey { if let Err(e) = daemon::parse_hotkey(h) { fail(&e); } }
            match autostart::install(hotkey.as_deref(), no_paste) { Ok(m) => eprintln!("ct: {m}"), Err(e) => fail(&format!("install failed: {e}")) }
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "uninstall" | "u" => match autostart::uninstall() { Ok(m) => eprintln!("ct: {m}"), Err(e) => fail(&format!("uninstall failed: {e}")) },
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        "daemon" | "d" | "install" | "i" | "uninstall" | "u" => fail("resident mode is not available on this platform; use the command line"),
        other => {
            let Some(target) = Target::parse(other) else {
                eprintln!("{} unknown command '{other}'\n", term::err_red("ct:"));
                print_help();
                std::process::exit(2);
            };
            if infile.is_none() && !to_stdout && !force {
                if let Some(reason) = convert::already_satisfied(target) {
                    eprintln!("{} clipboard unchanged: {reason} {}", term::err_green("="), term::err_dim("(-f to convert anyway)"));
                    return;
                }
            }
            let src = match &infile {
                Some(f) => {
                    let data = if f == "-" {
                        let mut v = Vec::new(); std::io::Read::read_to_end(&mut std::io::stdin(), &mut v).ok(); v
                    } else {
                        std::fs::read(f).unwrap_or_else(|e| fail(&format!("{f}: {e}")))
                    };
                    convert::Source::Markdown(String::from_utf8_lossy(&data).into_owned())
                }
                None => convert::read_source(&target.prefer()).unwrap_or_else(|| {
                    fail(&format!("nothing usable on the clipboard (types: {})", clipboard::types().join(", ")))
                }),
            };
            let out = convert::convert_for(&src, target, rtf_only);
            if to_stdout {
                let _ = std::io::stdout().write_all(out.result.as_bytes());
            } else {
                let keep = !exclusive && !target.replaces_all();
                if let Err(e) = clipboard::write(&out.items, keep) { fail(&e); }
                let present = clipboard::present();
                let names = present.iter().map(|f| f.label()).collect::<Vec<_>>().join(", ");
                let note = if keep && present.len() > 1 { term::err_dim("  (other flavors kept, -x to drop them)") } else { String::new() };
                eprintln!("{} clipboard: {}{note}", term::err_green("✓"), term::err_cyan(&names));
                if also_print { let _ = std::io::stdout().write_all(out.result.as_bytes()); }
            }
        }
    }
}
