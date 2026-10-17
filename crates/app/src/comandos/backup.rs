//! Backup cifrado do banco: pasta de destino, backup manual e automático, restauração.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use nfse_db::backup;

use super::comum::{R, emissor, exigir_cofre, texto};
use crate::Estado;

const CHAVE_PASTA_BACKUP: &str = "backup_pasta";
const CHAVE_ULTIMO_BACKUP: &str = "backup_ultimo";
/// O backup automático roda ao abrir o sistema quando o último tem mais que isso.
const INTERVALO_AUTOMATICO: chrono::Duration = chrono::Duration::hours(20);

#[derive(Serialize)]
pub struct SituacaoBackup {
    pasta: String,
    /// Data e hora (UTC, RFC 3339) do último backup feito com sucesso.
    ultimo: Option<String>,
}

#[tauri::command]
pub async fn obter_backup(estado: State<'_, Estado>) -> R<SituacaoBackup> {
    exigir_cofre(&estado).await?;
    Ok(SituacaoBackup {
        pasta: estado
            .banco()?
            .configuracao(CHAVE_PASTA_BACKUP)
            .await
            .map_err(texto)?
            .unwrap_or_default(),
        ultimo: estado.banco()?.configuracao(CHAVE_ULTIMO_BACKUP).await.map_err(texto)?,
    })
}

#[tauri::command]
pub async fn definir_pasta_backup(estado: State<'_, Estado>, pasta: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let pasta = pasta.trim();
    if !pasta.is_empty() {
        std::fs::create_dir_all(pasta).map_err(|e| format!("não foi possível usar a pasta: {e}"))?;
    }
    estado
        .banco()?
        .definir_configuracao(CHAVE_PASTA_BACKUP, pasta)
        .await
        .map_err(texto)
}

/// Faz o backup agora na pasta configurada e devolve o nome do arquivo.
#[tauri::command]
pub async fn fazer_backup(estado: State<'_, Estado>) -> R<String> {
    executar(&estado).await
}

async fn executar(estado: &Estado) -> R<String> {
    let pasta = estado
        .banco()?
        .configuracao(CHAVE_PASTA_BACKUP)
        .await
        .map_err(texto)?
        .filter(|p| !p.trim().is_empty())
        .ok_or("defina a pasta do backup em Configurações")?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("cofre conferido acima");
    let arquivo = estado
        .banco()?
        .fazer_backup(emissor.cofre(), Path::new(&pasta))
        .await
        .map_err(texto)?;
    estado
        .banco()?
        .definir_configuracao(CHAVE_ULTIMO_BACKUP, &chrono::Utc::now().to_rfc3339())
        .await
        .map_err(texto)?;
    Ok(arquivo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default())
}

/// Backup automático ao abrir o cofre: só roda com pasta definida e último backup antigo.
/// Falha não impede o uso do sistema; fica na auditoria.
pub(crate) async fn automatico(estado: &Estado) {
    let Ok(banco) = estado.banco() else { return };
    let Ok(Some(pasta)) = banco.configuracao(CHAVE_PASTA_BACKUP).await else {
        return;
    };
    if pasta.trim().is_empty() {
        return;
    }
    let ultimo = banco
        .configuracao(CHAVE_ULTIMO_BACKUP)
        .await
        .ok()
        .flatten()
        .and_then(|u| chrono::DateTime::parse_from_rfc3339(&u).ok());
    if ultimo.is_some_and(|u| chrono::Utc::now() - u.to_utc() < INTERVALO_AUTOMATICO) {
        return;
    }
    if let Err(e) = executar(estado).await {
        let _ = banco.auditar("backup_falhou", None, Some(&e)).await;
    }
}

/// Decifra um backup para um arquivo novo e confere a integridade (serve de teste de
/// restauração). Não substitui o banco em uso: para restaurar de verdade, feche o sistema e
/// troque o `emissor.db` e o `emissor.cofre` pelos arquivos gerados.
#[tauri::command]
pub async fn restaurar_backup(estado: State<'_, Estado>, arquivo: String, senha: String, destino: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let destino = PathBuf::from(destino.trim());
    backup::restaurar_backup(Path::new(arquivo.trim()), &senha, &destino)
        .await
        .map_err(texto)
}
