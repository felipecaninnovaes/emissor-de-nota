//! Notas emitidas: listagem, cancelamento, reconciliação e abertura dos arquivos.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use nfse_db::dps::SituacaoDps;
use nfse_xml::MotivoCancelamento;

use super::comum::{R, abrir_no_sistema, emissor, exigir_cofre, pasta_configurada, texto, vazio_para_none};
use super::tomadores::TomadorDto;
use crate::Estado;

#[derive(Serialize)]
pub struct NotaDto {
    dps_id: i64,
    nfse_id: Option<i64>,
    serie: String,
    numero_dps: u64,
    numero_nfse: Option<String>,
    chave: Option<String>,
    situacao: &'static str,
    erro: Option<String>,
    tomador: Option<String>,
    descricao: Option<String>,
    valor: Option<String>,
    competencia: Option<String>,
    criada_em: String,
    email_enviado_em: Option<String>,
}

#[tauri::command]
pub async fn listar_notas(estado: State<'_, Estado>, empresa_id: i64, mes: Option<String>) -> R<Vec<NotaDto>> {
    exigir_cofre(&estado).await?;
    let mes = vazio_para_none(mes);
    Ok(estado
        .banco
        .listar_notas(empresa_id, mes.as_deref())
        .await
        .map_err(texto)?
        .into_iter()
        .map(|n| NotaDto {
            dps_id: n.dps_id,
            nfse_id: n.nfse_id,
            serie: n.serie,
            numero_dps: n.numero_dps,
            numero_nfse: n.numero_nfse,
            chave: n.chave,
            situacao: n.situacao.texto(),
            erro: n.ultimo_erro,
            tomador: n.tomador,
            descricao: n.descricao,
            valor: n.valor.map(|v| v.to_string()),
            competencia: n.competencia,
            criada_em: n.criada_em,
            email_enviado_em: n.email_enviado_em,
        })
        .collect())
}

#[tauri::command]
pub async fn cancelar(
    estado: State<'_, Estado>,
    nfse_id: i64,
    motivo: MotivoCancelamento,
    justificativa: String,
) -> R<()> {
    let guarda = emissor!(estado);
    guarda
        .as_ref()
        .expect("verificado")
        .cancelar(nfse_id, motivo, &justificativa)
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn reconciliar(estado: State<'_, Estado>, dps_id: i64) -> R<String> {
    let guarda = emissor!(estado);
    Ok(
        match guarda
            .as_ref()
            .expect("verificado")
            .reconciliar(dps_id)
            .await
            .map_err(texto)?
        {
            Some(n) => format!(
                "A Sefin tinha autorizado: nota {} gravada.",
                n.numero.unwrap_or_default()
            ),
            None => "A Sefin não gerou nota para esta DPS; pode emitir de novo.".into(),
        },
    )
}

#[derive(Serialize)]
pub struct RefazerDto {
    empresa_id: i64,
    tomador: Option<TomadorDto>,
    servico_id: Option<i64>,
    valor: String,
    descricao: String,
    obra: Option<nfse_core::modelo::Obra>,
    informacoes_complementares: Option<String>,
}

/// Dados de uma nota rejeitada para preencher a tela de emissão de novo.
#[tauri::command]
pub async fn dados_para_refazer(estado: State<'_, Estado>, dps_id: i64) -> R<RefazerDto> {
    let guarda = emissor!(estado);
    let r = guarda
        .as_ref()
        .expect("verificado")
        .dados_para_refazer(dps_id)
        .await
        .map_err(texto)?;
    Ok(RefazerDto {
        empresa_id: r.empresa_id,
        tomador: r.tomador.map(Into::into),
        servico_id: r.servico_id,
        valor: r.valor.to_string(),
        descricao: r.descricao,
        obra: r.obra,
        informacoes_complementares: r.informacoes_complementares,
    })
}

/// Dados de uma nota autorizada ou cancelada para emitir outra igual.
#[tauri::command]
pub async fn dados_para_copiar(estado: State<'_, Estado>, dps_id: i64) -> R<RefazerDto> {
    let guarda = emissor!(estado);
    let r = guarda
        .as_ref()
        .expect("verificado")
        .dados_para_copiar(dps_id)
        .await
        .map_err(texto)?;
    Ok(RefazerDto {
        empresa_id: r.empresa_id,
        tomador: r.tomador.map(Into::into),
        servico_id: r.servico_id,
        valor: r.valor.to_string(),
        descricao: r.descricao,
        obra: r.obra,
        informacoes_complementares: r.informacoes_complementares,
    })
}

#[derive(Serialize)]
pub struct PendenteDto {
    dps_id: i64,
    empresa_id: i64,
    id_dps: String,
    erro: Option<String>,
}

#[tauri::command]
pub async fn pendentes(estado: State<'_, Estado>) -> R<Vec<PendenteDto>> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco
        .dps_pendentes()
        .await
        .map_err(texto)?
        .into_iter()
        .filter(|d| d.situacao == SituacaoDps::Incerta || d.situacao == SituacaoDps::Enviando)
        .map(|d| PendenteDto {
            dps_id: d.id,
            empresa_id: d.empresa_id,
            id_dps: d.id_dps,
            erro: d.ultimo_erro,
        })
        .collect())
}

/// Abre um arquivo ou pasta da pasta de saída com o programa padrão do Windows.
#[tauri::command]
pub async fn abrir_arquivo(estado: State<'_, Estado>, caminho: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let base = std::fs::canonicalize(pasta_configurada(&estado).await?)
        .map_err(|e| format!("pasta de saída indisponível: {e}"))?;
    let alvo = std::fs::canonicalize(caminho.trim()).map_err(|e| format!("caminho inválido: {e}"))?;
    if !alvo.starts_with(&base) {
        return Err("só é possível abrir arquivos da pasta de saída".into());
    }
    if alvo.is_file() {
        let ext = alvo
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !["pdf", "xml", "csv"].contains(&ext.as_str()) {
            return Err("tipo de arquivo não permitido".into());
        }
    }
    abrir_no_sistema(&alvo)
}

/// Abre o PDF (ou, com `pasta = true`, a pasta) da nota.
#[tauri::command]
pub async fn abrir_nota(estado: State<'_, Estado>, nfse_id: i64, pasta: bool) -> R<()> {
    let guarda = emissor!(estado);
    let xml = guarda
        .as_ref()
        .expect("verificado")
        .caminho_xml(nfse_id)
        .await
        .map_err(texto)?
        .ok_or("pasta de saída não configurada")?;
    let alvo = if pasta {
        xml.parent().map(PathBuf::from).unwrap_or(xml)
    } else {
        xml.with_extension("pdf")
    };
    if !alvo.exists() {
        return Err(format!("arquivo não encontrado: {}", alvo.display()));
    }
    abrir_no_sistema(&alvo)
}
