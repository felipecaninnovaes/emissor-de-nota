//! Substituição da NFS-e: a nova DPS leva o grupo `subst` com a chave da nota substituída. Ao
//! autorizá-la, a Sefin cancela a original (evento 105102) e o banco a marca como `substituida`
//! (`Banco::registrar_autorizacao`); aqui ficam a conferência do pedido e o PDF da original.

use chrono::NaiveDate;

use nfse_core::modelo::Substituicao;
use nfse_db::Banco;
use nfse_db::dps::SituacaoDps;
use nfse_db::empresas::Empresa;

use crate::{Conector, Emissor, ErroEmissao, SubstituirNota, agora_brasilia, arquivo};

/// Confere a nota a substituir (da mesma empresa e ambiente, autorizada e sem outra substituta
/// aguardando a Sefin) e devolve o grupo `subst` da nova DPS.
pub async fn substituicao_do_pedido(
    banco: &Banco,
    empresa: &Empresa,
    pedido: &SubstituirNota,
) -> Result<Substituicao, ErroEmissao> {
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
            dps.situacao.texto()
        )));
    }
    if let Some(id) = banco.substituta_em_andamento(&nfse.chave).await? {
        return Err(ErroEmissao::Regra(format!(
            "já há uma substituta da nota {nota} aguardando a Sefin (DPS {id}): verifique-a antes de emitir outra"
        )));
    }
    Ok(Substituicao {
        chave_substituida: nfse.chave,
        motivo: pedido.motivo.clone(),
        descricao_motivo: pedido.descricao_motivo.clone(),
    })
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
            let competencia = nfse_xml::ler_nfse(&original.xml)?
                .competencia
                .and_then(|c| NaiveDate::parse_from_str(&c, "%Y-%m-%d").ok())
                .unwrap_or_else(|| agora_brasilia().date_naive());
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
