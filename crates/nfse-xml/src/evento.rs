//! Pedido de registro de evento (`pedRegEvento`): por enquanto, o cancelamento (e101101).

use chrono::{DateTime, FixedOffset};

use nfse_core::modelo::{Ambiente, Documento, VERSAO_LEIAUTE};

use crate::dps::NS_NFSE;

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

fn escapar(texto: &str) -> String {
    texto.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Gera o pedido de cancelamento sem assinatura. Devolve o XML e o Id de `infPedReg`.
pub fn gerar_pedido_cancelamento(p: &PedidoCancelamento) -> Result<(String, String), PedidoInvalido> {
    if !nfse_core::dps::chave_de_acesso_valida(&p.chave) {
        return Err(PedidoInvalido::Chave(p.chave.len()));
    }
    let descricao = p.descricao.trim();
    let n = descricao.chars().count();
    if !(15..=255).contains(&n) {
        return Err(PedidoInvalido::Descricao(n));
    }
    nfse_core::modelo::conferir_tsstring(descricao).map_err(PedidoInvalido::Caractere)?;
    let id = format!("PRE{}101101", p.chave);
    let autor = match &p.autor {
        Documento::Cnpj(c) => format!("<CNPJAutor>{}</CNPJAutor>", c.as_str()),
        Documento::Cpf(c) => format!("<CPFAutor>{}</CPFAutor>", c.as_str()),
    };
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<pedRegEvento xmlns="{ns}" versao="{versao}"><infPedReg Id="{id}">"#,
            "<tpAmb>{amb}</tpAmb><verAplic>{ver}</verAplic><dhEvento>{quando}</dhEvento>{autor}",
            "<chNFSe>{chave}</chNFSe><e101101><xDesc>Cancelamento de NFS-e</xDesc>",
            "<cMotivo>{motivo}</cMotivo><xMotivo>{descricao}</xMotivo></e101101>",
            "</infPedReg></pedRegEvento>"
        ),
        ns = NS_NFSE,
        versao = VERSAO_LEIAUTE,
        id = id,
        amb = p.ambiente.codigo(),
        ver = escapar(&p.versao_aplicativo),
        quando = p.quando.format("%Y-%m-%dT%H:%M:%S%:z"),
        autor = autor,
        chave = p.chave,
        motivo = p.motivo.codigo(),
        descricao = escapar(descricao),
    );
    Ok((xml, id))
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
            .join("../../xsd/v1.01-prodrest-20260727/pedRegEvento_v1.01.xsd");
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
}
