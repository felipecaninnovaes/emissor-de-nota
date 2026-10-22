//! Diagnóstico: informações do aplicativo e do log para suporte, e o diagnóstico de uma nota.

use serde::Serialize;
use tauri::State;

use nfse_core::modelo::Ambiente;

use super::comum::{R, abrir_no_sistema, emissor, pasta_configurada, texto};
use crate::{Estado, registro};

/// Linhas finais do log que acompanham o diagnóstico.
const LINHAS_DE_LOG: usize = 40;

#[derive(Serialize)]
pub struct DiagnosticoDto {
    pasta_logs: String,
    detalhado: bool,
    /// Texto pronto para copiar: versão, sistema, leiautes e as últimas linhas do log.
    texto: String,
}

#[tauri::command]
pub async fn obter_diagnostico(estado: State<'_, Estado>, app: tauri::AppHandle) -> R<DiagnosticoDto> {
    let pasta_logs = registro::pasta_logs(&estado.dados);
    let detalhado = registro::detalhado(&estado.dados);
    let mut t = String::new();
    t.push_str(&format!("Emissor NFS-e {}\n", app.package_info().version));
    t.push_str(&format!(
        "Sistema: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    t.push_str(&format!(
        "Log detalhado: {}\n",
        if detalhado { "ligado" } else { "desligado" }
    ));
    // Sem a senha mestra o banco está fechado: o resto do diagnóstico fica só com o que não precisa dele.
    if let Ok(banco) = estado.banco() {
        for (nome, ambiente) in [
            ("Produção Restrita", Ambiente::Homologacao),
            ("Produção", Ambiente::Producao),
        ] {
            if let Ok(leiaute) = banco.leiaute_dps(ambiente).await {
                t.push_str(&format!("Leiaute da DPS em {nome}: {}\n", leiaute.codigo()));
            }
        }
        if let Ok(pasta) = pasta_configurada(&estado).await {
            t.push_str(&format!("Pasta de saída: {}\n", pasta.display()));
        }
    }
    let linhas = registro::ultimas_linhas(&estado.dados, LINHAS_DE_LOG);
    if !linhas.is_empty() {
        t.push_str(&format!("\n--- Últimas {LINHAS_DE_LOG} linhas do log ---\n{linhas}\n"));
    }
    Ok(DiagnosticoDto {
        pasta_logs: pasta_logs.display().to_string(),
        detalhado,
        texto: t,
    })
}

#[tauri::command]
pub fn definir_log_detalhado(estado: State<'_, Estado>, ligado: bool) -> R<()> {
    registro::definir_detalhado(&estado.dados, ligado)
}

#[tauri::command]
pub fn abrir_pasta_logs(estado: State<'_, Estado>) -> R<()> {
    let pasta = registro::pasta_logs(&estado.dados);
    std::fs::create_dir_all(&pasta).map_err(texto)?;
    abrir_no_sistema(&pasta)
}

/// Diagnóstico de uma DPS (erro, resposta da Sefin e XML enviado, com dados pessoais mascarados).
#[tauri::command]
pub async fn diagnostico_nota(estado: State<'_, Estado>, dps_id: i64) -> R<String> {
    let guarda = emissor!(estado);
    guarda
        .as_ref()
        .expect("verificado")
        .diagnostico_nota(dps_id)
        .await
        .map_err(texto)
}
