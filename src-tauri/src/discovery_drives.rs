use super::*;

/// Query the operating system instead of guessing which drive letters exist.
/// Fixed, removable and RAM disks are local scan targets. Mapped network
/// shares can still be selected explicitly in the folder picker.
#[cfg(windows)]
pub(crate) fn roots() -> Result<Vec<PathBuf>> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let mut roots = vec![];
    for index in 0..26 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let path = format!("{}:\\", (b'A' + index) as char);
        let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        if matches!(unsafe { GetDriveTypeW(wide.as_ptr()) }, 2 | 3 | 6) {
            roots.push(PathBuf::from(path));
        }
    }
    Ok(roots)
}

#[cfg(not(windows))]
pub(crate) fn roots() -> Result<Vec<PathBuf>> {
    // /Volumes mounts are traversed explicitly on macOS because nested links
    // and mount-point aliases are intentionally not followed by the scanner.
    let mut roots = vec![PathBuf::from("/")];
    if let Ok(volumes) = fs::read_dir("/Volumes") {
        roots.extend(
            volumes
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.path()),
        );
    }
    Ok(roots)
}

pub(crate) fn skip_entry(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    if matches!(
        name.as_ref(),
        ".trash" | "$RECYCLE.BIN" | "System Volume Information" | ".git" | "node_modules"
    ) || name.starts_with(".inzoimm-")
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points must not lead the traversal to
        // another volume or back into a directory it already visited.
        if entry.file_type().is_symlink()
            || entry.file_type().is_dir()
                && entry
                    .path()
                    .symlink_metadata()
                    .is_ok_and(|m| m.file_attributes() & 0x400 != 0)
        {
            return true;
        }
    }
    #[cfg(not(windows))]
    if entry.file_type().is_symlink()
        || entry.file_type().is_dir()
            && matches!(entry.path().to_str(), Some("/proc" | "/sys" | "/dev"))
    {
        return true;
    }
    false
}
