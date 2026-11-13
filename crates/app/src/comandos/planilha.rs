//! Importação de planilha (convênios).

use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Valor;
use nfse_core::modelo::Tomador;
use nfse_emissor::importacao::ItemImportado;
use nfse_emissor::importacao_email::{self, EnderecoDoCep};
use nfse_emissor::{ConectorSefin, Emissor};

use super::comum::{R, emissor, gravar_e_abrir_csv, mes_de, texto};
use super::emissao::{ResultadoDto, concluir};
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
    /// Diferenças entre o e-mail e o cadastro, que não impedem a emissão (só no e-mail).
    #[serde(default)]
    avisos: Vec<String>,
    /// Pessoa física do e-mail ainda sem cadastro, cadastrada ao emitir (só no e-mail).
    #[serde(default)]
    tomador_novo: Option<Tomador>,
}

/// Andamento das consultas de CEP do e-mail, no evento `andamento-email`.
#[derive(Clone, Serialize)]
struct AndamentoEmail {
    feito: usize,
    total: usize,
}

/// A linha para a tela, com a prévia do emissor somada aos erros da leitura.
async fn dto_do_item(emissor: &Emissor<ConectorSefin>, item: ItemImportado) -> ItemPlanilhaDto {
    let mut erros = item.erros.clone();
    if erros.is_empty() {
        match emissor.validar_importado(&item).await {
            Ok(e) => erros.extend(e),
            Err(e) => erros.push(texto(e)),
        }
    }
    ItemPlanilhaDto {
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
        avisos: item.avisos,
        tomador_novo: item.tomador_novo,
    }
}

/// A linha que a tela devolve para emitir. Os erros e a tentativa anterior são conferidos de novo
/// na emissão, então não vêm da tela.
fn importado_do_dto(item: ItemPlanilhaDto) -> R<ItemImportado> {
    Ok(ItemImportado {
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
        avisos: item.avisos,
        tomador_novo: item.tomador_novo,
    })
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
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let mes_padrao = mes_de(&mes)?;
    let linhas = nfse_emissor::planilha::ler(&nome, &bytes)?;
    let b = &estado.banco()?;
    let itens = nfse_emissor::importacao::resolver(b, &linhas, empresa_id, mes_padrao)
        .await
        .map_err(texto)?;
    let mut lista = Vec::with_capacity(itens.len());
    for item in itens {
        lista.push(dto_do_item(emissor, item).await);
    }
    Ok(lista)
}

/// Endereço da consulta de CEP que serve ao tomador novo: sem município (IBGE), não serve.
fn endereco_consultado(d: crate::cep::DadosCep) -> Option<EnderecoDoCep> {
    Some(EnderecoDoCep {
        municipio: d.municipio?,
        logradouro: d.logradouro.unwrap_or_default(),
        bairro: d.bairro.unwrap_or_default(),
    })
}

/// Lê os pedidos de nota de um e-mail (`.eml`/`.msg`, em `nome` + `bytes`) ou do texto colado,
/// consulta o CEP das pessoas sem cadastro (andamento no evento `andamento-email`) e devolve as
/// linhas como as da planilha. O corpo do e-mail não é gravado nem registrado em log (spec 0001).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn ler_email(
    app: tauri::AppHandle,
    estado: State<'_, Estado>,
    nome: Option<String>,
    bytes: Option<Vec<u8>>,
    texto_colado: Option<String>,
    empresa_id: Option<i64>,
    servico_id: Option<i64>,
    mes: String,
) -> R<Vec<ItemPlanilhaDto>> {
    use tauri::Emitter;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let mes_padrao = mes_de(&mes)?;
    let corpo = match (nome, bytes) {
        (Some(nome), Some(bytes)) => nfse_emissor::leitura_email::texto_do_email(&nome, &bytes)?,
        _ => texto_colado
            .filter(|t| !t.trim().is_empty())
            .ok_or("cole o texto do e-mail")?,
    };
    let b = &estado.banco()?;
    let cnpjs: Vec<String> = b
        .empresas()
        .await
        .map_err(texto)?
        .iter()
        .map(|e| e.cnpj.as_str().to_owned())
        .collect();
    let pedidos = nfse_emissor::pedido_email::extrair(&corpo, &cnpjs)?;
    drop(corpo);

    let a_consultar = importacao_email::ceps_a_consultar(b, &pedidos).await.map_err(texto)?;
    let total = a_consultar.len();
    let mut ceps = importacao_email::CepsConsultados::new();
    for (feito, cep) in a_consultar.into_iter().enumerate() {
        // O andamento é só informativo: a tela fechada não impede a leitura.
        let _ = app.emit("andamento-email", AndamentoEmail { feito, total });
        // Falha da consulta e CEP inexistente dão o mesmo erro na linha ("confira o CEP").
        let endereco = crate::cep::consultar(&cep).await.ok().and_then(endereco_consultado);
        ceps.insert(cep, endereco);
    }
    // Idem: só informativo.
    let _ = app.emit("andamento-email", AndamentoEmail { feito: total, total });

    let itens = importacao_email::resolver(b, &pedidos, empresa_id, servico_id, mes_padrao, &ceps)
        .await
        .map_err(texto)?;
    let mut lista = Vec::with_capacity(itens.len());
    for item in itens {
        lista.push(dto_do_item(emissor, item).await);
    }
    Ok(lista)
}

/// Emite uma linha da planilha ou do e-mail (a tela chama uma por vez para mostrar o andamento).
#[tauri::command]
pub async fn emitir_planilha(
    estado: State<'_, Estado>,
    item: ItemPlanilhaDto,
    arquivo: Option<String>,
) -> R<ResultadoDto> {
    let importado = importado_do_dto(item)?;
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

#[cfg(test)]
mod testes {
    use super::*;

    fn item_json(extra: &str) -> String {
        format!(
            r#"{{"linha":3,"empresa_id":1,"empresa":"E","pessoa_id":null,"documento_tomador":"52998224725",
            "tomador":"Ana","vincular_tomador":false,"servico_id":2,"servico":"S","valor":"10.00",
            "mes":"2026-10","descricao":"D","informacoes":null,"chave":"k"{extra}}}"#
        )
    }

    #[test]
    fn item_da_planilha_sem_campos_do_email() {
        let dto: ItemPlanilhaDto = serde_json::from_str(&item_json("")).unwrap();
        let item = importado_do_dto(dto).unwrap();
        assert!(item.avisos.is_empty());
        assert!(item.tomador_novo.is_none());
        assert_eq!(item.linha, 3);
    }

    #[test]
    fn tomador_novo_do_email_chega_a_emissao() {
        let novo = r#","avisos":["o nome do e-mail difere do cadastro; vale o cadastro"],
            "tomador_novo":{"documento":{"cpf":"52998224725"},"inscricao_municipal":null,"nome":"Ana",
            "endereco":{"municipio":"3511102","cep":"15800000","logradouro":"Rua A","numero":"1",
            "complemento":null,"bairro":"Centro"},"telefone":null,"email":null}"#;
        let dto: ItemPlanilhaDto = serde_json::from_str(&item_json(novo)).unwrap();
        let item = importado_do_dto(dto).unwrap();
        let t = item.tomador_novo.expect("tomador novo");
        assert_eq!(t.nome, "Ana");
        assert_eq!(t.endereco.unwrap().municipio, "3511102");
        assert_eq!(item.avisos.len(), 1);
    }

    #[test]
    fn cep_consultado_sem_municipio_nao_serve() {
        let sem = crate::cep::DadosCep {
            cep: "15800000".into(),
            ..Default::default()
        };
        assert_eq!(endereco_consultado(sem), None);
        let com = crate::cep::DadosCep {
            cep: "15800000".into(),
            municipio: Some("3511102".into()),
            bairro: Some("Centro".into()),
            ..Default::default()
        };
        assert_eq!(
            endereco_consultado(com),
            Some(nfse_emissor::importacao_email::EnderecoDoCep {
                municipio: "3511102".into(),
                logradouro: String::new(),
                bairro: "Centro".into(),
            })
        );
    }
}
