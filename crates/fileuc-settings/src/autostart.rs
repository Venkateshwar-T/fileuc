use std::env;

#[cfg(windows)]
use windows::core::{HSTRING, PCWSTR};
#[cfg(windows)]
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
};

#[cfg(windows)]
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const APP_NAME: &str = "FileUC";

pub fn set_auto_start(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        let subkey = HSTRING::from(RUN_SUBKEY);
        let app_name = HSTRING::from(APP_NAME);
        let mut key = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_WRITE,
            &mut key,
        )
        .ok()
        .map_err(|e| format!("Registry error: {}", e))?;

        let err = if enabled {
            let current_exe = env::current_exe().map_err(|e| format!("Current exe error: {}", e))?;
            let exe_path_str = format!("\"{}\" --autostart", current_exe.to_string_lossy());
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
            let del_err = RegDeleteValueW(key, PCWSTR::from_raw(app_name.as_ptr()));
            use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
            if del_err == ERROR_FILE_NOT_FOUND {
                ERROR_SUCCESS
            } else {
                del_err
            }
        };

        let _ = RegCloseKey(key);
        err.ok().map_err(|e| format!("Registry set error: {}", e))?;
    }
    Ok(())
}
