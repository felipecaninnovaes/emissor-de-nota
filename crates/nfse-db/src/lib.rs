//! Banco local (SQLite) do emissor: cofre dos certificados, cadastros, numeração e notas.
//!
//! Um arquivo por máquina (decisão do escopo). As migrações ficam em `migrations/` e rodam
//! ao abrir o banco.

mod abertura;
mod ajustes;
pub mod backup;
pub mod cofre;
pub mod dps;
pub mod emails;
pub mod empresas;
pub mod importacoes;
pub mod recorrencias;
pub mod servicos;
pub mod tomadores;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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
    /// Banco temporário em memória (testes). Sem cifra: não há arquivo.
    pub async fn em_memoria() -> Resultado<Self> {
        let opcoes = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // Uma conexão só: cada conexão em memória seria um banco diferente.
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opcoes).await?;
        let banco = Self::com_pool(pool);
        banco.preparar().await?;
        Ok(banco)
    }

    pub(crate) fn com_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Migrações e ajustes de esquema pendentes.
    pub(crate) async fn preparar(&self) -> Resultado<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        ajustes::aplicar(&self.pool).await
    }

    /// Fecha as conexões (e com elas o acesso ao arquivo cifrado).
    pub async fn fechar(&self) {
        self.pool.close().await;
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Preferência gravada no banco (ex.: pasta de saída).
    pub async fn configuracao(&self, chave: &str) -> Resultado<Option<String>> {
        Ok(sqlx::query("SELECT valor FROM configuracao WHERE chave = ?")
            .bind(chave)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.get(0)))
    }

    pub async fn definir_configuracao(&self, chave: &str, valor: &str) -> Resultado<()> {
        if chave == "cofre" {
            return Err(ErroBanco::Regra("chave reservada".into()));
        }
        sqlx::query("INSERT INTO configuracao (chave, valor) VALUES (?, ?) ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor")
            .bind(chave)
            .bind(valor)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Cofre com Argon2 leve para os testes dos crates que usam o banco em memória. Não grava
    /// o cabeçalho: quem precisa de arquivo usa [`Banco::criar_protegido_rapido`].
    #[cfg(any(test, feature = "teste"))]
    pub async fn criar_cofre_rapido(&self, senha: &str) -> Resultado<Cofre> {
        let (cofre, _) = Cofre::criar_com(senha, 1024, 1, 1)?;
        Ok(cofre)
    }

    pub async fn auditar(&self, acao: &str, empresa_id: Option<i64>, detalhe: Option<&str>) -> Resultado<()> {
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

/// Registra na auditoria dentro da transação, para a ação e o registro irem juntos.
pub(crate) async fn auditar_em(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    acao: &str,
    empresa_id: i64,
    detalhe: &str,
) -> Resultado<()> {
    sqlx::query("INSERT INTO auditoria (quando, acao, empresa_id, detalhe) VALUES (?, ?, ?, ?)")
        .bind(agora())
        .bind(acao)
        .bind(empresa_id)
        .bind(detalhe)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_temp(nome: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nfse-db-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn cofre_e_banco_cifrado_em_disco() {
        let dir = pasta_temp("cifrado");
        let caminho = dir.join("emissor.db");
        assert!(!Banco::cofre_existe(caminho.clone()).await.unwrap());
        let (banco, cofre) = Banco::criar_protegido_rapido(caminho.clone(), "senha-mestra")
            .await
            .unwrap();
        banco
            .definir_configuracao("marca", "texto-secreto-no-banco")
            .await
            .unwrap();
        let blob = cofre.cifrar(b"x", "c").unwrap();
        assert_eq!(banco.auditoria(10).await.unwrap()[0].1, "cofre_criado");
        banco.fechar().await;

        assert!(Banco::cofre_existe(caminho.clone()).await.unwrap());
        assert!(matches!(
            Banco::criar_protegido_rapido(caminho.clone(), "senha-mestra").await,
            Err(ErroBanco::Regra(_))
        ));
        // Nada legível no arquivo, nem no WAL.
        for arquivo in ["emissor.db", "emissor.db-wal"] {
            if let Ok(bytes) = std::fs::read(dir.join(arquivo)) {
                assert!(!bytes.starts_with(b"SQLite format 3"), "{arquivo} em claro");
                assert!(
                    !bytes.windows(22).any(|w| w == b"texto-secreto-no-banco"),
                    "{arquivo} legível"
                );
            }
        }

        assert!(matches!(
            Banco::abrir_protegido(caminho.clone(), "errada!!".into()).await,
            Err(ErroBanco::Cofre(ErroCofre::SenhaIncorreta))
        ));
        let (banco, reaberto) = Banco::abrir_protegido(caminho.clone(), "senha-mestra".into())
            .await
            .unwrap();
        assert_eq!(reaberto.decifrar(&blob, "c").unwrap().as_slice(), b"x");
        assert_eq!(
            banco.configuracao("marca").await.unwrap().as_deref(),
            Some("texto-secreto-no-banco")
        );
        banco.fechar().await;
        // No Windows o -wal/-shm pode continuar preso por alguns instantes; limpeza é só cortesia.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cabecalho_de_outro_banco_nao_abre() {
        let dir = pasta_temp("outro");
        let (a, _) = Banco::criar_protegido_rapido(dir.join("a.db"), "senha-mestra")
            .await
            .unwrap();
        a.fechar().await;
        let (b, _) = Banco::criar_protegido_rapido(dir.join("b.db"), "senha-mestra")
            .await
            .unwrap();
        b.fechar().await;
        std::fs::copy(dir.join("b.cofre"), dir.join("a.cofre")).unwrap();
        assert!(matches!(
            Banco::abrir_protegido(dir.join("a.db"), "senha-mestra".into()).await,
            Err(ErroBanco::Regra(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Banco de versão anterior: SQLite em claro, com o cabeçalho do cofre na tabela `configuracao`.
    #[tokio::test]
    async fn converte_banco_em_claro_de_versao_anterior() {
        let dir = pasta_temp("legado");
        let caminho = dir.join("emissor.db");
        let (cofre, cabecalho) = Cofre::criar_com("senha-mestra", 1024, 1, 1).unwrap();
        let blob = cofre.cifrar(b"certificado", "c").unwrap();
        {
            let opcoes = SqliteConnectOptions::new().filename(&caminho).create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opcoes)
                .await
                .unwrap();
            let antigo = Banco::com_pool(pool);
            antigo.preparar().await.unwrap();
            sqlx::query("INSERT INTO configuracao (chave, valor) VALUES ('cofre', ?)")
                .bind(json(&cabecalho))
                .execute(antigo.pool())
                .await
                .unwrap();
            antigo.definir_configuracao("marca", "dado-antigo").await.unwrap();
            antigo.fechar().await;
        }
        assert!(Banco::cofre_existe(caminho.clone()).await.unwrap());
        assert!(matches!(
            Banco::abrir_protegido(caminho.clone(), "errada!!".into()).await,
            Err(ErroBanco::Cofre(ErroCofre::SenhaIncorreta))
        ));
        assert!(std::fs::read(&caminho).unwrap().starts_with(b"SQLite format 3"));

        let (banco, reaberto) = Banco::abrir_protegido(caminho.clone(), "senha-mestra".into())
            .await
            .unwrap();
        // Mesma senha, mesma chave dos certificados: nada precisa ser reimportado.
        assert_eq!(reaberto.decifrar(&blob, "c").unwrap().as_slice(), b"certificado");
        assert_eq!(
            banco.configuracao("marca").await.unwrap().as_deref(),
            Some("dado-antigo")
        );
        assert_eq!(banco.configuracao("cofre").await.unwrap(), None);
        banco.fechar().await;
        assert!(!std::fs::read(&caminho).unwrap().starts_with(b"SQLite format 3"));
        assert!(dir.join("emissor.db.legado").exists());
        assert!(dir.join("emissor.cofre").exists());

        let (banco, _) = Banco::abrir_protegido(caminho.clone(), "senha-mestra".into())
            .await
            .unwrap();
        banco.fechar().await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
