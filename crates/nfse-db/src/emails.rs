//! Configuração do SMTP do escritório (a senha fica cifrada no cofre), dados para o e-mail de
//! uma NFS-e e o registro de cada envio.

use serde::{Deserialize, Serialize};
use sqlx::Row;
use zeroize::Zeroizing;

use nfse_core::{Dps, Valor};

use crate::{Banco, Cofre, ErroBanco, Resultado, agora, de_json, json};

const CHAVE_SMTP: &str = "smtp";
const CHAVE_SENHA_SMTP: &str = "smtp_senha";
const CONTEXTO_SENHA: &str = "senha-smtp";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seguranca {
    /// Porta 587: conexão comum que passa a cifrada (Gmail, Microsoft 365).
    #[default]
    StartTls,
    /// Porta 465: cifrada desde o início.
    Ssl,
    /// Sem cifra (só servidor na rede local).
    Nenhuma,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSmtp {
    pub servidor: String,
    pub porta: u16,
    #[serde(default)]
    pub seguranca: Seguranca,
    /// Usuário da conta (em geral o próprio e-mail); vazio = servidor sem autenticação.
    #[serde(default)]
    pub usuario: Option<String>,
    /// E-mail que aparece como remetente (a conta do escritório).
    pub remetente: String,
    /// Nome do remetente; vazio = nome da empresa emissora da nota.
    #[serde(default)]
    pub nome_remetente: Option<String>,
    pub assunto: String,
    pub corpo: String,
    /// Envia sozinho depois de cada emissão autorizada (manual, lote e planilha).
    #[serde(default)]
    pub enviar_ao_emitir: bool,
}

impl ConfigSmtp {
    pub const ASSUNTO_PADRAO: &str = "NFS-e nº {numero} - {empresa}";
    pub const CORPO_PADRAO: &str = "Prezado(a) {tomador},\n\n\
        Segue em anexo a NFS-e nº {numero} emitida por {empresa}, no valor de R$ {valor} \
        (competência {competencia}).\n\n\
        A nota também pode ser consultada em: {link}\n\n\
        Atenciosamente,\n{empresa}";
}

/// O que o e-mail de uma NFS-e precisa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosEmailNfse {
    pub nfse_id: i64,
    pub numero: Option<String>,
    pub chave: String,
    pub xml: String,
    pub cancelada: bool,
    pub empresa_id: i64,
    pub empresa: String,
    pub email_empresa: Option<String>,
    pub tomador: Option<String>,
    /// "E-mails de envio" do tomador nesta empresa ou, sem eles, o e-mail do cadastro.
    pub destinatarios: Vec<String>,
    pub valor: Option<Valor>,
    /// `AAAA-MM-DD`
    pub competencia: Option<String>,
}

/// Separa "a@x.com; b@y.com, c@z.com" em endereços.
pub fn separar_emails(texto: &str) -> Vec<String> {
    texto
        .split([';', ',', '\n'])
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex(texto: &str) -> Option<Vec<u8>> {
    texto
        .len()
        .is_multiple_of(2)
        .then(|| {
            (0..texto.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&texto[i..i + 2], 16).ok())
                .collect()
        })
        .flatten()
}

impl Banco {
    pub async fn smtp(&self) -> Resultado<Option<ConfigSmtp>> {
        self.configuracao(CHAVE_SMTP)
            .await?
            .map(|j| de_json(&j, "configuração de e-mail"))
            .transpose()
    }

    pub async fn definir_smtp(&self, cfg: &ConfigSmtp) -> Resultado<()> {
        if cfg.servidor.trim().is_empty() || cfg.remetente.trim().is_empty() || cfg.porta == 0 {
            return Err(ErroBanco::Regra("informe servidor, porta e e-mail do remetente".into()));
        }
        self.definir_configuracao(CHAVE_SMTP, &json(cfg)).await
    }

    /// Guarda a senha do SMTP cifrada com a chave do cofre.
    pub async fn definir_senha_smtp(&self, cofre: &Cofre, senha: &str) -> Resultado<()> {
        let cifrada = cofre.cifrar(senha.as_bytes(), CONTEXTO_SENHA)?;
        self.definir_configuracao(CHAVE_SENHA_SMTP, &hex(&cifrada)).await
    }

    pub async fn tem_senha_smtp(&self) -> Resultado<bool> {
        Ok(self.configuracao(CHAVE_SENHA_SMTP).await?.is_some())
    }

    pub async fn senha_smtp(&self, cofre: &Cofre) -> Resultado<Option<Zeroizing<String>>> {
        let Some(texto) = self.configuracao(CHAVE_SENHA_SMTP).await? else {
            return Ok(None);
        };
        let blob = de_hex(&texto).ok_or_else(|| ErroBanco::DadoInvalido("senha do e-mail".into()))?;
        let claro = cofre.decifrar(&blob, CONTEXTO_SENHA)?;
        let senha = String::from_utf8(claro.to_vec()).map_err(|_| ErroBanco::DadoInvalido("senha do e-mail".into()))?;
        Ok(Some(Zeroizing::new(senha)))
    }

    pub async fn dados_email_nfse(&self, nfse_id: i64) -> Resultado<DadosEmailNfse> {
        let nfse = self.nfse(nfse_id).await?;
        let r = sqlx::query(
            "SELECT d.empresa_id, d.situacao, d.modelo_json, pe.nome, pe.email, n.valor_servico, n.competencia
             FROM nfse n JOIN dps d ON d.id = n.dps_id JOIN empresa e ON e.id = d.empresa_id
             JOIN pessoa pe ON pe.id = e.pessoa_id WHERE n.id = ?",
        )
        .bind(nfse_id)
        .fetch_one(self.pool())
        .await?;
        let empresa_id: i64 = r.get(0);
        let modelo: Option<Dps> = r
            .get::<Option<String>, _>(2)
            .map(|j| de_json(&j, "modelo da DPS"))
            .transpose()?;
        let tomador = modelo.as_ref().and_then(|m| m.tomador.as_ref());

        let mut destinatarios = Vec::new();
        if let Some(t) = tomador {
            let documento = match &t.documento {
                nfse_core::modelo::Documento::Cnpj(c) => c.as_str().to_owned(),
                nfse_core::modelo::Documento::Cpf(c) => c.as_str().to_owned(),
            };
            let linha = sqlx::query(
                "SELECT v.emails, p.email FROM pessoa p
                 LEFT JOIN empresa_tomador v ON v.pessoa_id = p.id AND v.empresa_id = ?
                 WHERE p.documento = ?",
            )
            .bind(empresa_id)
            .bind(&documento)
            .fetch_optional(self.pool())
            .await?;
            let (de_envio, do_cadastro): (Option<String>, Option<String>) = match linha {
                Some(l) => (l.get(0), l.get(1)),
                None => (None, t.email.clone()),
            };
            destinatarios = de_envio
                .as_deref()
                .map(separar_emails)
                .filter(|v| !v.is_empty())
                .or_else(|| do_cadastro.as_deref().map(separar_emails))
                .unwrap_or_default();
        }

        Ok(DadosEmailNfse {
            nfse_id,
            numero: nfse.numero,
            chave: nfse.chave,
            xml: nfse.xml,
            cancelada: r.get::<String, _>(1) == "cancelada",
            empresa_id,
            empresa: r.get(3),
            email_empresa: r.get(4),
            tomador: tomador.map(|t| t.nome.clone()),
            destinatarios,
            valor: r.get::<Option<i64>, _>(5).map(Valor::de_centesimos),
            competencia: r.get(6),
        })
    }

    pub async fn registrar_envio_email(
        &self,
        nfse_id: i64,
        para: &[String],
        assunto: &str,
        erro: Option<&str>,
    ) -> Resultado<()> {
        sqlx::query("INSERT INTO envio_email (nfse_id, para, assunto, erro, criado_em) VALUES (?, ?, ?, ?, ?)")
            .bind(nfse_id)
            .bind(para.join("; "))
            .bind(assunto)
            .bind(erro)
            .bind(agora())
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::banco_com_empresa;

    #[test]
    fn separa_enderecos() {
        assert_eq!(
            separar_emails(" a@x.com; b@y.com,c@z.com ;; "),
            vec!["a@x.com", "b@y.com", "c@z.com"]
        );
    }

    #[tokio::test]
    async fn configuracao_e_senha_cifrada() {
        let (banco, cofre, _) = banco_com_empresa().await;
        assert_eq!(banco.smtp().await.unwrap(), None);
        let cfg = ConfigSmtp {
            servidor: "smtp.exemplo.com".into(),
            porta: 587,
            seguranca: Seguranca::StartTls,
            usuario: Some("notas@exemplo.com".into()),
            remetente: "notas@exemplo.com".into(),
            nome_remetente: None,
            assunto: ConfigSmtp::ASSUNTO_PADRAO.into(),
            corpo: ConfigSmtp::CORPO_PADRAO.into(),
            enviar_ao_emitir: true,
        };
        banco.definir_smtp(&cfg).await.unwrap();
        assert_eq!(banco.smtp().await.unwrap(), Some(cfg));

        assert!(!banco.tem_senha_smtp().await.unwrap());
        banco.definir_senha_smtp(&cofre, "segredo-teste").await.unwrap();
        let guardada = banco.configuracao("smtp_senha").await.unwrap().unwrap();
        assert!(!guardada.contains(&hex(b"segredo-teste")), "não fica em claro");
        assert_eq!(
            banco.senha_smtp(&cofre).await.unwrap().unwrap().as_str(),
            "segredo-teste"
        );
    }
}
