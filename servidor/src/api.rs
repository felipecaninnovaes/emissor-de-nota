//! API do aplicativo (`/api/v1`): o token do cliente vai em `Authorization: Bearer`.

use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::banco::{self, Backup, Cliente};
use crate::{Compartilhado, token};

pub const MSG_INATIVO: &str = "Assinatura inativa: novos backups não são enviados. Os backups anteriores continuam \
                               disponíveis para restaurar.";

/// Erro da API: sempre `{ "erro": "..." }`.
#[derive(Debug)]
pub struct ErroApi(pub StatusCode, pub String);

impl IntoResponse for ErroApi {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "erro": self.1 }))).into_response()
    }
}

fn interno(e: impl std::fmt::Display) -> ErroApi {
    tracing::error!("{e}");
    ErroApi(
        StatusCode::INTERNAL_SERVER_ERROR,
        "erro interno do servidor de backup; tente de novo mais tarde".into(),
    )
}

/// Cliente dono do token da requisição.
pub struct ClienteApi(pub Cliente);

impl FromRequestParts<Compartilhado> for ClienteApi {
    type Rejection = ErroApi;

    async fn from_request_parts(parts: &mut Parts, estado: &Compartilhado) -> Result<Self, Self::Rejection> {
        let invalido = || {
            ErroApi(
                StatusCode::UNAUTHORIZED,
                "token inválido: confira o token em Configurações → Backup na nuvem".into(),
            )
        };
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|t| t.trim().starts_with(token::PREFIXO))
            .ok_or_else(invalido)?;
        banco::cliente_por_token(&estado.banco, &token::hash(token))
            .await
            .map_err(interno)?
            .map(ClienteApi)
            .ok_or_else(invalido)
    }
}

#[derive(Serialize)]
struct Situacao {
    cliente: String,
    ativo: bool,
    mantidos: i64,
    tamanho_maximo: u64,
}

#[derive(Serialize)]
struct BackupJson {
    id: i64,
    nome: String,
    tamanho: i64,
    sha256: String,
    criado_em: DateTime<Utc>,
}

impl From<Backup> for BackupJson {
    fn from(b: Backup) -> Self {
        BackupJson {
            id: b.id,
            nome: b.nome,
            tamanho: b.tamanho,
            sha256: b.sha256,
            criado_em: b.criado_em,
        }
    }
}

pub fn rotas() -> Router<Compartilhado> {
    Router::new()
        .route("/api/v1/situacao", get(situacao))
        .route("/api/v1/backups", get(listar).post(enviar))
        .route("/api/v1/backups/{id}", get(baixar))
        // O limite é o TAMANHO_MAXIMO_MB, conferido no Content-Length.
        .layer(DefaultBodyLimit::disable())
}

async fn situacao(State(estado): State<Compartilhado>, ClienteApi(c): ClienteApi) -> Json<Situacao> {
    Json(Situacao {
        cliente: c.nome,
        ativo: c.ativo,
        mantidos: estado.config.manter_backups,
        tamanho_maximo: estado.config.tamanho_maximo,
    })
}

async fn listar(
    State(estado): State<Compartilhado>,
    ClienteApi(c): ClienteApi,
) -> Result<Json<Vec<BackupJson>>, ErroApi> {
    let lista = banco::backups(&estado.banco, c.id).await.map_err(interno)?;
    Ok(Json(lista.into_iter().map(BackupJson::from).collect()))
}

/// Só letras, números, ponto, hífen e sublinhado (o nome vai no `Content-Disposition` do download).
pub fn nome_arquivo(bruto: Option<&str>) -> String {
    let nome: String = bruto
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(100)
        .collect();
    let nome = nome.trim_start_matches('.');
    if nome.is_empty() {
        "backup.nfbk".into()
    } else {
        nome.into()
    }
}

fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

async fn enviar(
    State(estado): State<Compartilhado>,
    ClienteApi(c): ClienteApi,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Result<(StatusCode, Json<BackupJson>), ErroApi> {
    if !c.ativo {
        return Err(ErroApi(StatusCode::FORBIDDEN, MSG_INATIVO.into()));
    }
    let texto = |nome: &str| cabecalhos.get(nome).and_then(|v| v.to_str().ok()).map(str::trim);
    let tamanho: u64 = texto(header::CONTENT_LENGTH.as_str())
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| ErroApi(StatusCode::LENGTH_REQUIRED, "envio sem Content-Length".into()))?;
    if tamanho == 0 {
        return Err(ErroApi(
            StatusCode::BAD_REQUEST,
            "o arquivo do backup está vazio".into(),
        ));
    }
    let maximo = estado.config.tamanho_maximo;
    if tamanho > maximo {
        return Err(ErroApi(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "o backup tem {} MB e o limite do servidor é {} MB",
                megabytes(tamanho),
                megabytes(maximo)
            ),
        ));
    }
    let esperado = texto("x-sha256")
        .map(str::to_ascii_lowercase)
        .filter(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| ErroApi(StatusCode::BAD_REQUEST, "falta o cabeçalho X-Sha256 do arquivo".into()))?;
    let nome = nome_arquivo(texto("x-nome-arquivo"));
    let chave = format!("clientes/{}/{}.nfbk", c.id, token::aleatorio());

    // Calcula o SHA-256 e conta os bytes enquanto o arquivo passa para o S3.
    let conferencia = Arc::new(Mutex::new((Sha256::new(), 0u64)));
    let parcial = conferencia.clone();
    let fluxo = corpo.into_data_stream().map(move |parte| {
        let parte = parte.map_err(std::io::Error::other)?;
        let mut c = parcial.lock().expect("trava do SHA-256");
        c.1 += parte.len() as u64;
        if c.1 > tamanho {
            return Err(std::io::Error::other("o arquivo é maior que o Content-Length"));
        }
        c.0.update(&parte);
        Ok(parte)
    });
    if let Err(e) = estado.armazenamento.enviar(&chave, tamanho, Box::pin(fluxo)).await {
        tracing::warn!(cliente = c.id, "envio interrompido: {e}");
        let _ = estado.armazenamento.apagar(&chave).await;
        return Err(ErroApi(
            StatusCode::BAD_GATEWAY,
            "o backup não chegou inteiro ao servidor; envie de novo".into(),
        ));
    }
    let (hash, recebidos) = {
        let c = conferencia.lock().expect("trava do SHA-256");
        (hex::encode(c.0.clone().finalize()), c.1)
    };
    if recebidos != tamanho || hash != esperado {
        let _ = estado.armazenamento.apagar(&chave).await;
        return Err(ErroApi(
            StatusCode::UNPROCESSABLE_ENTITY,
            "o arquivo chegou diferente do enviado (SHA-256 não confere); envie de novo".into(),
        ));
    }
    let novo = banco::inserir_backup(&estado.banco, c.id, &chave, &nome, tamanho as i64, &hash)
        .await
        .map_err(interno)?;
    tracing::info!(cliente = c.id, backup = novo.id, tamanho, "backup recebido");
    podar(&estado, c.id).await;
    Ok((StatusCode::CREATED, Json(novo.into())))
}

/// Apaga os backups além dos `MANTER_BACKUPS` mais recentes. Falha no S3 mantém a linha para tentar de novo.
pub async fn podar(estado: &Compartilhado, cliente_id: i64) {
    let excedentes = match banco::excedentes(&estado.banco, cliente_id, estado.config.manter_backups).await {
        Ok(e) => e,
        Err(e) => return tracing::error!("retenção do cliente {cliente_id}: {e}"),
    };
    for b in excedentes {
        if let Err(e) = estado.armazenamento.apagar(&b.chave).await {
            tracing::error!("retenção do backup {}: {e}", b.id);
            continue;
        }
        if let Err(e) = banco::apagar_backup(&estado.banco, b.id).await {
            tracing::error!("retenção do backup {}: {e}", b.id);
        }
    }
}

/// Resposta de download com o arquivo em streaming (usada também pelo painel).
pub async fn resposta_download(estado: &Compartilhado, b: &Backup) -> Result<Response, String> {
    let fluxo = estado.armazenamento.baixar(&b.chave).await?;
    let mut resposta = Body::from_stream(fluxo).into_response();
    let h = resposta.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(b.tamanho));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", b.nome)) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    if let Ok(v) = HeaderValue::from_str(&b.sha256) {
        h.insert("x-sha256", v);
    }
    Ok(resposta)
}

/// Download liberado também para cliente inativo: a restauração não depende da assinatura.
async fn baixar(
    State(estado): State<Compartilhado>,
    ClienteApi(c): ClienteApi,
    Path(id): Path<i64>,
) -> Result<Response, ErroApi> {
    let b = banco::backup(&estado.banco, c.id, id)
        .await
        .map_err(interno)?
        .ok_or_else(|| ErroApi(StatusCode::NOT_FOUND, "backup não encontrado".into()))?;
    resposta_download(&estado, &b).await.map_err(interno)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_do_arquivo_so_com_caracteres_seguros() {
        assert_eq!(
            nome_arquivo(Some("emissor-20261027-101500.nfbk")),
            "emissor-20261027-101500.nfbk"
        );
        assert_eq!(nome_arquivo(Some("../a\"b;c.nfbk")), "abc.nfbk");
        assert_eq!(nome_arquivo(Some("çã")), "backup.nfbk");
        assert_eq!(nome_arquivo(None), "backup.nfbk");
        assert_eq!(nome_arquivo(Some(&"a".repeat(300))).len(), 100);
    }

    #[test]
    fn megabytes_arredonda_para_cima() {
        assert_eq!(megabytes(1), 1);
        assert_eq!(megabytes(1024 * 1024), 1);
        assert_eq!(megabytes(1024 * 1024 + 1), 2);
    }
}
