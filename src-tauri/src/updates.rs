//! Atualização automática (ver docs/decisoes/0007-atualizacao-automatica.md).
//!
//! 1. `update_check`: baixa o `latest.json` da VPS (URL em `tauri.conf.json`
//!    → `plugins.updater.endpoints`) e compara com a versão instalada;
//! 2. `update_install`: baixa o instalador novo, **confere a assinatura** com a
//!    chave pública embutida no app e instala.
//!
//! A assinatura é o que impede alguém de empurrar uma "atualização" falsa:
//! só quem tem a chave privada (o dono do projeto) consegue gerar uma válida.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

use crate::error::{Error, Result};

pub const PROGRESS_EVENT: &str = "update://progress";

/// Atualização encontrada no último `update_check`, à espera do "Atualizar".
#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Novidades da versão (o `--notes` do `npm run release`).
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

fn updater_error(e: tauri_plugin_updater::Error) -> Error {
    Error::Other(format!("atualização: {e}"))
}

#[tauri::command]
pub async fn update_check(app: AppHandle, updates: State<'_, Updates>) -> Result<Option<UpdateInfo>> {
    let update = app.updater().map_err(updater_error)?.check().await.map_err(updater_error)?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    });
    *updates.pending.lock().await = update;
    Ok(info)
}

#[tauri::command]
pub async fn update_install(app: AppHandle, updates: State<'_, Updates>) -> Result<()> {
    let update = updates
        .pending
        .lock()
        .await
        .take()
        .ok_or_else(|| Error::Other("nenhuma atualização pendente; verifique de novo".into()))?;

    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit(PROGRESS_EVENT, Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(updater_error)?;

    // Windows: o instalador já fechou o app e vai reabri-lo. macOS/Linux: reabrimos nós.
    app.restart()
}
