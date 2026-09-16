//! Simulate the paste shortcut in the frontmost app, so a popup choice inserts the result directly.
//! macOS posts ⌘V through CoreGraphics (needs the Accessibility permission), Windows sends Ctrl+V.

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
    }

    /// True when we may post key events. With `prompt`, macOS shows its "allow Accessibility" dialog once.
    pub fn trusted(prompt: bool) -> bool {
        let opts = CFDictionary::from_CFType_pairs(&[(CFString::new("AXTrustedCheckOptionPrompt"), CFBoolean::from(prompt))]);
        unsafe { AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef()) }
    }

    pub fn paste() -> Result<(), String> {
        if !trusted(true) {
            return Err("Accessibility permission needed to paste: System Settings > Privacy & Security > Accessibility, add ct".into());
        }
        let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| "event source")?;
        const KEY_V: u16 = 9;
        let down = CGEvent::new_keyboard_event(src.clone(), KEY_V, true).map_err(|_| "key event")?;
        let up = CGEvent::new_keyboard_event(src, KEY_V, false).map_err(|_| "key event")?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagCommand);
        down.post(CGEventTapLocation::HID);
        up.post(CGEventTapLocation::HID);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_CONTROL};

    pub fn trusted(_prompt: bool) -> bool { true }

    fn key(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        let mut input: INPUT = unsafe { std::mem::zeroed() };
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = vk;
        input.Anonymous.ki.dwFlags = flags;
        input
    }

    pub fn paste() -> Result<(), String> {
        const VK_V: u16 = 0x56;
        let inputs = [key(VK_CONTROL, 0), key(VK_V, 0), key(VK_V, KEYEVENTF_KEYUP), key(VK_CONTROL, KEYEVENTF_KEYUP)];
        let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 { Ok(()) } else { Err("SendInput failed".into()) }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn trusted(_prompt: bool) -> bool { false }
    pub fn paste() -> Result<(), String> { Err("auto-paste is not implemented on this platform".into()) }
}

pub use imp::{paste, trusted};
