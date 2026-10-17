//! Cofre (senha mestra) e pasta de saída.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

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
        existe: estado.banco.cofre_existe().await.map_err(texto)?,
        aberto: estado.emissor.read().await.is_some(),
    })
}

/// Logomarca da NFS-e no cabeçalho do DANFSe. O PNG vem do SVG oficial
/// (`logo-nfse-assinatura-horizontal.svg`), renderizado com a área alargada para o texto caber:
/// `rsvg-convert -w 1200 logo.svg -o logo-nfse.png`.
const LOGO_NFSE: &[u8] = include_bytes!("../../assets/logo-nfse.png");

#[derive(Serialize)]
pub struct Abertura {
    /// Resultado da reconciliação das DPS que ficaram sem desfecho.
    pendencias: Vec<String>,
}

async fn iniciar(estado: &Estado, cofre: nfse_db::Cofre) -> R<Abertura> {
    let pasta = pasta_configurada(estado).await?;
    let emissor = Emissor::novo(estado.banco.clone(), cofre, ConectorSefin)
        .com_versao(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
        .com_logo(LOGO_NFSE.to_vec())
        .com_pasta(pasta);
    let mut pendencias = Vec::new();
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
    Ok(Abertura { pendencias })
}

#[tauri::command]
pub async fn criar_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let cofre = estado.banco.criar_cofre(&senha).await.map_err(texto)?;
    iniciar(&estado, cofre).await
}

#[tauri::command]
pub async fn abrir_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let cofre = estado.banco.abrir_cofre(&senha).await.map_err(texto)?;
    iniciar(&estado, cofre).await
}

/// Descarta o emissor (e com ele a chave do cofre); a senha mestra volta a ser exigida.
/// Espera as operações em andamento terminarem, porque elas seguram o emissor em leitura.
#[tauri::command]
pub async fn bloquear_cofre(estado: State<'_, Estado>) -> R<()> {
    *estado.emissor.write().await = None;
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
        .banco
        .definir_configuracao(CHAVE_PASTA, &caminho.display().to_string())
        .await
        .map_err(texto)?;
    if let Some(emissor) = estado.emissor.write().await.as_mut() {
        emissor.definir_pasta(caminho);
    }
    Ok(())
}
