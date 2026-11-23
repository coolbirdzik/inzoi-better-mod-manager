use crate::Result;
use std::path::Path;

pub(crate) fn send(path: &Path) -> Result<()> {
    recycle(dunce::simplified(path)).map_err(|error| {
        format!(
            "Could not move {} to the system trash: {error}",
            path.display()
        )
    })?;
    match path.try_exists() {
        Ok(false) => Ok(()),
        Ok(true) => Err(format!(
            "{} is still in its original location",
            path.display()
        )),
        Err(error) => Err(format!(
            "Could not verify removal of {}: {error}",
            path.display()
        )),
    }
}

#[cfg(not(windows))]
fn recycle(path: &Path) -> junkyard::Result<()> {
    junkyard::discard(path).map(|_| ())
}

#[cfg(windows)]
fn recycle(path: &Path) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::{
            System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            UI::Shell::{
                BHID_Transfer, FOLDERID_RecycleBinFolder, IShellItem, ITransferSource,
                SHCreateItemFromParsingName, SHGetKnownFolderItem, KF_FLAG_DEFAULT, TSF_NORMAL,
            },
        },
    };

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            // The matching CoInitializeEx succeeded on this thread.
            unsafe { CoUninitialize() };
        }
    }

    let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    // The wide buffer and COM references live through all Shell calls. The
    // apartment is released after the interfaces, including on errors.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _apartment = ComApartment;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let source: ITransferSource = item.GetParent()?.BindToHandler(None, &BHID_Transfer)?;
        let bin: IShellItem =
            SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)?;
        source.RecycleItem(&item, &bin, TSF_NORMAL.0 as u32)?;
    }
    Ok(())
}
