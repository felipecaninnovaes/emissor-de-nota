//! Consultas ao Postgres do painel: clientes, backups e sessões.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Cliente {
    pub id: i64,
    pub nome: String,
    pub documento: String,
    pub email: String,
    pub observacao: String,
    pub ativo: bool,
    pub criado_em: DateTime<Utc>,
}

/// Linha da lista de clientes do painel.
#[derive(Debug, sqlx::FromRow)]
pub struct ResumoCliente {
    pub id: i64,
    pub nome: String,
    pub documento: String,
    pub ativo: bool,
    pub tem_token: bool,
    pub backups: i64,
    pub espaco: i64,
    pub ultimo: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Backup {
    pub id: i64,
    pub chave: String,
    pub nome: String,
    pub tamanho: i64,
    pub sha256: String,
    pub criado_em: DateTime<Utc>,
}

/// Campos que o administrador preenche no cadastro do cliente.
#[derive(Debug, Default, serde::Deserialize)]
pub struct DadosCliente {
    pub nome: String,
    #[serde(default)]
    pub documento: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub observacao: String,
}

const CAMPOS_CLIENTE: &str = "id, nome, documento, email, observacao, ativo, criado_em";
const CAMPOS_BACKUP: &str = "id, chave, nome, tamanho, sha256, criado_em";

pub async fn cliente_por_token(banco: &PgPool, token_hash: &str) -> sqlx::Result<Option<Cliente>> {
    sqlx::query_as(&format!("SELECT {CAMPOS_CLIENTE} FROM cliente WHERE token_hash = $1"))
        .bind(token_hash)
        .fetch_optional(banco)
        .await
}

pub async fn cliente(banco: &PgPool, id: i64) -> sqlx::Result<Option<Cliente>> {
    sqlx::query_as(&format!("SELECT {CAMPOS_CLIENTE} FROM cliente WHERE id = $1"))
        .bind(id)
        .fetch_optional(banco)
        .await
}

pub async fn listar_clientes(banco: &PgPool) -> sqlx::Result<Vec<ResumoCliente>> {
    sqlx::query_as(
        "SELECT c.id, c.nome, c.documento, c.ativo, c.token_hash IS NOT NULL AS tem_token,
                count(b.id) AS backups, coalesce(sum(b.tamanho), 0)::BIGINT AS espaco,
                max(b.criado_em) AS ultimo
         FROM cliente c LEFT JOIN backup b ON b.cliente_id = c.id
         GROUP BY c.id ORDER BY lower(c.nome), c.id",
    )
    .fetch_all(banco)
    .await
}

pub async fn criar_cliente(banco: &PgPool, d: &DadosCliente, token_hash: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO cliente (nome, documento, email, observacao, token_hash)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(d.nome.trim())
    .bind(d.documento.trim())
    .bind(d.email.trim())
    .bind(d.observacao.trim())
    .bind(token_hash)
    .fetch_one(banco)
    .await
}

pub async fn atualizar_cliente(banco: &PgPool, id: i64, d: &DadosCliente) -> sqlx::Result<()> {
    sqlx::query("UPDATE cliente SET nome = $2, documento = $3, email = $4, observacao = $5 WHERE id = $1")
        .bind(id)
        .bind(d.nome.trim())
        .bind(d.documento.trim())
        .bind(d.email.trim())
        .bind(d.observacao.trim())
        .execute(banco)
        .await
        .map(|_| ())
}

pub async fn definir_ativo(banco: &PgPool, id: i64, ativo: bool) -> sqlx::Result<()> {
    sqlx::query("UPDATE cliente SET ativo = $2 WHERE id = $1")
        .bind(id)
        .bind(ativo)
        .execute(banco)
        .await
        .map(|_| ())
}

pub async fn definir_token(banco: &PgPool, id: i64, token_hash: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE cliente SET token_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(token_hash)
        .execute(banco)
        .await
        .map(|_| ())
}

/// Apaga o cliente e as linhas dos backups (os objetos no S3 são apagados antes, por quem chama).
pub async fn excluir_cliente(banco: &PgPool, id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM cliente WHERE id = $1")
        .bind(id)
        .execute(banco)
        .await
        .map(|_| ())
}

/// Backups do cliente, do mais recente para o mais antigo.
pub async fn backups(banco: &PgPool, cliente_id: i64) -> sqlx::Result<Vec<Backup>> {
    sqlx::query_as(&format!(
        "SELECT {CAMPOS_BACKUP} FROM backup WHERE cliente_id = $1 ORDER BY criado_em DESC, id DESC"
    ))
    .bind(cliente_id)
    .fetch_all(banco)
    .await
}

pub async fn backup(banco: &PgPool, cliente_id: i64, id: i64) -> sqlx::Result<Option<Backup>> {
    sqlx::query_as(&format!(
        "SELECT {CAMPOS_BACKUP} FROM backup WHERE cliente_id = $1 AND id = $2"
    ))
    .bind(cliente_id)
    .bind(id)
    .fetch_optional(banco)
    .await
}

pub async fn inserir_backup(
    banco: &PgPool,
    cliente_id: i64,
    chave: &str,
    nome: &str,
    tamanho: i64,
    sha256: &str,
) -> sqlx::Result<Backup> {
    sqlx::query_as(&format!(
        "INSERT INTO backup (cliente_id, chave, nome, tamanho, sha256) VALUES ($1, $2, $3, $4, $5)
         RETURNING {CAMPOS_BACKUP}"
    ))
    .bind(cliente_id)
    .bind(chave)
    .bind(nome)
    .bind(tamanho)
    .bind(sha256)
    .fetch_one(banco)
    .await
}

pub async fn apagar_backup(banco: &PgPool, id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM backup WHERE id = $1")
        .bind(id)
        .execute(banco)
        .await
        .map(|_| ())
}

/// Backups além dos `manter` mais recentes do cliente.
pub async fn excedentes(banco: &PgPool, cliente_id: i64, manter: i64) -> sqlx::Result<Vec<Backup>> {
    sqlx::query_as(&format!(
        "SELECT {CAMPOS_BACKUP} FROM backup WHERE cliente_id = $1
         ORDER BY criado_em DESC, id DESC OFFSET $2"
    ))
    .bind(cliente_id)
    .bind(manter)
    .fetch_all(banco)
    .await
}

pub async fn criar_sessao(banco: &PgPool, id_hash: &str, expira_em: DateTime<Utc>) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO sessao (id_hash, expira_em) VALUES ($1, $2)")
        .bind(id_hash)
        .bind(expira_em)
        .execute(banco)
        .await
        .map(|_| ())
}

pub async fn sessao_valida(banco: &PgPool, id_hash: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM sessao WHERE id_hash = $1 AND expira_em > now())")
        .bind(id_hash)
        .fetch_one(banco)
        .await
}

pub async fn apagar_sessao(banco: &PgPool, id_hash: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessao WHERE id_hash = $1")
        .bind(id_hash)
        .execute(banco)
        .await
        .map(|_| ())
}

pub async fn limpar_sessoes(banco: &PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessao WHERE expira_em <= now()")
        .execute(banco)
        .await
        .map(|_| ())
}
