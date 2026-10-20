//! Cofre (senha mestra) e pasta de saída.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use nfse_db::Banco;
use nfse_emissor::{ConectorSefin, Emissor};

use super::comum::{CHAVE_PASTA, R, exigir_cofre, pasta_configurada, texto};
use crate::Estado;

#[derive(Serialize)]
pub struct SituacaoCofre {
    existe: bool,
    aberto: bool,
}

#[tauri::command]
pub async fn situacao_cofre(estado: State<'_, Estado>) -> R<SituacaoCofre> {
    Ok(SituacaoCofre {
        existe: Banco::cofre_existe(estado.caminho_banco()).await.map_err(texto)?,
        aberto: estado.emissor.read().await.is_some(),
    })
}

/// Logomarca da NFS-e no cabeçalho do DANFSe. O PNG sai de `assets/logo-nfse.svg`, cópia do SVG
/// oficial (`logo-nfse-assinatura-horizontal.svg`) com dois ajustes: fonte Liberation Sans no
/// lugar da Segoe UI e largura de 231,5 para 240, para o texto não ser cortado fora do Windows.
/// Para regerar: `rsvg-convert -w 1200 assets/logo-nfse.svg -o assets/logo-nfse.png`.
const LOGO_NFSE: &[u8] = include_bytes!("../../assets/logo-nfse.png");

#[derive(Serialize)]
pub struct Abertura {
    /// Resultado da reconciliação das DPS que ficaram sem desfecho.
    pendencias: Vec<String>,
}

/// Roda a abertura do banco numa thread bloqueante. As migrações do sqlx usam `&mut conexão`, e o
/// rustc não consegue provar `Send` para isso dentro de um comando do Tauri; ali o futuro nunca
/// cruza threads, então não precisa ser `Send`.
async fn em_thread_bloqueante<F, Fut, T>(trabalho: F) -> Result<T, tauri::Error>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = T>,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || tauri::async_runtime::block_on(trabalho())).await
}

async fn iniciar(estado: &Estado, banco: Banco, cofre: nfse_db::Cofre) -> R<Abertura> {
    estado.trocar_banco(Some(banco.clone()));
    let pasta = pasta_configurada(estado).await?;
    let emissor = Emissor::novo(banco, cofre, ConectorSefin)
        .com_versao(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
        .com_logo(LOGO_NFSE.to_vec())
        .com_pasta(pasta);
    // Falha aqui não impede a abertura: vira aviso, e a conferência fica com o operador.
    let restauracao = super::backup::concluir_restauracao(estado).await.unwrap_or_else(|e| {
        Some(format!(
            "Backup restaurado, mas a numeração não foi marcada para conferência ({e}): antes de emitir, use \
             Empresas → Numeração / IM → Consultar na Sefin em cada empresa."
        ))
    });
    let mut pendencias: Vec<String> = restauracao.into_iter().collect();
    for (dps_id, resultado) in emissor.reconciliar_pendentes().await.map_err(texto)? {
        pendencias.push(match resultado {
            Ok(Some(n)) => format!(
                "DPS {dps_id}: a Sefin tinha autorizado a nota {}; gravada.",
                n.numero.unwrap_or_default()
            ),
            Ok(None) => format!("DPS {dps_id}: a Sefin não gerou nota; pode emitir de novo."),
            Err(e) => format!("DPS {dps_id}: ainda sem resposta ({e}); será verificada na próxima abertura."),
        });
    }
    *estado.emissor.write().await = Some(emissor);
    super::backup::automatico(estado).await;
    Ok(Abertura { pendencias })
}

#[tauri::command]
pub async fn criar_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let caminho = estado.caminho_banco();
    let (banco, cofre) = em_thread_bloqueante(move || Banco::criar_protegido(caminho, senha))
        .await
        .map_err(texto)?
        .map_err(texto)?;
    iniciar(&estado, banco, cofre).await
}

#[tauri::command]
pub async fn abrir_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let caminho = estado.caminho_banco();
    let (banco, cofre) = em_thread_bloqueante(move || Banco::abrir_protegido(caminho, senha))
        .await
        .map_err(texto)?
        .map_err(texto)?;
    iniciar(&estado, banco, cofre).await
}

/// Descarta o emissor (e com ele a chave do cofre) e fecha o banco cifrado; a senha mestra volta
/// a ser exigida. Espera as operações em andamento terminarem, porque elas seguram o emissor em leitura.
#[tauri::command]
pub async fn bloquear_cofre(estado: State<'_, Estado>) -> R<()> {
    *estado.emissor.write().await = None;
    if let Some(banco) = estado.trocar_banco(None) {
        banco.fechar().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn obter_pasta(estado: State<'_, Estado>) -> R<String> {
    Ok(pasta_configurada(&estado).await?.display().to_string())
}

#[tauri::command]
pub async fn definir_pasta(estado: State<'_, Estado>, pasta: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let caminho = PathBuf::from(pasta.trim());
    std::fs::create_dir_all(&caminho).map_err(|e| format!("não foi possível usar a pasta: {e}"))?;
    estado
        .banco()?
        .definir_configuracao(CHAVE_PASTA, &caminho.display().to_string())
        .await
        .map_err(texto)?;
    if let Some(emissor) = estado.emissor.write().await.as_mut() {
        emissor.definir_pasta(caminho);
    }
    Ok(())
}
