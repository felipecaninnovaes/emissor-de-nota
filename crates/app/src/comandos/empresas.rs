//! Empresas, certificados A1 e numeração das DPS.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Cnpj;
use nfse_core::modelo::{Ambiente, RegimeTributario};
use nfse_db::empresas::{Exclusao, NovaEmpresa};

use super::comum::{R, emissor, exigir_cofre, texto, vazio_para_none};
use crate::Estado;

#[derive(Serialize)]
pub struct EmpresaDto {
    id: i64,
    cnpj: String,
    cnpj_formatado: String,
    nome: String,
    inscricao_municipal: Option<String>,
    im_na_dps_producao: bool,
    im_na_dps_homologacao: bool,
    municipio_emissor: String,
    regime: RegimeTributario,
    ambiente: Ambiente,
    serie: String,
    telefone: Option<String>,
    email: Option<String>,
    certificado: Option<CertificadoDto>,
}

#[derive(Serialize)]
pub struct CertificadoDto {
    titular: String,
    valido_ate: String,
    dias_para_vencer: i64,
}

#[tauri::command]
pub async fn listar_empresas(estado: State<'_, Estado>) -> R<Vec<EmpresaDto>> {
    exigir_cofre(&estado).await?;
    let mut lista = Vec::new();
    for e in estado.banco()?.empresas().await.map_err(texto)? {
        let certificado = estado
            .banco()?
            .certificado_resumo(e.id)
            .await
            .map_err(texto)?
            .map(|(titular, ate)| CertificadoDto {
                titular,
                valido_ate: ate.format("%d/%m/%Y").to_string(),
                dias_para_vencer: (ate - Utc::now()).num_days(),
            });
        lista.push(EmpresaDto {
            id: e.id,
            cnpj: e.cnpj.as_str().to_owned(),
            cnpj_formatado: e.cnpj.to_string(),
            nome: e.nome,
            inscricao_municipal: e.inscricao_municipal,
            im_na_dps_producao: e.im_na_dps_producao,
            im_na_dps_homologacao: e.im_na_dps_homologacao,
            municipio_emissor: e.municipio_emissor,
            regime: e.regime,
            ambiente: e.ambiente,
            serie: e.serie,
            telefone: e.telefone,
            email: e.email,
            certificado,
        });
    }
    Ok(lista)
}

#[derive(Deserialize)]
pub struct NovaEmpresaDto {
    cnpj: String,
    nome: String,
    inscricao_municipal: Option<String>,
    municipio_emissor: String,
    regime: RegimeTributario,
    ambiente: Ambiente,
    serie: String,
    telefone: Option<String>,
    email: Option<String>,
}

impl NovaEmpresaDto {
    fn empresa(self) -> R<NovaEmpresa> {
        Ok(NovaEmpresa {
            cnpj: Cnpj::parse(&self.cnpj).map_err(|e| format!("CNPJ: {e}"))?,
            nome: self.nome,
            inscricao_municipal: vazio_para_none(self.inscricao_municipal),
            municipio_emissor: self.municipio_emissor.trim().to_owned(),
            regime: self.regime,
            ambiente: self.ambiente,
            serie: self.serie.trim().to_owned(),
            telefone: vazio_para_none(self.telefone).map(|t| t.chars().filter(char::is_ascii_digit).collect()),
            email: vazio_para_none(self.email),
        })
    }
}

#[tauri::command]
pub async fn cadastrar_empresa(estado: State<'_, Estado>, empresa: NovaEmpresaDto) -> R<i64> {
    exigir_cofre(&estado).await?;
    let nova = empresa.empresa()?;
    Ok(estado.banco()?.cadastrar_empresa(&nova).await.map_err(texto)?.id)
}

#[tauri::command]
pub async fn atualizar_empresa(estado: State<'_, Estado>, empresa_id: i64, empresa: NovaEmpresaDto) -> R<()> {
    exigir_cofre(&estado).await?;
    let dados = empresa.empresa()?;
    estado
        .banco()?
        .atualizar_empresa(empresa_id, &dados)
        .await
        .map_err(texto)?;
    Ok(())
}

#[tauri::command]
pub async fn importar_certificado(
    estado: State<'_, Estado>,
    empresa_id: i64,
    pfx: Vec<u8>,
    senha: String,
) -> R<CertificadoDto> {
    let senha = zeroize::Zeroizing::new(senha);
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    let info = estado
        .banco()?
        .importar_certificado(cofre, empresa_id, &pfx, &senha)
        .await
        .map_err(texto)?;
    Ok(CertificadoDto {
        dias_para_vencer: info.dias_para_vencer(Utc::now()),
        valido_ate: info.valido_ate.format("%d/%m/%Y").to_string(),
        titular: info.titular,
    })
}

/// Exclui a empresa (sem notas) ou arquiva (com notas); devolve a mensagem para o operador.
#[tauri::command]
pub async fn excluir_empresa(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    exigir_cofre(&estado).await?;
    Ok(
        match estado.banco()?.excluir_empresa(empresa_id).await.map_err(texto)? {
            Exclusao::Excluida => "Empresa excluída.".into(),
            Exclusao::Arquivada { notas } => format!(
                "Empresa arquivada: ela tem {notas} NFS-e no banco, que continuam guardadas.              Cadastrar o mesmo CNPJ de novo traz a empresa de volta com as notas."
            ),
        },
    )
}

#[tauri::command]
pub async fn definir_im_na_dps(
    estado: State<'_, Estado>,
    empresa_id: i64,
    ambiente: Ambiente,
    informar: bool,
) -> R<()> {
    exigir_cofre(&estado).await?;
    estado
        .banco()?
        .definir_im_na_dps(empresa_id, ambiente, informar)
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn definir_proximo_numero(
    estado: State<'_, Estado>,
    empresa_id: i64,
    ambiente: Ambiente,
    serie: String,
    proximo: u64,
) -> R<()> {
    exigir_cofre(&estado).await?;
    estado
        .banco()?
        .definir_proximo_numero(empresa_id, ambiente, &serie, proximo)
        .await
        .map_err(texto)
}

/// Pergunta à Sefin quais números da série atual já geraram NFS-e e ajusta o próximo.
#[tauri::command]
pub async fn ajustar_numeracao(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    let guarda = emissor!(estado);
    let (ultimo, proximo) = guarda
        .as_ref()
        .expect("verificado")
        .ajustar_numeracao_pela_sefin(empresa_id)
        .await
        .map_err(texto)?;
    Ok(match ultimo {
        Some(u) => format!("A Sefin já tem DPS desta série até o nº {u}. A próxima nota sai com o nº {proximo}."),
        None => format!("Nenhum número usado na Sefin a partir daqui. A próxima nota sai com o nº {proximo}."),
    })
}

#[derive(Serialize)]
pub struct VencimentoDto {
    empresa_id: i64,
    empresa: String,
    valido_ate: String,
    dias: i64,
}

#[tauri::command]
pub async fn certificados_vencendo(estado: State<'_, Estado>, dias: i64) -> R<Vec<VencimentoDto>> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco()?
        .certificados_vencendo(dias)
        .await
        .map_err(texto)?
        .into_iter()
        .map(|v| VencimentoDto {
            empresa_id: v.empresa_id,
            empresa: v.empresa,
            valido_ate: v.valido_ate.format("%d/%m/%Y").to_string(),
            dias: (v.valido_ate - Utc::now()).num_days(),
        })
        .collect())
}
