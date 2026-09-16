//! Start the daemon at login: a LaunchAgent on macOS, a Run registry value on Windows.
use std::path::PathBuf;

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().and_then(|p| p.canonicalize()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
mod imp {
    use super::exe;
    const LABEL: &str = "me.jkn.clipto";

    fn plist_path() -> String {
        format!("{}/Library/LaunchAgents/{LABEL}.plist", std::env::var("HOME").unwrap_or_default())
    }
    fn launchctl(args: &[&str]) -> i32 {
        std::process::Command::new("/bin/launchctl").args(args)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
            .status().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)
    }
    fn domain() -> String { format!("gui/{}", unsafe { libc_getuid() }) }
    extern "C" { #[link_name = "getuid"] fn libc_getuid() -> u32; }

    pub fn install(hotkey: Option<&str>, no_paste: bool) -> Result<String, String> {
        let exe = exe()?;
        let mut args = vec![exe.to_string_lossy().to_string(), "daemon".into()];
        if let Some(h) = hotkey { args.push("--hotkey".into()); args.push(h.into()); }
        if no_paste { args.push("--no-paste".into()); }
        let items: String = args.iter().map(|a| format!("    <string>{}</string>\n", a.replace('&', "&amp;").replace('<', "&lt;"))).collect();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict>\n\
             <key>Label</key><string>{LABEL}</string>\n\
             <key>ProgramArguments</key><array>\n{items}</array>\n\
             <key>RunAtLoad</key><true/>\n<key>KeepAlive</key><true/>\n<key>ProcessType</key><string>Interactive</string>\n\
             </dict></plist>\n");
        let path = plist_path();
        if let Some(dir) = std::path::Path::new(&path).parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        std::fs::write(&path, plist).map_err(|e| e.to_string())?;
        launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
        let r = launchctl(&["bootstrap", &domain(), &path]);
        if r != 0 { return Err(format!("launchctl bootstrap failed ({r})")); }
        Ok(format!("daemon installed and started ({path}). It will start at login."))
    }

    pub fn uninstall() -> Result<String, String> {
        launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
        let path = plist_path();
        if std::path::Path::new(&path).exists() { std::fs::remove_file(&path).map_err(|e| e.to_string())?; }
        Ok("daemon stopped and removed.".into())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::exe;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "ClipTo";

    pub fn install(hotkey: Option<&str>, no_paste: bool) -> Result<String, String> {
        let exe = exe()?;
        let mut cmd = format!("\"{}\" daemon", exe.to_string_lossy().trim_start_matches(r"\\?\"));
        if let Some(h) = hotkey { cmd.push_str(&format!(" --hotkey {h}")); }
        if no_paste { cmd.push_str(" --no-paste"); }
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN).map_err(|e| e.to_string())?;
        key.set_value(NAME, &cmd).map_err(|e| e.to_string())?;
        let mut args = vec!["daemon".to_string()];
        if let Some(h) = hotkey { args.push("--hotkey".into()); args.push(h.into()); }
        if no_paste { args.push("--no-paste".into()); }
        std::process::Command::new(&exe).args(&args).spawn().map_err(|e| e.to_string())?;
        Ok(format!("daemon started and registered in HKCU\\...\\Run as {NAME}. It will start at login."))
    }

    pub fn uninstall() -> Result<String, String> {
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, winreg::enums::KEY_SET_VALUE) {
            let _ = key.delete_value(NAME);
        }
        let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "ct.exe"]).output();
        Ok("daemon stopped and removed from startup.".into())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn install(_hotkey: Option<&str>, _no_paste: bool) -> Result<String, String> { Err("autostart is not implemented on this platform yet".into()) }
    pub fn uninstall() -> Result<String, String> { Err("autostart is not implemented on this platform yet".into()) }
}

pub use imp::{install, uninstall};
