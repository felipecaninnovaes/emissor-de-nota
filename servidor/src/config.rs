//! Configuração do servidor, lida das variáveis de ambiente (ver `.env.exemplo`).

use std::net::SocketAddr;

use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};

pub struct Config {
    pub endereco: SocketAddr,
    pub banco_url: String,
    pub s3: ConfigS3,
    pub admin_usuario: String,
    /// Hash Argon2 da senha do administrador (a senha em claro não fica em memória).
    pub admin_senha_hash: String,
    /// Quantos backups ficam guardados por cliente; os mais antigos são apagados.
    pub manter_backups: i64,
    pub tamanho_maximo: u64,
    /// Endereço público do painel (`https://backup.exemplo.com.br`), para conferir o `Origin` dos formulários.
    pub url_publica: Option<String>,
    /// `false` só para testar em `http://` local: o navegador não manda cookie `Secure` sem HTTPS.
    pub cookie_seguro: bool,
    /// Redes dos proxies reversos cujo `X-Forwarded-For` vale para saber quem tenta entrar no painel.
    pub proxies_confiaveis: Vec<ipnet::IpNet>,
}

/// Padrão de `PROXIES_CONFIAVEIS`: loopback e redes privadas. O painel só escuta em 127.0.0.1 na
/// VPS (compose), então toda conexão chega pelo proxy: o Caddy do compose (rede do Docker) ou um
/// Nginx/Traefik da própria máquina (gateway do Docker).
pub const PROXIES_PADRAO: &str = "127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7";

/// Lista de redes separadas por vírgula; `nenhum` não confia em proxy algum (só o IP da conexão).
pub fn ler_proxies(texto: &str) -> Result<Vec<ipnet::IpNet>, String> {
    if texto.trim().eq_ignore_ascii_case("nenhum") {
        return Ok(Vec::new());
    }
    texto
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(|r| {
            r.parse::<ipnet::IpNet>()
                .or_else(|_| r.parse::<std::net::IpAddr>().map(ipnet::IpNet::from))
                .map_err(|_| format!("PROXIES_CONFIAVEIS: \"{r}\" não é um IP nem uma rede (ex.: 172.16.0.0/12)"))
        })
        .collect()
}

pub struct ConfigS3 {
    pub endpoint: String,
    pub regiao: String,
    pub bucket: String,
    pub chave: String,
    pub segredo: String,
}

fn obrigatoria(nome: &str) -> Result<String, String> {
    std::env::var(nome)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("defina a variável de ambiente {nome}"))
}

fn opcional(nome: &str, padrao: &str) -> String {
    std::env::var(nome)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| padrao.to_owned())
}

fn numero<T: std::str::FromStr>(nome: &str, padrao: &str) -> Result<T, String> {
    opcional(nome, padrao)
        .trim()
        .parse()
        .map_err(|_| format!("{nome} precisa ser um número"))
}

pub fn hash_senha(senha: &str) -> Result<String, String> {
    argon2::Argon2::default()
        .hash_password(senha.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|h| h.to_string())
        .map_err(|e| format!("não foi possível preparar a senha do administrador: {e}"))
}

impl Config {
    pub fn do_ambiente() -> Result<Config, String> {
        let senha = obrigatoria("ADMIN_SENHA")?;
        if senha.chars().count() < 12 {
            return Err("ADMIN_SENHA precisa ter pelo menos 12 caracteres".into());
        }
        let manter_backups: i64 = numero("MANTER_BACKUPS", "30")?;
        if manter_backups < 1 {
            return Err("MANTER_BACKUPS precisa ser pelo menos 1".into());
        }
        let tamanho_mb: u64 = numero("TAMANHO_MAXIMO_MB", "500")?;
        Ok(Config {
            endereco: opcional("ENDERECO", "0.0.0.0:8080")
                .parse()
                .map_err(|_| "ENDERECO inválido (ex.: 0.0.0.0:8080)")?,
            banco_url: obrigatoria("DATABASE_URL")?,
            s3: ConfigS3 {
                endpoint: obrigatoria("S3_ENDPOINT")?,
                regiao: opcional("S3_REGIAO", "us-east-1"),
                bucket: opcional("S3_BUCKET", "backups"),
                chave: obrigatoria("S3_CHAVE")?,
                segredo: obrigatoria("S3_SEGREDO")?,
            },
            admin_usuario: obrigatoria("ADMIN_USUARIO")?,
            admin_senha_hash: hash_senha(&senha)?,
            manter_backups,
            tamanho_maximo: tamanho_mb * 1024 * 1024,
            url_publica: std::env::var("URL_PUBLICA")
                .ok()
                .map(|u| u.trim().trim_end_matches('/').to_owned())
                .filter(|u| !u.is_empty()),
            cookie_seguro: opcional("COOKIE_SEGURO", "true") != "false",
            proxies_confiaveis: ler_proxies(&opcional("PROXIES_CONFIAVEIS", PROXIES_PADRAO))?,
        })
    }
}
