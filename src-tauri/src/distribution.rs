//! Immutable build edition and runtime installation policy shared by both branches.
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEdition {
    Standard,
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationKind {
    Installed,
    Portable,
    WindowsPackage,
    Unknown,
}

// This small build input differs between branches. It is embedded in the binary,
// so a Store MSI/ZIP keeps its edition even without an MSIX package identity.
pub fn edition() -> AppEdition {
    match include_str!("../app-edition").trim() {
        "standard" => AppEdition::Standard,
        "store" => AppEdition::Store,
        value => panic!("Invalid embedded app edition: {value}"),
    }
}

#[cfg(any(test, target_os = "windows"))]
const WIN32_ERROR_SUCCESS: i32 = 0;
#[cfg(any(test, target_os = "windows"))]
const WIN32_ERROR_INSUFFICIENT_BUFFER: i32 = 122;
#[cfg(any(test, target_os = "windows"))]
const WIN32_APPMODEL_ERROR_NO_PACKAGE: i32 = 15_700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
enum WindowsPackageIdentity {
    Unpackaged,
    Packaged,
    UnexpectedError(i32),
}

#[cfg(any(test, target_os = "windows"))]
fn classify_package_identity_code(code: i32) -> WindowsPackageIdentity {
    match code {
        WIN32_APPMODEL_ERROR_NO_PACKAGE => WindowsPackageIdentity::Unpackaged,
        WIN32_ERROR_SUCCESS | WIN32_ERROR_INSUFFICIENT_BUFFER => WindowsPackageIdentity::Packaged,
        other => WindowsPackageIdentity::UnexpectedError(other),
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_package_identity() -> Result<WindowsPackageIdentity, String> {
    use windows::core::{s, w};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    type PackageNameFn = unsafe extern "system" fn(*mut u32, *mut u16) -> i32;

    // Resolve dynamically so startup keeps the existing minimum Windows version
    // and avoids extra Windows-SDK crate features.
    let kernel32 = unsafe { GetModuleHandleW(w!("kernel32.dll")) }
        .map_err(|error| format!("GetModuleHandleW(kernel32.dll) failed: {error}"))?;
    let address = unsafe {
        GetProcAddress(kernel32, s!("GetCurrentPackageFullName"))
            .or_else(|| GetProcAddress(kernel32, s!("GetCurrentPackageFamilyName")))
    }
    .ok_or_else(|| "AppModel package-identity API is unavailable in kernel32.dll".to_string())?;

    // SAFETY: `GetProcAddress` returned the documented AppModel query symbol
    // (`LONG WINAPI (UINT32*, PWSTR)`).
    let query_fn: PackageNameFn = unsafe { std::mem::transmute(address) };
    let mut buffer = [0u16; 256];
    let mut length = buffer.len() as u32;
    // SAFETY: `length` holds `buffer`'s capacity in UTF-16 code units and
    // `buffer` is valid for writes up to `length` elements.
    let code = unsafe { query_fn(&mut length, buffer.as_mut_ptr()) };
    Ok(classify_package_identity_code(code))
}

fn host_identity() -> WindowsPackageIdentity {
    static IDENTITY: OnceLock<WindowsPackageIdentity> = OnceLock::new();
    *IDENTITY.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            match detect_windows_package_identity() {
                Ok(identity) => identity,
                Err(error) => {
                    log::warn!("Windows package identity check failed: {error}");
                    WindowsPackageIdentity::UnexpectedError(-1)
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        { WindowsPackageIdentity::Unpackaged }
    })
}

fn allows_unmanaged_features(edition: AppEdition, identity: WindowsPackageIdentity) -> bool {
    edition == AppEdition::Standard && identity == WindowsPackageIdentity::Unpackaged
}

pub fn allows_portable_storage() -> bool {
    allows_unmanaged_features(edition(), host_identity())
}

pub fn allows_update_checks() -> bool {
    allows_unmanaged_features(edition(), host_identity())
}

fn classify_installation(identity: WindowsPackageIdentity, marker: Result<bool, String>) -> InstallationKind {
    match identity {
        WindowsPackageIdentity::Packaged => InstallationKind::WindowsPackage,
        WindowsPackageIdentity::UnexpectedError(_) => InstallationKind::Unknown,
        WindowsPackageIdentity::Unpackaged => match marker {
            Ok(true) => InstallationKind::Portable,
            Ok(false) => InstallationKind::Installed,
            Err(_) => InstallationKind::Unknown,
        },
    }
}

pub fn installation() -> InstallationKind {
    let marker = std::env::current_exe().map_err(|error| error.to_string())
        .and_then(|exe| exe.parent().ok_or("Missing executable directory".to_string())
            .and_then(|dir| dir.join("portable").try_exists().map_err(|error| error.to_string())));
    classify_installation(host_identity(), marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_package_identity_classification_handles_packaged_unpackaged_and_errors() {
        assert_eq!(
            classify_package_identity_code(WIN32_APPMODEL_ERROR_NO_PACKAGE),
            WindowsPackageIdentity::Unpackaged
        );
        assert_eq!(
            classify_package_identity_code(WIN32_ERROR_SUCCESS),
            WindowsPackageIdentity::Packaged
        );
        assert_eq!(
            classify_package_identity_code(WIN32_ERROR_INSUFFICIENT_BUFFER),
            WindowsPackageIdentity::Packaged
        );
        assert_eq!(
            classify_package_identity_code(87),
            WindowsPackageIdentity::UnexpectedError(87)
        );
    }

    #[test]
    fn edition_policy_blocks_store_msi_zip_msix_and_unknown_hosts() {
        for edition in [AppEdition::Standard, AppEdition::Store] {
            for identity in [WindowsPackageIdentity::Unpackaged, WindowsPackageIdentity::Packaged,
                WindowsPackageIdentity::UnexpectedError(5)] {
                assert_eq!(allows_unmanaged_features(edition, identity),
                    edition == AppEdition::Standard && identity == WindowsPackageIdentity::Unpackaged);
            }
        }
    }

    #[test]
    fn installation_kind_is_independent_of_storage_and_store_edition() {
        assert_eq!(classify_installation(WindowsPackageIdentity::Unpackaged, Ok(true)), InstallationKind::Portable);
        assert_eq!(classify_installation(WindowsPackageIdentity::Unpackaged, Ok(false)), InstallationKind::Installed);
        assert_eq!(classify_installation(WindowsPackageIdentity::Packaged, Ok(true)), InstallationKind::WindowsPackage);
        assert_eq!(classify_installation(WindowsPackageIdentity::Unpackaged, Err("denied".into())), InstallationKind::Unknown);
        assert_eq!(classify_installation(WindowsPackageIdentity::UnexpectedError(5), Ok(false)), InstallationKind::Unknown);
    }

    #[test]
    fn embedded_edition_is_valid() {
        assert!(matches!(edition(), AppEdition::Standard | AppEdition::Store));
    }
}
