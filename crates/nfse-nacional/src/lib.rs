//! Cliente mTLS da API Sefin Nacional (NFS-e Padrão Nacional).
//!
//! Endpoints e formatos conforme as bibliotecas de referência citadas no escopo;
//! **confirmar no Manual de Integração / Swagger oficial** antes de ir para Produção.
//!
//! Regra de idempotência: se `emitir` devolver [`ErroSefin::Incerto`], a DPS pode ter sido
//! autorizada. Consultar com [`ClienteSefin::consultar_dps`] antes de qualquer reenvio.

pub mod mensagens;

use std::io::{Read, Write};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use reqwest::StatusCode;
use serde::Deserialize;

use nfse_cert::CertificadoA1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambiente {
    /// Homologação ("Produção Restrita"): notas sem valor fiscal.
    ProducaoRestrita,
    Producao,
}

impl Ambiente {
    pub fn url_sefin(self) -> &'static str {
        match self {
            Ambiente::ProducaoRestrita => "https://sefin.producaorestrita.nfse.gov.br/SefinNacional",
            Ambiente::Producao => "https://sefin.nfse.gov.br/SefinNacional",
        }
    }

    /// Valor de `tpAmb` na DPS.
    pub fn tp_amb(self) -> u8 {
        match self {
            Ambiente::Producao => 1,
            Ambiente::ProducaoRestrita => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MensagemSefin {
    #[serde(alias = "Codigo")]
    pub codigo: Option<String>,
    #[serde(alias = "Descricao")]
    pub descricao: Option<String>,
    #[serde(alias = "Complemento")]
    pub complemento: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErroSefin {
    #[error("certificado não aceito para mTLS: {0}")]
    Identidade(String),
    /// A requisição não chegou à Sefin (DNS, conexão, handshake TLS). Pode reenviar.
    #[error("sem conexão com a Sefin: {0}")]
    SemConexao(String),
    /// A requisição pode ter sido processada (timeout, conexão caiu no meio). **Não reenviar sem reconciliar.**
    #[error("resposta não recebida; situação da DPS incerta: {0}")]
    Incerto(String),
    #[error("rejeitada pela Sefin (HTTP {status}): {}", resumo(.erros))]
    Rejeitada {
        status: u16,
        erros: Vec<MensagemSefin>,
        corpo: String,
    },
    #[error("HTTP {status} inesperado: {corpo}")]
    Http { status: u16, corpo: String },
    #[error("limite de requisições atingido (HTTP 429)")]
    LimiteRequisicoes,
    #[error("resposta em formato inesperado: {0}")]
    Formato(String),
}

fn resumo(erros: &[MensagemSefin]) -> String {
    erros
        .iter()
        .map(|e| {
            format!(
                "{} {}",
                e.codigo.as_deref().unwrap_or("?"),
                e.descricao.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpoResposta {
    #[serde(alias = "idDPS")]
    id_dps: Option<String>,
    chave_acesso: Option<String>,
    nfse_xml_g_zip_b64: Option<String>,
    evento_xml_g_zip_b64: Option<String>,
    #[serde(default)]
    alertas: Option<Vec<MensagemSefin>>,
    #[serde(default)]
    erros: Option<Vec<MensagemSefin>>,
    #[serde(default)]
    erro: Option<MensagemSefin>,
}

#[derive(Debug, Clone)]
pub struct NfseAutorizada {
    pub id_dps: Option<String>,
    pub chave_acesso: String,
    /// XML da NFS-e como devolvido pela Sefin: é o documento fiscal, guardar sem alterar.
    pub xml: String,
    pub alertas: Vec<MensagemSefin>,
}

pub fn gzip_base64(xml: &str) -> String {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(xml.as_bytes()).expect("escrita em memória");
    B64.encode(gz.finish().expect("escrita em memória"))
}

pub fn de_gzip_base64(dados: &str) -> Result<String, ErroSefin> {
    let bytes = B64
        .decode(dados.trim())
        .map_err(|e| ErroSefin::Formato(format!("base64: {e}")))?;
    let mut xml = String::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut xml)
        .map_err(|e| ErroSefin::Formato(format!("gzip: {e}")))?;
    Ok(xml)
}

pub struct ClienteSefin {
    http: reqwest::Client,
    base: String,
}

impl ClienteSefin {
    pub fn novo(certificado: &CertificadoA1, ambiente: Ambiente) -> Result<Self, ErroSefin> {
        Self::com_url_base(certificado, ambiente.url_sefin())
    }

    pub fn com_url_base(certificado: &CertificadoA1, base: &str) -> Result<Self, ErroSefin> {
        let (pfx, senha) = certificado.pkcs12();
        let identidade =
            reqwest::Identity::from_pkcs12_der(pfx, senha).map_err(|e| ErroSefin::Identidade(e.to_string()))?;
        let http = reqwest::Client::builder()
            .tls_backend_native()
            .identity(identidade)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(90))
            .user_agent(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ErroSefin::Identidade(e.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        })
    }

    /// Envia a DPS assinada. Síncrono: em caso de sucesso já devolve a NFS-e autorizada.
    pub async fn emitir(&self, dps_assinada: &str) -> Result<NfseAutorizada, ErroSefin> {
        let corpo = serde_json::json!({ "dpsXmlGZipB64": gzip_base64(dps_assinada) });
        let resposta = self
            .http
            .post(format!("{}/nfse", self.base))
            .json(&corpo)
            .send()
            .await
            .map_err(classificar_envio)?;
        let (status, texto) = ler(resposta).await?;
        if status.is_success() {
            nfse_da_resposta(&texto)
        } else {
            Err(erro_http(status, texto))
        }
    }

    /// Chave de acesso da NFS-e gerada a partir da DPS, ou `None` se a Sefin não tem NFS-e para ela.
    /// É a base da reconciliação depois de um envio incerto.
    pub async fn consultar_dps(&self, id_dps: &str) -> Result<Option<String>, ErroSefin> {
        let resposta = self
            .http
            .get(format!("{}/dps/{}", self.base, id_dps))
            .send()
            .await
            .map_err(classificar_consulta)?;
        let (status, texto) = ler(resposta).await?;
        match status {
            StatusCode::NOT_FOUND => Ok(None),
            s if s.is_success() => {
                let corpo: CorpoResposta =
                    serde_json::from_str(&texto).map_err(|e| ErroSefin::Formato(e.to_string()))?;
                // Só o 404 prova que a NFS-e não existe: sucesso sem chave é resposta inesperada.
                corpo
                    .chave_acesso
                    .map(Some)
                    .ok_or_else(|| ErroSefin::Formato("consulta da DPS sem chaveAcesso".into()))
            }
            s => Err(erro_http(s, texto)),
        }
    }

    pub async fn consultar_nfse(&self, chave_acesso: &str) -> Result<NfseAutorizada, ErroSefin> {
        let resposta = self
            .http
            .get(format!("{}/nfse/{}", self.base, chave_acesso))
            .send()
            .await
            .map_err(classificar_consulta)?;
        let (status, texto) = ler(resposta).await?;
        if status.is_success() {
            nfse_da_resposta(&texto)
        } else {
            Err(erro_http(status, texto))
        }
    }

    /// Registra um evento (ex.: cancelamento) e devolve o XML do evento homologado.
    /// Repetir o mesmo pedido não duplica o evento (o Id do pedido é fixo por chave e tipo).
    pub async fn registrar_evento(&self, chave_acesso: &str, pedido_assinado: &str) -> Result<String, ErroSefin> {
        let corpo = serde_json::json!({ "pedidoRegistroEventoXmlGZipB64": gzip_base64(pedido_assinado) });
        let resposta = self
            .http
            .post(format!("{}/nfse/{}/eventos", self.base, chave_acesso))
            .json(&corpo)
            .send()
            .await
            .map_err(classificar_envio)?;
        let (status, texto) = ler(resposta).await?;
        if !status.is_success() {
            return Err(erro_http(status, texto));
        }
        let corpo: CorpoResposta = serde_json::from_str(&texto).map_err(|e| ErroSefin::Formato(e.to_string()))?;
        match (corpo.evento_xml_g_zip_b64, corpo.erros.filter(|e| !e.is_empty())) {
            (Some(evento), _) => de_gzip_base64(&evento),
            (None, Some(erros)) => Err(ErroSefin::Rejeitada {
                status: status.as_u16(),
                erros,
                corpo: texto,
            }),
            (None, None) => Err(ErroSefin::Formato("resposta sem eventoXmlGZipB64".into())),
        }
    }
}

/// Operações da Sefin usadas pelo emissor. Existe para trocar o cliente real por um simulado nos testes.
#[allow(async_fn_in_trait)]
pub trait Sefin {
    async fn emitir(&self, dps_assinada: &str) -> Result<NfseAutorizada, ErroSefin>;
    async fn consultar_dps(&self, id_dps: &str) -> Result<Option<String>, ErroSefin>;
    async fn consultar_nfse(&self, chave_acesso: &str) -> Result<NfseAutorizada, ErroSefin>;
    async fn registrar_evento(&self, chave_acesso: &str, pedido_assinado: &str) -> Result<String, ErroSefin>;
}

impl Sefin for ClienteSefin {
    async fn emitir(&self, dps_assinada: &str) -> Result<NfseAutorizada, ErroSefin> {
        ClienteSefin::emitir(self, dps_assinada).await
    }

    async fn consultar_dps(&self, id_dps: &str) -> Result<Option<String>, ErroSefin> {
        ClienteSefin::consultar_dps(self, id_dps).await
    }

    async fn consultar_nfse(&self, chave_acesso: &str) -> Result<NfseAutorizada, ErroSefin> {
        ClienteSefin::consultar_nfse(self, chave_acesso).await
    }

    async fn registrar_evento(&self, chave_acesso: &str, pedido_assinado: &str) -> Result<String, ErroSefin> {
        ClienteSefin::registrar_evento(self, chave_acesso, pedido_assinado).await
    }
}

/// Erro no envio da emissão: só é seguro reenviar se a conexão nem chegou a ser estabelecida.
fn classificar_envio(e: reqwest::Error) -> ErroSefin {
    if e.is_connect() && !e.is_timeout() {
        ErroSefin::SemConexao(e.to_string())
    } else {
        ErroSefin::Incerto(e.to_string())
    }
}

/// Em consultas não há risco de duplicidade.
fn classificar_consulta(e: reqwest::Error) -> ErroSefin {
    ErroSefin::SemConexao(e.to_string())
}

async fn ler(resposta: reqwest::Response) -> Result<(StatusCode, String), ErroSefin> {
    let status = resposta.status();
    // Falha ao ler o corpo depois do status: a Sefin já processou, então é incerto.
    let texto = resposta.text().await.map_err(|e| ErroSefin::Incerto(e.to_string()))?;
    Ok((status, texto))
}

fn nfse_da_resposta(texto: &str) -> Result<NfseAutorizada, ErroSefin> {
    let corpo: CorpoResposta = serde_json::from_str(texto).map_err(|e| ErroSefin::Formato(e.to_string()))?;
    let chave_acesso = corpo
        .chave_acesso
        .ok_or_else(|| ErroSefin::Formato("resposta sem chaveAcesso".into()))?;
    let xml = de_gzip_base64(
        &corpo
            .nfse_xml_g_zip_b64
            .ok_or_else(|| ErroSefin::Formato("resposta sem nfseXmlGZipB64".into()))?,
    )?;
    Ok(NfseAutorizada {
        id_dps: corpo.id_dps,
        chave_acesso,
        xml,
        alertas: corpo.alertas.unwrap_or_default(),
    })
}

fn erro_http(status: StatusCode, corpo: String) -> ErroSefin {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return ErroSefin::LimiteRequisicoes;
    }
    match serde_json::from_str::<CorpoResposta>(&corpo) {
        Ok(c) if c.erros.as_ref().is_some_and(|e| !e.is_empty()) || c.erro.is_some() => {
            let mut erros = c.erros.unwrap_or_default();
            erros.extend(c.erro);
            ErroSefin::Rejeitada {
                status: status.as_u16(),
                erros,
                corpo,
            }
        }
        _ => ErroSefin::Http {
            status: status.as_u16(),
            corpo,
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn gzip_base64_ida_e_volta() {
        let xml = "<DPS>acentuação</DPS>";
        assert_eq!(de_gzip_base64(&gzip_base64(xml)).unwrap(), xml);
    }

    #[test]
    fn le_resposta_de_sucesso() {
        let json = serde_json::json!({
            "tipoAmbiente": 2,
            "idDps": "DPS1",
            "chaveAcesso": "35111022112223330001810000000000000042261012345678",
            "nfseXmlGZipB64": gzip_base64("<NFSe/>"),
            "alertas": [{"codigo": "A001", "descricao": "aviso"}]
        });
        let nfse = nfse_da_resposta(&json.to_string()).unwrap();
        assert_eq!(nfse.xml, "<NFSe/>");
        assert_eq!(nfse.chave_acesso.len(), 50);
        assert_eq!(nfse.alertas[0].codigo.as_deref(), Some("A001"));
    }

    #[test]
    fn le_rejeicao() {
        let corpo = r#"{"idDPS":"DPS1","erros":[{"Codigo":"E0717","Descricao":"Assinatura inválida"}]}"#;
        match erro_http(StatusCode::BAD_REQUEST, corpo.into()) {
            ErroSefin::Rejeitada { status, erros, .. } => {
                assert_eq!(status, 400);
                assert_eq!(erros[0].codigo.as_deref(), Some("E0717"));
            }
            outro => panic!("{outro:?}"),
        }
        assert!(matches!(
            erro_http(StatusCode::TOO_MANY_REQUESTS, String::new()),
            ErroSefin::LimiteRequisicoes
        ));
        assert!(matches!(
            erro_http(StatusCode::BAD_GATEWAY, "<html>".into()),
            ErroSefin::Http { status: 502, .. }
        ));
    }

    #[test]
    fn identidade_mtls_a_partir_do_pfx() {
        let pfx = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-legacy.pfx");
        let cert = CertificadoA1::carregar(pfx, "1234").unwrap();
        // Só monta o cliente (SChannel no Windows); não faz requisição.
        ClienteSefin::novo(&cert, Ambiente::ProducaoRestrita).unwrap();
    }
}
