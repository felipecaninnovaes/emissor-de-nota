//! Notas recorrentes e lote do mês.

use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Valor;
use nfse_emissor::{Origem, PedidoEmissao};

use super::comum::{R, emissor, exigir_cofre, mes_de, texto, vazio_para_none};
use super::{ResultadoDto, concluir, validar_pedido};
use crate::Estado;

#[derive(Serialize)]
pub struct TentativaDto {
    dps_id: i64,
    situacao: &'static str,
    numero_nfse: Option<String>,
    nfse_id: Option<i64>,
    erro: Option<String>,
    /// Rejeitada ou cancelada: pode emitir de novo neste mês.
    libera: bool,
}

pub(crate) fn tentativa_dto(t: nfse_db::recorrencias::TentativaNoMes) -> TentativaDto {
    TentativaDto {
        libera: t.libera_o_mes(),
        dps_id: t.dps_id,
        situacao: t.situacao.texto(),
        numero_nfse: t.numero_nfse,
        nfse_id: t.nfse_id,
        erro: t.erro,
    }
}

#[derive(Serialize)]
pub struct RecorrenciaDto {
    id: i64,
    empresa_id: i64,
    empresa: String,
    tomador_id: Option<i64>,
    tomador: Option<String>,
    servico_id: i64,
    servico: String,
    /// Valor próprio da recorrência (`None` = usa o do serviço).
    valor: Option<String>,
    valor_efetivo: Option<String>,
    descricao: Option<String>,
    /// Descrição que vai na nota, com o mês aplicado.
    descricao_no_mes: String,
    informacoes_complementares: Option<String>,
    no_mes: Option<TentativaDto>,
    /// O que impede a emissão no mês (vazio = pode emitir).
    erros: Vec<String>,
}

/// Recorrências (da empresa ou de todas, com `empresa_id` nulo) e a situação de cada uma no mês.
#[tauri::command]
pub async fn listar_recorrencias(
    estado: State<'_, Estado>,
    empresa_id: Option<i64>,
    mes: String,
) -> R<Vec<RecorrenciaDto>> {
    exigir_cofre(&estado).await?;
    let data = mes_de(&mes)?;
    let b = &estado.banco;
    let mut lista = Vec::new();
    for item in b.recorrencias(empresa_id, mes.trim()).await.map_err(texto)? {
        let r = &item.recorrencia;
        let mut erros = Vec::new();
        if !item.servico_ativo {
            erros.push(format!(
                "o serviço \"{}\" foi removido; edite a recorrência",
                item.servico
            ));
        }
        match item.valor_efetivo {
            Some(valor) if item.servico_ativo => {
                let pedido = PedidoEmissao {
                    empresa_id: r.empresa_id,
                    tomador: r.tomador,
                    servico_id: r.servico_id,
                    valor,
                    descricao: r.descricao.clone(),
                    mes_referencia: Some(data),
                    obra: None,
                    informacoes_complementares: r.informacoes_complementares.clone(),
                    origem: r.id.map(Origem::Recorrencia).unwrap_or_default(),
                };
                match validar_pedido(b, &pedido).await {
                    Ok((_, _, e)) => erros.extend(e),
                    Err(e) => erros.push(e),
                }
            }
            Some(_) => {}
            None => erros.push("sem valor".into()),
        }
        lista.push(RecorrenciaDto {
            id: r.id.expect("gravada"),
            empresa_id: r.empresa_id,
            empresa: item.empresa.clone(),
            tomador_id: r.tomador,
            tomador: item.tomador.clone(),
            servico_id: r.servico_id,
            servico: item.servico.clone(),
            valor: r.valor.map(|v| v.to_string()),
            valor_efetivo: item.valor_efetivo.map(|v| v.to_string()),
            descricao: r.descricao.clone(),
            descricao_no_mes: nfse_emissor::aplicar_mes_ref(&item.descricao_efetiva, data),
            informacoes_complementares: r.informacoes_complementares.clone(),
            no_mes: item.no_mes.map(|t| TentativaDto {
                libera: t.libera_o_mes(),
                dps_id: t.dps_id,
                situacao: t.situacao.texto(),
                numero_nfse: t.numero_nfse,
                nfse_id: t.nfse_id,
                erro: t.erro,
            }),
            erros,
        });
    }
    Ok(lista)
}

#[derive(Deserialize)]
pub struct RecorrenciaEntrada {
    id: Option<i64>,
    empresa_id: i64,
    tomador: Option<i64>,
    servico_id: i64,
    valor: Option<Valor>,
    descricao: Option<String>,
    informacoes_complementares: Option<String>,
}

#[tauri::command]
pub async fn salvar_recorrencia(estado: State<'_, Estado>, recorrencia: RecorrenciaEntrada) -> R<i64> {
    exigir_cofre(&estado).await?;
    estado
        .banco
        .salvar_recorrencia(&nfse_db::recorrencias::Recorrencia {
            id: recorrencia.id,
            empresa_id: recorrencia.empresa_id,
            tomador: recorrencia.tomador,
            servico_id: recorrencia.servico_id,
            valor: recorrencia.valor,
            descricao: vazio_para_none(recorrencia.descricao),
            informacoes_complementares: vazio_para_none(recorrencia.informacoes_complementares),
        })
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn desativar_recorrencia(estado: State<'_, Estado>, id: i64) -> R<()> {
    exigir_cofre(&estado).await?;
    estado.banco.desativar_recorrencia(id).await.map_err(texto)
}

/// Emite uma recorrência no mês (`AAAA-MM`); o lote chama um item por vez para mostrar o andamento.
#[tauri::command]
pub async fn emitir_recorrencia(estado: State<'_, Estado>, recorrencia_id: i64, mes: String) -> R<ResultadoDto> {
    let data = mes_de(&mes)?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor.emitir_recorrencia(recorrencia_id, data).await.map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}
