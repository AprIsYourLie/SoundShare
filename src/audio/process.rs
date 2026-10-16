use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub name: String,
    pub path: String,
}

pub fn query_process(process_id: u32) -> Option<ProcessInfo> {
    if process_id == 0 {
        return None;
    }

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };

    let mut path_buffer = vec![0_u16; 32_768];
    let mut path_length = path_buffer.len() as u32;
    let query_result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path_buffer.as_mut_ptr()),
            &mut path_length,
        )
    };
    let _ = unsafe { CloseHandle(handle) };

    query_result.ok()?;
    path_buffer.truncate(path_length as usize);
    let path = OsString::from_wide(&path_buffer)
        .to_string_lossy()
        .into_owned();
    let name = Path::new(&path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());

    Some(ProcessInfo { name, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_can_be_queried() {
        let info = query_process(std::process::id()).expect("current process should be queryable");
        assert!(!info.name.is_empty());
        assert!(!info.path.is_empty());
    }
}
