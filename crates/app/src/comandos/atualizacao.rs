//! Atualização do aplicativo pelas Releases do GitHub.

use serde::Serialize;

use super::comum::{R, texto};

#[derive(Serialize)]
pub struct AtualizacaoDto {
    versao: String,
    notas: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ProgressoAtualizacao {
    baixado: usize,
    total: Option<u64>,
}

/// Versão instalada (a de `crates/app/Cargo.toml`).
#[tauri::command]
pub fn versao_app(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Consulta a última Release no GitHub; `None` quando já está na versão mais nova.
#[tauri::command]
pub async fn procurar_atualizacao(app: tauri::AppHandle) -> R<Option<AtualizacaoDto>> {
    use tauri_plugin_updater::UpdaterExt;
    let nova = app.updater().map_err(texto)?.check().await.map_err(texto)?;
    Ok(nova.map(|u| AtualizacaoDto {
        versao: u.version,
        notas: u.body,
    }))
}

/// Baixa, confere a assinatura e instala a nova versão. No Windows o instalador fecha o sistema e
/// o abre de novo ao terminar. O andamento sai no evento `progresso-atualizacao`.
#[tauri::command]
pub async fn instalar_atualizacao(app: tauri::AppHandle) -> R<()> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;
    let Some(nova) = app.updater().map_err(texto)?.check().await.map_err(texto)? else {
        return Err("o sistema já está na versão mais nova".into());
    };
    let mut baixado = 0;
    nova.download_and_install(
        |pedaco, total| {
            baixado += pedaco;
            let _ = app.emit("progresso-atualizacao", ProgressoAtualizacao { baixado, total });
        },
        || {},
    )
    .await
    .map_err(|e| format!("não foi possível atualizar: {e}"))
}
