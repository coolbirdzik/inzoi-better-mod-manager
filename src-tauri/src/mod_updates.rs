use super::*;
use std::collections::HashSet;

#[derive(Clone)]
struct Origin {
    provider: String,
    mod_id: u64,
    file_id: u64,
    version: String,
    file: serde_json::Value,
}
#[derive(Clone, Serialize)]
pub(crate) struct Update {
    id: String,
    provider: String,
    mod_id: u64,
    current_version: String,
    latest_version: String,
    status: String,
    url: String,
    token: Option<String>,
    file_id: u64,
}
#[derive(Clone)]
pub(crate) struct Plan {
    id: String,
    project: String,
    path: String,
    origin: Origin,
    info: NexusLink,
    created: std::time::Instant,
}
fn number(data: &serde_json::Value, key: &str) -> Option<u64> {
    value(data, key)
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        .filter(|id| *id > 0)
}
fn origin(m: &Mod) -> Option<Origin> {
    for provider in ["nexus", "curseforge"] {
        if let Some(record) = m
            .metadata
            .get(&format!("{provider}.json"))
            .and_then(|v| v.get("Download"))
        {
            let version = record["file"]["version"]
                .as_str()
                .or_else(|| record["file"]["displayName"].as_str())
                .unwrap_or(&m.version)
                .to_string();
            return Some(Origin {
                provider: provider.into(),
                mod_id: number(record, "mod_id")?,
                file_id: number(record, "file_id").unwrap_or(0),
                version,
                file: record["file"].clone(),
            });
        }
    }
    let mut urls = vec![m.source.as_str()];
    if let Some(manifest) = m.metadata.get("mod_manifest.json") {
        if let Some(url) = value(manifest, "Url").and_then(|v| v.as_str()) {
            urls.push(url);
        }
    }
    for raw in urls {
        let Ok(url) = url::Url::parse(raw) else {
            continue;
        };
        if !url.username().is_empty() || url.password().is_some() {
            continue;
        }
        if !matches!(url.host_str(), Some("www.nexusmods.com" | "nexusmods.com")) {
            continue;
        }
        let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
        if let ["inzoi", "mods", id] = parts.as_slice() {
            let mod_id = id.parse::<u64>().ok().filter(|id| *id > 0)?;
            let file_id = url
                .query_pairs()
                .find(|(key, _)| key == "file_id")
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0);
            return Some(Origin {
                provider: "nexus".into(),
                mod_id,
                file_id,
                version: m.version.clone(),
                file: serde_json::Value::Null,
            });
        }
    }
    if let Some(details) = m
        .metadata
        .get("Game library")
        .or_else(|| m.metadata.get("curseforge.json"))
    {
        if number(details, "gameId") == Some(88849) {
            let file = details.get("Installed file").cloned().unwrap_or_default();
            return Some(Origin {
                provider: "curseforge".into(),
                mod_id: number(details, "id")?,
                file_id: number(&file, "id").unwrap_or(0),
                version: m.version.clone(),
                file,
            });
        }
    }
    if let Some(ini) = m.metadata.get("meta.ini").and_then(|v| v.as_object()) {
        let find = |names: &[&str]| {
            ini.iter()
                .find(|(key, _)| {
                    names.iter().any(|name| {
                        key.rsplit('.')
                            .next()
                            .is_some_and(|key| key.eq_ignore_ascii_case(name))
                    })
                })
                .map(|(_, v)| v)
        };
        if find(&["gameName", "gameDomain"])
            .and_then(|v| v.as_str())
            .is_some_and(|game| !game.eq_ignore_ascii_case("inzoi"))
        {
            return None;
        }
        let id = |names: &[&str]| {
            find(names)
                .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
                .filter(|id| *id > 0)
        };
        return Some(Origin {
            provider: "nexus".into(),
            mod_id: id(&["modID", "nexusModID"])?,
            file_id: id(&["fileID", "nexusFileID"]).unwrap_or(0),
            version: m.version.clone(),
            file: serde_json::Value::Null,
        });
    }
    None
}

fn current_mod(id: &str, state: &State, local: Option<&Path>) -> Result<Mod> {
    let mut m = library::catalogue(&*state.store.lock().map_err(err)?)
        .0
        .into_iter()
        .find(|m| m.id == id && !m.archive)
        .ok_or("Mod no longer exists")?;
    if let Some(local) = local {
        if let Ok(details) =
            metadata::from_game_library(&local.join("inZOI/User_data/88849/library.json"))
        {
            if let Some(details) = details.get(&m.project.to_lowercase()) {
                m.metadata.insert("Game library".into(), details.clone());
            }
        }
    }
    Ok(m)
}
fn nexus_candidate(data: &serde_json::Value, current: u64) -> Result<Option<serde_json::Value>> {
    let files = data["files"]
        .as_array()
        .ok_or("Nexus returned no file list")?;
    let updates = data["file_updates"].as_array().cloned().unwrap_or_default();
    let mut id = current;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(id) {
            return Err("modUpdateManual".into());
        }
        let next: HashSet<_> = updates
            .iter()
            .filter(|edge| number(edge, "old_file_id") == Some(id))
            .filter_map(|edge| number(edge, "new_file_id"))
            .collect();
        if next.len() > 1 {
            return Err("modUpdateManual".into());
        }
        let Some(next) = next.into_iter().next() else {
            break;
        };
        id = next;
    }
    if id == current {
        return Ok(None);
    }
    let file = files
        .iter()
        .find(|file| number(file, "file_id") == Some(id))
        .ok_or("modUpdateManual")?;
    if !matches!(number(file, "category_id"), Some(1 | 3)) {
        return Err("modUpdateManual".into());
    }
    Ok(Some(file.clone()))
}
fn cf_candidate(project: &serde_json::Value, name: &str) -> Option<serde_json::Value> {
    let files = project["latestFiles"].as_array()?;
    files
        .iter()
        .filter(|f| {
            f["isAvailable"] != false
                && f["releaseType"] == 1
                && f["isServerPack"] != true
                && f["exposeAsAlternative"] != true
        })
        .filter(|f| {
            f["modules"].as_array().is_some_and(|modules| {
                modules.iter().any(|module| {
                    value(module, "name")
                        .and_then(|v| v.as_str())
                        .is_some_and(|module| module.eq_ignore_ascii_case(name))
                })
            })
        })
        .max_by(|a, b| a["fileDate"].as_str().cmp(&b["fileDate"].as_str()))
        .cloned()
}
fn available(
    result: &mut Update,
    file: &serde_json::Value,
    id_key: &str,
    version_key: &str,
    filename_key: &str,
) {
    result.file_id = number(file, id_key).unwrap_or(0);
    result.latest_version = value(file, version_key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let zip = value(file, filename_key)
        .and_then(|v| v.as_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".zip"));
    result.status = if zip { "available" } else { "manual" }.into();
}
async fn check(m: &Mod, key: &str) -> Result<(Update, Option<Origin>)> {
    let Some(source) = origin(m) else {
        return Ok((
            Update {
                id: m.id.clone(),
                provider: String::new(),
                mod_id: 0,
                current_version: m.version.clone(),
                latest_version: String::new(),
                status: "unsupported".into(),
                url: m.source.clone(),
                token: None,
                file_id: 0,
            },
            None,
        ));
    };
    if key.trim().is_empty() {
        return Err(if source.provider == "nexus" {
            "nexusApiKeyRequired"
        } else {
            "curseforgeApiKeyRequired"
        }
        .into());
    }
    let mut result = Update {
        id: m.id.clone(),
        provider: source.provider.clone(),
        mod_id: source.mod_id,
        current_version: source.version.clone(),
        latest_version: String::new(),
        status: "upToDate".into(),
        url: String::new(),
        token: None,
        file_id: source.file_id,
    };
    if source.provider == "nexus" {
        result.url = format!(
            "https://www.nexusmods.com/inzoi/mods/{}?tab=files",
            source.mod_id
        );
        let data = nexus::api_json(
            &nexus::api_client()?,
            url::Url::parse(&format!(
                "https://api.nexusmods.com/v1/games/inzoi/mods/{}/files.json",
                source.mod_id
            ))
            .map_err(err)?,
            key,
        )
        .await?;
        if source.file_id == 0 {
            result.status = "manual".into();
        } else {
            match nexus_candidate(&data, source.file_id) {
                Ok(Some(file)) => available(&mut result, &file, "file_id", "version", "file_name"),
                Ok(None) => {
                    let other_files = data["files"].as_array().is_some_and(|files| {
                        files.iter().any(|f| {
                            number(f, "file_id") != Some(source.file_id)
                                && number(f, "category_id") == Some(1)
                                && f["version"].as_str().is_some_and(|v| v != source.version)
                        })
                    });
                    if other_files {
                        result.status = "manual".into();
                    }
                }
                Err(error) if error == "modUpdateManual" => result.status = "manual".into(),
                Err(error) => return Err(error),
            }
        }
        if result.file_id != source.file_id {
            result.url = format!(
                "https://www.nexusmods.com/inzoi/mods/{}?tab=files&file_id={}",
                source.mod_id, result.file_id
            );
        }
    } else {
        let client = curseforge::api_client()?;
        let project = curseforge::api_data(
            &client,
            curseforge::endpoint(&format!("mods/{}", source.mod_id))?,
            key,
        )
        .await?;
        if number(&project, "gameId") != Some(88849)
            || number(&project, "id") != Some(source.mod_id)
        {
            return Err("CurseForge returned a different game or project".into());
        }
        let slug = project["slug"]
            .as_str()
            .ok_or("CurseForge returned no project slug")?;
        result.url = format!("https://www.curseforge.com/inzoi/mods/{slug}/files");
        let candidate = cf_candidate(&project, &m.project);
        if source.file_id == 0 {
            result.status = "manual".into();
        } else if let Some(file) = candidate {
            if number(&file, "id") != Some(source.file_id) {
                let current = if source.file["fileDate"].as_str().is_some() {
                    source.file.clone()
                } else {
                    curseforge::api_data(
                        &client,
                        curseforge::endpoint(&format!(
                            "mods/{}/files/{}",
                            source.mod_id, source.file_id
                        ))?,
                        key,
                    )
                    .await?
                };
                match (current["fileDate"].as_str(), file["fileDate"].as_str()) {
                    (Some(old), Some(new)) if new > old => {
                        available(&mut result, &file, "id", "displayName", "fileName");
                        result.url = format!(
                            "https://www.curseforge.com/inzoi/mods/{slug}/files/{}",
                            result.file_id
                        );
                    }
                    (Some(_), Some(_)) => {}
                    _ => result.status = "manual".into(),
                }
            }
        } else {
            result.status = "manual".into();
        }
        if project["allowModDistribution"] == false && result.status == "available" {
            result.status = "manual".into();
        }
    }
    Ok((result, Some(source)))
}

#[tauri::command]
pub(crate) async fn check_mod_update(
    id: String,
    api_key: String,
    log_id: Option<String>,
    run_id: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<Update> {
    let started = std::time::Instant::now();
    let local = app.path().local_data_dir().ok();
    let m = current_mod(&id, &state, local.as_deref());
    let provider = m
        .as_ref()
        .ok()
        .and_then(origin)
        .map(|source| source.provider)
        .unwrap_or_default();
    let result = match &m {
        Ok(m) => perform_check_mod_update(m.clone(), api_key.clone(), &state).await,
        Err(error) => Err(error.clone()),
    };
    let (status, detail) = match &result {
        Ok(update) => (
            update.status.as_str(),
            format!(
                "{} -> {} | file #{}",
                update.current_version, update.latest_version, update.file_id
            ),
        ),
        Err(error) => ("failed", error.clone()),
    };
    let entry = update_check_log::Entry::new(
        log_id,
        run_id,
        &id,
        m.as_ref().ok(),
        &provider,
        status,
        &detail,
        started.elapsed().as_millis() as u64,
        &api_key,
    );
    let _ = update_check_log::append(&state.file, entry);
    result
}

async fn perform_check_mod_update(m: Mod, api_key: String, state: &State) -> Result<Update> {
    let id = m.id.clone();
    let (mut result, source) = check(&m, &api_key).await?;
    let mut plans = state.mod_updates.lock().map_err(err)?;
    plans.retain(|_, p| p.created.elapsed().as_secs() < 86400);
    if result.status == "available" {
        let source = source.ok_or("Missing update source")?;
        let raw = if source.provider == "nexus" {
            format!(
                "nxm://inzoi/mods/{}/files/{}",
                source.mod_id, result.file_id
            )
        } else {
            format!(
                "curseforge://install?gameId=88849&addonId={}&fileId={}",
                source.mod_id, result.file_id
            )
        };
        let info = nexus::parse_request(&raw)?.info;
        let token = uuid::Uuid::new_v4().to_string();
        plans.insert(
            token.clone(),
            Plan {
                id,
                project: m.project,
                path: m.path,
                origin: source,
                info,
                created: std::time::Instant::now(),
            },
        );
        result.token = Some(token);
    }
    Ok(result)
}
fn plan(token: &str, state: &State) -> Result<Plan> {
    let plan = state
        .mod_updates
        .lock()
        .map_err(err)?
        .get(token)
        .cloned()
        .ok_or("Check this mod for updates again")?;
    if plan.created.elapsed().as_secs() >= 86400 {
        return Err("Check this mod for updates again".into());
    }
    Ok(plan)
}
#[tauri::command]
pub(crate) fn prepare_mod_update(token: String, state: tauri::State<State>) -> Result<NexusLink> {
    let plan = plan(&token, &state)?;
    let current = current_mod(&plan.id, &state, None)?;
    if current.superseded_by.is_some() || !library::same_path(&current.path, &plan.path) {
        return Err("The original mod changed; check for updates again".into());
    }
    let raw = if plan.info.provider == "nexus" {
        format!(
            "nxm://inzoi/mods/{}/files/{}",
            plan.info.mod_id, plan.info.file_id
        )
    } else {
        format!(
            "curseforge://install?gameId=88849&addonId={}&fileId={}",
            plan.info.mod_id, plan.info.file_id
        )
    };
    let request = nexus::parse_request(&raw)?;
    let info = request.info.clone();
    state
        .requests
        .lock()
        .map_err(err)?
        .insert(info.request_id.clone(), request);
    Ok(info)
}

// Publish the new deployment only after a complete copy is ready. One store
// write commits both versions, so rollback can restore the old deployment.
fn switch_version(old_id: &str, new_id: &str, state: &State, restoring: bool) -> Result<String> {
    let mut store = state.store.lock().map_err(err)?;
    let old_pos = store
        .mods
        .iter()
        .position(|m| m.id == old_id)
        .ok_or("Original mod no longer exists")?;
    let new_pos = store
        .mods
        .iter()
        .position(|m| m.id == new_id)
        .ok_or("New mod no longer exists")?;
    let old = store.mods[old_pos].clone();
    let new = store.mods[new_pos].clone();
    if old_pos == new_pos
        || !old.project.eq_ignore_ascii_case(&new.project)
        || new.enabled
        || old.superseded_by.is_some()
    {
        return Err("Cannot replace this mod version in its current state".into());
    }
    let previous = store.mods.clone();
    let mut deployment = None;
    if old.enabled {
        let target = deployment_target(&store.settings, &old)?;
        if fs::read_to_string(target.join(".inzoimm-owner"))
            .ok()
            .as_deref()
            != Some(&old.id)
        {
            return Err("Ownership marker mismatch; original game files were kept".into());
        }
        if store
            .mods
            .iter()
            .any(|m| m.id != old.id && m.enabled && m.project.eq_ignore_ascii_case(&old.project))
        {
            return Err("Another version is enabled; original files were kept".into());
        }
        let game = Path::new(&store.settings.mods_dir)
            .canonicalize()
            .map_err(err)?;
        let staging = game
            .parent()
            .ok_or("Missing Mods parent")?
            .join(".inzoimm-staging");
        let recovery = game.parent().unwrap().join(".inzoimm-disabled");
        fs::create_dir_all(&staging).map_err(err)?;
        fs::create_dir_all(&recovery).map_err(err)?;
        let staged = staging.join(uuid::Uuid::new_v4().to_string());
        let prepared = copy_tree(Path::new(&new.path), &staged)
            .and_then(|_| enable_manifest(&staged))
            .and_then(|_| fs::write(staged.join(".inzoimm-owner"), &new.id).map_err(err));
        if let Err(error) = prepared {
            let _ = fs::remove_dir_all(&staged);
            return Err(error);
        }
        let backup = recovery.join(uuid::Uuid::new_v4().to_string());
        if let Err(error) = fs::rename(&target, &backup) {
            let _ = fs::remove_dir_all(staged);
            return Err(err(error));
        }
        if let Err(error) = fs::rename(&staged, &target) {
            let rollback = fs::rename(&backup, &target);
            let _ = fs::remove_dir_all(staged);
            return Err(format!(
                "Could not activate update: {error}. Restore result: {rollback:?}"
            ));
        }
        store.mods[new_pos].deployed_path = Some(target.to_string_lossy().into_owned());
        deployment = Some((target, backup, Some(staged)));
    }
    if !old.enabled
        && old
            .deployed_path
            .as_ref()
            .is_some_and(|path| Path::new(path).exists())
    {
        let target = deployment_target(&store.settings, &old)?;
        if fs::read_to_string(target.join(".inzoimm-owner"))
            .ok()
            .as_deref()
            == Some(&old.id)
        {
            let game = Path::new(&store.settings.mods_dir)
                .canonicalize()
                .map_err(err)?;
            let recovery = game
                .parent()
                .ok_or("Missing Mods parent")?
                .join(".inzoimm-disabled");
            fs::create_dir_all(&recovery).map_err(err)?;
            let backup = recovery.join(uuid::Uuid::new_v4().to_string());
            fs::rename(&target, &backup).map_err(err)?;
            deployment = Some((target, backup, None));
        }
    }
    if let Ok(original) = read_mod(Path::new(&old.path), old.id.clone(), true) {
        if old.name != original.name {
            store.mods[new_pos].name = old.name.clone();
        }
        if old.description != original.description {
            store.mods[new_pos].description = old.description.clone();
        }
    }
    store.mods[old_pos].enabled = false;
    store.mods[old_pos].superseded_by = Some(new.id.clone());
    store.mods[new_pos].enabled = old.enabled;
    store.mods[new_pos].superseded_by = None;
    if !restoring {
        store.mods[new_pos].previous_version = Some(old.id.clone());
    }
    if let Err(error) = write_store(state, &store) {
        store.mods = previous;
        if let Some((target, backup, staged)) = deployment {
            let move_new = if let Some(staged) = &staged {
                fs::rename(&target, staged)
            } else {
                Ok(())
            };
            let restore_old = fs::rename(&backup, &target);
            if move_new.is_err() || restore_old.is_err() {
                return Err(format!("Could not save update: {error}. Recovery files: {}. Rollback: {move_new:?}, {restore_old:?}",backup.display()));
            }
            if let Some(staged) = staged {
                let _ = fs::remove_dir_all(staged);
            }
        }
        return Err(error);
    }
    Ok(new.id)
}
fn install(token: &str, path: &str, state: &State, local: Option<&Path>) -> Result<String> {
    let plan = plan(token, state)?;
    let current = current_mod(&plan.id, state, local)?;
    if current.project != plan.project
        || !library::same_path(&current.path, &plan.path)
        || current.superseded_by.is_some()
    {
        return Err("The original mod changed; check for updates again".into());
    }
    let source = origin(&current).ok_or("Original mod source changed")?;
    if source.provider != plan.origin.provider
        || source.mod_id != plan.origin.mod_id
        || source.file_id != plan.origin.file_id
    {
        return Err("The original mod source changed; check again".into());
    }
    let archive = Path::new(path).canonicalize().map_err(err)?;
    let (folder, root) = metadata::download_destination(&archive)?;
    let cached =
        download_storage::cached(&folder, &plan.info)?.ok_or("Missing update download record")?;
    if !library::same_path(&archive.to_string_lossy(), &cached.0.to_string_lossy()) {
        return Err("Archive does not match the checked update".into());
    }
    let temp = state
        .file
        .parent()
        .ok_or("Missing app data")?
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&temp).map_err(err)?;
    let result = (|| {
        extract_zip(&archive, &temp)?;
        metadata::attach_download(&archive, &temp)?;
        let matches: Vec<_> = scan_with_options(&temp, true)
            .0
            .into_iter()
            .filter(|m| m.project.eq_ignore_ascii_case(&plan.project))
            .collect();
        if matches.len() != 1 {
            return Err("The update does not contain exactly one matching ProjectName. Original mod was kept.".into());
        }
        import_folder_to_library(
            matches[0].path.clone(),
            false,
            state,
            Some((folder.clone(), root)),
        )?;
        let new_id = {
            let s = state.store.lock().map_err(err)?;
            s.mods
                .iter()
                .find(|m| {
                    library::same_path(&m.path, &folder.join(&matches[0].project).to_string_lossy())
                })
                .map(|m| m.id.clone())
                .ok_or("New mod was not registered")?
        };
        let old_id = manage_catalogue_mod(plan.id.clone(), state)?;
        if let Some(saved_plan) = state.mod_updates.lock().map_err(err)?.get_mut(token) {
            saved_plan.id = old_id.clone();
            saved_plan.path = state
                .store
                .lock()
                .map_err(err)?
                .mods
                .iter()
                .find(|m| m.id == old_id)
                .ok_or("Original mod no longer exists")?
                .path
                .clone();
        }
        // Game-installed CurseForge mods had their source metadata outside the
        // mod directory. Preserve it in the library backup for later rollback.
        if plan.origin.provider == "curseforge" && current.metadata.contains_key("Game library") {
            let old_path = {
                state
                    .store
                    .lock()
                    .map_err(err)?
                    .mods
                    .iter()
                    .find(|m| m.id == old_id)
                    .unwrap()
                    .path
                    .clone()
            };
            let record = serde_json::json!({"Download":{"provider":"curseforge","mod_id":plan.origin.mod_id,"file_id":plan.origin.file_id,"file":curseforge::public_metadata(plan.origin.file.clone())}});
            let metadata_path = Path::new(&old_path).join("curseforge.json");
            if !metadata_path.exists() {
                fs::write(
                    metadata_path,
                    serde_json::to_vec_pretty(&record).map_err(err)?,
                )
                .map_err(err)?;
            }
        }
        switch_version(&old_id, &new_id, state, false)
    })();
    let _ = fs::remove_dir_all(temp);
    result
}
#[tauri::command(async)]
pub(crate) fn install_mod_update(
    token: String,
    path: String,
    app: tauri::AppHandle,
    state: tauri::State<State>,
) -> Result<String> {
    let result = install(
        &token,
        &path,
        &state,
        app.path().local_data_dir().ok().as_deref(),
    );
    if result.is_ok() {
        state.mod_updates.lock().map_err(err)?.remove(&token);
    }
    result
}
#[tauri::command(async)]
pub(crate) fn restore_previous_mod(id: String, state: tauri::State<State>) -> Result<String> {
    let previous = state
        .store
        .lock()
        .map_err(err)?
        .mods
        .iter()
        .find(|m| m.id == id)
        .and_then(|m| m.previous_version.clone())
        .ok_or("No previous version is available")?;
    switch_version(&id, &previous, &state, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    #[test]
    #[ignore = "Read-only check of current library and credential availability; opt in with INZOIMM_DIAGNOSE_CHECKS=1"]
    fn diagnose_current_checks_read_only() {
        assert_eq!(std::env::var("INZOIMM_DIAGNOSE_CHECKS").as_deref(), Ok("1"));
        let store_path = PathBuf::from(std::env::var("APPDATA").unwrap())
            .join("com.coolbirdzik.inzoi-better-mod-manager/library.json");
        let store: Store = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        let (mut mods, _) = library::catalogue(&store);
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if let Ok(details) = metadata::from_game_library(
                &Path::new(&local).join("inZOI/User_data/88849/library.json"),
            ) {
                for m in &mut mods {
                    if let Some(details) = details.get(&m.project.to_lowercase()) {
                        m.metadata.insert("Game library".into(), details.clone());
                    }
                }
            }
        }
        for provider in ["nexus", "curseforge"] {
            let matching: Vec<_> = mods
                .iter()
                .filter(|m| {
                    !m.archive
                        && m.superseded_by.is_none()
                        && origin(m).is_some_and(|source| source.provider == provider)
                })
                .collect();
            let key = if provider == "nexus" {
                credentials::load_nexus_api_key()
            } else {
                credentials::load_curseforge_api_key()
            };
            let (present, detail) = match key {
                Ok(Some(key)) => {
                    let count = if std::env::var("INZOIMM_DIAGNOSE_ALL").as_deref() == Ok("1") {
                        matching.len()
                    } else {
                        1
                    };
                    let mut statuses = std::collections::BTreeMap::<String, usize>::new();
                    for m in matching.iter().take(count) {
                        let status = match tauri::async_runtime::block_on(check(m, &key)) {
                            Ok((update, _)) => update.status,
                            Err(error) => format!("failed:{}", update_check_log::code(&error)),
                        };
                        *statuses.entry(status).or_default() += 1;
                    }
                    (true, serde_json::to_string(&statuses).unwrap())
                }
                Ok(None) => (false, "missing_key".into()),
                Err(error) => (false, error),
            };
            println!(
                "{}",
                serde_json::json!({"provider":provider,"recognized_mods":matching.len(),"saved_key_present":present,"detail":detail})
            );
        }
    }

    fn versions(enabled: bool) -> (PathBuf, State, String, String, PathBuf) {
        let (root, state, game) = crate::lifecycle_tests::fixture();
        fs::write(
            game.join("nexus.json"),
            r#"{"Download":{"mod_id":42,"file_id":1,"file":{"version":"1.0"}}}"#,
        )
        .unwrap();
        adopt_mod_core(game.to_string_lossy().into_owned(), &state).unwrap();
        let old = state.store.lock().unwrap().mods[0].id.clone();
        if !enabled {
            set_managed_enabled(old.clone(), false, &state).unwrap();
        }
        let new_path = root.join("library/new-version");
        fs::create_dir_all(&new_path).unwrap();
        fs::write(
            new_path.join("mod_manifest.json"),
            r#"{"ProjectName":"Hair_ABC","FriendlyName":"Hair","Version":"2.0"}"#,
        )
        .unwrap();
        fs::write(new_path.join("payload.pak"), b"new mod payload").unwrap();
        let mut new = read_mod(&new_path, "new-version".into(), true).unwrap();
        new.library_root = Some(root.join("library").to_string_lossy().into_owned());
        state.store.lock().unwrap().mods.push(new);
        (root, state, old, "new-version".into(), game)
    }
    #[test]
    fn nexus_follows_only_explicit_replacement_chain_not_optional_or_highest_ids() {
        let mut data = serde_json::json!({"files":[{"file_id":3,"category_id":1,"version":"3"},{"file_id":99,"category_id":3,"version":"optional"}],"file_updates":[{"old_file_id":1,"new_file_id":2},{"old_file_id":2,"new_file_id":3}]});
        assert_eq!(nexus_candidate(&data, 1).unwrap().unwrap()["file_id"], 3);
        assert!(nexus_candidate(&data, 3).unwrap().is_none());
        data["file_updates"] = serde_json::json!([{"old_file_id":1,"new_file_id":2},{"old_file_id":1,"new_file_id":99}]);
        assert!(nexus_candidate(&data, 1).is_err());
        data["file_updates"] = serde_json::json!([{"old_file_id":1,"new_file_id":2},{"old_file_id":2,"new_file_id":1}]);
        assert!(nexus_candidate(&data, 1).is_err());
    }
    #[test]
    fn curseforge_matches_the_project_and_ignores_beta_server_and_alternative_files() {
        let data = serde_json::json!({"latestFiles":[
            {"id":2,"releaseType":1,"fileDate":"2026-11-01T00:00:00Z","modules":[{"name":"Hair_ABC"}]},
            {"id":3,"releaseType":1,"fileDate":"2026-11-02T00:00:00Z","modules":[{"name":"OtherVariant"}]},
            {"id":4,"releaseType":2,"fileDate":"2026-11-03T00:00:00Z","modules":[{"name":"Hair_ABC"}]},
            {"id":5,"releaseType":1,"exposeAsAlternative":true,"fileDate":"2026-11-04T00:00:00Z","modules":[{"name":"Hair_ABC"}]}
        ]});
        assert_eq!(cf_candidate(&data, "hair_abc").unwrap()["id"], 2);
        assert!(cf_candidate(&data, "Missing").is_none());
    }
    #[test]
    fn installed_curseforge_identity_uses_actual_installed_file_not_latest_file() {
        let (root, state, game) = crate::lifecycle_tests::fixture();
        let mut m = read_mod(&game, "id".into(), false).unwrap();
        m.metadata.insert("Game library".into(),serde_json::json!({"iD":123,"gameId":88849,"mainFileId":99,"Installed file":{"iD":1,"fileDate":"2026-01-01T00:00:00Z"}}));
        let cf = origin(&m).unwrap();
        assert_eq!(cf.file_id, 1);
        assert_eq!(cf.mod_id, 123);
        m.source = "https://www.nexusmods.com/inzoi/mods/42?tab=files&file_id=2".into();
        let nexus = origin(&m).unwrap();
        assert_eq!(nexus.provider, "nexus");
        assert_eq!(nexus.file_id, 2);
        assert!(state.store.lock().unwrap().mods.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn enabled_update_publishes_new_bytes_preserves_old_library_and_restores_previous() {
        let (root, state, old, new, game) = versions(true);
        state.store.lock().unwrap().mods[0].name = "My custom name".into();
        switch_version(&old, &new, &state, false).unwrap();
        assert_eq!(
            fs::read(game.join("payload.pak")).unwrap(),
            b"new mod payload"
        );
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert!(!saved.mods[0].enabled);
        assert_eq!(saved.mods[0].superseded_by.as_deref(), Some(new.as_str()));
        assert!(saved.mods[1].enabled);
        assert_eq!(
            saved.mods[1].previous_version.as_deref(),
            Some(old.as_str())
        );
        assert_eq!(saved.mods[1].name, "My custom name");
        assert_eq!(
            fs::read(Path::new(&saved.mods[0].path).join("payload.pak")).unwrap(),
            [1, 2, 3, 4]
        );
        switch_version(&new, &old, &state, true).unwrap();
        assert_eq!(fs::read(game.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        assert!(state.store.lock().unwrap().mods[0].superseded_by.is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn disabled_update_stays_disabled_and_does_not_touch_another_installed_variant() {
        let (root, state, old, new, game) = versions(false);
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("unmanaged.pak"), b"external").unwrap();
        switch_version(&old, &new, &state, false).unwrap();
        assert!(state.store.lock().unwrap().mods.iter().all(|m| !m.enabled));
        assert_eq!(fs::read(game.join("unmanaged.pak")).unwrap(), b"external");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_save_restores_game_files_enabled_state_and_version_history() {
        let (root, state, old, new, game) = versions(true);
        fs::remove_file(&state.file).unwrap();
        fs::create_dir(&state.file).unwrap();
        assert!(switch_version(&old, &new, &state, false).is_err());
        assert_eq!(fs::read(game.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        let s = state.store.lock().unwrap();
        assert!(s.mods[0].enabled);
        assert!(!s.mods[1].enabled);
        assert!(s.mods[0].superseded_by.is_none() && s.mods[1].previous_version.is_none());
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ownership_mismatch_never_replaces_game_files() {
        let (root, state, old, new, game) = versions(true);
        fs::write(game.join(".inzoimm-owner"), "another-owner").unwrap();
        assert!(switch_version(&old, &new, &state, false).is_err());
        assert_eq!(fs::read(game.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        fs::remove_dir_all(root).unwrap();
    }
    fn archive(state: &State, root: &Path, project: &str) -> (String, PathBuf) {
        let old = state.store.lock().unwrap().mods[0].clone();
        let info = nexus::parse_request("nxm://inzoi/mods/42/files/2")
            .unwrap()
            .info;
        let parent = download_storage::mod_folder(&root.join("downloads"), &info, "Hair").unwrap();
        let folder = download_storage::file_folder(&parent, 2).unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(
            "Hair/mod_manifest.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            serde_json::json!({"ProjectName":project,"FriendlyName":"Hair","Version":"2"})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        zip.start_file("Hair/payload.pak", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"updated zip payload").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let pending = download_storage::PendingArchive::new(&folder, "update.zip").unwrap();
        let mut file = pending.create().unwrap();
        file.write_all(&bytes).unwrap();
        drop(file);
        let path = pending
            .finish(
                metadata::download_record(&info, &serde_json::json!({"version":"2"})),
                bytes.len() as u64,
                format!("{:x}", Sha256::digest(&bytes)),
            )
            .unwrap();
        let token = "test-plan".to_string();
        state.mod_updates.lock().unwrap().insert(
            token.clone(),
            Plan {
                id: old.id.clone(),
                project: old.project.clone(),
                path: old.path.clone(),
                origin: origin(&old).unwrap(),
                info,
                created: std::time::Instant::now(),
            },
        );
        (token, path)
    }
    #[test]
    fn checked_zip_install_uses_verified_archive_and_selects_exact_project() {
        let (root, state, _, _, game) = versions(true);
        let (token, path) = archive(&state, &root, "Hair_ABC");
        let id = install(&token, &path.to_string_lossy(), &state, None).unwrap();
        assert_eq!(
            fs::read(game.join("payload.pak")).unwrap(),
            b"updated zip payload"
        );
        let s = state.store.lock().unwrap();
        assert!(s.mods.iter().find(|m| m.id == id).unwrap().enabled);
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn changed_archive_or_wrong_project_keeps_original_mod_and_files() {
        for changed in [false, true] {
            let (root, state, _, _, game) = versions(true);
            let (token, path) = archive(
                &state,
                &root,
                if changed { "Hair_ABC" } else { "WrongVariant" },
            );
            if changed {
                fs::write(&path, b"tampered").unwrap();
            }
            assert!(install(&token, &path.to_string_lossy(), &state, None).is_err());
            assert_eq!(fs::read(game.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
            assert_eq!(state.store.lock().unwrap().mods.len(), 2);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
