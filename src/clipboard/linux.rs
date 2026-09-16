//! Linux: X11 and Wayland through arboard, text and HTML only, command line only.
//!
//! On Linux the clipboard is served by the process that copied. `ct` therefore re-executes
//! itself as a small background server (`ct __serve`) that keeps offering the content until
//! another application takes the clipboard over.
use crate::convert::Flavor;
use arboard::{Clipboard, SetExtLinux};
use std::io::{Read, Write};

fn cb() -> Option<Clipboard> { Clipboard::new().ok() }

pub fn types() -> Vec<String> {
    let mut v = Vec::new();
    if has(Flavor::Text) { v.push("text/plain".into()); }
    if has(Flavor::Html) { v.push("text/html".into()); }
    v
}

pub fn has(f: Flavor) -> bool { read(f).is_some() }

pub fn read(f: Flavor) -> Option<Vec<u8>> {
    let mut c = cb()?;
    match f {
        Flavor::Text => c.get().text().ok().map(String::into_bytes),
        Flavor::Html => c.get().html().ok().map(String::into_bytes),
        Flavor::Rtf | Flavor::Md => None,
    }
}

pub fn write(items: &[(Flavor, Vec<u8>)], keep_others: bool) -> Result<(), String> {
    let pick = |f: Flavor| items.iter().find(|(x, _)| *x == f).map(|(_, b)| String::from_utf8_lossy(b).into_owned());
    let text = pick(Flavor::Text);
    let mut html = pick(Flavor::Html);
    if keep_others && html.is_none() {
        html = read(Flavor::Html).map(|b| String::from_utf8_lossy(&b).into_owned());
    }
    let (Some(text), html) = (text.or_else(|| html.clone()), html) else { return Err("nothing to write".into()) };

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = {
        use std::os::unix::process::CommandExt;
        std::process::Command::new(exe)
            .arg("__serve")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| format!("could not start the clipboard server: {e}"))?
    };
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let html = html.unwrap_or_default();
    write!(stdin, "{} {}\n", text.len(), html.len()).map_err(|e| e.to_string())?;
    stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    stdin.write_all(html.as_bytes()).map_err(|e| e.to_string())?;
    drop(stdin);
    // Give the server a moment to take ownership before we return.
    std::thread::sleep(std::time::Duration::from_millis(150));
    Ok(())
}

/// Background server: read text and HTML from stdin, own the clipboard until replaced.
pub fn serve() -> ! {
    let mut input = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut input);
    let nl = input.iter().position(|&b| b == b'\n').unwrap_or(0);
    let header = String::from_utf8_lossy(&input[..nl]).to_string();
    let mut parts = header.split(' ').filter_map(|n| n.parse::<usize>().ok());
    let (tl, hl) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let body = &input[nl + 1..];
    let text = String::from_utf8_lossy(&body[..tl.min(body.len())]).into_owned();
    let html = String::from_utf8_lossy(&body[tl.min(body.len())..(tl + hl).min(body.len())]).into_owned();
    if let Ok(mut c) = Clipboard::new() {
        let set = c.set().wait();
        let _ = if html.is_empty() { set.text(text) } else { set.html(html, Some(text)) };
    }
    std::process::exit(0)
}

pub fn rtf_to_html(_rtf: &[u8]) -> Option<String> { None }
pub fn html_to_rtf(_html: &str) -> Option<Vec<u8>> { None }
