pub const FRAME_EVENT_NAME: &str = "Local\\DemoFrameReady";

#[cfg(target_os = "windows")]
pub struct NamedEvent {
    handle: windows_sys::Win32::Foundation::HANDLE,
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

#[cfg(target_os = "windows")]
impl Drop for NamedEvent {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for NamedEvent {}

#[cfg(target_os = "windows")]
unsafe impl Sync for NamedEvent {}
