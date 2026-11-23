use super::*;
use std::io::ErrorKind;

const DIRECTORY: &str = ".inzoimm-staging";

// Canonicalize existing ancestors so a new folder has the same path validation
// as an existing one, including when its parent is a junction.
fn resolve(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let parent = path.parent().zip(path.file_name()).ok_or(error)?;
            Ok(resolve(parent.0)?.join(parent.1))
        }
        result => result,
    }
}

fn path_prefix(path: &Path, prefix: &Path) -> bool {
    if cfg!(windows) {
        let path = dunce::simplified(path).to_string_lossy().to_lowercase();
        let prefix = dunce::simplified(prefix).to_string_lossy().to_lowercase();
        Path::new(&path).starts_with(prefix)
    } else {
        path.starts_with(prefix)
    }
}

fn game_parent(game: &Path) -> Result<&Path> {
    game.parent().ok_or_else(|| "Missing Mods parent".into())
}

fn root(settings: &Settings, game: &Path) -> Result<PathBuf> {
    let parent = if settings.staging_dir.is_empty() {
        game_parent(game)?.to_path_buf()
    } else {
        let path = Path::new(&settings.staging_dir);
        if !path.is_absolute() {
            return Err("Use an absolute path for the staging location".into());
        }
        resolve(path).map_err(err)?
    };
    let root = parent.join(DIRECTORY);
    if path_prefix(&root, game) || path_prefix(game, &root) {
        return Err("Staging must be outside the game Mods folder".into());
    }
    Ok(root)
}

fn validate(settings: &Settings, root: &Path) -> Result<()> {
    if !settings.paks().is_empty() {
        let paks = resolve(Path::new(settings.paks())).map_err(err)?;
        if path_prefix(root, &paks) || path_prefix(&paks, root) {
            return Err("Staging must be outside the legacy PAK folder".into());
        }
    }
    let library = resolve(Path::new(&settings.library_dir)).map_err(err)?;
    if path_prefix(&library, root) {
        return Err("The managed mods folder cannot be inside staging".into());
    }
    let downloads = nexus::downloads_root(settings);
    for source in settings
        .source_dirs
        .iter()
        .map(|source| Path::new(&source.path))
        .chain(std::iter::once(downloads.as_path()))
    {
        let path = resolve(source).unwrap_or_else(|_| source.to_path_buf());
        if path_prefix(&path, root) {
            return Err("A library source or download folder cannot be inside staging".into());
        }
    }
    Ok(())
}

fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
                if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    return Err(format!(
                        "Staging cannot be a junction or link: {}",
                        path.display()
                    ));
                }
            }
            if !metadata.is_dir() {
                return Err(format!("Staging must be a directory: {}", path.display()));
            }
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(err(error)),
    }
}

pub(crate) struct Relocation {
    old: PathBuf,
    new: PathBuf,
    retired: Option<PathBuf>,
}

impl Relocation {
    pub(crate) fn rollback(self) -> Result<()> {
        if let Some(retired) = self.retired {
            fs::rename(&retired, &self.old).map_err(|error| {
                format!(
                    "Could not restore staging from {}: {error}",
                    retired.display()
                )
            })?;
            fs::remove_dir_all(&self.new).map_err(err)
        } else {
            fs::rename(&self.new, &self.old).map_err(|error| {
                format!(
                    "Could not restore staging from {}: {error}",
                    self.new.display()
                )
            })
        }
    }

    pub(crate) fn finish(self) {
        if let Some(retired) = self.retired {
            if let Err(error) = fs::remove_dir_all(&retired) {
                eprintln!(
                    "Could not remove previous staging at {}: {error}",
                    retired.display()
                );
            }
        }
    }
}

pub(crate) fn relocate(old: &Settings, new: &Settings) -> Result<Option<Relocation>> {
    if new.staging_dir == old.staging_dir && new.mods_dir == old.mods_dir {
        return Ok(None);
    }
    let new_game = resolve(Path::new(&new.mods_dir)).map_err(err)?;
    let destination = root(new, &new_game)?;
    validate(new, &destination)?;
    let source = root(old, &resolve(Path::new(&old.mods_dir)).map_err(err)?)?;
    if path_prefix(&source, &destination) && path_prefix(&destination, &source) {
        return Ok(None);
    }
    if path_prefix(&source, &destination) || path_prefix(&destination, &source) {
        return Err("The old and new staging folders must be separate".into());
    }
    if exists(&destination)? {
        fs::remove_dir(&destination).map_err(|error| {
            format!(
                "Could not use staging at {}: {error}",
                destination.display()
            )
        })?;
    }
    if !exists(&source)? {
        fs::create_dir_all(&destination).map_err(err)?;
        return Ok(None);
    }
    fs::create_dir_all(destination.parent().ok_or("Missing staging parent")?).map_err(err)?;
    let retired = match fs::rename(&source, &destination) {
        Ok(()) => None,
        Err(error) if error.kind() == ErrorKind::CrossesDevices => {
            Some(move_across_volumes(&source, &destination)?)
        }
        Err(error) => return Err(format!("Could not move staging: {error}")),
    };
    Ok(Some(Relocation {
        old: source,
        new: destination,
        retired,
    }))
}

fn move_across_volumes(source: &Path, destination: &Path) -> Result<PathBuf> {
    let name = format!(".inzoimm-staging-{}", uuid::Uuid::new_v4());
    let temporary = destination.with_file_name(&name);
    let retired = source.with_file_name(&name);
    let result = copy_tree(source, &temporary).and_then(|_| {
        fs::rename(source, &retired).map_err(err)?;
        if let Err(error) = fs::rename(&temporary, destination) {
            fs::rename(&retired, source).map_err(|restore| {
                format!(
                    "Could not move staging: {error}. Original files are in {}: {restore}",
                    retired.display()
                )
            })?;
            return Err(err(error));
        }
        Ok(retired)
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

// Return a complete deployment on the game volume. The final rename into Mods
// is atomic even when the configured staging location is on another drive.
pub(crate) fn prepare_deployment(settings: &Settings, m: &Mod) -> Result<PathBuf> {
    let game = Path::new(&settings.mods_dir).canonicalize().map_err(err)?;
    let staging = root(settings, &game)?;
    let source = Path::new(&m.path).canonicalize().map_err(err)?;
    if path_prefix(&staging, &source) || path_prefix(&source, &staging) {
        return Err("The mod source and staging folders must be separate".into());
    }
    exists(&staging)?;
    fs::create_dir_all(&staging).map_err(err)?;
    let name = uuid::Uuid::new_v4().to_string();
    let staged = staging.join(&name);
    let result = (|| {
        copy_tree(Path::new(&m.path), &staged)?;
        enable_manifest(&staged)?;
        fs::write(staged.join(".inzoimm-owner"), &m.id).map_err(err)?;
        if settings.staging_dir.is_empty() {
            return Ok(staged.clone());
        }
        let publication = game_parent(&game)?.join(".inzoimm-publish");
        exists(&publication)?;
        fs::create_dir_all(&publication).map_err(err)?;
        let ready = publication.join(name);
        match fs::rename(&staged, &ready) {
            Ok(()) => Ok(ready),
            Err(error) if error.kind() == ErrorKind::CrossesDevices => {
                if let Err(error) = copy_tree(&staged, &ready) {
                    let _ = fs::remove_dir_all(&ready);
                    return Err(error);
                }
                let _ = fs::remove_dir_all(&staged);
                Ok(ready)
            }
            Err(error) => Err(err(error)),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staged);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_staging_moves_existing_data_and_keeps_deployment_working() {
        let (fixture, state, installed) = lifecycle_tests::fixture();
        let saved = state.store.lock().unwrap().settings.clone();
        let old_root = root(&saved, Path::new(&saved.mods_dir)).unwrap();
        fs::create_dir_all(old_root.join("pending")).unwrap();
        fs::write(old_root.join("pending/payload.pak"), b"existing").unwrap();
        let parent = match std::env::var_os("INZOIMM_TEST_STAGING_PARENT") {
            Some(path) => tempfile::tempdir_in(path).unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let unrelated = parent.path().join("notes.txt");
        fs::write(&unrelated, b"keep").unwrap();

        let mut changed = saved.clone();
        changed.staging_dir = parent.path().to_string_lossy().into_owned();
        save_settings_core(changed.clone(), &state).unwrap();
        let moved_root = parent.path().join(DIRECTORY);
        assert_eq!(
            fs::read(moved_root.join("pending/payload.pak")).unwrap(),
            b"existing"
        );
        assert!(!old_root.exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
        let reloaded: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert_eq!(reloaded.settings.staging_dir, changed.staging_dir);

        adopt_mod_core(installed.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        set_mod_enabled_core(id.clone(), false, &state).unwrap();
        set_mod_enabled_core(id, true, &state).unwrap();
        assert_eq!(
            fs::read(installed.join("payload.pak")).unwrap(),
            [1, 2, 3, 4]
        );
        assert_eq!(fs::read_dir(&moved_root).unwrap().count(), 1);
        assert_eq!(
            fs::read_dir(fixture.join("game/.inzoimm-publish"))
                .unwrap()
                .count(),
            0
        );

        save_settings_core(saved, &state).unwrap();
        assert!(!moved_root.exists());
        assert_eq!(
            fs::read(old_root.join("pending/payload.pak")).unwrap(),
            b"existing"
        );
        assert!(installed.is_dir());
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn occupied_destination_and_failed_save_keep_existing_staging() {
        let (fixture, state, _) = lifecycle_tests::fixture();
        let mut settings = state.store.lock().unwrap().settings.clone();
        let original = root(&settings, Path::new(&settings.mods_dir)).unwrap();
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("old.pak"), b"original").unwrap();
        settings.staging_dir = fixture.join("new").to_string_lossy().into_owned();
        let destination = root(&settings, Path::new(&settings.mods_dir)).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("other.pak"), b"occupied").unwrap();

        assert!(save_settings_core(settings.clone(), &state).is_err());
        assert_eq!(
            fs::read(destination.join("other.pak")).unwrap(),
            b"occupied"
        );
        assert_eq!(fs::read(original.join("old.pak")).unwrap(), b"original");
        fs::remove_file(destination.join("other.pak")).unwrap();

        fs::create_dir(&state.file).unwrap();
        assert!(save_settings_core(settings, &state).is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read(original.join("old.pak")).unwrap(), b"original");
        assert!(state.store.lock().unwrap().settings.staging_dir.is_empty());
        fs::remove_dir_all(fixture).unwrap();
    }
}
