//! E-mail da NFS-e ao tomador: PDF (DANFSe) e XML anexos, pela conta SMTP do escritório, com o
//! nome da empresa emissora como remetente e "responder para" o e-mail dela.

use std::future::Future;
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use nfse_db::emails::{ConfigSmtp, DadosEmailNfse, Seguranca};

use crate::arquivo;

/// Quem entrega a mensagem: o SMTP de verdade ou, nos testes, um carteiro simulado.
pub trait Carteiro {
    fn entregar(
        &self,
        cfg: &ConfigSmtp,
        senha: Option<&str>,
        mensagem: Message,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// Envio pelo servidor SMTP configurado.
pub struct CarteiroSmtp;

impl Carteiro for CarteiroSmtp {
    async fn entregar(&self, cfg: &ConfigSmtp, senha: Option<&str>, mensagem: Message) -> Result<(), String> {
        let servidor = cfg.servidor.trim();
        let construtor = match cfg.seguranca {
            Seguranca::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(servidor),
            Seguranca::Ssl => AsyncSmtpTransport::<Tokio1Executor>::relay(servidor),
            Seguranca::Nenhuma => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(servidor)),
        }
        .map_err(|e| format!("servidor de e-mail inválido: {e}"))?;
        let mut construtor = construtor.port(cfg.porta).timeout(Some(Duration::from_secs(30)));
        if let Some(usuario) = cfg.usuario.as_deref().filter(|u| !u.trim().is_empty()) {
            construtor = construtor.credentials(Credentials::new(
                usuario.trim().to_owned(),
                senha.unwrap_or_default().to_owned(),
            ));
        }
        construtor
            .build()
            .send(mensagem)
            .await
            .map(|_| ())
            .map_err(|e| format!("o servidor de e-mail recusou: {e}"))
    }
}

/// URL da consulta pública da nota no portal nacional (a mesma do QR Code do DANFSe).
pub fn link_consulta(chave: &str) -> String {
    format!("https://www.nfse.gov.br/ConsultaPublica/?tpc=1&chave={chave}")
}

fn valor_br(dados: &DadosEmailNfse) -> String {
    let Some(v) = dados.valor else { return String::new() };
    let texto = v.to_string(); // "1250.00"
    let (inteiro, centavos) = texto.split_once('.').unwrap_or((&texto, "00"));
    let mut com_pontos = String::new();
    for (i, c) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i) % 3 == 0 {
            com_pontos.push('.');
        }
        com_pontos.push(c);
    }
    format!("{com_pontos},{centavos}")
}

/// Troca as variáveis do assunto e do corpo pelos dados da nota.
pub fn preencher(modelo: &str, dados: &DadosEmailNfse) -> String {
    let competencia = dados
        .competencia
        .as_deref()
        .and_then(|c| Some(format!("{}/{}", c.get(5..7)?, c.get(0..4)?)))
        .unwrap_or_default();
    modelo
        .replace("{numero}", dados.numero.as_deref().unwrap_or(""))
        .replace("{empresa}", &dados.empresa)
        .replace("{tomador}", dados.tomador.as_deref().unwrap_or("cliente"))
        .replace("{valor}", &valor_br(dados))
        .replace("{competencia}", &competencia)
        .replace("{chave}", &dados.chave)
        .replace("{link}", &link_consulta(&dados.chave))
}

fn caixa(endereco: &str, nome: Option<&str>) -> Result<Mailbox, String> {
    let endereco = endereco
        .trim()
        .parse()
        .map_err(|_| format!("e-mail inválido: \"{}\"", endereco.trim()))?;
    Ok(Mailbox::new(nome.map(str::to_owned), endereco))
}

/// Monta a mensagem com o PDF (se houver) e o XML anexos. Devolve também o assunto.
pub fn montar(
    cfg: &ConfigSmtp,
    dados: &DadosEmailNfse,
    para: &[String],
    pdf: Option<Vec<u8>>,
) -> Result<(Message, String), String> {
    if para.is_empty() {
        return Err("sem destinatário: cadastre os e-mails de envio do tomador".into());
    }
    let mut assunto = preencher(&cfg.assunto, dados);
    let mut corpo = preencher(&cfg.corpo, dados);
    if dados.cancelada {
        assunto = format!("CANCELADA - {assunto}");
        corpo = format!("ATENÇÃO: esta NFS-e foi CANCELADA.\n\n{corpo}");
    }
    let nome = cfg
        .nome_remetente
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&dados.empresa);
    let mut construtor = Message::builder()
        .from(caixa(&cfg.remetente, Some(nome))?)
        .subject(assunto.clone());
    if let Some(email) = dados.email_empresa.as_deref().filter(|e| !e.trim().is_empty()) {
        construtor = construtor.reply_to(caixa(email, Some(&dados.empresa))?);
    }
    for endereco in para {
        construtor = construtor.to(caixa(endereco, None)?);
    }

    let nome_xml = arquivo::nome_xml_nfse(dados.numero.as_deref().unwrap_or("sem-numero"), &dados.chave);
    let mut partes = MultiPart::mixed().singlepart(SinglePart::plain(corpo));
    if let Some(pdf) = pdf {
        partes = partes.singlepart(
            Attachment::new(nome_xml.replace(".xml", ".pdf"))
                .body(pdf, ContentType::parse("application/pdf").expect("tipo fixo")),
        );
    }
    partes = partes.singlepart(Attachment::new(nome_xml).body(
        dados.xml.clone().into_bytes(),
        ContentType::parse("application/xml").expect("tipo fixo"),
    ));
    let mensagem = construtor
        .multipart(partes)
        .map_err(|e| format!("não foi possível montar o e-mail: {e}"))?;
    Ok((mensagem, assunto))
}

/// Mensagem simples para testar a configuração.
pub fn mensagem_teste(cfg: &ConfigSmtp, para: &str) -> Result<Message, String> {
    Message::builder()
        .from(caixa(
            &cfg.remetente,
            cfg.nome_remetente.as_deref().filter(|n| !n.trim().is_empty()),
        )?)
        .to(caixa(para, None)?)
        .subject("Teste do emissor de NFS-e")
        .body("Se você recebeu esta mensagem, o envio de e-mail do emissor de NFS-e está funcionando.".to_owned())
        .map_err(|e| format!("não foi possível montar o e-mail: {e}"))
}

#[cfg(test)]
mod testes {
    use nfse_core::Valor;

    use super::*;

    pub fn dados() -> DadosEmailNfse {
        DadosEmailNfse {
            nfse_id: 1,
            numero: Some("77".into()),
            chave: "35111022211222333000181000000000007726101234567890".into(),
            xml: "<NFSe/>".into(),
            cancelada: false,
            empresa_id: 1,
            empresa: "EMPRESA EXEMPLO LTDA".into(),
            email_empresa: Some("contato@exemplo.com".into()),
            tomador: Some("CLIENTE EXEMPLO LTDA".into()),
            destinatarios: vec!["financeiro@cliente.com".into()],
            valor: Some(Valor::parse("1250").unwrap()),
            competencia: Some("2026-10-16".into()),
        }
    }

    pub fn cfg() -> ConfigSmtp {
        ConfigSmtp {
            servidor: "smtp.exemplo.com".into(),
            porta: 587,
            seguranca: Seguranca::StartTls,
            usuario: Some("notas@escritorio.com".into()),
            remetente: "notas@escritorio.com".into(),
            nome_remetente: None,
            assunto: ConfigSmtp::ASSUNTO_PADRAO.into(),
            corpo: ConfigSmtp::CORPO_PADRAO.into(),
            enviar_ao_emitir: true,
        }
    }

    #[test]
    fn preenche_variaveis() {
        let texto = preencher("{numero}|{empresa}|{tomador}|{valor}|{competencia}|{link}", &dados());
        assert_eq!(
            texto,
            "77|EMPRESA EXEMPLO LTDA|CLIENTE EXEMPLO LTDA|1.250,00|10/2026|\
             https://www.nfse.gov.br/ConsultaPublica/?tpc=1&chave=35111022211222333000181000000000007726101234567890"
        );
    }

    #[test]
    fn mensagem_com_anexos_e_responder_para() {
        let (msg, assunto) = montar(&cfg(), &dados(), &dados().destinatarios, Some(b"%PDF-1.7".to_vec())).unwrap();
        assert_eq!(assunto, "NFS-e nº 77 - EMPRESA EXEMPLO LTDA");
        let bruto = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(
            bruto.contains("From: \"EMPRESA EXEMPLO LTDA\" <notas@escritorio.com>"),
            "{bruto}"
        );
        assert!(
            bruto.contains("Reply-To: \"EMPRESA EXEMPLO LTDA\" <contato@exemplo.com>"),
            "{bruto}"
        );
        assert!(bruto.contains("To: financeiro@cliente.com"), "{bruto}");
        // Nome longo vai em partes (RFC 2231: filename*0, filename*1), que o leitor de e-mail junta.
        let nomes = bruto
            .split_whitespace()
            .collect::<String>()
            .replace("\";filename*1=\"", "");
        assert!(
            nomes.contains("NFSe-77-35111022211222333000181000000000007726101234567890.pdf"),
            "{bruto}"
        );
        assert!(
            nomes.contains("NFSe-77-35111022211222333000181000000000007726101234567890.xml"),
            "{bruto}"
        );
        assert!(bruto.contains("Content-Type: application/pdf") && bruto.contains("Content-Type: application/xml"));
    }

    #[test]
    fn cancelada_e_sem_destinatario() {
        let mut d = dados();
        d.cancelada = true;
        let (_, assunto) = montar(&cfg(), &d, &d.destinatarios, None).unwrap();
        assert!(assunto.starts_with("CANCELADA - "));
        assert!(montar(&cfg(), &d, &[], None).unwrap_err().contains("sem destinatário"));
        assert!(
            montar(&cfg(), &d, &["nao-e-email".into()], None)
                .unwrap_err()
                .contains("inválido")
        );
    }
}
