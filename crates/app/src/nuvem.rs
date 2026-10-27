//! Cliente do servidor de backup na nuvem (API `/api/v1`, token do cliente no `Authorization`).
//! O arquivo enviado é o `.nfbk` já cifrado com a senha mestra: o servidor nunca vê os dados.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;

/// Consultas curtas (situação e lista); envio e download só têm limite de conexão e de leitura.
const PRAZO_CONSULTA: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Situacao {
    pub cliente: String,
    pub ativo: bool,
    /// Quantos backups o servidor guarda por cliente.
    pub mantidos: i64,
    /// Em bytes.
    pub tamanho_maximo: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupNuvem {
    pub id: i64,
    pub nome: String,
    pub tamanho: i64,
    #[serde(default)]
    pub sha256: String,
    pub criado_em: String,
}

#[derive(Deserialize)]
struct Erro {
    erro: String,
}

pub struct ClienteNuvem {
    base: String,
    token: Zeroizing<String>,
    http: reqwest::Client,
}

impl ClienteNuvem {
    pub fn novo(endereco: &str, token: Zeroizing<String>) -> Result<Self, String> {
        let endereco = nfse_db::nuvem::normalizar_endereco(endereco)?;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(120))
            .user_agent(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: format!("{endereco}/api/v1"),
            token,
            http,
        })
    }

    fn pedido(&self, metodo: reqwest::Method, caminho: &str) -> reqwest::RequestBuilder {
        self.http
            .request(metodo, format!("{}{caminho}", self.base))
            .bearer_auth(self.token.as_str())
    }

    pub async fn situacao(&self) -> Result<Situacao, String> {
        let resposta = enviar(self.pedido(reqwest::Method::GET, "/situacao").timeout(PRAZO_CONSULTA)).await?;
        ler_json(resposta).await
    }

    pub async fn listar(&self) -> Result<Vec<BackupNuvem>, String> {
        let resposta = enviar(self.pedido(reqwest::Method::GET, "/backups").timeout(PRAZO_CONSULTA)).await?;
        ler_json(resposta).await
    }

    /// Envia o arquivo em streaming, com o SHA-256 para o servidor conferir.
    pub async fn enviar(&self, arquivo: &Path, sha256: &str) -> Result<BackupNuvem, String> {
        let nome = arquivo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let aberto = tokio::fs::File::open(arquivo)
            .await
            .map_err(|e| format!("não foi possível ler o backup: {e}"))?;
        let tamanho = aberto.metadata().await.map_err(|e| e.to_string())?.len();
        let pedido = self
            .pedido(reqwest::Method::POST, "/backups")
            .header(reqwest::header::CONTENT_LENGTH, tamanho)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Sha256", sha256)
            .header("X-Nome-Arquivo", nome)
            .body(reqwest::Body::from(aberto));
        ler_json(enviar(pedido).await?).await
    }

    /// Baixa o backup `id` para `destino` (arquivo novo).
    pub async fn baixar(&self, id: i64, destino: &Path) -> Result<(), String> {
        let mut resposta = enviar(self.pedido(reqwest::Method::GET, &format!("/backups/{id}"))).await?;
        let mut arquivo = tokio::fs::File::create_new(destino)
            .await
            .map_err(|e| format!("não foi possível gravar o backup baixado: {e}"))?;
        while let Some(pedaco) = resposta
            .chunk()
            .await
            .map_err(|e| format!("o download do backup foi interrompido: {e}"))?
        {
            arquivo.write_all(&pedaco).await.map_err(|e| e.to_string())?;
        }
        arquivo.sync_all().await.map_err(|e| e.to_string())
    }
}

/// Faz o pedido; status de erro vira a mensagem do servidor (`{ "erro": "..." }`).
async fn enviar(pedido: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    let resposta = pedido
        .send()
        .await
        .map_err(|e| format!("sem conexão com o servidor de backup: {e}"))?;
    let status = resposta.status();
    if status.is_success() {
        return Ok(resposta);
    }
    let corpo = resposta.text().await.unwrap_or_default();
    Err(mensagem_de_erro(status, &corpo))
}

fn mensagem_de_erro(status: reqwest::StatusCode, corpo: &str) -> String {
    let do_servidor = serde_json::from_str::<Erro>(corpo)
        .ok()
        .map(|e| e.erro)
        .filter(|e| !e.trim().is_empty());
    match (status.as_u16(), do_servidor) {
        (401, _) => "token do backup na nuvem inválido; confira o token informado pelo suporte".into(),
        (_, Some(erro)) => erro,
        (403, None) => "assinatura do backup na nuvem inativa".into(),
        (413, None) => "backup maior que o limite do servidor".into(),
        _ => format!("o servidor de backup respondeu HTTP {status}"),
    }
}

async fn ler_json<T: serde::de::DeserializeOwned>(resposta: reqwest::Response) -> Result<T, String> {
    resposta
        .json()
        .await
        .map_err(|e| format!("resposta inválida do servidor de backup: {e}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn erro_do_servidor_vira_mensagem() {
        use reqwest::StatusCode;
        assert_eq!(
            mensagem_de_erro(StatusCode::FORBIDDEN, r#"{"erro":"assinatura inativa"}"#),
            "assinatura inativa"
        );
        assert!(mensagem_de_erro(StatusCode::UNAUTHORIZED, r#"{"erro":"x"}"#).contains("token"));
        assert!(mensagem_de_erro(StatusCode::PAYLOAD_TOO_LARGE, "").contains("limite"));
        assert!(mensagem_de_erro(StatusCode::BAD_GATEWAY, "<html>").contains("502"));
    }

    #[test]
    fn recusa_http_fora_de_localhost() {
        let token = || Zeroizing::new("t".to_string());
        assert!(ClienteNuvem::novo("http://backup.exemplo.com", token()).is_err());
        let cliente = ClienteNuvem::novo("https://backup.exemplo.com/", token()).unwrap();
        assert_eq!(cliente.base, "https://backup.exemplo.com/api/v1");
    }
}
