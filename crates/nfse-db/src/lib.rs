//! Banco local (SQLite) do emissor: cofre dos certificados, cadastros, numeração e notas.
//!
//! Um arquivo por máquina (decisão do escopo). As migrações ficam em `migrations/` e rodam
//! ao abrir o banco.

pub mod cofre;
pub mod dps;
pub mod empresas;
pub mod servicos;
pub mod tomadores;

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

pub use cofre::{CabecalhoCofre, Cofre, ErroCofre};

#[derive(Debug, thiserror::Error)]
pub enum ErroBanco {
    #[error("erro no banco de dados: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("erro ao migrar o banco: {0}")]
    Migracao(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Cofre(#[from] ErroCofre),
    #[error(transparent)]
    Certificado(#[from] nfse_cert::ErroCertificado),
    #[error("dado gravado inválido ({0}); o banco pode ter sido alterado fora do sistema")]
    DadoInvalido(String),
    #[error("{0}")]
    Regra(String),
    #[error("{0} não encontrado(a)")]
    NaoEncontrado(&'static str),
}

pub type Resultado<T> = Result<T, ErroBanco>;

pub(crate) fn agora() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub(crate) fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("tipos do modelo sempre serializam")
}

pub(crate) fn de_json<T: serde::de::DeserializeOwned>(s: &str, campo: &str) -> Resultado<T> {
    serde_json::from_str(s).map_err(|e| ErroBanco::DadoInvalido(format!("{campo}: {e}")))
}

#[derive(Clone)]
pub struct Banco {
    pool: SqlitePool,
}

impl Banco {
    /// Abre (ou cria) o arquivo do banco e aplica as migrações pendentes.
    pub async fn abrir(caminho: &Path) -> Resultado<Self> {
        let opcoes = SqliteConnectOptions::new()
            .filename(caminho)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(10));
        Self::conectar(opcoes, 4).await
    }

    /// Banco temporário em memória (testes).
    pub async fn em_memoria() -> Resultado<Self> {
        let opcoes = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // Uma conexão só: cada conexão em memória seria um banco diferente.
        Self::conectar(opcoes, 1).await
    }

    async fn conectar(opcoes: SqliteConnectOptions, conexoes: u32) -> Resultado<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(conexoes)
            .connect_with(opcoes)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn configuracao(&self, chave: &str) -> Resultado<Option<String>> {
        Ok(sqlx::query("SELECT valor FROM configuracao WHERE chave = ?")
            .bind(chave)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.get(0)))
    }

    pub async fn cofre_existe(&self) -> Resultado<bool> {
        Ok(self.configuracao("cofre").await?.is_some())
    }

    /// Cria o cofre no primeiro uso. Recusa se já existir (trocar a senha exige recifrar tudo).
    pub async fn criar_cofre(&self, senha: &str) -> Resultado<Cofre> {
        if self.cofre_existe().await? {
            return Err(ErroBanco::Regra(
                "o cofre já foi criado; use a senha mestra existente".into(),
            ));
        }
        let (cofre, cabecalho) = Cofre::criar(senha)?;
        self.gravar_cabecalho(&cabecalho).await?;
        Ok(cofre)
    }

    #[cfg(test)]
    pub(crate) async fn criar_cofre_rapido(&self, senha: &str) -> Resultado<Cofre> {
        let (cofre, cabecalho) = cofre::testes::cofre_rapido(senha);
        self.gravar_cabecalho(&cabecalho).await?;
        Ok(cofre)
    }

    async fn gravar_cabecalho(&self, cabecalho: &CabecalhoCofre) -> Resultado<()> {
        sqlx::query("INSERT INTO configuracao (chave, valor) VALUES ('cofre', ?)")
            .bind(json(cabecalho))
            .execute(&self.pool)
            .await?;
        self.auditar("cofre_criado", None, None).await
    }

    pub async fn abrir_cofre(&self, senha: &str) -> Resultado<Cofre> {
        let cabecalho = self
            .configuracao("cofre")
            .await?
            .ok_or(ErroBanco::NaoEncontrado("cofre"))?;
        Ok(Cofre::abrir(senha, &de_json(&cabecalho, "cofre")?)?)
    }

    pub(crate) async fn auditar(&self, acao: &str, empresa_id: Option<i64>, detalhe: Option<&str>) -> Resultado<()> {
        sqlx::query("INSERT INTO auditoria (quando, acao, empresa_id, detalhe) VALUES (?, ?, ?, ?)")
            .bind(agora())
            .bind(acao)
            .bind(empresa_id)
            .bind(detalhe)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Últimas ações registradas (mais recentes primeiro).
    pub async fn auditoria(&self, limite: i64) -> Resultado<Vec<(String, String, Option<i64>, Option<String>)>> {
        let linhas = sqlx::query("SELECT quando, acao, empresa_id, detalhe FROM auditoria ORDER BY id DESC LIMIT ?")
            .bind(limite)
            .fetch_all(&self.pool)
            .await?;
        Ok(linhas
            .into_iter()
            .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
            .collect())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[tokio::test]
    async fn cofre_no_banco() {
        let banco = Banco::em_memoria().await.unwrap();
        assert!(!banco.cofre_existe().await.unwrap());
        let cofre = banco.criar_cofre_rapido("senha-mestra").await.unwrap();
        let blob = cofre.cifrar(b"x", "c").unwrap();
        assert!(banco.cofre_existe().await.unwrap());
        assert!(matches!(
            banco.criar_cofre("senha-mestra").await,
            Err(ErroBanco::Regra(_))
        ));
        let reaberto = banco.abrir_cofre("senha-mestra").await.unwrap();
        assert_eq!(reaberto.decifrar(&blob, "c").unwrap().as_slice(), b"x");
        assert!(matches!(
            banco.abrir_cofre("errada!!").await,
            Err(ErroBanco::Cofre(ErroCofre::SenhaIncorreta))
        ));
        assert_eq!(banco.auditoria(10).await.unwrap()[0].1, "cofre_criado");
    }

    #[tokio::test]
    async fn arquivo_em_disco_com_wal() {
        let dir = std::env::temp_dir().join(format!("nfse-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("emissor.db");
        {
            let banco = Banco::abrir(&caminho).await.unwrap();
            banco.criar_cofre_rapido("senha-mestra").await.unwrap();
            banco.pool().close().await;
        }
        let banco = Banco::abrir(&caminho).await.unwrap();
        assert!(banco.cofre_existe().await.unwrap());
        banco.pool().close().await;
        // No Windows o -wal/-shm pode continuar preso por alguns instantes; limpeza é só cortesia.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
