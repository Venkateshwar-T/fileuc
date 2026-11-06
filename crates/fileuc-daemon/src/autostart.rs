use std::env;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, REG_VALUE_TYPE,
};

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "FileUC";

#[derive(Debug)]
pub enum AutoStartError {
    CurrentExeUnavailable,
    WindowsError(windows::core::Error),
}

pub fn set_auto_start(enabled: bool) -> Result<(), AutoStartError> {
    let subkey = HSTRING::from(RUN_SUBKEY);
    let app_name = HSTRING::from(APP_NAME);

    unsafe {
        let mut key = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_WRITE,
            &mut key,
        )
        .ok()
        .map_err(AutoStartError::WindowsError)?;

        let result = if enabled {
            let current_exe = env::current_exe().map_err(|_| AutoStartError::CurrentExeUnavailable)?;
            let exe_path_str = format!("\"{}\"", current_exe.to_string_lossy());
            let wide_exe = HSTRING::from(exe_path_str.as_str());

            RegSetValueExW(
                key,
                PCWSTR::from_raw(app_name.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide_exe.as_ptr() as *const u8,
                    (wide_exe.len() + 1) * 2,
                )),
            )
        } else {
            RegDeleteValueW(key, PCWSTR::from_raw(app_name.as_ptr()))
        };

        let _ = RegCloseKey(key);
        result.ok().map_err(AutoStartError::WindowsError)?;
    }

    Ok(())
}

pub fn is_auto_start_enabled() -> bool {
    let subkey = HSTRING::from(RUN_SUBKEY);
    let app_name = HSTRING::from(APP_NAME);

    unsafe {
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return false;
        }

        let mut val_type = REG_VALUE_TYPE::default();
        let mut data_len = 0u32;
        let res = RegQueryValueExW(
            key,
            PCWSTR::from_raw(app_name.as_ptr()),
            None,
            Some(&mut val_type),
            None,
            Some(&mut data_len),
        );

        let _ = RegCloseKey(key);
        res.is_ok()
    }
}
