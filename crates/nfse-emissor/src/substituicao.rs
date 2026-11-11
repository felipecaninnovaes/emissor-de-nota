//! Substituição da NFS-e: a nova DPS leva o grupo `subst` com a chave da nota substituída. Ao
//! autorizá-la, a Sefin cancela a original (evento 105102) e o banco a marca como `substituida`
//! (`Banco::registrar_autorizacao`); aqui ficam a conferência do pedido e o PDF da original.

use chrono::NaiveDate;

use nfse_core::Valor;
use nfse_core::modelo::{Documento, Dps, SituacaoSimples, Substituicao};
use nfse_db::Banco;
use nfse_db::dps::SituacaoDps;
use nfse_db::empresas::Empresa;

use crate::{Conector, Emissor, ErroEmissao, SubstituirNota, arquivo};

/// Confere a nota a substituir (da mesma empresa e ambiente, autorizada e sem outra substituta
/// aguardando a Sefin) e prepara a nova DPS como substituta: grupo `subst` e a competência da
/// original. Recusa, antes de reservar número, o que a Sefin rejeitaria com E0063.
pub async fn aplicar_substituicao(
    banco: &Banco,
    empresa: &Empresa,
    pedido: &SubstituirNota,
    modelo: &mut Dps,
) -> Result<(), ErroEmissao> {
    let nfse = banco.nfse(pedido.nfse_id).await?;
    let dps = banco.dps(nfse.dps_id).await?;
    let nota = nfse.numero.as_deref().unwrap_or(&nfse.chave);
    if dps.empresa_id != empresa.id {
        return Err(ErroEmissao::Regra(format!(
            "a nota {nota} a substituir é de outra empresa"
        )));
    }
    if dps.ambiente != empresa.ambiente {
        return Err(ErroEmissao::Regra(format!(
            "a nota {nota} a substituir foi emitida em outro ambiente"
        )));
    }
    if dps.situacao != SituacaoDps::Autorizada {
        return Err(ErroEmissao::Regra(format!(
            "a nota {nota} está \"{}\" e não pode ser substituída",
            dps.situacao.como_texto()
        )));
    }
    if let Some(id) = banco.substituta_em_andamento(&nfse.chave).await? {
        return Err(ErroEmissao::Regra(format!(
            "já há uma substituta da nota {nota} aguardando a Sefin (DPS {id}): verifique-a antes de emitir outra"
        )));
    }
    let original = nfse_xml::ler_nfse(&nfse.xml)?;
    // A substituta corrige a mesma prestação: fica na competência da original, não na data de hoje.
    if let Some(competencia) = original.competencia {
        modelo.competencia = competencia;
    }
    if let Some(erro) = campos_fixos_alterados(&original, modelo) {
        return Err(ErroEmissao::Regra(format!(
            "a nota {nota} não pode ser substituída assim: {erro}. Para corrigir, cancele a nota {nota} e emita outra"
        )));
    }
    modelo.substituicao = Some(Substituicao {
        chave_substituida: nfse.chave,
        motivo: pedido.motivo.clone(),
        descricao_motivo: pedido.descricao_motivo.clone(),
    });
    Ok(())
}

/// Regra da Sefin (E0063): se a original foi emitida como ME/EPP do Simples Nacional
/// (`opSimpNac` = 3) e a substituta sai como ME/EPP ou MEI, a substituta não pode mudar a
/// competência, o tomador identificado na original nem o valor do serviço. Devolve o que mudou.
pub fn campos_fixos_alterados(original: &nfse_xml::DadosNfse, nova: &Dps) -> Option<String> {
    let tomador = nova.tomador.as_ref().map(|t| match &t.documento {
        Documento::Cnpj(c) => c.as_str(),
        Documento::Cpf(c) => c.as_str(),
    });
    comparar(
        original,
        nova.prestador.regime.simples,
        nova.competencia,
        tomador,
        nova.valores.servico,
    )
}

fn comparar(
    original: &nfse_xml::DadosNfse,
    simples: SituacaoSimples,
    competencia: NaiveDate,
    tomador: Option<&str>,
    valor: Valor,
) -> Option<String> {
    if original.opcao_simples.as_deref() != Some(SituacaoSimples::MeEpp.codigo())
        || !matches!(simples, SituacaoSimples::MeEpp | SituacaoSimples::Mei)
    {
        return None;
    }
    let mut mudou = Vec::new();
    if original.competencia.is_some_and(|c| c != competencia) {
        mudou.push("a data de competência".to_owned());
    }
    if let Some(doc) = &original.documento_tomador
        && tomador != Some(doc.as_str())
    {
        mudou.push("o tomador".to_owned());
    }
    if let Some(v) = original.valor_servico
        && v != valor
    {
        mudou.push(format!("o valor do serviço (de {v} para {valor})"));
    }
    if mudou.is_empty() {
        return None;
    }
    Some(format!(
        "para empresa ME/EPP do Simples Nacional, a Sefin não aceita mudar {} na substituição (E0063)",
        juntar(&mudou)
    ))
}

fn juntar(itens: &[String]) -> String {
    match itens {
        [] => String::new(),
        [um] => um.clone(),
        [inicio @ .., ultimo] => format!("{} nem {ultimo}", inicio.join(", ")),
    }
}

impl<C: Conector> Emissor<C> {
    /// Se a DPS autorizada substituiu outra nota, refaz o PDF da original com a marca d'água.
    /// Devolve o aviso se não conseguir (a substituição já está gravada).
    pub(crate) async fn atualizar_pdf_da_substituida(
        &self,
        raiz: &std::path::Path,
        empresa: &Empresa,
        dps_id: i64,
    ) -> Option<String> {
        let chave = self
            .banco
            .modelo_dps(dps_id)
            .await
            .ok()
            .flatten()?
            .substituicao?
            .chave_substituida;
        let resultado = async {
            let id = self
                .banco
                .nfse_id_por_chave(&chave)
                .await?
                .ok_or_else(|| ErroEmissao::Regra("a nota substituída não está no banco".into()))?;
            let original = self.banco.nfse(id).await?;
            let competencia = crate::competencia_da_nfse(&original.xml)?;
            let caminho = arquivo::pasta_mes(raiz, empresa, competencia).join(arquivo::nome_xml_nfse(
                original.numero.as_deref().unwrap_or("sem-numero"),
                &original.chave,
            ));
            self.gravar_pdf(&caminho, &original.xml, nfse_pdf::Situacao::Substituida)
                .await
                .map_err(ErroEmissao::Regra)
        };
        resultado
            .await
            .err()
            .map(|e| format!("a nota {chave} foi substituída, mas o PDF dela não foi atualizado: {e}"))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const TOMADOR: &str = "11222333000181";

    fn original(opcao: &str) -> nfse_xml::DadosNfse {
        nfse_xml::DadosNfse {
            chave: String::new(),
            numero: "1".into(),
            situacao: Some("100".into()),
            id_dps: None,
            competencia: NaiveDate::from_ymd_opt(2026, 11, 8),
            valor_servico: Some(Valor::de_centesimos(100_000)),
            opcao_simples: Some(opcao.into()),
            documento_tomador: Some(TOMADOR.into()),
        }
    }

    fn dia(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
    }

    fn valor(v: &str) -> Valor {
        Valor::parse(v).unwrap()
    }

    #[test]
    fn me_epp_sem_mudanca_passa() {
        let o = original("3");
        assert_eq!(
            comparar(&o, SituacaoSimples::MeEpp, dia(8), Some(TOMADOR), valor("1000")),
            None
        );
    }

    #[test]
    fn me_epp_nao_muda_valor_competencia_nem_tomador() {
        let o = original("3");
        let erro = comparar(&o, SituacaoSimples::Mei, dia(9), Some("12ABC34501DE35"), valor("1100")).unwrap();
        assert!(
            erro.contains("a data de competência, o tomador nem o valor do serviço"),
            "{erro}"
        );
        assert!(erro.contains("E0063"), "{erro}");
        let erro = comparar(&o, SituacaoSimples::MeEpp, dia(8), None, valor("1000")).unwrap();
        assert!(
            erro.contains("mudar o tomador na"),
            "tirar o tomador também muda: {erro}"
        );
    }

    #[test]
    fn outras_opcoes_do_simples_podem_mudar() {
        let o = original("3");
        assert_eq!(
            comparar(&o, SituacaoSimples::NaoOptante, dia(8), Some(TOMADOR), valor("1100")),
            None,
            "substituta não optante"
        );
        let o = original("1");
        assert_eq!(
            comparar(&o, SituacaoSimples::MeEpp, dia(8), Some(TOMADOR), valor("1100")),
            None,
            "original não optante"
        );
    }
}
