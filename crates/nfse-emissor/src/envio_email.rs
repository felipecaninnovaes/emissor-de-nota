//! Envio da NFS-e por e-mail (a mensagem é montada em [`crate::email`]).

use crate::{Conector, Emissor, ErroEmissao, email};

impl<C: Conector> Emissor<C> {
    /// Envia a NFS-e (PDF e XML) por e-mail. `para` vazio usa os destinatários do cadastro do
    /// tomador. Cada tentativa fica registrada. Devolve para quem foi.
    pub async fn enviar_email(
        &self,
        nfse_id: i64,
        para: &[String],
        carteiro: &impl email::Carteiro,
    ) -> Result<Vec<String>, ErroEmissao> {
        let cfg = self
            .banco
            .smtp()
            .await?
            .ok_or_else(|| ErroEmissao::Regra("configure o envio de e-mail em Configurações".into()))?;
        let senha = self.banco.senha_smtp(&self.cofre).await?;
        let dados = self.banco.dados_email_nfse(nfse_id).await?;
        let destinatarios = if para.is_empty() {
            dados.destinatarios.clone()
        } else {
            para.to_vec()
        };
        let situacao = if dados.cancelada {
            nfse_pdf::Situacao::Cancelada
        } else {
            nfse_pdf::Situacao::Normal
        };
        // Sem PDF (fontes indisponíveis), vai só o XML, que é o documento fiscal.
        let pdf = self.gerar_pdf(&dados.xml, situacao).await.ok().flatten();
        let (mensagem, assunto) = match email::montar(&cfg, &dados, &destinatarios, pdf) {
            Ok(m) => m,
            Err(e) => {
                self.banco
                    .registrar_envio_email(nfse_id, &destinatarios, &cfg.assunto, Some(&e))
                    .await?;
                return Err(ErroEmissao::Regra(e));
            }
        };
        let resultado = carteiro
            .entregar(&cfg, senha.as_deref().map(String::as_str), mensagem)
            .await;
        self.banco
            .registrar_envio_email(
                nfse_id,
                &destinatarios,
                &assunto,
                resultado.as_ref().err().map(String::as_str),
            )
            .await?;
        resultado.map_err(ErroEmissao::Regra)?;
        self.banco
            .auditar("email_enviado", Some(dados.empresa_id), Some(&destinatarios.join("; ")))
            .await?;
        Ok(destinatarios)
    }

    /// Envia uma mensagem de teste com a configuração gravada.
    pub async fn enviar_email_teste(&self, para: &str, carteiro: &impl email::Carteiro) -> Result<(), ErroEmissao> {
        let cfg = self
            .banco
            .smtp()
            .await?
            .ok_or_else(|| ErroEmissao::Regra("salve a configuração antes de testar".into()))?;
        let senha = self.banco.senha_smtp(&self.cofre).await?;
        let mensagem = email::mensagem_teste(&cfg, para).map_err(ErroEmissao::Regra)?;
        carteiro
            .entregar(&cfg, senha.as_deref().map(String::as_str), mensagem)
            .await
            .map_err(ErroEmissao::Regra)
    }

    /// Depois de uma emissão autorizada: envia se a opção estiver ligada. Devolve o que dizer ao
    /// operador (`None` = envio automático desligado ou e-mail não configurado).
    pub async fn enviar_email_apos_emissao(&self, nfse_id: i64, carteiro: &impl email::Carteiro) -> Option<String> {
        let cfg = self.banco.smtp().await.ok().flatten()?;
        if !cfg.enviar_ao_emitir {
            return None;
        }
        Some(match self.enviar_email(nfse_id, &[], carteiro).await {
            Ok(para) => format!("E-mail enviado para {}.", para.join(", ")),
            Err(e) => format!("E-mail não enviado: {e}. Reenvie em \"Notas emitidas\"."),
        })
    }
}
