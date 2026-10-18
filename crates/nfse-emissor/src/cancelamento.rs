//! Cancelamento da NFS-e (evento e101101), feito aqui ou detectado na Sefin.

use chrono::NaiveDate;

use nfse_core::modelo::Documento;
use nfse_db::dps::{NfseGravada, SituacaoDps};
use nfse_db::empresas::Empresa;
use nfse_nacional::{ErroSefin, Sefin};
use nfse_xml::{MotivoCancelamento, PedidoCancelamento};

use crate::{Conector, Emissor, ErroEmissao, agora_brasilia, arquivo, assinar_em_thread, mensagem_rejeicao};

/// Tipo do evento de cancelamento da NFS-e (e101101).
const EVENTO_CANCELAMENTO: &str = "101101";

impl<C: Conector> Emissor<C> {
    pub async fn cancelar(
        &self,
        nfse_id: i64,
        motivo: MotivoCancelamento,
        justificativa: &str,
    ) -> Result<(), ErroEmissao> {
        let nfse = self.banco.nfse(nfse_id).await?;
        let dps = self.banco.dps(nfse.dps_id).await?;
        if dps.situacao != SituacaoDps::Autorizada {
            return Err(ErroEmissao::Regra(format!(
                "a nota está \"{}\" e não pode ser cancelada",
                dps.situacao.texto()
            )));
        }
        let empresa = self.banco.empresa(dps.empresa_id).await?;
        let (cert, sefin) = self.conectar(&empresa, dps.ambiente).await?;
        let pedido = PedidoCancelamento {
            ambiente: dps.ambiente,
            versao_aplicativo: self.versao_aplicativo.clone(),
            quando: agora_brasilia(),
            autor: Documento::Cnpj(empresa.cnpj.clone()),
            chave: nfse.chave.clone(),
            motivo,
            descricao: justificativa.to_owned(),
        };
        let (xml, id) = nfse_xml::gerar_pedido_cancelamento(&pedido)?;
        let assinado = assinar_em_thread(xml, id, cert.clone()).await?;
        // O Id do pedido é fixo por chave e tipo de evento: repetir após falha não duplica.
        let evento = match sefin.registrar_evento(&nfse.chave, &assinado).await {
            Ok(evento) => evento,
            // Rejeitado (ex.: já cancelada no portal) ou sem resposta: a Sefin pode já ter o evento.
            Err(e) => match self.evento_de_cancelamento(&sefin, &nfse.chave).await {
                Some(evento) => evento,
                None => {
                    return Err(match e {
                        ErroSefin::Rejeitada { erros, .. } => ErroEmissao::Rejeitada {
                            dps_id: None,
                            mensagem: mensagem_rejeicao(&erros),
                        },
                        e @ (ErroSefin::Incerto(_) | ErroSefin::Http { .. } | ErroSefin::Formato(_)) => {
                            ErroEmissao::Regra(format!(
                                "a Sefin não confirmou o cancelamento ({e}); ele pode ter sido registrado. \
                                 Tente cancelar de novo: o pedido repetido não duplica o evento"
                            ))
                        }
                        outro => ErroEmissao::Sefin(outro),
                    });
                }
            },
        };
        self.registrar_cancelamento(&empresa, &nfse, &evento).await
    }

    /// Confere na Sefin se a nota autorizada foi cancelada fora do sistema (ex.: no portal
    /// nacional) e, se foi, registra o evento como no cancelamento feito aqui. Devolve se cancelou.
    pub async fn sincronizar_cancelamento(&self, nfse_id: i64) -> Result<bool, ErroEmissao> {
        let nfse = self.banco.nfse(nfse_id).await?;
        let dps = self.banco.dps(nfse.dps_id).await?;
        match dps.situacao {
            SituacaoDps::Autorizada => {}
            SituacaoDps::Cancelada => return Ok(false),
            outra => {
                return Err(ErroEmissao::Regra(format!(
                    "a nota está \"{}\" e não tem o que sincronizar",
                    outra.texto()
                )));
            }
        }
        let empresa = self.banco.empresa(dps.empresa_id).await?;
        let (_, sefin) = self.conectar(&empresa, dps.ambiente).await?;
        let evento = sefin
            .consultar_evento(&nfse.chave, EVENTO_CANCELAMENTO, 1)
            .await
            .map_err(ErroEmissao::Sefin)?;
        let Some(evento) = evento else {
            return Ok(false);
        };
        if !nfse_xml::e_cancelamento_da_nfse(&evento, &nfse.chave) {
            return Err(ErroEmissao::Regra(format!(
                "a Sefin devolveu um evento que não é o cancelamento da NFS-e {}",
                nfse.chave
            )));
        }
        self.registrar_cancelamento(&empresa, &nfse, &evento).await?;
        Ok(true)
    }

    /// Evento de cancelamento já registrado na Sefin para a chave, se a consulta o encontrar.
    async fn evento_de_cancelamento(&self, sefin: &C::S, chave: &str) -> Option<String> {
        let evento = sefin.consultar_evento(chave, EVENTO_CANCELAMENTO, 1).await.ok()??;
        nfse_xml::e_cancelamento_da_nfse(&evento, chave).then_some(evento)
    }

    /// Grava o evento homologado, marca a nota como cancelada e refaz o PDF com a marca d'água.
    /// Depois da gravação, falha ao arquivar vira só aviso na auditoria.
    async fn registrar_cancelamento(
        &self,
        empresa: &Empresa,
        nfse: &NfseGravada,
        evento: &str,
    ) -> Result<(), ErroEmissao> {
        self.banco.registrar_cancelamento(nfse.id, evento).await?;

        if let Some(raiz) = &self.pasta {
            let competencia = nfse_xml::ler_nfse(&nfse.xml)?
                .competencia
                .and_then(|c| NaiveDate::parse_from_str(&c, "%Y-%m-%d").ok())
                .unwrap_or_else(|| agora_brasilia().date_naive());
            let caminho = arquivo::pasta_mes(raiz, empresa, competencia).join(arquivo::nome_xml_evento(
                nfse.numero.as_deref().unwrap_or("sem-numero"),
                &nfse.chave,
                "cancelamento",
            ));
            if let Err(e) = arquivo::gravar(&caminho, evento) {
                let aviso = format!("cancelamento registrado, mas não gravado em {}: {e}", caminho.display());
                let _ = self.banco.auditar("aviso", Some(empresa.id), Some(&aviso)).await;
            }
            // O PDF da nota passa a ter a marca d'água CANCELADA.
            let caminho_xml = arquivo::pasta_mes(raiz, empresa, competencia).join(arquivo::nome_xml_nfse(
                nfse.numero.as_deref().unwrap_or("sem-numero"),
                &nfse.chave,
            ));
            if let Err(e) = self
                .gravar_pdf(&caminho_xml, &nfse.xml, nfse_pdf::Situacao::Cancelada)
                .await
            {
                let aviso = format!("cancelamento registrado, mas o PDF não foi atualizado: {e}");
                let _ = self.banco.auditar("aviso", Some(empresa.id), Some(&aviso)).await;
            }
        }
        Ok(())
    }
}
