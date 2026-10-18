//! Criar e abrir o banco cifrado (SQLCipher).
//!
//! O arquivo `emissor.db` inteiro é cifrado com uma chave derivada da senha mestra (ver
//! [`crate::cofre`]); o sal e os parâmetros do Argon2 ficam em `emissor.cofre`, ao lado do banco,
//! porque o banco não abre sem a chave. Bancos de versões anteriores (SQLite em claro, com o
//! cabeçalho do cofre numa tabela) são convertidos na primeira abertura com a senha mestra: o
//! original fica ao lado como `emissor.db.legado`.

use std::io::Read;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::cofre::{CabecalhoCofre, Cofre, caminho_cabecalho, gravar_cabecalho, ler_cabecalho};
use crate::{Banco, ErroBanco, Resultado, de_json};

const CABECALHO_SQLITE: &[u8] = b"SQLite format 3\0";

/// O arquivo existe, tem conteúdo e começa com o cabeçalho do SQLite (banco em claro).
fn em_claro(caminho: &Path) -> bool {
    let mut inicio = [0u8; 16];
    std::fs::File::open(caminho)
        .and_then(|mut f| f.read_exact(&mut inicio))
        .is_ok()
        && inicio == CABECALHO_SQLITE
}

fn com_sufixo(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut nome = caminho.as_os_str().to_owned();
    nome.push(sufixo);
    PathBuf::from(nome)
}

fn erro_io(e: std::io::Error) -> ErroBanco {
    ErroBanco::Regra(format!("falha de arquivo ao proteger o banco: {e}"))
}

/// Valor do `PRAGMA key` para uma chave bruta (sem o PBKDF2 do SQLCipher: a chave já vem do Argon2).
fn pragma_chave(chave_hex: &str) -> String {
    format!("\"x'{chave_hex}'\"")
}

pub(crate) fn opcoes_do_arquivo(caminho: &Path, chave_hex: Option<&str>) -> SqliteConnectOptions {
    let opcoes = SqliteConnectOptions::new()
        .filename(caminho)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(10));
    match chave_hex {
        // O sqlx envia o `key` antes de qualquer outro pragma, como o SQLCipher exige.
        Some(chave) => opcoes.pragma("key", pragma_chave(chave)),
        None => opcoes,
    }
}

/// Um pool de uma só conexão (o ATTACH e o PRAGMA key valem por conexão). Usa o pool em vez de
/// `&mut SqliteConnection` porque o rustc não consegue provar `Send` para esse executor dentro
/// dos comandos do Tauri.
async fn pool_unico(opcoes: SqliteConnectOptions) -> Resultado<SqlitePool> {
    Ok(SqlitePoolOptions::new().max_connections(1).connect_with(opcoes).await?)
}

/// `PRAGMA integrity_check` num arquivo (cifrado, se vier a chave). Erro se não for "ok".
pub(crate) async fn conferir_integridade(caminho: PathBuf, chave_hex: Option<Zeroizing<String>>) -> Resultado<()> {
    let mut opcoes = SqliteConnectOptions::new().filename(&caminho).read_only(true);
    if let Some(chave) = chave_hex {
        opcoes = opcoes.pragma("key", pragma_chave(&chave));
    }
    let conexao = pool_unico(opcoes).await?;
    let resultado: String = sqlx::query("PRAGMA integrity_check").fetch_one(&conexao).await?.get(0);
    conexao.close().await;
    if resultado == "ok" {
        Ok(())
    } else {
        Err(ErroBanco::DadoInvalido(format!("banco com problemas: {resultado}")))
    }
}

/// Cabeçalho do cofre guardado na tabela `configuracao` por versões anteriores ao banco cifrado.
async fn cabecalho_legado(caminho: PathBuf) -> Resultado<Option<CabecalhoCofre>> {
    if !em_claro(&caminho) {
        return Ok(None);
    }
    let opcoes = SqliteConnectOptions::new().filename(&caminho);
    let conexao = pool_unico(opcoes).await?;
    let valor: Option<String> = sqlx::query("SELECT valor FROM configuracao WHERE chave = 'cofre'")
        .fetch_optional(&conexao)
        .await
        .ok()
        .flatten()
        .map(|r| r.get(0));
    conexao.close().await;
    valor.map(|v| de_json(&v, "cofre")).transpose()
}

/// Copia o banco em claro para um arquivo cifrado com a chave do cofre e troca os arquivos,
/// guardando o original como `.legado`. Só mexe nos arquivos depois de conferir a cópia.
async fn cifrar_banco_em_claro(caminho: PathBuf, chave: Zeroizing<String>) -> Resultado<()> {
    let novo = com_sufixo(&caminho, ".novo");
    let _ = std::fs::remove_file(&novo);

    // `create_if_missing` vale também para o ATTACH, que cria o arquivo cifrado.
    let opcoes = SqliteConnectOptions::new().filename(&caminho).create_if_missing(true);
    let conexao = pool_unico(opcoes).await?;
    // Leva o que ainda está no WAL para o arquivo principal antes de copiar.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&conexao).await?;
    sqlx::query(&format!("ATTACH DATABASE ? AS cifrado KEY {}", pragma_chave(&chave)))
        .bind(novo.to_string_lossy().into_owned())
        .execute(&conexao)
        .await?;
    sqlx::query("SELECT sqlcipher_export('cifrado')")
        .execute(&conexao)
        .await?;
    let versao: i64 = sqlx::query("PRAGMA user_version").fetch_one(&conexao).await?.get(0);
    sqlx::query(&format!("PRAGMA cifrado.user_version = {versao}"))
        .execute(&conexao)
        .await?;
    sqlx::query("DETACH DATABASE cifrado").execute(&conexao).await?;
    conexao.close().await;

    if let Err(e) = conferir_integridade(novo.clone(), Some(chave.clone())).await {
        let _ = std::fs::remove_file(&novo);
        return Err(e);
    }
    let legado = com_sufixo(&caminho, ".legado");
    std::fs::rename(&caminho, &legado).map_err(erro_io)?;
    for lateral in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(com_sufixo(&caminho, lateral));
    }
    std::fs::rename(&novo, &caminho).map_err(erro_io)
}

impl Banco {
    /// O cofre já existe? Vale tanto para o banco cifrado (arquivo `.cofre`) quanto para um banco
    /// de versão anterior, em claro, que ainda guarda o cabeçalho numa tabela.
    pub async fn cofre_existe(caminho: PathBuf) -> Resultado<bool> {
        Ok(caminho_cabecalho(&caminho).exists() || cabecalho_legado(caminho).await?.is_some())
    }

    /// Primeiro uso: cria o cofre com a senha mestra e o banco cifrado. Recusa se o cofre já existir
    /// (trocar a senha exigiria recifrar tudo).
    pub async fn criar_protegido(caminho: PathBuf, senha: String) -> Resultado<(Self, Cofre)> {
        // Assume o buffer da senha: zerado quando a derivação terminar.
        let senha = Zeroizing::new(senha);
        let (cofre, _) = tokio::task::spawn_blocking(move || Cofre::criar(&senha))
            .await
            .map_err(|e| ErroBanco::Regra(format!("falha ao criar o cofre: {e}")))??;
        Self::criar_com_cofre(caminho, cofre).await
    }

    /// Cofre com Argon2 leve, só para testes.
    #[cfg(any(test, feature = "teste"))]
    pub async fn criar_protegido_rapido(caminho: PathBuf, senha: &str) -> Resultado<(Self, Cofre)> {
        let (cofre, _) = Cofre::criar_com(senha, 1024, 1, 1)?;
        Self::criar_com_cofre(caminho, cofre).await
    }

    async fn criar_com_cofre(caminho: PathBuf, cofre: Cofre) -> Resultado<(Self, Cofre)> {
        if Self::cofre_existe(caminho.clone()).await? {
            return Err(ErroBanco::Regra(
                "o cofre já foi criado; use a senha mestra existente".into(),
            ));
        }
        if caminho.metadata().is_ok_and(|m| m.len() > 0) {
            if !em_claro(&caminho) {
                return Err(ErroBanco::Regra(
                    "existe um banco cifrado sem o arquivo emissor.cofre; restaure os dois arquivos juntos".into(),
                ));
            }
            // Banco em claro sem cofre (nunca chegou a criar a senha): só cifra.
            cifrar_banco_em_claro(caminho.clone(), cofre.chave_banco_hex()).await?;
        }
        let banco = Self::abrir_com_chave(caminho.clone(), cofre.chave_banco_hex()).await?;
        // O cabeçalho só vai para o disco com o banco já aberto e migrado.
        gravar_cabecalho(&caminho, cofre.cabecalho())?;
        banco.auditar("cofre_criado", None, None).await?;
        Ok((banco, cofre))
    }

    /// Abre o banco com a senha mestra. Converte, na primeira vez, um banco em claro de versão anterior.
    pub async fn abrir_protegido(caminho: PathBuf, senha: String) -> Resultado<(Self, Cofre)> {
        let senha = Zeroizing::new(senha);
        let (cabecalho, legado) = match ler_cabecalho(&caminho)? {
            Some(c) => (c, false),
            None => match cabecalho_legado(caminho.clone()).await? {
                Some(c) => (c, true),
                None => return Err(ErroBanco::NaoEncontrado("cofre")),
            },
        };
        let cofre = tokio::task::spawn_blocking(move || Cofre::abrir(&senha, &cabecalho))
            .await
            .map_err(|e| ErroBanco::Regra(format!("falha ao abrir o cofre: {e}")))??;
        if legado {
            cifrar_banco_em_claro(caminho.clone(), cofre.chave_banco_hex()).await?;
            gravar_cabecalho(&caminho, cofre.cabecalho())?;
        }
        let banco = Self::abrir_com_chave(caminho, cofre.chave_banco_hex()).await?;
        if legado {
            sqlx::query("DELETE FROM configuracao WHERE chave = 'cofre'")
                .execute(banco.pool())
                .await?;
            banco
                .auditar("banco_cifrado", None, Some("emissor.db.legado guarda a cópia em claro"))
                .await?;
        }
        Ok((banco, cofre))
    }

    /// Abre (ou cria) o arquivo cifrado com a chave do cofre e aplica as migrações pendentes.
    pub(crate) async fn abrir_com_chave(caminho: PathBuf, chave: Zeroizing<String>) -> Resultado<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opcoes_do_arquivo(&caminho, Some(&chave)))
            .await
            .map_err(|_| nao_abre())?;
        // Chave errada só aparece na primeira leitura.
        if sqlx::query("SELECT count(*) FROM sqlite_master")
            .fetch_one(&pool)
            .await
            .is_err()
        {
            pool.close().await;
            return Err(nao_abre());
        }
        let banco = Self::com_pool(pool);
        banco.preparar().await?;
        Ok(banco)
    }
}

fn nao_abre() -> ErroBanco {
    ErroBanco::Regra("o banco não abriu com esta senha mestra; o arquivo emissor.cofre pode ser de outro banco".into())
}
