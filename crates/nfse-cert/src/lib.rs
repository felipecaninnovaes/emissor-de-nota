//! Certificado digital A1 (.pfx/.p12) do prestador: leitura, diagnóstico e assinatura RSA.
//!
//! O mesmo certificado serve para assinar a DPS/eventos (XMLDSIG) e para autenticar
//! a conexão mTLS com a Sefin Nacional; por isso os bytes originais do .pfx e a senha
//! ficam guardados (em memória, zerados ao descartar) para montar a identidade TLS.

use chrono::{DateTime, Utc};
use der::asn1::ObjectIdentifier;
use der::{Decode, Tag, Tagged};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};
use x509_cert::Certificate;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::{ExtendedKeyUsage, SubjectAltName};
use zeroize::Zeroizing;

use nfse_core::Cnpj;

const OID_CN: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.4.3");
const OID_EKU: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.37");
const OID_SAN: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.17");
/// EKU "Client Authentication", exigido pela Sefin no mTLS.
pub const OID_CLIENT_AUTH: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.3.2");
/// ICP-Brasil, otherName com o CNPJ do titular do e-CNPJ.
const OID_ICP_CNPJ: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.76.1.3.3");
/// ICP-Brasil, otherName com o nome do responsável pelo e-CNPJ.
const OID_ICP_RESPONSAVEL: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.76.1.3.2");

#[derive(Debug, thiserror::Error)]
pub enum ErroCertificado {
    #[error("não foi possível abrir o .pfx (senha incorreta ou arquivo corrompido): {0}")]
    Pkcs12(String),
    #[error("o .pfx não contém chave privada com certificado")]
    SemChave,
    #[error("a chave privada não é RSA ou não pôde ser lida: {0}")]
    Chave(String),
    #[error("nenhum certificado do .pfx corresponde à chave privada")]
    SemCertificadoDaChave,
    #[error("certificado inválido: {0}")]
    X509(String),
    #[error("falha ao assinar: {0}")]
    Assinatura(String),
}

/// Dados do certificado para cadastro, painel de vencimentos e diagnóstico de suporte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoCertificado {
    /// Common Name do titular (no e-CNPJ costuma ser `RAZAO SOCIAL:CNPJ`).
    pub titular: String,
    /// CNPJ do otherName ICP-Brasil (2.16.76.1.3.3); ausente em e-CPF ou certificado fora do padrão.
    pub cnpj: Option<Cnpj>,
    pub responsavel: Option<String>,
    /// Common Name da AC emissora.
    pub emissor: String,
    pub numero_serie: String,
    pub valido_de: DateTime<Utc>,
    pub valido_ate: DateTime<Utc>,
    /// O EKU contém Client Authentication (necessário para o mTLS).
    pub client_auth: bool,
    /// SHA-1 do certificado em hexadecimal maiúsculo, como o Windows exibe.
    pub thumbprint: String,
}

impl InfoCertificado {
    pub fn de_der(der: &[u8]) -> Result<Self, ErroCertificado> {
        let cert = Certificate::from_der(der).map_err(|e| ErroCertificado::X509(e.to_string()))?;
        let tbs = &cert.tbs_certificate;

        let mut client_auth = false;
        let mut cnpj = None;
        let mut responsavel = None;
        for ext in tbs.extensions.iter().flatten() {
            let bytes = ext.extn_value.as_bytes();
            if ext.extn_id == OID_EKU {
                let eku = ExtendedKeyUsage::from_der(bytes).map_err(|e| ErroCertificado::X509(e.to_string()))?;
                client_auth = eku.0.contains(&OID_CLIENT_AUTH);
            } else if ext.extn_id == OID_SAN {
                let san = SubjectAltName::from_der(bytes).map_err(|e| ErroCertificado::X509(e.to_string()))?;
                for nome in san.0 {
                    let GeneralName::OtherName(outro) = nome else { continue };
                    // ICP-Brasil grava os valores como OCTET STRING ou string ASCII; nos dois casos o conteúdo é texto.
                    let texto = String::from_utf8_lossy(outro.value.value()).trim().to_owned();
                    if outro.type_id == OID_ICP_CNPJ {
                        cnpj = Cnpj::parse(&texto).ok();
                    } else if outro.type_id == OID_ICP_RESPONSAVEL && !texto.is_empty() {
                        responsavel = Some(texto);
                    }
                }
            }
        }

        Ok(Self {
            titular: common_name(&tbs.subject).unwrap_or_default(),
            cnpj,
            responsavel,
            emissor: common_name(&tbs.issuer).unwrap_or_default(),
            numero_serie: hex::encode_upper(tbs.serial_number.as_bytes()),
            valido_de: para_chrono(tbs.validity.not_before.to_unix_duration()),
            valido_ate: para_chrono(tbs.validity.not_after.to_unix_duration()),
            client_auth,
            thumbprint: hex::encode_upper(Sha1::digest(der)),
        })
    }

    pub fn vencido_em(&self, agora: DateTime<Utc>) -> bool {
        agora < self.valido_de || agora > self.valido_ate
    }

    /// Dias até o vencimento (negativo se já venceu).
    pub fn dias_para_vencer(&self, agora: DateTime<Utc>) -> i64 {
        (self.valido_ate - agora).num_days()
    }

    /// O certificado pode ser vinculado à empresa: mesma raiz de CNPJ (o e-CNPJ da matriz
    /// costuma ser usado pelas filiais). Sem CNPJ no certificado, o vínculo é recusado.
    pub fn pertence_a(&self, empresa: &Cnpj) -> bool {
        self.cnpj.as_ref().is_some_and(|c| c.raiz() == empresa.raiz())
    }
}

fn para_chrono(d: std::time::Duration) -> DateTime<Utc> {
    DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default()
}

fn common_name(nome: &x509_cert::name::Name) -> Option<String> {
    nome.0
        .iter()
        .flat_map(|rdn| rdn.0.iter())
        .find(|attr| attr.oid == OID_CN)
        .and_then(|attr| match attr.value.tag() {
            Tag::Utf8String | Tag::PrintableString | Tag::Ia5String | Tag::TeletexString => {
                Some(String::from_utf8_lossy(attr.value.value()).into_owned())
            }
            _ => None,
        })
}

/// Chave pública RSA de um certificado X.509 em DER (para verificar assinaturas).
pub fn chave_publica_do_certificado(der: &[u8]) -> Result<RsaPublicKey, ErroCertificado> {
    let cert = Certificate::from_der(der).map_err(|e| ErroCertificado::X509(e.to_string()))?;
    let bits = cert.tbs_certificate.subject_public_key_info.subject_public_key;
    let bytes = bits
        .as_bytes()
        .ok_or_else(|| ErroCertificado::X509("chave pública malformada".into()))?;
    RsaPublicKey::from_pkcs1_der(bytes).map_err(|e| ErroCertificado::X509(format!("chave pública não é RSA: {e}")))
}

/// Certificado A1 aberto em memória.
pub struct CertificadoA1 {
    chave: RsaPrivateKey,
    certificado: Vec<u8>,
    info: InfoCertificado,
    pfx: Zeroizing<Vec<u8>>,
    senha: Zeroizing<String>,
}

impl std::fmt::Debug for CertificadoA1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Nunca imprimir chave, .pfx ou senha.
        f.debug_struct("CertificadoA1")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl CertificadoA1 {
    pub fn carregar(pfx: &[u8], senha: &str) -> Result<Self, ErroCertificado> {
        let store = p12_keystore::KeyStore::from_pkcs12(pfx, senha, p12_keystore::Pkcs12ImportPolicy::Relaxed)
            .map_err(|e| ErroCertificado::Pkcs12(e.to_string()))?;
        let (_, cadeia) = store.private_key_chain().ok_or(ErroCertificado::SemChave)?;

        let chave =
            RsaPrivateKey::from_pkcs8_der(cadeia.key().as_der()).map_err(|e| ErroCertificado::Chave(e.to_string()))?;

        // O certificado do titular é o que tem a mesma chave pública; os demais são a cadeia da AC.
        let certificado = cadeia
            .certs()
            .iter()
            .map(|c| c.as_der())
            .find(|der| chave_publica_do_certificado(der).is_ok_and(|p| p.n() == chave.n() && p.e() == chave.e()))
            .ok_or(ErroCertificado::SemCertificadoDaChave)?
            .to_vec();
        let info = InfoCertificado::de_der(&certificado)?;

        Ok(Self {
            chave,
            certificado,
            info,
            pfx: Zeroizing::new(pfx.to_vec()),
            senha: Zeroizing::new(senha.to_owned()),
        })
    }

    pub fn info(&self) -> &InfoCertificado {
        &self.info
    }

    /// Certificado do titular em DER (vai no `KeyInfo` da assinatura).
    pub fn certificado_der(&self) -> &[u8] {
        &self.certificado
    }

    /// Bytes originais do .pfx e senha, para montar a identidade do cliente mTLS.
    pub fn pkcs12(&self) -> (&[u8], &str) {
        (&self.pfx, &self.senha)
    }

    /// Assinatura RSA PKCS#1 v1.5 com SHA-1 (exigida pelo leiaute da DPS).
    pub fn assinar_rsa_sha1(&self, dados: &[u8]) -> Result<Vec<u8>, ErroCertificado> {
        let hash = Sha1::digest(dados);
        self.chave
            .sign(Pkcs1v15Sign::new::<Sha1>(), &hash)
            .map_err(|e| ErroCertificado::Assinatura(e.to_string()))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const PFX_LEGACY: &[u8] = include_bytes!("../tests/fixtures/teste-ecnpj-legacy.pfx");
    const PFX_AES: &[u8] = include_bytes!("../tests/fixtures/teste-ecnpj-aes.pfx");

    #[test]
    fn carrega_pfx_legacy_e_aes() {
        for pfx in [PFX_LEGACY, PFX_AES] {
            let cert = CertificadoA1::carregar(pfx, "1234").unwrap();
            let info = cert.info();
            assert_eq!(info.titular, "EMPRESA TESTE LTDA:11222333000181");
            assert_eq!(info.cnpj.as_ref().map(|c| c.as_str()), Some("11222333000181"));
            assert_eq!(info.responsavel.as_deref(), Some("FULANO DE TAL"));
            assert!(info.client_auth);
            assert_eq!(info.thumbprint.len(), 40);
            assert!(info.valido_ate > info.valido_de);
        }
    }

    #[test]
    fn senha_errada() {
        assert!(matches!(
            CertificadoA1::carregar(PFX_AES, "errada"),
            Err(ErroCertificado::Pkcs12(_))
        ));
    }

    #[test]
    fn vinculo_pela_raiz_do_cnpj() {
        let info = CertificadoA1::carregar(PFX_AES, "1234").unwrap().info().clone();
        assert!(info.pertence_a(&Cnpj::parse("11222333000181").unwrap()));
        // Filial 0002 da mesma raiz.
        assert!(info.pertence_a(&Cnpj::parse("11222333000262").unwrap()));
        assert!(!info.pertence_a(&Cnpj::parse("12ABC34501DE35").unwrap()));
    }

    #[test]
    fn assinatura_confere_com_chave_publica() {
        let cert = CertificadoA1::carregar(PFX_AES, "1234").unwrap();
        let assinatura = cert.assinar_rsa_sha1(b"teste").unwrap();
        let publica = chave_publica_do_certificado(cert.certificado_der()).unwrap();
        publica
            .verify(Pkcs1v15Sign::new::<Sha1>(), &Sha1::digest(b"teste"), &assinatura)
            .unwrap();
    }

    #[test]
    fn debug_nao_vaza_segredos() {
        let cert = CertificadoA1::carregar(PFX_AES, "1234").unwrap();
        assert!(!format!("{cert:?}").contains("1234\""));
    }
}
