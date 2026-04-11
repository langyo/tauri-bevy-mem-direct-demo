use std::sync::atomic::{AtomicU32, AtomicU64};

pub const SHM_NAME: &str = "Local\\DemoFrame";
pub const FRAME_HEADER_SIZE: usize = 64;
pub const MAX_FRAME_DATA: usize = 4 * 3840 * 2160;
pub const BUFFER_SIZE: usize = FRAME_HEADER_SIZE + MAX_FRAME_DATA;
pub const DUAL_CONTROL_SIZE: usize = 64;
pub const SHM_MAX_SIZE: usize = DUAL_CONTROL_SIZE + 2 * BUFFER_SIZE;

#[repr(C)]
pub struct DualControl {
    pub write_index: AtomicU32,
    pub ready_index: AtomicU32,
    pub _pad: [u8; 56],
}

impl DualControl {
    pub fn as_bytes(slice: &[u8]) -> &Self {
        debug_assert!(slice.len() >= DUAL_CONTROL_SIZE);
        unsafe { &*(slice.as_ptr() as *const DualControl) }
    }

    pub fn buffer_slice<'a>(&self, shm: &'a [u8], index: u32) -> &'a [u8] {
        let offset = DUAL_CONTROL_SIZE + index as usize * BUFFER_SIZE;
        &shm[offset..offset + BUFFER_SIZE]
    }

    pub fn buffer_slice_mut<'a>(&self, shm: &'a mut [u8], index: u32) -> &'a mut [u8] {
        let offset = DUAL_CONTROL_SIZE + index as usize * BUFFER_SIZE;
        &mut shm[offset..offset + BUFFER_SIZE]
    }
}

#[repr(C)]
pub struct FrameHeader {
    pub seq: AtomicU64,
    pub width: AtomicU32,
    pub height: AtomicU32,
    pub data_len: AtomicU32,
    pub _pad: [u8; 48],
}

impl FrameHeader {
    pub fn from_buffer(buffer: &[u8]) -> &Self {
        debug_assert!(buffer.len() >= FRAME_HEADER_SIZE);
        unsafe { &*(buffer.as_ptr() as *const FrameHeader) }
    }

    pub fn from_buffer_mut(buffer: &mut [u8]) -> &mut Self {
        debug_assert!(buffer.len() >= FRAME_HEADER_SIZE);
        unsafe { &mut *(buffer.as_mut_ptr() as *mut FrameHeader) }
    }
}

#[cfg(target_os = "windows")]
pub struct ShmHandle {
    handle: windows_sys::Win32::Foundation::HANDLE,
    ptr: *mut u8,
    size: usize,
}

#[cfg(target_os = "windows")]
impl ShmHandle {
    pub fn create(name: &str, size: usize) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::System::Memory::*;

        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE as HANDLE,
                std::ptr::null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                wide_name.as_ptr(),
            )
        };

        if handle.is_null() {
            return Err(format!(
                "CreateFileMappingW failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size) };

        if ptr.Value.is_null() {
            unsafe { CloseHandle(handle) };
            return Err(format!(
                "MapViewOfFile failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let raw_ptr = ptr.Value as *mut u8;
        unsafe { std::ptr::write_bytes(raw_ptr, 0, size) };

        Ok(Self {
            handle,
            ptr: raw_ptr,
            size,
        })
    }

    pub fn open(name: &str, size: usize) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::System::Memory::*;

        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide_name.as_ptr()) };

        if handle.is_null() {
            return Err(format!(
                "OpenFileMappingW failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size) };

        if ptr.Value.is_null() {
            unsafe { CloseHandle(handle) };
            return Err(format!(
                "MapViewOfFile failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(Self {
            handle,
            ptr: ptr.Value as *mut u8,
            size,
        })
    }

    pub fn slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }

    pub fn slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

#[cfg(target_os = "windows")]
impl Drop for ShmHandle {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Memory::*;
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr as *mut _,
            });
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

unsafe impl Send for ShmHandle {}
unsafe impl Sync for ShmHandle {}
