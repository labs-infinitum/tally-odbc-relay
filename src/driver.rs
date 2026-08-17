use std::path::{Path, PathBuf};

use thiserror::Error;

#[cfg_attr(not(windows), allow(dead_code))]
const DRIVER_NAME: &str = "Tally ODBC Driver64";
#[cfg_attr(not(windows), allow(dead_code))]
const INST_KEY: &str = r"Software\ODBC\ODBCINST.INI\Tally ODBC Driver64";

#[cfg_attr(not(windows), allow(dead_code))]
const SEARCH_DAT: &[&str] = &[
    r"C:\Program Files\TallyPrime\TallyWin.Dat",
    r"C:\Program Files\TallyPrimeEL\TallyWin.Dat",
    r"C:\Program Files (x86)\TallyPrime\TallyWin.Dat",
    r"C:\Program Files (x86)\TallyPrimeEL\TallyWin.Dat",
];

#[derive(Debug, Error)]
pub enum DriverFixError {
    #[error("{0}")]
    Message(String),
}

impl DriverFixError {
    #[cfg_attr(not(windows), allow(dead_code))]
    fn from_msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

/// Copy `TallyWin.Dat` to `TallyWin.dll` and point the ODBC DSN at the `.dll`.
///
/// Wine only `LoadLibrary`s drivers whose path ends in `.dll`. Tally registers
/// `TallyWin.Dat` and sometimes rewrites the registry back to that name.
pub fn ensure_dll_driver(dsn: &str) -> Result<Option<String>, DriverFixError> {
    ensure_dll_driver_impl(dsn)
}

#[cfg(windows)]
fn ensure_dll_driver_impl(dsn: &str) -> Result<Option<String>, DriverFixError> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let inst_driver = read_reg_value(&hklm, INST_KEY, "Driver");
    let dsn_key = format!(r"Software\ODBC\ODBC.INI\{dsn}");
    let dsn_driver = read_reg_value(&hklm, &dsn_key, "driver")
        .or_else(|| read_reg_value(&hklm, &dsn_key, "Driver"));

    if inst_driver
        .as_deref()
        .is_some_and(|value| is_dll_path(value) && Path::new(value).is_file())
    {
        let dll = inst_driver.unwrap();
        let mut notes = Vec::new();
        if dsn_driver.as_deref() != Some(dll.as_str()) {
            write_reg_value(&hklm, &dsn_key, "driver", &dll)?;
            notes.push(format!("pointed DSN {dsn} at {dll}"));
        }
        return Ok(if notes.is_empty() {
            None
        } else {
            Some(notes.join("; "))
        });
    }

    let dat = find_dat_file(inst_driver.as_deref(), dsn_driver.as_deref()).ok_or_else(|| {
        DriverFixError::from_msg(
            "could not find TallyWin.Dat next to the ODBC driver or in Program Files",
        )
    })?;
    let dll = copy_dat_to_dll(&dat)?;

    let dll_str = dll.to_string_lossy().into_owned();
    write_reg_value(&hklm, INST_KEY, "Driver", &dll_str)?;
    write_reg_value(&hklm, &dsn_key, "driver", &dll_str)?;

    let _ = hklm.create_subkey(r"Software\ODBC\ODBCINST.INI\ODBC Drivers");
    if let Ok(drivers) = hklm.open_subkey_with_flags(
        r"Software\ODBC\ODBCINST.INI\ODBC Drivers",
        KEY_READ | KEY_SET_VALUE,
    ) {
        let _ = drivers.set_value(DRIVER_NAME, &"Installed");
    }

    Ok(Some(format!(
        "prepared Wine-compatible ODBC driver at {dll_str}"
    )))
}

#[cfg(not(windows))]
fn ensure_dll_driver_impl(_dsn: &str) -> Result<Option<String>, DriverFixError> {
    Ok(None)
}

#[cfg(windows)]
fn read_reg_value(hklm: &winreg::RegKey, subkey: &str, name: &str) -> Option<String> {
    use winreg::enums::KEY_READ;

    let key = hklm.open_subkey_with_flags(subkey, KEY_READ).ok()?;
    key.get_value::<String, _>(name).ok()
}

#[cfg(windows)]
fn write_reg_value(
    hklm: &winreg::RegKey,
    subkey: &str,
    name: &str,
    value: &str,
) -> Result<(), DriverFixError> {
    use winreg::enums::{KEY_READ, KEY_SET_VALUE};

    let (key, _) = hklm
        .create_subkey_with_flags(subkey, KEY_READ | KEY_SET_VALUE)
        .map_err(|err| DriverFixError::from_msg(format!("registry {subkey}: {err}")))?;
    key.set_value(name, &value)
        .map_err(|err| DriverFixError::from_msg(format!("registry {subkey}\\{name}: {err}")))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn is_dll_path(value: &str) -> bool {
    Path::new(value)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn is_dat_path(value: &str) -> bool {
    Path::new(value)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dat"))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn dll_path_for_dat(dat: &Path) -> PathBuf {
    dat.with_extension("dll")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn copy_dat_to_dll(dat: &Path) -> Result<PathBuf, DriverFixError> {
    let dll = dll_path_for_dat(dat);
    if !dll.is_file() || dat_is_newer(dat, &dll) {
        std::fs::copy(dat, &dll).map_err(|err| {
            DriverFixError::from_msg(format!(
                "failed to copy {} to {}: {err}",
                dat.display(),
                dll.display()
            ))
        })?;
    }
    Ok(dll)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn dat_is_newer(dat: &Path, dll: &Path) -> bool {
    match (dat.metadata(), dll.metadata()) {
        (Ok(dat_meta), Ok(dll_meta)) => dat_meta.len() != dll_meta.len(),
        _ => true,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn find_dat_file(inst_driver: Option<&str>, dsn_driver: Option<&str>) -> Option<PathBuf> {
    for value in [inst_driver, dsn_driver].into_iter().flatten() {
        if let Some(path) = resolve_dat_value(value) {
            return Some(path);
        }
    }
    SEARCH_DAT
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn resolve_dat_value(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_file() && is_dat_path(value) {
        return Some(path);
    }
    if path.is_file() && is_dll_path(value) {
        let dat = path.with_extension("dat");
        if dat.is_file() {
            return Some(dat);
        }
    }
    if !path.is_absolute() && is_dat_path(value) {
        return SEARCH_DAT
            .iter()
            .map(PathBuf::from)
            .find(|candidate| candidate.is_file());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{dll_path_for_dat, is_dat_path, is_dll_path};
    use std::path::Path;

    #[test]
    fn detects_dat_and_dll_suffixes() {
        assert!(is_dat_path(r"C:\Program Files\TallyPrime\TallyWin.Dat"));
        assert!(is_dat_path("TallyWin.dat"));
        assert!(is_dll_path(r"C:\Program Files\TallyPrime\TallyWin.dll"));
        assert!(!is_dll_path(r"C:\Program Files\TallyPrime\TallyWin.Dat"));
    }

    #[test]
    fn maps_dat_to_sibling_dll() {
        let dll = dll_path_for_dat(Path::new("TallyWin.Dat"));
        assert_eq!(dll, Path::new("TallyWin.dll"));
    }

    #[test]
    fn copies_dat_to_sibling_dll_when_missing() {
        let dir = std::env::temp_dir().join(format!("tally-odbc-relay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dat = dir.join("TallyWin.Dat");
        let dll = dir.join("TallyWin.dll");
        std::fs::write(&dat, b"driver-bytes").unwrap();
        let _ = std::fs::remove_file(&dll);

        let written = super::copy_dat_to_dll(&dat).unwrap();
        assert_eq!(written, dll);
        assert_eq!(std::fs::read(&dll).unwrap(), b"driver-bytes");

        std::fs::write(&dat, b"driver-bytes-v2").unwrap();
        super::copy_dat_to_dll(&dat).unwrap();
        assert_eq!(std::fs::read(&dll).unwrap(), b"driver-bytes-v2");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
