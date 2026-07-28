//! Atomically publishes a complete same-directory temporary file without
//! replacing a destination that appeared after the caller's preflight check.

use std::io;
use std::path::Path;

#[cfg(windows)]
pub fn publish_new_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVE_FILE_FLAGS};

    let temporary_wide = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    // No MOVEFILE_REPLACE_EXISTING flag: Windows must fail if another process
    // created the destination after our preflight check.
    unsafe {
        MoveFileExW(
            PCWSTR(temporary_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVE_FILE_FLAGS(0),
        )
    }
    .map_err(|_| io::Error::last_os_error())
}

#[cfg(not(windows))]
pub fn publish_new_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    // Both paths are same-directory siblings in every caller. Linking fails
    // atomically when the destination already exists and never exposes partial
    // bytes. Removing the temporary name leaves the completed inode published.
    std::fs::hard_link(temporary, destination)?;
    if let Err(error) = std::fs::remove_file(temporary) {
        log::warn!(
            "Published {} but could not remove temporary file {}: {}",
            destination.display(),
            temporary.display(),
            error
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_complete_file_without_replacing_an_existing_destination() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-no-clobber-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let first_temporary = directory.join("first.partial");
        let destination = directory.join("result.txt");
        std::fs::write(&first_temporary, b"first").unwrap();
        publish_new_file(&first_temporary, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"first");

        let second_temporary = directory.join("second.partial");
        std::fs::write(&second_temporary, b"second").unwrap();
        assert!(publish_new_file(&second_temporary, &destination).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"first");

        std::fs::remove_dir_all(directory).unwrap();
    }
}
