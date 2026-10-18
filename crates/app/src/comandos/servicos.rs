//! Catálogo de serviços por empresa, com importação e exportação por planilha.

use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Valor;
use nfse_core::tabela_codigos::ItemCodigo;
use nfse_core::{lista_servicos, nbs};
use nfse_db::servicos::{Servico, Tributacao};

use super::comum::{R, exigir_cofre, gravar_e_abrir_csv, texto, vazio_para_none};
use crate::Estado;

#[derive(Serialize, Deserialize)]
pub struct ServicoDto {
    id: Option<i64>,
    empresa_id: i64,
    nome: String,
    descricao: String,
    codigo_trib_nac: String,
    codigo_trib_mun: Option<String>,
    nbs: Option<String>,
    tributacao: Tributacao,
    valor_sugerido: Option<Valor>,
}

impl From<Servico> for ServicoDto {
    fn from(s: Servico) -> Self {
        Self {
            id: s.id,
            empresa_id: s.empresa_id,
            nome: s.nome,
            descricao: s.descricao,
            codigo_trib_nac: s.codigo_trib_nac,
            codigo_trib_mun: s.codigo_trib_mun,
            nbs: s.nbs,
            tributacao: s.tributacao,
            valor_sugerido: s.valor_sugerido,
        }
    }
}

#[tauri::command]
pub async fn listar_servicos(estado: State<'_, Estado>, empresa_id: i64) -> R<Vec<ServicoDto>> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco()?
        .servicos(empresa_id)
        .await
        .map_err(texto)?
        .into_iter()
        .map(Into::into)
        .collect())
}

impl From<ServicoDto> for Servico {
    fn from(servico: ServicoDto) -> Self {
        Servico {
            id: servico.id,
            empresa_id: servico.empresa_id,
            nome: servico.nome,
            descricao: servico.descricao,
            codigo_trib_nac: servico.codigo_trib_nac.chars().filter(char::is_ascii_digit).collect(),
            codigo_trib_mun: vazio_para_none(servico.codigo_trib_mun),
            nbs: vazio_para_none(servico.nbs).map(|n| n.chars().filter(char::is_ascii_digit).collect()),
            tributacao: servico.tributacao,
            valor_sugerido: servico.valor_sugerido,
        }
    }
}

#[tauri::command]
pub async fn salvar_servico(estado: State<'_, Estado>, servico: ServicoDto) -> R<i64> {
    exigir_cofre(&estado).await?;
    estado.banco()?.salvar_servico(&servico.into()).await.map_err(texto)
}

/// Resultado da busca numa tabela oficial embutida; `fonte` é o arquivo de origem (a versão).
#[derive(Serialize)]
pub struct ListaNacionalDto {
    fonte: &'static str,
    itens: Vec<&'static ItemCodigo>,
}

/// Busca na Lista de Serviços Nacional embutida (por nome ou código), para o cadastro do serviço.
/// Não lê dados do usuário: dispensa o cofre.
#[tauri::command]
pub fn buscar_lista_servicos(termo: String) -> ListaNacionalDto {
    ListaNacionalDto {
        fonte: lista_servicos::fonte(),
        itens: lista_servicos::buscar(&termo, 30),
    }
}

/// Busca na NBS embutida (por nome ou código), para o cadastro do serviço. Não lê dados do usuário:
/// dispensa o cofre.
#[tauri::command]
pub fn buscar_nbs(termo: String) -> ListaNacionalDto {
    ListaNacionalDto {
        fonte: nbs::fonte(),
        itens: nbs::buscar(&termo, 30),
    }
}

#[derive(Serialize)]
pub struct LinhaServicoDto {
    linha: usize,
    servico: ServicoDto,
    erros: Vec<String>,
}

/// Lê a planilha de serviços para a tela conferir antes de importar.
#[tauri::command]
pub async fn ler_planilha_servicos(nome: String, bytes: Vec<u8>) -> R<Vec<LinhaServicoDto>> {
    Ok(nfse_emissor::planilha_servicos::ler(&nome, &bytes)?
        .into_iter()
        .map(|l| LinhaServicoDto {
            linha: l.numero,
            servico: l.servico.into(),
            erros: l.erros,
        })
        .collect())
}

#[derive(Serialize)]
pub struct ImportacaoServicosDto {
    empresa: String,
    novos: usize,
    atualizados: usize,
    /// Serviços cuja tributação a Sefin vai recusar para o regime desta empresa.
    avisos: Vec<String>,
}

/// Grava os serviços (da planilha ou copiados de outra empresa) em cada empresa escolhida.
/// Serviço com o mesmo nome já cadastrado é atualizado.
#[tauri::command]
pub async fn importar_servicos(
    estado: State<'_, Estado>,
    servicos: Vec<ServicoDto>,
    empresa_ids: Vec<i64>,
) -> R<Vec<ImportacaoServicosDto>> {
    exigir_cofre(&estado).await?;
    if servicos.is_empty() {
        return Err("Nenhum serviço escolhido.".into());
    }
    if empresa_ids.is_empty() {
        return Err("Escolha ao menos uma empresa.".into());
    }
    let servicos: Vec<Servico> = servicos.into_iter().map(Into::into).collect();
    let b = &estado.banco()?;
    let mut resultado = Vec::with_capacity(empresa_ids.len());
    for id in empresa_ids {
        let empresa = b.empresa(id).await.map_err(texto)?;
        let resumo = b
            .importar_servicos(id, &servicos)
            .await
            .map_err(|e| format!("{}: {e}", empresa.nome))?;
        let avisos = servicos
            .iter()
            .filter_map(|s| {
                nfse_core::modelo::conferir_total_tributos(empresa.regime.simples, &s.tributacao.total_tributos)
                    .map(|motivo| format!("{}: {motivo}", s.nome.trim()))
            })
            .collect();
        resultado.push(ImportacaoServicosDto {
            empresa: empresa.nome,
            novos: resumo.novos,
            atualizados: resumo.atualizados,
            avisos,
        });
    }
    Ok(resultado)
}

#[tauri::command]
pub async fn salvar_modelo_servicos(estado: State<'_, Estado>) -> R<String> {
    gravar_e_abrir_csv(
        &estado,
        "modelo-importacao-servicos.csv",
        &nfse_emissor::planilha_servicos::modelo_csv(),
    )
    .await
}

/// Exporta os serviços da empresa no formato da importação (para levar a outras empresas).
#[tauri::command]
pub async fn exportar_servicos(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    exigir_cofre(&estado).await?;
    let b = &estado.banco()?;
    let empresa = b.empresa(empresa_id).await.map_err(texto)?;
    let servicos = b.servicos(empresa_id).await.map_err(texto)?;
    if servicos.is_empty() {
        return Err("Esta empresa não tem serviços para exportar.".into());
    }
    let nome = format!("servicos-{}.csv", empresa.cnpj.as_str());
    gravar_e_abrir_csv(&estado, &nome, &nfse_emissor::planilha_servicos::para_csv(&servicos)).await
}

#[tauri::command]
pub async fn desativar_servico(estado: State<'_, Estado>, id: i64) -> R<()> {
    exigir_cofre(&estado).await?;
    estado.banco()?.desativar_servico(id).await.map_err(texto)
}
