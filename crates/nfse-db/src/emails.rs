//! Configuração do SMTP que envia as notas (a senha fica cifrada no cofre), dados para o e-mail de
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
    /// E-mail que aparece como remetente (a conta que envia as notas).
    pub remetente: String,
    /// Nome do remetente; vazio = nome da empresa emissora da nota.
    #[serde(default)]
    pub nome_remetente: Option<String>,
    pub assunto: String,
    pub corpo: String,
    /// Envia sozinho depois de cada emissão autorizada (manual, lote e planilha).
    #[serde(default)]
    pub enviar_ao_emitir: bool,
    /// Assunto e corpo do e-mail de nota cancelada (configuração anterior: o texto padrão).
    #[serde(default = "assunto_cancelamento_padrao")]
    pub assunto_cancelamento: String,
    #[serde(default = "corpo_cancelamento_padrao")]
    pub corpo_cancelamento: String,
}

fn assunto_cancelamento_padrao() -> String {
    ConfigSmtp::ASSUNTO_CANCELAMENTO_PADRAO.into()
}

fn corpo_cancelamento_padrao() -> String {
    ConfigSmtp::CORPO_CANCELAMENTO_PADRAO.into()
}

impl ConfigSmtp {
    pub const ASSUNTO_CANCELAMENTO_PADRAO: &str = "CANCELAMENTO da NFS-e nº {numero} - {empresa}";
    pub const CORPO_CANCELAMENTO_PADRAO: &str = "Prezado(a) {tomador},\n\n\
        Informamos que a NFS-e nº {numero} emitida por {empresa}, no valor de R$ {valor} \
        (competência {competencia}), foi CANCELADA.\n\n\
        Motivo: {motivo}\nJustificativa: {justificativa}\n\n\
        Seguem em anexo o PDF e o XML da nota cancelada. A situação pode ser consultada em: {link}\n\n\
        Atenciosamente,\n{empresa}";

    /// Configuração em branco, com os textos padrão (o que a tela mostra antes de salvar).
    pub fn padrao() -> Self {
        ConfigSmtp {
            servidor: String::new(),
            porta: 587,
            seguranca: Seguranca::StartTls,
            usuario: None,
            remetente: String::new(),
            nome_remetente: None,
            assunto: Self::ASSUNTO_PADRAO.into(),
            corpo: Self::CORPO_PADRAO.into(),
            enviar_ao_emitir: false,
            assunto_cancelamento: Self::ASSUNTO_CANCELAMENTO_PADRAO.into(),
            corpo_cancelamento: Self::CORPO_CANCELAMENTO_PADRAO.into(),
        }
    }

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
    /// XML do evento de cancelamento gravado (de onde saem o motivo e a justificativa).
    pub xml_cancelamento: Option<String>,
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

/// Sem cifra, usuário e senha iriam abertos pela rede: só vale para servidor local sem login.
pub fn conferir_seguranca(cfg: &ConfigSmtp) -> Option<&'static str> {
    let com_login = cfg.usuario.as_deref().is_some_and(|u| !u.trim().is_empty());
    (cfg.seguranca == Seguranca::Nenhuma && com_login)
        .then_some("conexão sem cifra não aceita usuário e senha (iriam abertos pela rede); use STARTTLS ou SSL")
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
        if let Some(e) = conferir_seguranca(cfg) {
            return Err(ErroBanco::Regra(e.into()));
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

    /// Para quem vai o e-mail da nota: os e-mails de envio do vínculo do tomador com a empresa;
    /// sem eles, o e-mail do cadastro do tomador (ou o da DPS, se ele não estiver cadastrado).
    /// Não lê o XML da nota (o diálogo do envio em lote pede os de várias notas de uma vez).
    pub async fn destinatarios_nfse(&self, nfse_id: i64) -> Resultado<Vec<String>> {
        let r =
            sqlx::query("SELECT d.empresa_id, d.modelo_json FROM nfse n JOIN dps d ON d.id = n.dps_id WHERE n.id = ?")
                .bind(nfse_id)
                .fetch_optional(self.pool())
                .await?
                .ok_or(ErroBanco::NaoEncontrado("NFS-e"))?;
        let modelo: Option<Dps> = r
            .get::<Option<String>, _>(1)
            .map(|j| de_json(&j, "modelo da DPS"))
            .transpose()?;
        self.destinatarios_do_tomador(r.get(0), modelo.as_ref().and_then(|m| m.tomador.as_ref()))
            .await
    }

    async fn destinatarios_do_tomador(
        &self,
        empresa_id: i64,
        tomador: Option<&nfse_core::modelo::Tomador>,
    ) -> Resultado<Vec<String>> {
        let Some(t) = tomador else {
            return Ok(Vec::new());
        };
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
        Ok(de_envio
            .as_deref()
            .map(separar_emails)
            .filter(|v| !v.is_empty())
            .or_else(|| do_cadastro.as_deref().map(separar_emails))
            .unwrap_or_default())
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
        let destinatarios = self.destinatarios_do_tomador(empresa_id, tomador).await?;

        let xml_cancelamento: Option<String> = sqlx::query_scalar(
            "SELECT xml FROM evento WHERE nfse_id = ? AND tipo = 'cancelamento' ORDER BY id DESC LIMIT 1",
        )
        .bind(nfse_id)
        .fetch_optional(self.pool())
        .await?;

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
            xml_cancelamento,
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
            usuario: Some("notas@exemplo.com".into()),
            remetente: "notas@exemplo.com".into(),
            enviar_ao_emitir: true,
            ..ConfigSmtp::padrao()
        };
        banco.definir_smtp(&cfg).await.unwrap();
        assert_eq!(banco.smtp().await.unwrap(), Some(cfg.clone()));

        // Sem cifra, só sem login (servidor local); com usuário, a senha iria aberta.
        let aberto = ConfigSmtp {
            seguranca: Seguranca::Nenhuma,
            ..cfg.clone()
        };
        assert!(matches!(banco.definir_smtp(&aberto).await, Err(ErroBanco::Regra(_))));
        banco
            .definir_smtp(&ConfigSmtp {
                usuario: None,
                ..aberto
            })
            .await
            .unwrap();

        assert!(!banco.tem_senha_smtp().await.unwrap());
        banco.definir_senha_smtp(&cofre, "segredo-teste").await.unwrap();
        let guardada = banco.configuracao("smtp_senha").await.unwrap().unwrap();
        assert!(!guardada.contains(&hex(b"segredo-teste")), "não fica em claro");
        assert_eq!(
            banco.senha_smtp(&cofre).await.unwrap().unwrap().as_str(),
            "segredo-teste"
        );
    }

    /// Configuração gravada antes do modelo de cancelamento: lê com o texto padrão.
    #[tokio::test]
    async fn config_antiga_le_modelo_de_cancelamento_padrao() {
        let (banco, _, _) = banco_com_empresa().await;
        banco
            .definir_configuracao(
                CHAVE_SMTP,
                r#"{"servidor":"smtp.exemplo.com","porta":587,"remetente":"notas@exemplo.com",
                    "assunto":"Nota {numero}","corpo":"Segue a nota."}"#,
            )
            .await
            .unwrap();
        let cfg = banco.smtp().await.unwrap().unwrap();
        assert_eq!(cfg.assunto, "Nota {numero}");
        assert_eq!(cfg.assunto_cancelamento, ConfigSmtp::ASSUNTO_CANCELAMENTO_PADRAO);
        assert_eq!(cfg.corpo_cancelamento, ConfigSmtp::CORPO_CANCELAMENTO_PADRAO);
        assert!(ConfigSmtp::CORPO_CANCELAMENTO_PADRAO.contains("{motivo}"));
        assert!(ConfigSmtp::CORPO_CANCELAMENTO_PADRAO.contains("{justificativa}"));
    }

    #[tokio::test]
    async fn dados_email_traz_evento_de_cancelamento() {
        use crate::dps::testes::{modelo_para, nfse_recebida};

        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        let nfse = banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        let dados = banco.dados_email_nfse(nfse.id).await.unwrap();
        assert!(!dados.cancelada);
        assert_eq!(dados.xml_cancelamento, None);

        banco
            .registrar_cancelamento(nfse.id, "<evento>1</evento>")
            .await
            .unwrap();
        let dados = banco.dados_email_nfse(nfse.id).await.unwrap();
        assert!(dados.cancelada);
        assert_eq!(dados.xml_cancelamento.as_deref(), Some("<evento>1</evento>"));
    }

    /// Nota autorizada com o tomador da DPS de exemplo, com o e-mail dado (ou sem e-mail).
    async fn nota_com_tomador(
        banco: &Banco,
        empresa_id: i64,
        email: Option<&str>,
    ) -> (i64, nfse_core::modelo::Tomador) {
        use crate::dps::testes::{modelo_para, nfse_recebida};

        let r = banco.reservar_dps(empresa_id).await.unwrap();
        let mut modelo = modelo_para(&r);
        let tomador = modelo.tomador.as_mut().expect("o exemplo tem tomador");
        tomador.email = email.map(Into::into);
        let tomador = tomador.clone();
        banco.registrar_envio(r.id, &modelo, "<DPS/>").await.unwrap();
        let nfse = banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        (nfse.id, tomador)
    }

    /// Os destinatários mostrados antes do envio em lote são os mesmos para os quais o envio vai.
    async fn conferir_destinatarios(banco: &Banco, nfse_id: i64, esperado: &[&str]) {
        let para = banco.destinatarios_nfse(nfse_id).await.unwrap();
        assert_eq!(para, esperado);
        assert_eq!(banco.dados_email_nfse(nfse_id).await.unwrap().destinatarios, para);
    }

    #[tokio::test]
    async fn destinatarios_nfse_sem_cadastro_usa_o_email_da_dps() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let (id, _) = nota_com_tomador(&banco, empresa.id, Some("dps@cliente.com")).await;
        conferir_destinatarios(&banco, id, &["dps@cliente.com"]).await;
    }

    #[tokio::test]
    async fn destinatarios_nfse_sem_email_fica_vazio() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let (id, _) = nota_com_tomador(&banco, empresa.id, None).await;
        conferir_destinatarios(&banco, id, &[]).await;
    }

    #[tokio::test]
    async fn destinatarios_nfse_prefere_os_emails_de_envio_ao_do_cadastro() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let (id, tomador) = nota_com_tomador(&banco, empresa.id, Some("cadastro@cliente.com")).await;
        let pessoa = banco.salvar_tomador(empresa.id, &tomador, "teste").await.unwrap();
        conferir_destinatarios(&banco, id, &["cadastro@cliente.com"]).await;

        banco
            .definir_emails_tomador(empresa.id, pessoa, Some("a@cliente.com; b@cliente.com"), None)
            .await
            .unwrap();
        conferir_destinatarios(&banco, id, &["a@cliente.com", "b@cliente.com"]).await;
    }
}
