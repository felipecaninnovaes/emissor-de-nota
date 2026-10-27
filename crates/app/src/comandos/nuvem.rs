//! Backup na nuvem: configuração (endereço e token), envio manual e automático, lista e
//! restauração a partir do servidor.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use nfse_db::nuvem::{self, ConfigNuvem};

use super::comum::{R, emissor, exigir_cofre, texto};
use crate::Estado;
use crate::nuvem::{BackupNuvem, ClienteNuvem, Situacao};

const INATIVA: &str =
    "Assinatura inativa: novos backups não são enviados; os anteriores continuam disponíveis para restaurar.";

#[derive(Serialize)]
pub struct ConfigNuvemDto {
    endereco: String,
    /// Já existe token guardado (o campo da tela fica vazio para não mostrá-lo).
    tem_token: bool,
    /// Data e hora (UTC, RFC 3339) do último backup enviado.
    ultimo: Option<String>,
}

#[tauri::command]
pub async fn obter_nuvem(estado: State<'_, Estado>) -> R<ConfigNuvemDto> {
    exigir_cofre(&estado).await?;
    let b = estado.banco()?;
    Ok(ConfigNuvemDto {
        endereco: b.nuvem().await.map_err(texto)?.map(|c| c.endereco).unwrap_or_default(),
        tem_token: b.tem_token_nuvem().await.map_err(texto)?,
        ultimo: b.nuvem_ultimo().await.map_err(texto)?,
    })
}

/// Grava o endereço; `token` vazio mantém o que já estava guardado.
#[tauri::command]
pub async fn salvar_nuvem(estado: State<'_, Estado>, endereco: String, token: Option<String>) -> R<()> {
    let token = token.map(zeroize::Zeroizing::new);
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    let b = estado.banco()?;
    b.definir_nuvem(&ConfigNuvem { endereco }).await.map_err(texto)?;
    if let Some(token) = token.filter(|t| !t.trim().is_empty()) {
        b.definir_token_nuvem(cofre, &token).await.map_err(texto)?;
    }
    Ok(())
}

/// Confere endereço e token e devolve a situação da assinatura.
#[tauri::command]
pub async fn testar_nuvem(estado: State<'_, Estado>) -> R<Situacao> {
    cliente(&estado).await?.situacao().await
}

#[tauri::command]
pub async fn listar_backups_nuvem(estado: State<'_, Estado>) -> R<Vec<BackupNuvem>> {
    cliente(&estado).await?.listar().await
}

/// Faz um backup agora e envia ao servidor; devolve o nome do arquivo enviado.
#[tauri::command]
pub async fn enviar_backup_nuvem(estado: State<'_, Estado>) -> R<String> {
    executar(&estado).await
}

/// Baixa o backup, confere o SHA-256 da lista do servidor e restaura como o arquivo local:
/// o banco do backup entra no lugar do atual na próxima abertura e o sistema reinicia.
#[tauri::command]
pub async fn restaurar_da_nuvem(app: tauri::AppHandle, estado: State<'_, Estado>, id: i64, senha: String) -> R<()> {
    let senha = zeroize::Zeroizing::new(senha);
    let cliente = cliente(&estado).await?;
    let item = cliente
        .listar()
        .await?
        .into_iter()
        .find(|b| b.id == id)
        .ok_or("backup não encontrado no servidor; atualize a lista")?;
    let pasta = PastaTemporaria::nova()?;
    // Nome nosso: o que vem do servidor não entra no caminho.
    let arquivo = pasta.0.join(format!("nuvem-{id}.{}", nfse_db::backup::EXTENSAO));
    cliente.baixar(id, &arquivo).await?;
    let sha = nuvem::sha256_arquivo(&arquivo).map_err(texto)?;
    if !item.sha256.is_empty() && !sha.eq_ignore_ascii_case(&item.sha256) {
        return Err("o backup baixado não confere com o do servidor (SHA-256 diferente); tente de novo".into());
    }
    super::backup::preparar_restauracao(&estado, &arquivo, &senha).await?;
    drop(pasta);
    super::backup::reiniciar(&app, &estado).await
}

/// Cliente com o endereço e o token guardados (o cofre precisa estar aberto para decifrar o token).
async fn cliente(estado: &Estado) -> R<ClienteNuvem> {
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    let b = estado.banco()?;
    let cfg = b
        .nuvem()
        .await
        .map_err(texto)?
        .ok_or("informe o endereço do servidor de backup")?;
    let token = b
        .token_nuvem(cofre)
        .await
        .map_err(texto)?
        .ok_or("informe o token do backup na nuvem")?;
    ClienteNuvem::novo(&cfg.endereco, token)
}

async fn executar(estado: &Estado) -> R<String> {
    let cliente = cliente(estado).await?;
    // Com a assinatura inativa o servidor recusaria o envio: nem gera o backup.
    if !cliente.situacao().await?.ativo {
        return Err(INATIVA.into());
    }
    // `fazer_backup` poda a pasta: uma só nossa, no temporário do sistema, apagada no fim.
    let pasta = PastaTemporaria::nova()?;
    let arquivo = {
        let guarda = emissor!(estado);
        let cofre = guarda.as_ref().expect("verificado").cofre();
        estado.banco()?.fazer_backup(cofre, &pasta.0).await.map_err(texto)?
    };
    let sha = nuvem::sha256_arquivo(&arquivo).map_err(texto)?;
    let enviado = cliente.enviar(&arquivo, &sha).await?;
    let b = estado.banco()?;
    b.marcar_envio_nuvem().await.map_err(texto)?;
    let _ = b.auditar("backup_nuvem_enviado", None, Some(&enviado.nome)).await;
    Ok(enviado.nome)
}

/// Envio automático ao abrir o cofre: só com token guardado e último envio antigo. Falha (sem
/// conexão, assinatura inativa) não impede o uso do sistema; fica na auditoria.
pub(crate) async fn automatico(estado: &Estado) {
    let Ok(banco) = estado.banco() else { return };
    if !banco.tem_token_nuvem().await.unwrap_or(false) {
        return;
    }
    let ultimo = banco.nuvem_ultimo().await.ok().flatten();
    if !super::backup::vencido(ultimo.as_deref()) {
        return;
    }
    if let Err(e) = executar(estado).await {
        let _ = banco.auditar("backup_nuvem_falhou", None, Some(&e)).await;
    }
}

/// Pasta própria no temporário do sistema, apagada com tudo ao sair de escopo.
struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    fn nova() -> R<Self> {
        static SEQUENCIA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let pasta = std::env::temp_dir().join(format!(
            "nfse-nuvem-{}-{}",
            std::process::id(),
            SEQUENCIA.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).map_err(|e| format!("não foi possível criar a pasta temporária: {e}"))?;
        Ok(Self(pasta))
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
