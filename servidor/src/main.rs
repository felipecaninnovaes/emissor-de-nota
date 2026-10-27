//! Servidor do backup na nuvem do Emissor NFS-e (ADR 0017): painel do administrador e API do aplicativo.
//! O aplicativo envia o backup `.nfbk`, já cifrado com a senha mestra; o servidor só guarda o arquivo.

mod api;
mod armazenamento;
mod banco;
mod config;
mod painel;
mod token;

#[cfg(test)]
mod testes;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::header;
use axum::routing::get;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::armazenamento::{Armazenamento, S3};
use crate::config::Config;

pub struct Estado {
    pub banco: PgPool,
    pub armazenamento: Armazenamento,
    pub config: Config,
    /// Tentativas de login erradas por origem: (quantas, início da janela).
    pub tentativas: Mutex<HashMap<String, (u32, Instant)>>,
}

pub type Compartilhado = Arc<Estado>;

const ESTILO: &str = include_str!("../estatico/estilo.css");

pub fn aplicacao(estado: Compartilhado) -> Router {
    Router::new()
        .route("/saude", get(|| async { "ok" }))
        .route(
            "/estilo.css",
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], ESTILO) }),
        )
        .merge(api::rotas())
        .merge(painel::rotas(estado.clone()))
        .with_state(estado)
}

/// Tenta de novo enquanto o Postgres e o S3 do compose ainda estão subindo.
async fn com_tentativas<T, F, Fut>(o_que: &str, mut acao: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut ultima = String::new();
    for tentativa in 1..=30 {
        match acao().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                tracing::warn!("{o_que} (tentativa {tentativa}): {e}");
                ultima = e;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(format!("{o_que}: {ultima}"))
}

async fn iniciar() -> Result<(), String> {
    let config = Config::do_ambiente()?;
    let banco = com_tentativas("conectar ao Postgres", || async {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(&config.banco_url)
            .await
            .map_err(|e| e.to_string())
    })
    .await?;
    sqlx::migrate!("./migrations")
        .run(&banco)
        .await
        .map_err(|e| format!("migrações do banco: {e}"))?;
    let armazenamento = Armazenamento::S3(S3::novo(&config.s3)?);
    com_tentativas("preparar o bucket do S3", || armazenamento.preparar()).await?;

    let endereco = config.endereco;
    let estado = Arc::new(Estado {
        banco,
        armazenamento,
        config,
        tentativas: Mutex::default(),
    });

    let limpeza = estado.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            if let Err(e) = banco::limpar_sessoes(&limpeza.banco).await {
                tracing::warn!("limpeza das sessões: {e}");
            }
        }
    });

    let ouvinte = tokio::net::TcpListener::bind(endereco)
        .await
        .map_err(|e| format!("não foi possível escutar em {endereco}: {e}"))?;
    tracing::info!("painel em http://{endereco}");
    axum::serve(
        ouvinte,
        aplicacao(estado).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(desligar())
    .await
    .map_err(|e| e.to_string())
}

async fn desligar() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let termino = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let termino = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = termino => {},
    }
}

/// `emissor-nuvem saude`: o HEALTHCHECK do container (a imagem não tem curl). Pede `/saude` na porta local.
async fn conferir_saude() -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let porta = std::env::var("ENDERECO")
        .ok()
        .and_then(|e| e.parse::<SocketAddr>().ok())
        .map_or(8080, |e| e.port());
    let Ok(mut conexao) = tokio::net::TcpStream::connect(("127.0.0.1", porta)).await else {
        return false;
    };
    let mut resposta = Vec::new();
    conexao
        .write_all(b"GET /saude HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await
        .is_ok()
        && conexao.read_to_end(&mut resposta).await.is_ok()
        && String::from_utf8_lossy(&resposta)
            .lines()
            .next()
            .is_some_and(|linha| linha.contains(" 200 "))
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("saude") {
        std::process::exit(if conferir_saude().await { 0 } else { 1 });
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "emissor_nuvem=info,tower_http=warn".into()),
        )
        .init();
    if let Err(e) = iniciar().await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
}
