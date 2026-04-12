pub const FRAME_EVENT_NAME: &str = "Local\\DemoFrameReady";

#[cfg(not(target_os = "windows"))]
fn to_posix_name(name: &str) -> String {
    let mut out = String::from("/");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out
}

#[cfg(target_os = "windows")]
pub struct NamedEvent {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(not(target_os = "windows"))]
pub struct NamedEvent {
    sem: *mut libc::sem_t,
}

#[cfg(target_os = "windows")]
impl NamedEvent {
    pub fn create(name: &str) -> Result<Self, String> {
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            windows_sys::Win32::System::Threading::CreateEventW(
                std::ptr::null(),
                0,
                0,
                wide_name.as_ptr(),
            )
        };

        if handle.is_null() {
            return Err(format!(
                "CreateEventW failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(Self { handle })
    }

    pub fn open(name: &str) -> Result<Self, String> {
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            windows_sys::Win32::System::Threading::OpenEventW(
                0x100000 | 0x0002,
                0,
                wide_name.as_ptr(),
            )
        };

        if handle.is_null() {
            return Err(format!(
                "OpenEventW failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(Self { handle })
    }

    pub fn set(&self) {
        unsafe {
            windows_sys::Win32::System::Threading::SetEvent(self.handle);
        }
    }

    pub fn wait(&self, timeout_ms: u32) -> bool {
        unsafe {
            windows_sys::Win32::System::Threading::WaitForSingleObject(self.handle, timeout_ms) == 0
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl NamedEvent {
    pub fn create(name: &str) -> Result<Self, String> {
        let name = to_posix_name(name);
        let c_name = std::ffi::CString::new(name).map_err(|e| e.to_string())?;
        let sem = unsafe { libc::sem_open(c_name.as_ptr(), libc::O_CREAT, 0o666, 0) };
        if sem == libc::SEM_FAILED {
            return Err(format!("sem_open(create) failed: {}", std::io::Error::last_os_error()));
        }
        Ok(Self { sem })
    }

    pub fn open(name: &str) -> Result<Self, String> {
        let name = to_posix_name(name);
        let c_name = std::ffi::CString::new(name).map_err(|e| e.to_string())?;
        let sem = unsafe { libc::sem_open(c_name.as_ptr(), 0) };
        if sem == libc::SEM_FAILED {
            return Err(format!("sem_open(open) failed: {}", std::io::Error::last_os_error()));
        }
        Ok(Self { sem })
    }

    pub fn set(&self) {
        unsafe {
            libc::sem_post(self.sem);
        }
    }

    pub fn wait(&self, timeout_ms: u32) -> bool {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) };
        let add_sec = (timeout_ms / 1000) as i64;
        let add_nsec = ((timeout_ms % 1000) as i64) * 1_000_000;
        ts.tv_sec += add_sec;
        ts.tv_nsec += add_nsec;
        if ts.tv_nsec >= 1_000_000_000 {
            ts.tv_sec += 1;
            ts.tv_nsec -= 1_000_000_000;
        }

        loop {
            let rc = unsafe { libc::sem_timedwait(self.sem, &ts) };
            if rc == 0 {
                return true;
            }
            let err = std::io::Error::last_os_error();
            if let Some(code) = err.raw_os_error() {
                if code == libc::ETIMEDOUT {
                    return false;
                }
                if code == libc::EINTR {
                    continue;
                }
            }
            return false;
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for NamedEvent {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl Drop for NamedEvent {
    fn drop(&mut self) {
        unsafe {
            libc::sem_close(self.sem);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for NamedEvent {}

#[cfg(target_os = "windows")]
unsafe impl Sync for NamedEvent {}

#[cfg(not(target_os = "windows"))]
unsafe impl Send for NamedEvent {}

#[cfg(not(target_os = "windows"))]
unsafe impl Sync for NamedEvent {}
