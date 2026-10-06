//! Session-local single instance and graceful shutdown, independent of the exe name.
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, WAIT_OBJECT_0, WAIT_ABANDONED};
use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, ReleaseMutex, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE};

const NAME: &str = "Local\\me.jkn.clipto.daemon.v1";
fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
fn error() -> String { std::io::Error::last_os_error().to_string() }

pub struct Instance { mutex: HANDLE, event: HANDLE }

impl Instance {
    pub fn start() -> Result<Self, String> { Self::start_named(NAME) }

    fn start_named(name: &str) -> Result<Self, String> {
        unsafe {
            let mutex = CreateMutexW(std::ptr::null(), 1, wide(name).as_ptr());
            if mutex.is_null() { return Err(error()); }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(mutex);
                return Err("ClipTo is already running in this session".into());
            }
            let event = CreateEventW(std::ptr::null(), 1, 0, wide(&format!("{name}.stop")).as_ptr());
            if event.is_null() {
                let err = error();
                ReleaseMutex(mutex); CloseHandle(mutex);
                return Err(err);
            }
            Ok(Self { mutex, event })
        }
    }

    pub fn stop_requested(&self) -> bool { unsafe { WaitForSingleObject(self.event, 0) == WAIT_OBJECT_0 } }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.event); ReleaseMutex(self.mutex); CloseHandle(self.mutex); }
    }
}

pub fn stop() -> Result<(), String> { stop_named(NAME) }

fn stop_named(name: &str) -> Result<(), String> {
    unsafe {
        let mutex = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide(name).as_ptr());
        if mutex.is_null() {
            return if GetLastError() == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(error()) };
        }
        let event = OpenEventW(EVENT_MODIFY_STATE, 0, wide(&format!("{name}.stop")).as_ptr());
        if event.is_null() { let err = error(); CloseHandle(mutex); return Err(err); }
        let sent = SetEvent(event);
        CloseHandle(event);
        if sent == 0 { let err = error(); CloseHandle(mutex); return Err(err); }
        let result = WaitForSingleObject(mutex, 5000);
        let stopped = result == WAIT_OBJECT_0 || result == WAIT_ABANDONED;
        if stopped { ReleaseMutex(mutex); }
        CloseHandle(mutex);
        if stopped { Ok(()) } else { Err("ClipTo did not stop within 5 seconds".into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_targets_only_the_resident_instance() {
        let name = format!("{NAME}.test.{}", std::process::id());
        stop_named(&name).unwrap(); // idempotent, even with no daemon
        let instance = Instance::start_named(&name).unwrap();
        assert!(Instance::start_named(&name).is_err());
        let stop_name = name.clone();
        let stopper = std::thread::spawn(move || stop_named(&stop_name));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !instance.stop_requested() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        drop(instance);
        stopper.join().unwrap().unwrap();
        let restarted = Instance::start_named(&name).unwrap();
        assert!(!restarted.stop_requested());
    }
}
