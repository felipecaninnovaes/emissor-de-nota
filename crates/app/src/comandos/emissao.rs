//! Pedido de emissão, prévia da DPS e emissão avulsa.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::Valor;
use nfse_core::modelo::{Ambiente, SituacaoSimples};
use nfse_db::servicos::AplicarRetencao;
use nfse_emissor::{ConectorSefin, Emissor, Origem, PedidoEmissao, ResultadoEmissao, SubstituirNota};

use super::comum::{R, emissor, exigir_cofre, texto, vazio_para_none};
use crate::Estado;

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
    /// Marcados na revisão; ausente = o cadastro do tomador.
    #[serde(default)]
    reter_federais: Option<bool>,
    #[serde(default)]
    reter_iss: Option<bool>,
    /// Nota autorizada que esta substitui.
    #[serde(default)]
    substituir: Option<SubstituirDto>,
}

#[derive(Deserialize)]
pub struct SubstituirDto {
    nfse_id: i64,
    /// `cMotivo`: 01 a 05 ou 99.
    motivo: String,
    descricao_motivo: Option<String>,
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
            reter: nfse_emissor::ReterNota {
                federais: self.reter_federais,
                iss: self.reter_iss,
            },
            origem: Origem::Manual,
            substituir: self.substituir.map(|s| SubstituirNota {
                nfse_id: s.nfse_id,
                motivo: s.motivo,
                descricao_motivo: vazio_para_none(s.descricao_motivo),
            }),
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
    /// Retenções federais (IRRF + CP + CSLL/PIS/COFINS retidos), já em centavos formatados.
    retencao_federal: Option<String>,
    /// Valor da nota menos as retenções federais (o ISS retido não entra: a Sefin o calcula).
    valor_liquido: Option<String>,
    iss_retido: bool,
    /// ISS retido (só com a alíquota do ISS conhecida); já descontado de `valor_liquido`.
    iss_retido_valor: Option<String>,
    retencao_irrf: Option<String>,
    retencao_cp: Option<String>,
    /// CSLL e, quando retidos, PIS e COFINS (somados em `vRetCSLL`).
    retencao_csll: Option<String>,
    /// PIS e COFINS de apuração própria (`vPis` e `vCofins`), quando o serviço os calcula.
    pis_proprio: Option<String>,
    cofins_proprio: Option<String>,
    ibs_cbs: Option<IbsCbsPreviaDto>,
    /// XML da DPS como seria enviado, sem assinatura (número e hora são provisórios).
    xml: Option<String>,
    /// O serviço tem retenção que depende do tomador: a tela mostra as caixas "Reter nesta nota".
    retencao_condicional: bool,
    reter_federais: bool,
    reter_iss: bool,
    erros: Vec<String>,
    /// Alertas que não impedem a emissão (retenção possivelmente indevida).
    avisos: Vec<String>,
}

/// IBS/CBS da DPS: só os códigos que o emissor envia; as alíquotas e os valores são da Sefin.
#[derive(Serialize)]
pub struct IbsCbsPreviaDto {
    indicador_operacao: String,
    cst: String,
    classificacao_tributaria: String,
    /// Valor − ISS − PIS − COFINS, como a Sefin calcula a base; estimativa para conferência.
    base_estimada: String,
}

/// Dispensa de retenção de PIS/COFINS/CSLL para pagamentos até este valor (Lei 10.833/2003, art. 31), em centavos.
const LIMITE_RETENCAO_CSLL: i64 = 21_505;

/// Avisos de retenção federal que parece indevida; o operador decide, nada é bloqueado.
fn avisos_retencao(simples: SituacaoSimples, f: &nfse_core::modelo::TributosFederais, valor_nota: i64) -> Vec<String> {
    let mut avisos = Vec::new();
    let retem_csll = f.retencao_csll.is_some_and(|v| v.centesimos() > 0);
    let retem_federal = retem_csll
        || [f.retencao_irrf, f.retencao_cp]
            .into_iter()
            .flatten()
            .any(|v| v.centesimos() > 0);
    if simples != SituacaoSimples::NaoOptante && retem_federal {
        avisos.push(
            "A empresa é optante pelo Simples Nacional: em regra não sofre retenção federal. Confira antes de emitir."
                .into(),
        );
    }
    if retem_csll && valor_nota <= LIMITE_RETENCAO_CSLL {
        avisos.push("PIS/COFINS/CSLL costumam ser dispensados de retenção em pagamentos até R$ 215,05.".into());
    }
    avisos
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
    let mut dps = nfse_emissor::montar_dps(
        &empresa,
        tomador.as_ref(),
        &servico,
        pedido,
        nfse_emissor::agora_brasilia(),
    );
    dps.leiaute = b.leiaute_dps(empresa.ambiente).await.map_err(texto)?;
    let mut erro_substituicao = None;
    if let Some(s) = &pedido.substituir {
        match nfse_emissor::substituicao_do_pedido(b, &empresa, s).await {
            Ok(subst) => dps.substituicao = Some(subst),
            Err(e) => erro_substituicao = Some(e.to_string()),
        }
    }
    let mut erros = dps.validar();
    erros.extend(erro_substituicao);
    let desconhecidas = nfse_emissor::variaveis_desconhecidas(&dps.servico.descricao);
    if !desconhecidas.is_empty() {
        erros.push(nfse_emissor::texto_variaveis_desconhecidas(&desconhecidas));
    }
    if b.certificado_resumo(empresa.id).await.map_err(texto)?.is_none() {
        erros.push("a empresa não tem certificado digital importado".into());
    }
    Ok((dps, empresa, erros))
}

/// Mostra como a DPS vai sair (descrição com o mês já aplicado) e os erros de validação, sem emitir.
#[tauri::command]
pub async fn previa_dps(estado: State<'_, Estado>, pedido: PedidoDto) -> R<PreviaDto> {
    exigir_cofre(&estado).await?;
    let mut pedido = pedido.pedido()?;
    completar_mes(&estado, &mut pedido).await?;
    let (dps, empresa, erros) = validar_pedido(&estado.banco()?, &pedido).await?;
    let servico = estado.banco()?.servico(pedido.servico_id).await.map_err(texto)?;
    let tomador = match pedido.tomador {
        Some(id) => Some(estado.banco()?.tomador(empresa.id, id).await.map_err(texto)?),
        None => None,
    };
    let t = &servico.tributacao;
    // Sem tomador não há quem retenha: as caixas "Reter nesta nota" não aparecem.
    let retencao_condicional = tomador.is_some()
        && (t.retencoes.aplicar != AplicarRetencao::Sempre || t.aplicar_retencao_iss != AplicarRetencao::Sempre);
    let f = &dps.valores.federais;
    let retido: i64 = [f.retencao_irrf, f.retencao_cp, f.retencao_csll]
        .into_iter()
        .flatten()
        .map(|v| v.centesimos())
        .sum();
    let valor_nota = dps.valores.servico;
    let issqn = &dps.valores.issqn;
    let iss_valor = issqn
        .aliquota
        .filter(|_| issqn.tributacao == nfse_core::modelo::TributacaoIssqn::Tributavel)
        .map(|a| valor_nota.aplicar(a));
    let iss_retido = issqn.retencao != nfse_core::modelo::RetencaoIssqn::NaoRetido;
    let iss_retido_valor = iss_valor.filter(|_| iss_retido);
    let (pis, cofins) = f
        .pis_cofins
        .as_ref()
        .map_or((None, None), |pc| (pc.valor_pis, pc.valor_cofins));
    let ibs_cbs = dps.ibs_cbs.as_ref().map(|i| IbsCbsPreviaDto {
        indicador_operacao: i.indicador_operacao.clone(),
        cst: i.cst.clone(),
        classificacao_tributaria: i.classificacao_tributaria.clone(),
        base_estimada: Valor::de_centesimos(
            valor_nota.centesimos()
                - [iss_valor, pis, cofins]
                    .into_iter()
                    .flatten()
                    .map(|v| v.centesimos())
                    .sum::<i64>(),
        )
        .to_string(),
    });
    let xml = nfse_xml::gerar_dps(&dps).ok().map(|(xml, _)| xml);
    let formatado = |v: Option<Valor>| v.filter(|v| v.centesimos() > 0).map(|v| v.to_string());
    let (reter_federais, reter_iss) = pedido.reter.efetiva(tomador.as_ref());
    let avisos = avisos_retencao(empresa.regime.simples, f, dps.valores.servico.centesimos());
    Ok(PreviaDto {
        ambiente: dps.ambiente,
        serie: dps.serie,
        prestador: format!("{} ({})", empresa.nome, empresa.cnpj),
        tomador: dps.tomador.map(|t| t.nome),
        descricao: dps.servico.descricao,
        valor: dps.valores.servico.to_string(),
        competencia: dps.competencia.format("%d/%m/%Y").to_string(),
        retencao_federal: (retido > 0).then(|| Valor::de_centesimos(retido).to_string()),
        valor_liquido: (retido > 0 || iss_retido_valor.is_some()).then(|| {
            let iss = iss_retido_valor.map_or(0, Valor::centesimos);
            Valor::de_centesimos(dps.valores.servico.centesimos() - retido - iss).to_string()
        }),
        iss_retido,
        iss_retido_valor: formatado(iss_retido_valor),
        retencao_irrf: formatado(f.retencao_irrf),
        retencao_cp: formatado(f.retencao_cp),
        retencao_csll: formatado(f.retencao_csll),
        pis_proprio: formatado(pis),
        cofins_proprio: formatado(cofins),
        ibs_cbs,
        xml,
        retencao_condicional,
        reter_federais,
        reter_iss,
        erros,
        avisos,
    })
}

/// Mês de referência que as telas sugerem: `atual` (padrão) ou `anterior` (para quem cobra o mês
/// que passou, como mensalidades). Só muda o texto da nota (`{mes_ref}`); a competência é a data da
/// emissão (ADR 0004).
const CHAVE_MES_PADRAO: &str = "mes_referencia_padrao";

async fn mes_padrao(estado: &Estado) -> R<&'static str> {
    let gravado = estado.banco()?.configuracao(CHAVE_MES_PADRAO).await.map_err(texto)?;
    Ok(if gravado.as_deref() == Some("anterior") {
        "anterior"
    } else {
        "atual"
    })
}

/// Mês de referência apagado na tela: vale o sugerido na configuração (e não o padrão da biblioteca).
async fn completar_mes(estado: &Estado, pedido: &mut PedidoEmissao) -> R<()> {
    if pedido.mes_referencia.is_none() {
        let hoje = nfse_emissor::agora_brasilia().date_naive();
        pedido.mes_referencia = Some(primeiro_dia(hoje, mes_padrao(estado).await? == "anterior"));
    }
    Ok(())
}

/// Dia 1 do mês de `data` ou, com `anterior`, do mês antes dele.
fn primeiro_dia(data: NaiveDate, anterior: bool) -> NaiveDate {
    let inicio = data.with_day(1).expect("dia 1 existe");
    if !anterior {
        return inicio;
    }
    inicio
        .pred_opt()
        .expect("data válida")
        .with_day(1)
        .expect("dia 1 existe")
}

#[tauri::command]
pub async fn obter_mes_padrao(estado: State<'_, Estado>) -> R<String> {
    exigir_cofre(&estado).await?;
    Ok(mes_padrao(&estado).await?.into())
}

#[tauri::command]
pub async fn definir_mes_padrao(estado: State<'_, Estado>, mes: String) -> R<()> {
    exigir_cofre(&estado).await?;
    if !matches!(mes.as_str(), "atual" | "anterior") {
        return Err("mês de referência padrão inválido (atual ou anterior)".into());
    }
    estado
        .banco()?
        .definir_configuracao(CHAVE_MES_PADRAO, &mes)
        .await
        .map_err(texto)
}

#[derive(Serialize)]
pub struct ResultadoDto {
    autorizada: bool,
    nfse_id: Option<i64>,
    numero: Option<String>,
    chave: Option<String>,
    avisos: Vec<String>,
    /// Quando incerta: o que aconteceu e a DPS a verificar na Sefin.
    motivo: Option<String>,
    dps_id: Option<i64>,
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
                dps_id: None,
            },
            ResultadoEmissao::Incerta { dps_id, motivo } => ResultadoDto {
                autorizada: false,
                nfse_id: None,
                numero: None,
                chave: None,
                avisos: vec![],
                motivo: Some(format!(
                    "{motivo}. A nota pode ter sido autorizada: o sistema consulta a Sefin antes de qualquer reenvio."
                )),
                dps_id: Some(dps_id),
            },
        }
    }
}

/// Converte o resultado e, se autorizada e o envio automático estiver ligado, manda o e-mail
/// (o resultado do envio entra nos avisos; falha no e-mail não desfaz a nota).
pub(crate) async fn concluir(emissor: &Emissor<ConectorSefin>, resultado: ResultadoEmissao) -> ResultadoDto {
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
    exigir_cofre(&estado).await?;
    let mut pedido = pedido.pedido()?;
    completar_mes(&estado, &mut pedido).await?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor.emitir(&pedido).await.map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}

#[cfg(test)]
mod testes {
    use super::*;
    use nfse_core::modelo::TributosFederais;

    fn csll(centavos: i64) -> TributosFederais {
        TributosFederais {
            retencao_csll: Some(Valor::de_centesimos(centavos)),
            ..Default::default()
        }
    }

    #[test]
    fn primeiro_dia_do_mes_atual_ou_anterior() {
        let d = |a, m, dia| NaiveDate::from_ymd_opt(a, m, dia).unwrap();
        assert_eq!(primeiro_dia(d(2026, 10, 20), false), d(2026, 10, 1));
        assert_eq!(primeiro_dia(d(2026, 10, 20), true), d(2026, 9, 1));
        assert_eq!(primeiro_dia(d(2026, 1, 1), true), d(2025, 12, 1));
    }

    #[test]
    fn avisa_retencao_no_simples_e_abaixo_do_limite() {
        let f = csll(1_000);
        assert!(avisos_retencao(SituacaoSimples::NaoOptante, &f, 100_000).is_empty());
        assert_eq!(avisos_retencao(SituacaoSimples::MeEpp, &f, 100_000).len(), 1);
        assert_eq!(avisos_retencao(SituacaoSimples::NaoOptante, &f, 21_505).len(), 1);
        assert_eq!(avisos_retencao(SituacaoSimples::Mei, &f, 10_000).len(), 2);
        assert!(avisos_retencao(SituacaoSimples::Mei, &TributosFederais::default(), 10_000).is_empty());
    }
}
