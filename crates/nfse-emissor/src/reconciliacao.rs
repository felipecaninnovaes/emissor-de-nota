//! DPS sem desfecho (queda, timeout, resposta incerta): consulta à Sefin antes de qualquer reenvio,
//! e ajuste da numeração pelo que a Sefin já tem.

use chrono::{DateTime, FixedOffset, Utc};

use nfse_db::dps::{NfseGravada, RegistroDps, SituacaoDps};
use nfse_nacional::Sefin;

use crate::{Conector, Emissor, ErroEmissao};

/// Depois de um envio, a consulta da Sefin pode ainda não enxergar a DPS (ou o envio pode estar em
/// curso, até o timeout de 90 s do cliente): antes desta janela, um 404 não prova que a nota não
/// existe, e a DPS continua `incerta` em vez de ser liberada para reenvio.
pub const JANELA_RECONCILIACAO: chrono::Duration = chrono::Duration::minutes(5);

impl<C: Conector> Emissor<C> {
    /// Consulta a Sefin pelo Id da DPS: grava a NFS-e se existir, ou libera para reenvio se não.
    pub async fn reconciliar(&self, dps_id: i64) -> Result<Option<NfseGravada>, ErroEmissao> {
        let dps = self.banco.dps(dps_id).await?;
        let empresa = self.banco.empresa(dps.empresa_id).await?;
        let (_, sefin) = self.conectar(&empresa, dps.ambiente).await?;
        self.reconciliar_com(&sefin, dps_id).await
    }

    pub(crate) async fn reconciliar_com(&self, sefin: &C::S, dps_id: i64) -> Result<Option<NfseGravada>, ErroEmissao> {
        let dps = self.banco.dps(dps_id).await?;
        if !matches!(dps.situacao, SituacaoDps::Enviando | SituacaoDps::Incerta) {
            return Err(ErroEmissao::Regra(format!(
                "DPS em situação \"{}\" não precisa de reconciliação",
                dps.situacao.texto()
            )));
        }
        // `enviando` recente é um envio em curso (ex.: num lote): a resposta dele decide.
        if dps.situacao == SituacaoDps::Enviando && self.liberacao_em(&dps).is_some() {
            return Err(ErroEmissao::Regra(
                "a DPS ainda está sendo enviada à Sefin; aguarde a resposta".into(),
            ));
        }
        match sefin.consultar_dps(&dps.id_dps).await.map_err(ErroEmissao::Sefin)? {
            Some(chave) => {
                let nfse = sefin.consultar_nfse(&chave).await.map_err(ErroEmissao::Sefin)?;
                Ok(Some(self.gravar(dps_id, &nfse.chave_acesso, &nfse.xml).await?.0))
            }
            None => {
                if let Some(quando) = self.liberacao_em(&dps) {
                    return Err(ErroEmissao::Regra(format!(
                        "a Sefin ainda não mostra esta DPS, mas ela foi enviada há pouco; verifique de novo a partir das {}",
                        quando
                            .with_timezone(&FixedOffset::west_opt(3 * 3600).expect("fuso fixo"))
                            .format("%H:%M")
                    )));
                }
                self.banco.liberar_para_reenvio(dps_id).await?;
                Ok(None)
            }
        }
    }

    /// Quando a DPS sai da [`JANELA_RECONCILIACAO`], se ainda estiver dentro dela.
    fn liberacao_em(&self, dps: &RegistroDps) -> Option<DateTime<Utc>> {
        let enviada = DateTime::parse_from_rfc3339(dps.enviado_em.as_deref()?).ok()?;
        let fim = enviada.with_timezone(&Utc) + self.janela_reconciliacao;
        (Utc::now() < fim).then_some(fim)
    }

    /// Ao abrir o sistema: resolve DPS que ficaram sem desfecho. Devolve o resultado de cada uma.
    pub async fn reconciliar_pendentes(
        &self,
    ) -> Result<Vec<(i64, Result<Option<NfseGravada>, ErroEmissao>)>, ErroEmissao> {
        self.banco.liberar_reservadas().await?;
        let mut resultados = Vec::new();
        for dps in self.banco.dps_pendentes().await? {
            if dps.situacao == SituacaoDps::Enviando {
                self.banco
                    .registrar_incerta(dps.id, "o programa foi fechado durante o envio")
                    .await?;
            }
            resultados.push((dps.id, self.reconciliar(dps.id).await));
        }
        Ok(resultados)
    }

    /// Procura na Sefin o primeiro número da série atual da empresa que ainda não gerou NFS-e
    /// (outro emissor pode ter usado a mesma série: rejeição E0014) e o grava como próximo.
    /// Usa `GET /dps/{id}` em busca exponencial a partir do próximo número local e depois binária,
    /// supondo que o outro emissor numerou em sequência. Devolve (último usado, próximo).
    pub async fn ajustar_numeracao_pela_sefin(&self, empresa_id: i64) -> Result<(Option<u64>, u64), ErroEmissao> {
        const LIMITE: u64 = 999_999_999_999_999;
        let empresa = self.banco.empresa(empresa_id).await?;
        let (_, sefin) = self.conectar(&empresa, empresa.ambiente).await?;
        let inscricao = nfse_core::Inscricao::Cnpj(empresa.cnpj.clone());
        let usado = |numero: u64| {
            let id = nfse_core::dps::DpsId::novo(&empresa.municipio_emissor, &inscricao, &empresa.serie, numero);
            let sefin = &sefin;
            async move {
                let id = id.map_err(|e| ErroEmissao::Regra(e.to_string()))?;
                Ok::<_, ErroEmissao>(
                    sefin
                        .consultar_dps(id.as_str())
                        .await
                        .map_err(ErroEmissao::Sefin)?
                        .is_some(),
                )
            }
        };

        let inicio = self
            .banco
            .proximo_numero(empresa.id, empresa.ambiente, &empresa.serie)
            .await?;
        // Exponencial: `ocupado` é o maior número que sabemos usado; `livre`, um que sabemos livre.
        let mut ocupado = inicio - 1;
        let mut passo = 1;
        let mut livre = inicio;
        while usado(livre).await? {
            ocupado = livre;
            livre = inicio.saturating_add(passo).min(LIMITE);
            if livre == ocupado {
                return Err(ErroEmissao::Regra("a série não tem mais números livres".into()));
            }
            passo *= 2;
        }
        // Binária entre os dois.
        while livre - ocupado > 1 {
            let meio = ocupado + (livre - ocupado) / 2;
            if usado(meio).await? {
                ocupado = meio;
            } else {
                livre = meio;
            }
        }
        if livre != inicio {
            self.banco
                .definir_proximo_numero(empresa.id, empresa.ambiente, &empresa.serie, livre)
                .await?;
        }
        self.banco.marcar_numeracao_conferida(empresa.id).await?;
        Ok(((ocupado > 0).then_some(ocupado), livre))
    }
}
