//! Comandos chamados pela interface. Erros voltam como texto pronto para mostrar ao operador.

use std::path::PathBuf;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::modelo::Ambiente;
use nfse_core::{Cnpj, Valor};
use nfse_db::dps::SituacaoDps;
use nfse_emissor::{ConectorSefin, Emissor, Origem, PedidoEmissao, ResultadoEmissao};
use nfse_xml::MotivoCancelamento;

use crate::Estado;

pub(crate) mod aparencia;
pub(crate) mod atualizacao;
pub(crate) mod cofre;
mod comum;
pub(crate) mod email;
pub(crate) mod empresas;
pub(crate) mod planilha;
pub(crate) mod recorrencias;
pub(crate) mod servicos;
pub(crate) mod tomadores;

use tomadores::TomadorDto;

use comum::{R, abrir_no_sistema, emissor, exigir_cofre, pasta_configurada, texto, vazio_para_none};

// ---------- emissão e notas ----------

#[derive(Deserialize)]
pub struct PedidoDto {
    empresa_id: i64,
    tomador: Option<i64>,
    servico_id: i64,
    valor: Valor,
    descricao: Option<String>,
    /// `AAAA-MM`
    mes_referencia: Option<String>,
    #[serde(default)]
    obra: Option<nfse_core::modelo::Obra>,
    informacoes_complementares: Option<String>,
}

impl PedidoDto {
    fn pedido(self) -> R<PedidoEmissao> {
        let mes_referencia = match vazio_para_none(self.mes_referencia) {
            Some(m) => Some(
                NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").map_err(|_| "mês de referência inválido")?,
            ),
            None => None,
        };
        Ok(PedidoEmissao {
            empresa_id: self.empresa_id,
            tomador: self.tomador,
            servico_id: self.servico_id,
            valor: self.valor,
            descricao: vazio_para_none(self.descricao),
            mes_referencia,
            obra: self.obra,
            informacoes_complementares: vazio_para_none(self.informacoes_complementares),
            origem: Origem::Manual,
        })
    }
}

#[derive(Serialize)]
pub struct PreviaDto {
    ambiente: Ambiente,
    serie: String,
    prestador: String,
    tomador: Option<String>,
    descricao: String,
    valor: String,
    competencia: String,
    erros: Vec<String>,
}

/// Monta a DPS do pedido como ela sairia agora e lista os erros que impedem a emissão.
pub(crate) async fn validar_pedido(
    b: &nfse_db::Banco,
    pedido: &PedidoEmissao,
) -> R<(nfse_core::Dps, nfse_db::empresas::Empresa, Vec<String>)> {
    let empresa = b.empresa(pedido.empresa_id).await.map_err(texto)?;
    let servico = b.servico(pedido.servico_id).await.map_err(texto)?;
    let tomador = match pedido.tomador {
        Some(id) => Some(b.tomador(empresa.id, id).await.map_err(texto)?),
        None => None,
    };
    let dps = nfse_emissor::montar_dps(
        &empresa,
        tomador.as_ref(),
        &servico,
        pedido,
        nfse_emissor::agora_brasilia(),
    );
    let mut erros = dps.validar();
    if b.certificado_resumo(empresa.id).await.map_err(texto)?.is_none() {
        erros.push("a empresa não tem certificado digital importado".into());
    }
    Ok((dps, empresa, erros))
}

/// Mostra como a DPS vai sair (descrição com o mês já aplicado) e os erros de validação, sem emitir.
#[tauri::command]
pub async fn previa_dps(estado: State<'_, Estado>, pedido: PedidoDto) -> R<PreviaDto> {
    exigir_cofre(&estado).await?;
    let pedido = pedido.pedido()?;
    let (dps, empresa, erros) = validar_pedido(&estado.banco, &pedido).await?;
    Ok(PreviaDto {
        ambiente: dps.ambiente,
        serie: dps.serie,
        prestador: format!("{} ({})", empresa.nome, empresa.cnpj),
        tomador: dps.tomador.map(|t| t.nome),
        descricao: dps.servico.descricao,
        valor: dps.valores.servico.to_string(),
        competencia: dps.competencia.format("%d/%m/%Y").to_string(),
        erros,
    })
}

#[derive(Serialize)]
pub struct ResultadoDto {
    autorizada: bool,
    nfse_id: Option<i64>,
    numero: Option<String>,
    chave: Option<String>,
    avisos: Vec<String>,
    /// Quando incerta: o que aconteceu.
    motivo: Option<String>,
}

impl From<ResultadoEmissao> for ResultadoDto {
    fn from(r: ResultadoEmissao) -> Self {
        match r {
            ResultadoEmissao::Autorizada { nfse, avisos } => ResultadoDto {
                autorizada: true,
                nfse_id: Some(nfse.id),
                numero: nfse.numero,
                chave: Some(nfse.chave),
                avisos,
                motivo: None,
            },
            ResultadoEmissao::Incerta { motivo, .. } => ResultadoDto {
                autorizada: false,
                nfse_id: None,
                numero: None,
                chave: None,
                avisos: vec![],
                motivo: Some(format!(
                    "{motivo}. A nota pode ter sido autorizada: o sistema consulta a Sefin antes de qualquer reenvio."
                )),
            },
        }
    }
}

/// Converte o resultado e, se autorizada e o envio automático estiver ligado, manda o e-mail
/// (o resultado do envio entra nos avisos; falha no e-mail não desfaz a nota).
async fn concluir(emissor: &Emissor<ConectorSefin>, resultado: ResultadoEmissao) -> ResultadoDto {
    let nfse_id = match &resultado {
        ResultadoEmissao::Autorizada { nfse, .. } => Some(nfse.id),
        ResultadoEmissao::Incerta { .. } => None,
    };
    let mut dto = ResultadoDto::from(resultado);
    if let Some(id) = nfse_id
        && let Some(aviso) = emissor
            .enviar_email_apos_emissao(id, &nfse_emissor::email::CarteiroSmtp)
            .await
    {
        dto.avisos.push(aviso);
    }
    dto
}

#[tauri::command]
pub async fn emitir(estado: State<'_, Estado>, pedido: PedidoDto) -> R<ResultadoDto> {
    let pedido = pedido.pedido()?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor.emitir(&pedido).await.map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}

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

/// Consulta pública do CNPJ para sugerir nome, endereço e contato no cadastro.
#[tauri::command]
pub async fn consultar_cnpj(cnpj: String) -> R<crate::cnpj::DadosCnpj> {
    let cnpj = Cnpj::parse(&cnpj).map_err(|e| format!("CNPJ: {e}"))?;
    crate::cnpj::consultar(cnpj.as_str()).await
}

/// Consulta pública do CEP para sugerir logradouro, bairro e município.
#[tauri::command]
pub async fn consultar_cep(cep: String) -> R<crate::cep::DadosCep> {
    let cep: String = cep.chars().filter(char::is_ascii_digit).collect();
    if cep.len() != 8 {
        return Err("CEP com 8 dígitos".into());
    }
    crate::cep::consultar(&cep).await
}
