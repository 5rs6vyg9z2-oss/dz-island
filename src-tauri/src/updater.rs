use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{sync::{Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter};

use crate::update_policy::{version, valid_package, REPO, MAX_PACKAGE};
static STATE: Mutex<Option<(Instant, UpdateCheckResult, Option<Asset>)>> = Mutex::new(None);
static INSTALLING: AtomicBool = AtomicBool::new(false);
#[derive(Serialize, Clone, Default)]
pub struct UpdateCheckResult { available: bool, version: Option<String>, date: Option<String>, body: Option<String> }
#[derive(Deserialize)]
struct Release { tag_name: String, draft: bool, prerelease: bool, published_at: Option<String>, body: Option<String>, assets: Vec<Asset> }
#[derive(Deserialize, Clone)]
struct Asset { name: String, browser_download_url: String, digest: Option<String>, size: usize }

fn valid_asset(asset: &Asset) -> bool {
    valid_package(&asset.name, &asset.browser_download_url, asset.digest.as_deref(), asset.size)
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(concat!("DZ-Island/", env!("CARGO_PKG_VERSION")))
        .https_only(true).connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt.url().host_str().unwrap_or_default();
            if attempt.previous().len() < 5 && attempt.url().scheme() == "https"
                && matches!(host, "github.com" | "api.github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com") {
                attempt.follow()
            } else { attempt.error("Untrusted update redirect") }
        })).build().map_err(|e| e.to_string())
}
async fn bounded_body(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if response.content_length().is_some_and(|n| n > limit as u64) { return Err("Update response is too large".into()); }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > limit { return Err("Update response is too large".into()); }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
#[tauri::command]
pub fn get_update_state() -> UpdateCheckResult {
    STATE.lock().ok().and_then(|s| s.as_ref().map(|(_, r, _)| r.clone())).unwrap_or_default()
}
#[tauri::command]
pub async fn check_for_updates(app: AppHandle, force: bool) -> Result<UpdateCheckResult, String> {
    if !force {
        if let Some(result) = STATE.lock().ok().and_then(|s| s.as_ref().filter(|(at, _, _)| at.elapsed() < Duration::from_secs(3600)).map(|(_, r, _)| r.clone())) { return Ok(result); }
    }
    let response = client()?.get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json").send().await.map_err(|e| e.to_string())?;
    let (result, asset) = if response.status() == reqwest::StatusCode::NOT_FOUND {
        (UpdateCheckResult::default(), None)
    } else {
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        let release: Release = serde_json::from_slice(&bounded_body(response, 2 * 1024 * 1024).await?).map_err(|e| e.to_string())?;
        let candidate = version(&release.tag_name);
        let newer = !release.draft && !release.prerelease && candidate.zip(version(env!("CARGO_PKG_VERSION"))).is_some_and(|(a,b)| a > b);
        let asset = release.assets.into_iter().find(valid_asset).filter(|_| newer);
        (UpdateCheckResult { available: asset.is_some(), version: candidate.map(|v| format!("{}.{}.{}", v[0],v[1],v[2])), date: release.published_at, body: release.body }, asset)
    };
    *STATE.lock().map_err(|_| "Update state unavailable")? = Some((Instant::now(), result.clone(), asset));
    let _ = app.emit("update-available", &result);
    Ok(result)
}
fn pending_question() -> bool { crate::agent_api::has_pending_question() }
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    if pending_question() { return Err("Responda à pergunta da ilha antes de atualizar.".into()); }
    if INSTALLING.swap(true, Ordering::SeqCst) { return Err("Update already running".into()); }
    let result = install_inner(&app).await;
    if result.is_err() {
        INSTALLING.store(false, Ordering::SeqCst);
        let _ = app.emit("auto-update-status", serde_json::json!({"status":"error"}));
    }
    result
}
async fn install_inner(app: &AppHandle) -> Result<(), String> {
    let (release, asset) = STATE.lock().ok().and_then(|s| s.as_ref().and_then(|(_,r,a)| a.clone().map(|a|(r.clone(),a)))).ok_or("Nenhuma atualização válida disponível.")?;
    if !valid_asset(&asset) { return Err("Invalid release package".into()); }
    let _ = app.emit("auto-update-status", serde_json::json!({"status":"downloading"}));
    let response = client()?.get(&asset.browser_download_url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let bytes = bounded_body(response, MAX_PACKAGE).await?;
    if bytes.len() != asset.size || format!("sha256:{:x}", Sha256::digest(&bytes)) != asset.digest.unwrap_or_default().to_ascii_lowercase() {
        return Err("Falha na verificação SHA-256 do pacote do GitHub.".into());
    }
    if pending_question() { return Err("Atualização adiada: há uma pergunta aguardando resposta.".into()); }
    let exe = std::env::current_exe().map_err(|e|e.to_string())?;
    let root = exe.parent().ok_or("Missing install directory")?;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
    let stage = crate::utils::portable_data_dir().map_err(|e|e.to_string())?.join("updates").join(format!("{nonce}"));
    std::fs::create_dir_all(&stage).map_err(|e|e.to_string())?;
    std::fs::write(stage.join("package.zip"), bytes).map_err(|e|e.to_string())?;
    std::fs::write(stage.join("apply.ps1"), include_str!("../../scripts/apply-portable-update.ps1")).map_err(|e|e.to_string())?;
    std::fs::write(stage.join("plan.json"), serde_json::json!({"root":root,"parent":std::process::id(),"version":release.version}).to_string()).map_err(|e|e.to_string())?;
    use std::os::windows::process::CommandExt;
    let powershell = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).ok_or("Missing SystemRoot")?.join("System32/WindowsPowerShell/v1.0/powershell.exe");
    std::process::Command::new(powershell).args(["-NoProfile","-NonInteractive","-File"])
        .arg(stage.join("apply.ps1")).arg("-Plan").arg(stage.join("plan.json"))
        .creation_flags(0x08000000).spawn().map_err(|e|e.to_string())?;
    let _ = app.emit("auto-update-status", serde_json::json!({"status":"installing"}));
    crate::commands::restore_taskbar_and_exit(app);
    Ok(())
}
pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) { return; }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            match check_for_updates(app.clone(), false).await {
                Ok(result) if result.available && !pending_question() => {
                    if let Err(error) = install_update(app.clone()).await { crate::diagnostics::log(&format!("updater: {error}")); }
                }
                Err(error) => crate::diagnostics::log(&format!("updater check: {error}")),
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
