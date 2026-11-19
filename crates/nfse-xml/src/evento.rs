//! Pedido de registro de evento (`pedRegEvento`): por enquanto, o cancelamento (e101101).

use chrono::{DateTime, FixedOffset};

use nfse_core::modelo::{Ambiente, Documento};

use crate::dps::NS_NFSE;
use crate::escrita::Xml;

/// `cMotivo` do cancelamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoCancelamento {
    ErroNaEmissao,
    ServicoNaoPrestado,
    Outros,
}

impl MotivoCancelamento {
    fn codigo(self) -> &'static str {
        match self {
            MotivoCancelamento::ErroNaEmissao => "1",
            MotivoCancelamento::ServicoNaoPrestado => "2",
            MotivoCancelamento::Outros => "9",
        }
    }

    /// Motivo pelo `cMotivo` do evento.
    pub fn de_codigo(codigo: &str) -> Option<Self> {
        match codigo.trim() {
            "1" => Some(MotivoCancelamento::ErroNaEmissao),
            "2" => Some(MotivoCancelamento::ServicoNaoPrestado),
            "9" => Some(MotivoCancelamento::Outros),
            _ => None,
        }
    }

    /// Nome do motivo para quem lê (o mesmo da tela de cancelamento).
    pub fn nome(self) -> &'static str {
        match self {
            MotivoCancelamento::ErroNaEmissao => "Erro na emissão",
            MotivoCancelamento::ServicoNaoPrestado => "Serviço não prestado",
            MotivoCancelamento::Outros => "Outros",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoCancelamento {
    pub ambiente: Ambiente,
    pub versao_aplicativo: String,
    pub quando: DateTime<FixedOffset>,
    /// Quem pede: o prestador emitente.
    pub autor: Documento,
    pub chave: String,
    pub motivo: MotivoCancelamento,
    /// `xMotivo`: 15 a 255 caracteres.
    pub descricao: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PedidoInvalido {
    #[error("chave de acesso deve ter 50 caracteres (recebida com {0})")]
    Chave(usize),
    #[error("a justificativa do cancelamento deve ter de 15 a 255 caracteres (tem {0})")]
    Descricao(usize),
    #[error("a justificativa do cancelamento {0}")]
    Caractere(String),
}

/// Confere a justificativa (`xMotivo`) como o pedido a envia: aparada, de 15 a 255 caracteres e
/// só com os caracteres do `TSString`. Serve para conferir um lote antes do primeiro envio.
pub fn conferir_justificativa(justificativa: &str) -> Result<(), PedidoInvalido> {
    let texto = justificativa.trim();
    let n = texto.chars().count();
    if !(15..=255).contains(&n) {
        return Err(PedidoInvalido::Descricao(n));
    }
    nfse_core::modelo::conferir_tsstring(texto).map_err(PedidoInvalido::Caractere)
}

/// Gera o pedido de cancelamento sem assinatura. Devolve o XML e o Id de `infPedReg`.
pub fn gerar_pedido_cancelamento(p: &PedidoCancelamento) -> Result<(String, String), PedidoInvalido> {
    if !nfse_core::dps::chave_de_acesso_valida(&p.chave) {
        return Err(PedidoInvalido::Chave(p.chave.len()));
    }
    conferir_justificativa(&p.descricao)?;
    let descricao = p.descricao.trim();
    let id = format!("PRE{}101101", p.chave);
    let mut x = Xml::raiz("pedRegEvento", "infPedReg", &id, 768);
    x.campo("tpAmb", p.ambiente.codigo());
    x.campo("verAplic", &p.versao_aplicativo);
    x.campo("dhEvento", p.quando.format("%Y-%m-%dT%H:%M:%S%:z").to_string());
    match &p.autor {
        Documento::Cnpj(c) => x.campo("CNPJAutor", c.as_str()),
        Documento::Cpf(c) => x.campo("CPFAutor", c.as_str()),
    }
    x.campo("chNFSe", &p.chave);
    x.abrir("e101101");
    x.campo("xDesc", "Cancelamento de NFS-e");
    x.campo("cMotivo", p.motivo.codigo());
    x.campo("xMotivo", descricao);
    x.fechar("e101101");
    x.fechar("infPedReg");
    x.fechar("pedRegEvento");
    Ok((x.0, id))
}

/// Confere se o XML é o evento de cancelamento (e101101) registrado pela Sefin para a NFS-e
/// `chave`: o `infEvento` homologado, com o pedido dessa chave dentro.
pub fn e_cancelamento_da_nfse(xml: &str, chave: &str) -> bool {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return false;
    };
    let tem = |nome: &str| {
        doc.descendants()
            .any(|n| n.tag_name().namespace() == Some(NS_NFSE) && n.tag_name().name() == nome)
    };
    let chave_confere = doc.descendants().any(|n| {
        n.tag_name().namespace() == Some(NS_NFSE) && n.tag_name().name() == "chNFSe" && n.text() == Some(chave)
    });
    tem("infEvento") && tem("e101101") && chave_confere
}

/// Motivo (`cMotivo`, `None` se o código não for conhecido) e justificativa (`xMotivo`, vazia se
/// faltar) do evento de cancelamento gravado. `None` se o XML não abre ou não tem o `e101101`.
pub fn motivo_do_cancelamento(xml_evento: &str) -> Option<(Option<MotivoCancelamento>, String)> {
    let doc = roxmltree::Document::parse(xml_evento).ok()?;
    let e101101 = doc
        .descendants()
        .find(|n| n.tag_name().namespace() == Some(NS_NFSE) && n.tag_name().name() == "e101101")?;
    let texto = |nome: &str| {
        e101101
            .children()
            .find(|n| n.tag_name().namespace() == Some(NS_NFSE) && n.tag_name().name() == nome)
            .and_then(|n| n.text())
            .map(str::trim)
    };
    Some((
        texto("cMotivo").and_then(MotivoCancelamento::de_codigo),
        texto("xMotivo").unwrap_or_default().to_owned(),
    ))
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;
    use nfse_core::Cnpj;

    pub fn pedido() -> PedidoCancelamento {
        PedidoCancelamento {
            ambiente: Ambiente::Producao,
            versao_aplicativo: "emissor-nfse/0.1.0".into(),
            quando: DateTime::parse_from_rfc3339("2026-10-16T16:30:00-03:00").unwrap(),
            autor: Documento::Cnpj(Cnpj::parse("11222333000181").unwrap()),
            chave: "35111022211222333000181000000000007726107413634280".into(),
            motivo: MotivoCancelamento::ErroNaEmissao,
            descricao: "Nota emitida em teste de integração".into(),
        }
    }

    #[test]
    fn reconhece_evento_de_cancelamento() {
        let p = pedido();
        let (pedido_xml, _) = gerar_pedido_cancelamento(&p).unwrap();
        let pedido_xml = pedido_xml.trim_start_matches(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        let evento = format!(
            r#"<evento xmlns="{NS_NFSE}" versao="1.01"><infEvento Id="EVT{}1011010001"><nSeqEvento>1</nSeqEvento>{pedido_xml}</infEvento></evento>"#,
            p.chave
        );
        assert!(e_cancelamento_da_nfse(&evento, &p.chave));
        assert!(!e_cancelamento_da_nfse(&evento, "outra-chave"));
        assert!(!e_cancelamento_da_nfse(pedido_xml, &p.chave), "pedido sem homologação");
        assert!(!e_cancelamento_da_nfse("<evento", &p.chave));
    }

    #[test]
    fn gera_pedido() {
        let (xml, id) = gerar_pedido_cancelamento(&pedido()).unwrap();
        assert_eq!(id, "PRE35111022211222333000181000000000007726107413634280101101");
        assert_eq!(id.len(), 59);
        assert!(xml.contains("<CNPJAutor>11222333000181</CNPJAutor><chNFSe>"));
        assert!(xml.contains("<e101101><xDesc>Cancelamento de NFS-e</xDesc><cMotivo>1</cMotivo>"));
        assert_eq!(
            xml,
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?><pedRegEvento xmlns="http://www.sped.fazenda.gov.br/nfse" versao="1.01">"#,
                r#"<infPedReg Id="PRE35111022211222333000181000000000007726107413634280101101"><tpAmb>1</tpAmb>"#,
                "<verAplic>emissor-nfse/0.1.0</verAplic><dhEvento>2026-10-16T16:30:00-03:00</dhEvento>",
                "<CNPJAutor>11222333000181</CNPJAutor><chNFSe>35111022211222333000181000000000007726107413634280</chNFSe>",
                "<e101101><xDesc>Cancelamento de NFS-e</xDesc><cMotivo>1</cMotivo>",
                "<xMotivo>Nota emitida em teste de integração</xMotivo></e101101></infPedReg></pedRegEvento>"
            )
        );
        // Texto livre escapado pelo mesmo escritor da DPS.
        let mut p = pedido();
        p.descricao = "Serviço A & B <cancelado>".into();
        let (xml, _) = gerar_pedido_cancelamento(&p).unwrap();
        assert!(
            xml.contains("<xMotivo>Serviço A &amp; B &lt;cancelado&gt;</xMotivo>"),
            "{xml}"
        );
    }

    #[test]
    #[ignore = "requer xmllint no PATH"]
    fn valida_no_xsd_oficial() {
        let pfx = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
        let cert = nfse_cert::CertificadoA1::carregar(pfx, "1234").unwrap();
        let (xml, id) = gerar_pedido_cancelamento(&pedido()).unwrap();
        let assinado = crate::assinar(&xml, &id, &cert).unwrap();
        let arquivo = std::env::temp_dir().join("nfse-pedido-cancelamento.xml");
        std::fs::write(&arquivo, assinado).unwrap();
        let xsd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../xsd/v1.01-prodrest-20261020/pedRegEvento_v1.01.xsd");
        let saida = std::process::Command::new("xmllint")
            .args(["--noout", "--schema"])
            .arg(xsd)
            .arg(&arquivo)
            .output()
            .unwrap();
        assert!(saida.status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
    }

    #[test]
    fn valida_campos() {
        let mut p = pedido();
        p.descricao = "curta".into();
        assert_eq!(gerar_pedido_cancelamento(&p), Err(PedidoInvalido::Descricao(5)));
        let mut p = pedido();
        p.chave.pop();
        assert_eq!(gerar_pedido_cancelamento(&p), Err(PedidoInvalido::Chave(49)));
        let mut p = pedido();
        p.descricao = "Valor errado\nna emissão da nota".into();
        assert!(matches!(
            gerar_pedido_cancelamento(&p),
            Err(PedidoInvalido::Caractere(_))
        ));
    }

    #[test]
    fn justificativa_fora_das_regras() {
        assert_eq!(
            conferir_justificativa("Catorze caract"),
            Err(PedidoInvalido::Descricao(14))
        );
        assert_eq!(
            conferir_justificativa(&"a".repeat(256)),
            Err(PedidoInvalido::Descricao(256))
        );
        assert!(matches!(
            conferir_justificativa("Valor errado\nna emissão da nota"),
            Err(PedidoInvalido::Caractere(_))
        ));
        assert!(matches!(
            conferir_justificativa("Valor errado — na emissão da nota"),
            Err(PedidoInvalido::Caractere(_))
        ));
    }

    #[test]
    fn justificativa_valida() {
        assert_eq!(conferir_justificativa("Quinze caracter"), Ok(()));
        // Os espaços das pontas não contam (o pedido leva o texto aparado).
        assert_eq!(conferir_justificativa("  Valor errado na emissão  "), Ok(()));
    }

    fn evento_com(e101101: &str) -> String {
        format!(
            r#"<evento xmlns="{NS_NFSE}" versao="1.01"><infEvento Id="EVT1"><pedRegEvento versao="1.01"><infPedReg Id="PRE1"><e101101>{e101101}</e101101></infPedReg></pedRegEvento></infEvento></evento>"#
        )
    }

    #[test]
    fn motivo_do_cancelamento_lido_do_evento() {
        let evento = evento_com(
            "<xDesc>Cancelamento de NFS-e</xDesc><cMotivo>2</cMotivo><xMotivo>Serviço A &amp; B não prestado</xMotivo>",
        );
        assert_eq!(
            motivo_do_cancelamento(&evento),
            Some((
                Some(MotivoCancelamento::ServicoNaoPrestado),
                "Serviço A & B não prestado".into()
            ))
        );
        assert_eq!(MotivoCancelamento::ServicoNaoPrestado.nome(), "Serviço não prestado");
        for m in [
            MotivoCancelamento::ErroNaEmissao,
            MotivoCancelamento::ServicoNaoPrestado,
            MotivoCancelamento::Outros,
        ] {
            assert_eq!(MotivoCancelamento::de_codigo(m.codigo()), Some(m));
        }
        assert_eq!(MotivoCancelamento::de_codigo("7"), None);
    }

    #[test]
    fn motivo_do_cancelamento_sem_xmotivo_ou_ilegivel() {
        assert_eq!(
            motivo_do_cancelamento(&evento_com("<cMotivo>9</cMotivo>")),
            Some((Some(MotivoCancelamento::Outros), String::new()))
        );
        assert_eq!(
            motivo_do_cancelamento(&evento_com(
                "<cMotivo>5</cMotivo><xMotivo>Outro texto qualquer</xMotivo>"
            )),
            Some((None, "Outro texto qualquer".into()))
        );
        assert_eq!(motivo_do_cancelamento("<evento"), None);
        assert_eq!(motivo_do_cancelamento(&format!(r#"<evento xmlns="{NS_NFSE}"/>"#)), None);
    }
}
