//! Cadastro de tomadores.

use serde::Serialize;
use tauri::State;

use nfse_core::modelo::Tomador;
use nfse_db::tomadores::TomadorVinculado;

use super::comum::{R, exigir_cofre, texto, vazio_para_none};
use crate::Estado;

#[derive(Serialize)]
pub struct TomadorDto {
    pessoa_id: i64,
    tomador: Tomador,
    emails: Option<String>,
    retem_federais: bool,
    retem_iss: bool,
}

impl From<TomadorVinculado> for TomadorDto {
    fn from(t: TomadorVinculado) -> Self {
        Self {
            pessoa_id: t.pessoa_id,
            tomador: t.tomador,
            emails: t.emails,
            retem_federais: t.retem_federais,
            retem_iss: t.retem_iss,
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
) -> R<i64> {
    exigir_cofre(&estado).await?;
    let id = estado
        .banco()?
        .salvar_tomador(empresa_id, &tomador, "manual")
        .await
        .map_err(texto)?;
    estado
        .banco()?
        .definir_emails_tomador(empresa_id, id, vazio_para_none(emails).as_deref(), None)
        .await
        .map_err(texto)?;
    estado
        .banco()?
        .definir_retencao_tomador(
            empresa_id,
            id,
            retem_federais.unwrap_or(false),
            retem_iss.unwrap_or(false),
        )
        .await
        .map_err(texto)?;
    Ok(id)
}
