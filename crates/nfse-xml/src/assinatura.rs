//! Assinatura XMLDSIG enveloped no formato exigido pela Sefin Nacional:
//! C14N inclusivo, RSA-SHA1, digest SHA-1, só o certificado do titular no `KeyInfo`
//! e `Signature` com namespace padrão (sem prefixo `ds:`, que causa rejeição E1228).
//!
//! A `Signature` é inserida como último filho do pai do elemento referenciado
//! (ex.: dentro de `DPS`, depois de `infDPS`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use rsa::Pkcs1v15Sign;
use sha1::{Digest, Sha1};

use crate::c14n::{Opcoes, canonicalizar, canonicalizar_com};

pub const NS_DSIG: &str = "http://www.w3.org/2000/09/xmldsig#";
const ALG_C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const ALG_C14N_COMENTARIOS: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments";
const ALG_EXC_C14N: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";
const ALG_EXC_C14N_COMENTARIOS: &str = "http://www.w3.org/2001/10/xml-exc-c14n#WithComments";
const ALG_ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const ALG_RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
const ALG_RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const ALG_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#sha1";
const ALG_SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";

#[derive(Debug, thiserror::Error)]
pub enum ErroAssinatura {
    #[error("XML malformado: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("nenhum elemento com Id=\"{0}\"")]
    IdNaoEncontrado(String),
    #[error("mais de um elemento com Id=\"{0}\"")]
    IdDuplicado(String),
    #[error("o elemento referenciado não tem elemento pai para receber a assinatura")]
    SemPai,
    #[error("o XML não contém assinatura")]
    SemAssinatura,
    #[error("assinatura malformada: falta {0}")]
    Incompleta(&'static str),
    #[error("algoritmo não suportado: {0}")]
    Algoritmo(String),
    #[error("digest não confere: o conteúdo assinado foi alterado")]
    DigestDivergente,
    #[error("valor da assinatura não confere com o certificado")]
    AssinaturaInvalida,
    #[error("base64 inválido: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error(transparent)]
    Certificado(#[from] nfse_cert::ErroCertificado),
}

/// Quem assina: hoje o certificado A1; no futuro pode ser um A3 via repositório do Windows.
pub trait Assinante {
    fn certificado_der(&self) -> &[u8];
    fn assinar_rsa_sha1(&self, dados: &[u8]) -> Result<Vec<u8>, ErroAssinatura>;
}

impl Assinante for nfse_cert::CertificadoA1 {
    fn certificado_der(&self) -> &[u8] {
        nfse_cert::CertificadoA1::certificado_der(self)
    }

    fn assinar_rsa_sha1(&self, dados: &[u8]) -> Result<Vec<u8>, ErroAssinatura> {
        Ok(nfse_cert::CertificadoA1::assinar_rsa_sha1(self, dados)?)
    }
}

fn elemento_por_id<'a, 'i>(
    doc: &'a roxmltree::Document<'i>,
    id: &str,
) -> Result<roxmltree::Node<'a, 'i>, ErroAssinatura> {
    let mut achados = doc
        .descendants()
        .filter(|n| n.is_element() && n.attribute("Id") == Some(id));
    let no = achados
        .next()
        .ok_or_else(|| ErroAssinatura::IdNaoEncontrado(id.to_owned()))?;
    if achados.next().is_some() {
        return Err(ErroAssinatura::IdDuplicado(id.to_owned()));
    }
    Ok(no)
}

fn digest_b64(dados: &[u8]) -> String {
    B64.encode(Sha1::digest(dados))
}

/// Assina o elemento cujo atributo `Id` é `id` e devolve o XML com a `Signature` inserida.
/// O restante do documento é preservado byte a byte.
pub fn assinar(xml: &str, id: &str, assinante: &impl Assinante) -> Result<String, ErroAssinatura> {
    let doc = roxmltree::Document::parse(xml)?;
    let alvo = elemento_por_id(&doc, id)?;
    let pai = alvo.parent_element().ok_or(ErroAssinatura::SemPai)?;
    let digest = digest_b64(canonicalizar(alvo, None).as_bytes());

    let signed_info = format!(
        concat!(
            r#"<SignedInfo><CanonicalizationMethod Algorithm="{c14n}"></CanonicalizationMethod>"#,
            r#"<SignatureMethod Algorithm="{rsa}"></SignatureMethod>"#,
            r##"<Reference URI="#{id}"><Transforms><Transform Algorithm="{env}"></Transform>"##,
            r#"<Transform Algorithm="{c14n}"></Transform></Transforms>"#,
            r#"<DigestMethod Algorithm="{sha1}"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
        ),
        c14n = ALG_C14N,
        rsa = ALG_RSA_SHA1,
        env = ALG_ENVELOPED,
        sha1 = ALG_SHA1,
        id = id,
        digest = digest,
    );

    // Insere a assinatura sem SignatureValue e canoniza o SignedInfo já no lugar definitivo,
    // para que os namespaces herdados entrem na forma canônica exatamente como o validador verá.
    let fim_pai = pai.range().end;
    let inicio_fechamento = xml[..fim_pai].rfind("</").ok_or(ErroAssinatura::SemPai)?;
    let rascunho = format!(
        r#"{}<Signature xmlns="{NS_DSIG}">{signed_info}</Signature>{}"#,
        &xml[..inicio_fechamento],
        &xml[inicio_fechamento..]
    );
    let doc_rascunho = roxmltree::Document::parse(&rascunho)?;
    let no_signed_info = doc_rascunho
        .descendants()
        .find(|n| n.tag_name().namespace() == Some(NS_DSIG) && n.tag_name().name() == "SignedInfo")
        .ok_or(ErroAssinatura::Incompleta("SignedInfo"))?;
    let valor = B64.encode(assinante.assinar_rsa_sha1(canonicalizar(no_signed_info, None).as_bytes())?);
    let certificado = B64.encode(assinante.certificado_der());

    Ok(format!(
        r#"{}<Signature xmlns="{NS_DSIG}">{signed_info}<SignatureValue>{valor}</SignatureValue><KeyInfo><X509Data><X509Certificate>{certificado}</X509Certificate></X509Data></KeyInfo></Signature>{}"#,
        &xml[..inicio_fechamento],
        &xml[inicio_fechamento..]
    ))
}

fn filho<'a, 'i>(no: roxmltree::Node<'a, 'i>, nome: &'static str) -> Result<roxmltree::Node<'a, 'i>, ErroAssinatura> {
    no.children()
        .find(|n| n.tag_name().namespace() == Some(NS_DSIG) && n.tag_name().name() == nome)
        .ok_or(ErroAssinatura::Incompleta(nome))
}

fn algoritmo<'a>(no: roxmltree::Node<'a, '_>) -> &'a str {
    no.attribute("Algorithm").unwrap_or("")
}

/// Texto base64 de um elemento, ignorando quebras de linha que alguns emissores inserem.
fn base64_do_elemento(no: roxmltree::Node) -> Result<Vec<u8>, ErroAssinatura> {
    let texto: String = no.text().unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
    Ok(B64.decode(texto)?)
}

#[derive(Clone, Copy)]
enum Hash {
    Sha1,
    Sha256,
}

impl Hash {
    fn resumo(self, dados: &[u8]) -> Vec<u8> {
        match self {
            Hash::Sha1 => Sha1::digest(dados).to_vec(),
            Hash::Sha256 => sha2::Sha256::digest(dados).to_vec(),
        }
    }

    fn esquema(self) -> Pkcs1v15Sign {
        match self {
            Hash::Sha1 => Pkcs1v15Sign::new::<Sha1>(),
            Hash::Sha256 => Pkcs1v15Sign::new::<sha2::Sha256>(),
        }
    }
}

fn opcoes_c14n(algoritmo: &str) -> Option<Opcoes> {
    let (exclusivo, comentarios) = match algoritmo {
        ALG_C14N => (false, false),
        ALG_C14N_COMENTARIOS => (false, true),
        ALG_EXC_C14N => (true, false),
        ALG_EXC_C14N_COMENTARIOS => (true, true),
        _ => return None,
    };
    Some(Opcoes { exclusivo, comentarios })
}

/// Verifica todas as assinaturas do documento (digest e valor) e devolve o certificado de cada uma.
/// Aceita o esquema que usamos (C14N inclusivo + RSA-SHA1) e o da NFS-e devolvida pela Sefin
/// (C14N exclusivo + RSA-SHA256).
pub fn verificar(xml: &str) -> Result<Vec<Vec<u8>>, ErroAssinatura> {
    let doc = roxmltree::Document::parse(xml)?;
    let assinaturas: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().namespace() == Some(NS_DSIG) && n.tag_name().name() == "Signature")
        .collect();
    if assinaturas.is_empty() {
        return Err(ErroAssinatura::SemAssinatura);
    }

    let mut certificados = Vec::new();
    for assinatura in assinaturas {
        let signed_info = filho(assinatura, "SignedInfo")?;
        let metodo_c14n = algoritmo(filho(signed_info, "CanonicalizationMethod")?);
        let c14n_signed_info =
            opcoes_c14n(metodo_c14n).ok_or_else(|| ErroAssinatura::Algoritmo(metodo_c14n.to_owned()))?;
        let metodo_assinatura = algoritmo(filho(signed_info, "SignatureMethod")?);
        let hash_assinatura = match metodo_assinatura {
            ALG_RSA_SHA1 => Hash::Sha1,
            ALG_RSA_SHA256 => Hash::Sha256,
            outro => return Err(ErroAssinatura::Algoritmo(outro.to_owned())),
        };

        let referencia = filho(signed_info, "Reference")?;
        let uri = referencia
            .attribute("URI")
            .ok_or(ErroAssinatura::Incompleta("Reference/@URI"))?;
        let id = uri
            .strip_prefix('#')
            .ok_or_else(|| ErroAssinatura::Algoritmo(format!("URI {uri}")))?;
        // Sem transformação de C14N explícita, o padrão é o inclusivo.
        let mut c14n_referencia = Opcoes::default();
        for transform in filho(referencia, "Transforms")?.children().filter(|n| n.is_element()) {
            match algoritmo(transform) {
                ALG_ENVELOPED => {}
                alg => c14n_referencia = opcoes_c14n(alg).ok_or_else(|| ErroAssinatura::Algoritmo(alg.to_owned()))?,
            }
        }
        // Referência "#id" descarta comentários antes das transformações (XMLDSig, 4.3.3.3).
        c14n_referencia.comentarios = false;
        let hash_digest = match algoritmo(filho(referencia, "DigestMethod")?) {
            ALG_SHA1 => Hash::Sha1,
            ALG_SHA256 => Hash::Sha256,
            outro => return Err(ErroAssinatura::Algoritmo(outro.to_owned())),
        };

        let alvo = elemento_por_id(&doc, id)?;
        let esperado = base64_do_elemento(filho(referencia, "DigestValue")?)?;
        let calculado = hash_digest.resumo(canonicalizar_com(alvo, Some(assinatura.id()), c14n_referencia).as_bytes());
        if esperado != calculado {
            return Err(ErroAssinatura::DigestDivergente);
        }

        let certificado = base64_do_elemento(filho(
            filho(filho(assinatura, "KeyInfo")?, "X509Data")?,
            "X509Certificate",
        )?)?;
        let publica = nfse_cert::chave_publica_do_certificado(&certificado)?;
        let valor = base64_do_elemento(filho(assinatura, "SignatureValue")?)?;
        let resumo = hash_assinatura.resumo(canonicalizar_com(signed_info, None, c14n_signed_info).as_bytes());
        publica
            .verify(hash_assinatura.esquema(), &resumo, &valor)
            .map_err(|_| ErroAssinatura::AssinaturaInvalida)?;
        certificados.push(certificado);
    }
    Ok(certificados)
}

#[cfg(test)]
mod testes {
    use super::*;
    use nfse_cert::CertificadoA1;

    const PFX: &[u8] = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
    const ID: &str = "DPS351110221122233300018100001000000000000042";

    fn dps() -> String {
        format!(
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                "<DPS xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\">",
                "<infDPS Id=\"{}\"><tpAmb>2</tpAmb><dhEmi>2026-10-16T10:00:00-03:00</dhEmi>",
                "<serv><cServ><xDescServ>Consulta m&#233;dica &amp; exames</xDescServ></cServ></serv>",
                "</infDPS></DPS>"
            ),
            ID
        )
    }

    fn certificado() -> CertificadoA1 {
        CertificadoA1::carregar(PFX, "1234").unwrap()
    }

    #[test]
    fn assina_e_verifica() {
        let cert = certificado();
        let assinado = assinar(&dps(), ID, &cert).unwrap();

        assert!(assinado.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><DPS "));
        assert!(assinado.contains("</infDPS><Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\"><SignedInfo>"));
        assert!(assinado.ends_with("</Signature></DPS>"));
        assert!(!assinado.contains("ds:"));

        let certs = verificar(&assinado).unwrap();
        assert_eq!(certs, vec![cert.certificado_der().to_vec()]);
    }

    /// NFS-e fictícia assinada pelo signxml (Python) no esquema da Sefin:
    /// C14N exclusivo com comentários, RSA-SHA256, digest SHA-256.
    const NFSE_SEFIN_SIMULADA: &str = include_str!("../tests/fixtures/nfse-sefin-simulada.xml");

    #[test]
    fn verifica_esquema_da_sefin() {
        let certs = verificar(NFSE_SEFIN_SIMULADA).unwrap();
        let info = nfse_cert::InfoCertificado::de_der(&certs[0]).unwrap();
        assert_eq!(info.titular, "EMPRESA TESTE LTDA:11222333000181");

        let adulterado = NFSE_SEFIN_SIMULADA.replace("150.00", "1500.00");
        assert!(matches!(verificar(&adulterado), Err(ErroAssinatura::DigestDivergente)));
    }

    #[test]
    fn detecta_alteracao_do_conteudo() {
        let assinado = assinar(&dps(), ID, &certificado()).unwrap();
        let adulterado = assinado.replace("<tpAmb>2</tpAmb>", "<tpAmb>1</tpAmb>");
        assert!(matches!(verificar(&adulterado), Err(ErroAssinatura::DigestDivergente)));
    }

    #[test]
    fn detecta_alteracao_do_signed_info() {
        let assinado = assinar(&dps(), ID, &certificado()).unwrap();
        let adulterado = assinado.replacen("<DigestValue>", "<DigestValue>AAAA", 1);
        assert!(verificar(&adulterado).is_err());
    }

    #[test]
    fn id_inexistente() {
        assert!(matches!(
            assinar(&dps(), "DPS0", &certificado()),
            Err(ErroAssinatura::IdNaoEncontrado(_))
        ));
    }

    /// Confere a assinatura com uma implementação independente (xmlsec1), quando instalada.
    #[test]
    #[ignore = "requer xmlsec1 no PATH"]
    fn confere_com_xmlsec1() {
        let dir = std::env::temp_dir().join("nfse-xml-xmlsec1");
        std::fs::create_dir_all(&dir).unwrap();
        let arquivo = dir.join("dps-assinada.xml");
        std::fs::write(&arquivo, assinar(&dps(), ID, &certificado()).unwrap()).unwrap();
        let saida = std::process::Command::new("xmlsec1")
            .args([
                "--verify",
                "--insecure",
                "--id-attr:Id",
                "http://www.sped.fazenda.gov.br/nfse:infDPS",
            ])
            .arg(&arquivo)
            .output()
            .unwrap();
        assert!(saida.status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
    }
}
