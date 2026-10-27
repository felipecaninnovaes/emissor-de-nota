//! Onde ficam os arquivos dos backups: um bucket S3 (o `versitygw` do compose) ou, nos testes, a memória.

use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use reqwest::header::CONTENT_LENGTH;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::config::ConfigS3;

pub type Fluxo = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// Validade das URLs assinadas que o painel usa internamente para falar com o S3.
const VALIDADE: Duration = Duration::from_secs(600);

pub enum Armazenamento {
    S3(S3),
    #[cfg(test)]
    Memoria(std::sync::Mutex<std::collections::HashMap<String, Bytes>>),
}

pub struct S3 {
    bucket: Bucket,
    credenciais: Credentials,
    http: reqwest::Client,
}

impl S3 {
    pub fn novo(c: &ConfigS3) -> Result<S3, String> {
        let endpoint = c.endpoint.parse().map_err(|_| "S3_ENDPOINT inválido")?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, c.bucket.clone(), c.regiao.clone())
            .map_err(|e| format!("bucket inválido: {e}"))?;
        Ok(S3 {
            bucket,
            credenciais: Credentials::new(c.chave.clone(), c.segredo.clone()),
            http: reqwest::Client::new(),
        })
    }
}

async fn conferir(resposta: reqwest::Response, acao: &str) -> Result<reqwest::Response, String> {
    let status = resposta.status();
    if status.is_success() {
        return Ok(resposta);
    }
    let texto = resposta.text().await.unwrap_or_default();
    Err(format!("o S3 recusou {acao} ({status}): {}", texto.trim()))
}

fn falha(acao: &str) -> impl Fn(reqwest::Error) -> String + '_ {
    move |e| format!("sem resposta do S3 ao {acao}: {e}")
}

impl Armazenamento {
    /// Cria o bucket se ainda não existir (primeira subida do compose).
    pub async fn preparar(&self) -> Result<(), String> {
        match self {
            Armazenamento::S3(s3) => {
                let url = s3.bucket.head_bucket(Some(&s3.credenciais)).sign(VALIDADE);
                let existe = s3.http.head(url).send().await.map_err(falha("conferir o bucket"))?;
                if existe.status().is_success() {
                    return Ok(());
                }
                let url = s3.bucket.create_bucket(&s3.credenciais).sign(VALIDADE);
                let resposta = s3.http.put(url).send().await.map_err(falha("criar o bucket"))?;
                conferir(resposta, "criar o bucket").await.map(|_| ())
            }
            #[cfg(test)]
            Armazenamento::Memoria(_) => Ok(()),
        }
    }

    /// Grava o arquivo em streaming. `tamanho` vai no `Content-Length` (o S3 não aceita envio sem ele).
    pub async fn enviar(&self, chave: &str, tamanho: u64, corpo: Fluxo) -> Result<(), String> {
        match self {
            Armazenamento::S3(s3) => {
                let url = s3.bucket.put_object(Some(&s3.credenciais), chave).sign(VALIDADE);
                let resposta = s3
                    .http
                    .put(url)
                    .header(CONTENT_LENGTH, tamanho)
                    .body(reqwest::Body::wrap_stream(corpo))
                    .send()
                    .await
                    .map_err(falha("gravar o backup"))?;
                conferir(resposta, "gravar o backup").await.map(|_| ())
            }
            #[cfg(test)]
            Armazenamento::Memoria(m) => {
                let partes: Vec<Bytes> = corpo.try_collect().await.map_err(|e| e.to_string())?;
                m.lock().unwrap().insert(chave.to_owned(), partes.concat().into());
                Ok(())
            }
        }
    }

    pub async fn baixar(&self, chave: &str) -> Result<Fluxo, String> {
        match self {
            Armazenamento::S3(s3) => {
                let url = s3.bucket.get_object(Some(&s3.credenciais), chave).sign(VALIDADE);
                let resposta = s3.http.get(url).send().await.map_err(falha("ler o backup"))?;
                let resposta = conferir(resposta, "ler o backup").await?;
                Ok(Box::pin(resposta.bytes_stream().map_err(std::io::Error::other)))
            }
            #[cfg(test)]
            Armazenamento::Memoria(m) => {
                let dados = m
                    .lock()
                    .unwrap()
                    .get(chave)
                    .cloned()
                    .ok_or("backup não encontrado no armazenamento")?;
                Ok(Box::pin(futures_util::stream::iter([Ok(dados)])))
            }
        }
    }

    /// Apaga o arquivo; já apagado não é erro.
    pub async fn apagar(&self, chave: &str) -> Result<(), String> {
        match self {
            Armazenamento::S3(s3) => {
                let url = s3.bucket.delete_object(Some(&s3.credenciais), chave).sign(VALIDADE);
                let resposta = s3.http.delete(url).send().await.map_err(falha("apagar o backup"))?;
                if resposta.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(());
                }
                conferir(resposta, "apagar o backup").await.map(|_| ())
            }
            #[cfg(test)]
            Armazenamento::Memoria(m) => {
                m.lock().unwrap().remove(chave);
                Ok(())
            }
        }
    }

    #[cfg(test)]
    pub fn memoria() -> Armazenamento {
        Armazenamento::Memoria(Default::default())
    }

    #[cfg(test)]
    pub fn quantos(&self) -> usize {
        match self {
            Armazenamento::Memoria(m) => m.lock().unwrap().len(),
            Armazenamento::S3(_) => unreachable!("só nos testes"),
        }
    }
}
