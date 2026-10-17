//! Importação de planilha (convênios).

use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Valor;

use super::comum::{R, emissor, exigir_cofre, gravar_e_abrir_csv, mes_de, texto};
use super::emissao::{ResultadoDto, concluir, validar_pedido};
use super::recorrencias::{TentativaDto, tentativa_dto};
use crate::Estado;

#[derive(Serialize, Deserialize)]
pub struct ItemPlanilhaDto {
    linha: usize,
    empresa_id: Option<i64>,
    empresa: Option<String>,
    pessoa_id: Option<i64>,
    documento_tomador: String,
    tomador: Option<String>,
    vincular_tomador: bool,
    servico_id: Option<i64>,
    servico: Option<String>,
    valor: Option<Valor>,
    /// `AAAA-MM`
    mes: String,
    descricao: String,
    informacoes: Option<String>,
    chave: String,
    #[serde(default, skip_deserializing)]
    anterior: Option<TentativaDto>,
    #[serde(default)]
    erros: Vec<String>,
}

/// Lê a planilha e devolve cada linha resolvida contra o cadastro, com o que impede a emissão.
#[tauri::command]
pub async fn ler_planilha(
    estado: State<'_, Estado>,
    nome: String,
    bytes: Vec<u8>,
    empresa_id: Option<i64>,
    mes: String,
) -> R<Vec<ItemPlanilhaDto>> {
    exigir_cofre(&estado).await?;
    let mes_padrao = mes_de(&mes)?;
    let linhas = nfse_emissor::planilha::ler(&nome, &bytes)?;
    let b = &estado.banco()?;
    let itens = nfse_emissor::importacao::resolver(b, &linhas, empresa_id, mes_padrao)
        .await
        .map_err(texto)?;
    let mut lista = Vec::with_capacity(itens.len());
    for item in itens {
        let mut erros = item.erros.clone();
        if erros.is_empty()
            && let Some(mut pedido) = item.pedido(None)
        {
            // Tomador ainda não vinculado à empresa: confere o resto (ele é vinculado ao emitir).
            if item.vincular_tomador {
                pedido.tomador = None;
            }
            match validar_pedido(b, &pedido).await {
                Ok((_, _, e)) => erros.extend(e),
                Err(e) => erros.push(e),
            }
        }
        lista.push(ItemPlanilhaDto {
            linha: item.linha,
            empresa_id: item.empresa_id,
            empresa: item.empresa,
            pessoa_id: item.pessoa_id,
            documento_tomador: item.documento_tomador,
            tomador: item.tomador,
            vincular_tomador: item.vincular_tomador,
            servico_id: item.servico_id,
            servico: item.servico,
            valor: item.valor,
            mes: item.mes.format("%Y-%m").to_string(),
            descricao: item.descricao,
            informacoes: item.informacoes,
            chave: item.chave,
            anterior: item.anterior.map(tentativa_dto),
            erros,
        });
    }
    Ok(lista)
}

/// Emite uma linha da planilha (a tela chama uma por vez para mostrar o andamento).
#[tauri::command]
pub async fn emitir_planilha(
    estado: State<'_, Estado>,
    item: ItemPlanilhaDto,
    arquivo: Option<String>,
) -> R<ResultadoDto> {
    let importado = nfse_emissor::importacao::ItemImportado {
        linha: item.linha,
        empresa_id: item.empresa_id,
        empresa: item.empresa,
        pessoa_id: item.pessoa_id,
        documento_tomador: item.documento_tomador,
        tomador: item.tomador,
        vincular_tomador: item.vincular_tomador,
        servico_id: item.servico_id,
        servico: item.servico,
        valor: item.valor,
        mes: mes_de(&item.mes)?,
        descricao: item.descricao,
        informacoes: item.informacoes,
        chave: item.chave,
        anterior: None,
        erros: vec![],
    };
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor
        .emitir_importado(&importado, arquivo.as_deref())
        .await
        .map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}

/// Grava o modelo de planilha na pasta das notas e abre no programa padrão (Excel).
#[tauri::command]
pub async fn salvar_modelo_planilha(estado: State<'_, Estado>) -> R<String> {
    gravar_e_abrir_csv(
        &estado,
        "modelo-importacao-notas.csv",
        &nfse_emissor::planilha::modelo_csv(),
    )
    .await
}
