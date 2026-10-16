use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

pub fn query_process_name(process_id: u32) -> Option<String> {
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
    Some(
        Path::new(&path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or(path),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_can_be_queried() {
        let name =
            query_process_name(std::process::id()).expect("current process should be queryable");
        assert!(name.ends_with(".exe"));
    }

    #[test]
    fn system_process_has_no_application_name() {
        assert!(query_process_name(0).is_none());
    }
}
