use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE},
    System::{
        Registry::{
            HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
        },
        Threading::CreateMutexW,
    },
};

const MUTEX_NAME: &str = r"Local\LumaNext.Engine.Singleton.v1";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "LumaNext";

pub enum InstanceGuard {
    Acquired(NamedMutex),
    AlreadyRunning,
}

pub struct NamedMutex(HANDLE);

impl Drop for NamedMutex {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub fn acquire_single_instance() -> Result<InstanceGuard, String> {
    let name = wide(MUTEX_NAME);
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(format!("创建单实例锁失败（Windows error {}）", unsafe {
            GetLastError()
        }));
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) };
        Ok(InstanceGuard::AlreadyRunning)
    } else {
        Ok(InstanceGuard::Acquired(NamedMutex(handle)))
    }
}

pub fn autostart_command(executable: &Path) -> Result<String, String> {
    if !executable.is_absolute() {
        return Err("开机启动需要绝对可执行文件路径".into());
    }
    let command = format!("\"{}\"", executable.display());
    if command.encode_utf16().count() >= 260 {
        return Err("开机启动命令超过 Windows Run 键 260 字符限制".into());
    }
    Ok(command)
}

pub fn is_development_executable(executable: &Path) -> bool {
    let components: Vec<_> = executable
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    components
        .windows(2)
        .any(|pair| pair[0] == "target" && matches!(pair[1].as_str(), "debug" | "release"))
}

pub fn install_autostart(executable: &Path) -> Result<String, String> {
    let command = autostart_command(executable)?;
    let key_path = wide(RUN_KEY);
    let value_name = wide(RUN_VALUE);
    let value = wide(&command);
    let mut key = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(format!(
            "打开当前用户 Run 注册表键失败（Windows error {status}）"
        ));
    }
    let bytes = unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2) };
    let status = unsafe {
        RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_SZ,
            bytes.as_ptr(),
            bytes.len() as u32,
        )
    };
    unsafe { RegCloseKey(key) };
    if status != 0 {
        return Err(format!("写入开机启动项失败（Windows error {status}）"));
    }
    Ok(command)
}

pub fn uninstall_autostart() -> Result<bool, String> {
    let key_path = wide(RUN_KEY);
    let value_name = wide(RUN_VALUE);
    let mut key = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if status != 0 {
        return Err(format!(
            "打开当前用户 Run 注册表键失败（Windows error {status}）"
        ));
    }
    let status = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
    unsafe { RegCloseKey(key) };
    if status == ERROR_FILE_NOT_FOUND {
        Ok(false)
    } else if status != 0 {
        Err(format!("删除开机启动项失败（Windows error {status}）"))
    } else {
        Ok(true)
    }
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_command_quotes_absolute_executable() {
        let command =
            autostart_command(Path::new(r"C:\Program Files\Luma\luma-engine.exe")).unwrap();
        assert_eq!(command, r#""C:\Program Files\Luma\luma-engine.exe""#);
        assert!(!command.contains("cargo"));
        assert!(!command.contains("--open-ui"));
    }

    #[test]
    fn autostart_rejects_relative_paths() {
        assert!(autostart_command(Path::new("target/debug/luma-engine.exe")).is_err());
    }

    #[test]
    fn target_builds_are_identified_as_development_paths() {
        assert!(is_development_executable(Path::new(
            r"C:\src\luma-next\target\release\luma-engine.exe"
        )));
        assert!(!is_development_executable(Path::new(
            r"C:\Users\me\AppData\Local\LumaNext\bin\luma-engine.exe"
        )));
    }
}
