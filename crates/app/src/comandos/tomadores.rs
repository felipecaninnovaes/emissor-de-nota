//! Cadastro de tomadores, com importação por planilha.

use serde::Serialize;
use tauri::State;

use nfse_core::modelo::Tomador;
use nfse_db::tomadores::{
    ResumoImportacaoTomadores, RetencaoDoVinculo, TipoTomador, TomadorImportado, TomadorVinculado,
};

use super::comum::{R, exigir_cofre, gravar_e_abrir_csv, texto, vazio_para_none};
use crate::Estado;

#[derive(Serialize)]
pub struct TomadorDto {
    pessoa_id: i64,
    tomador: Tomador,
    emails: Option<String>,
    retem_federais: bool,
    retem_iss: bool,
    tipo: TipoTomador,
}

impl From<TomadorVinculado> for TomadorDto {
    fn from(t: TomadorVinculado) -> Self {
        Self {
            pessoa_id: t.pessoa_id,
            tomador: t.tomador,
            emails: t.emails,
            retem_federais: t.retem_federais,
            retem_iss: t.retem_iss,
            tipo: t.tipo,
        }
    }
}

#[tauri::command]
pub async fn buscar_tomadores(estado: State<'_, Estado>, empresa_id: i64, termo: String) -> R<Vec<TomadorDto>> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco()?
        .buscar_tomadores(empresa_id, &termo)
        .await
        .map_err(texto)?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Tomadores de outras empresas que esta ainda não vinculou (para vincular em vez de cadastrar de novo).
#[tauri::command]
pub async fn buscar_tomadores_de_outras_empresas(
    estado: State<'_, Estado>,
    empresa_id: i64,
    termo: String,
) -> R<Vec<TomadorDto>> {
    exigir_cofre(&estado).await?;
    if termo.trim().is_empty() {
        return Ok(vec![]);
    }
    Ok(estado
        .banco()?
        .buscar_tomadores_de_outras_empresas(empresa_id, &termo)
        .await
        .map_err(texto)?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub async fn vincular_tomador(estado: State<'_, Estado>, empresa_id: i64, pessoa_id: i64) -> R<TomadorDto> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco()?
        .vincular_tomador_cadastrado(empresa_id, pessoa_id)
        .await
        .map_err(texto)?
        .into())
}

#[tauri::command]
pub async fn salvar_tomador(
    estado: State<'_, Estado>,
    empresa_id: i64,
    tomador: Tomador,
    emails: Option<String>,
    retem_federais: Option<bool>,
    retem_iss: Option<bool>,
    tipo: Option<TipoTomador>,
) -> R<i64> {
    exigir_cofre(&estado).await?;
    estado
        .banco()?
        .salvar_tomador_vinculado(
            empresa_id,
            &tomador,
            "manual",
            vazio_para_none(emails).as_deref(),
            RetencaoDoVinculo {
                federais: retem_federais.unwrap_or(false),
                iss: retem_iss.unwrap_or(false),
                tipo: tipo.unwrap_or_default(),
            },
        )
        .await
        .map_err(texto)
}

#[derive(Serialize)]
pub struct LinhaTomadorDto {
    linha: usize,
    documento: String,
    nome: String,
    /// Vazio quando a linha tem erro.
    item: Option<TomadorImportado>,
    erros: Vec<String>,
}

/// Lê a planilha de tomadores para a tela conferir antes de importar.
#[tauri::command]
pub async fn ler_planilha_tomadores(nome: String, bytes: Vec<u8>) -> R<Vec<LinhaTomadorDto>> {
    Ok(nfse_emissor::planilha_tomadores::ler(&nome, &bytes)?
        .into_iter()
        .map(|l| LinhaTomadorDto {
            linha: l.numero,
            documento: l.documento,
            nome: l.nome,
            item: l.item,
            erros: l.erros,
        })
        .collect())
}

/// Grava na empresa os tomadores marcados da planilha (tudo ou nada).
#[tauri::command]
pub async fn importar_tomadores(
    estado: State<'_, Estado>,
    empresa_id: i64,
    itens: Vec<TomadorImportado>,
) -> R<ResumoImportacaoTomadores> {
    exigir_cofre(&estado).await?;
    if itens.is_empty() {
        return Err("Nenhum tomador escolhido.".into());
    }
    estado
        .banco()?
        .importar_tomadores(empresa_id, &itens)
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn salvar_modelo_tomadores(estado: State<'_, Estado>) -> R<String> {
    gravar_e_abrir_csv(
        &estado,
        "modelo-importacao-tomadores.csv",
        &nfse_emissor::planilha_tomadores::modelo_csv(),
    )
    .await
}
