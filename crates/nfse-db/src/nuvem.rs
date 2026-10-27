//! Backup na nuvem: endereço do servidor, token do cliente (cifrado no cofre) e data do último
//! envio. O envio e a restauração (HTTP) ficam no aplicativo; aqui só o que é gravado no banco.

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Banco, Cofre, ErroBanco, Resultado, de_json, json};

const CHAVE_NUVEM: &str = "nuvem";
const CHAVE_TOKEN: &str = "nuvem_token";
const CHAVE_ULTIMO: &str = "nuvem_ultimo";
const CONTEXTO_TOKEN: &str = "token-nuvem";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigNuvem {
    /// Endereço do servidor, sem o `/api/v1` (ex.: `https://backup.exemplo.com.br`).
    pub endereco: String,
}

/// Endereço sem barra no fim; `http://` só em `localhost` (o token iria aberto pela rede).
pub fn normalizar_endereco(endereco: &str) -> Result<String, &'static str> {
    let endereco = endereco.trim().trim_end_matches('/');
    let resto = if let Some(r) = endereco.strip_prefix("https://") {
        r
    } else if let Some(r) = endereco.strip_prefix("http://") {
        let autoridade = r.split(['/', '?', '#']).next().unwrap_or_default();
        // Sem a porta; o IPv6 vem entre colchetes.
        let host = match autoridade.find(']') {
            Some(fim) if autoridade.starts_with('[') => &autoridade[..=fim],
            _ => autoridade.split(':').next().unwrap_or_default(),
        };
        if !matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
            return Err("use https:// (http:// só para servidor nesta máquina, em localhost)");
        }
        r
    } else {
        return Err("o endereço do servidor começa com https://");
    };
    if resto.is_empty() {
        return Err("informe o endereço do servidor");
    }
    Ok(endereco.to_owned())
}

/// SHA-256 (hex minúsculo) do arquivo, lido em blocos.
pub fn sha256_arquivo(caminho: &Path) -> Resultado<String> {
    let erro = |e: std::io::Error| ErroBanco::Regra(format!("não foi possível ler o arquivo: {e}"));
    let mut arquivo = std::fs::File::open(caminho).map_err(erro)?;
    let mut h = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let lidos = arquivo.read(&mut buffer).map_err(erro)?;
        if lidos == 0 {
            break;
        }
        h.update(&buffer[..lidos]);
    }
    Ok(hex::encode(h.finalize()))
}

impl Banco {
    pub async fn nuvem(&self) -> Resultado<Option<ConfigNuvem>> {
        self.configuracao(CHAVE_NUVEM)
            .await?
            .map(|j| de_json(&j, "configuração do backup na nuvem"))
            .transpose()
    }

    pub async fn definir_nuvem(&self, cfg: &ConfigNuvem) -> Resultado<()> {
        let endereco = normalizar_endereco(&cfg.endereco).map_err(|e| ErroBanco::Regra(e.into()))?;
        self.definir_configuracao(CHAVE_NUVEM, &json(&ConfigNuvem { endereco }))
            .await
    }

    /// Guarda o token do cliente cifrado com a chave do cofre.
    pub async fn definir_token_nuvem(&self, cofre: &Cofre, token: &str) -> Resultado<()> {
        let cifrado = cofre.cifrar(token.trim().as_bytes(), CONTEXTO_TOKEN)?;
        self.definir_configuracao(CHAVE_TOKEN, &hex::encode(cifrado)).await
    }

    pub async fn tem_token_nuvem(&self) -> Resultado<bool> {
        Ok(self.configuracao(CHAVE_TOKEN).await?.is_some())
    }

    pub async fn token_nuvem(&self, cofre: &Cofre) -> Resultado<Option<Zeroizing<String>>> {
        let Some(texto) = self.configuracao(CHAVE_TOKEN).await? else {
            return Ok(None);
        };
        let invalido = || ErroBanco::DadoInvalido("token do backup na nuvem".into());
        let blob = hex::decode(&texto).map_err(|_| invalido())?;
        let claro = cofre.decifrar(&blob, CONTEXTO_TOKEN)?;
        let token = String::from_utf8(claro.to_vec()).map_err(|_| invalido())?;
        Ok(Some(Zeroizing::new(token)))
    }

    /// Data e hora (UTC, RFC 3339) do último backup enviado à nuvem.
    pub async fn nuvem_ultimo(&self) -> Resultado<Option<String>> {
        self.configuracao(CHAVE_ULTIMO).await
    }

    pub async fn marcar_envio_nuvem(&self) -> Resultado<()> {
        self.definir_configuracao(CHAVE_ULTIMO, &chrono::Utc::now().to_rfc3339())
            .await
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::banco_com_empresa;

    #[test]
    fn endereco_exige_https_fora_de_localhost() {
        assert_eq!(
            normalizar_endereco(" https://backup.exemplo.com/ ").unwrap(),
            "https://backup.exemplo.com"
        );
        assert_eq!(
            normalizar_endereco("http://localhost:8080").unwrap(),
            "http://localhost:8080"
        );
        assert!(normalizar_endereco("http://127.0.0.1/").is_ok());
        assert!(normalizar_endereco("http://[::1]:3000").is_ok());
        assert!(normalizar_endereco("http://backup.exemplo.com").is_err());
        assert!(normalizar_endereco("http://localhost.exemplo.com").is_err());
        assert!(normalizar_endereco("backup.exemplo.com").is_err());
        assert!(normalizar_endereco("https://").is_err());
    }

    #[test]
    fn sha256_do_arquivo() {
        let caminho = std::env::temp_dir().join(format!("nfse-teste-sha-{}.bin", std::process::id()));
        std::fs::write(&caminho, b"abc").unwrap();
        assert_eq!(
            sha256_arquivo(&caminho).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        std::fs::remove_file(&caminho).unwrap();
    }

    #[tokio::test]
    async fn configuracao_e_token_cifrado() {
        let (banco, cofre, _) = banco_com_empresa().await;
        assert_eq!(banco.nuvem().await.unwrap(), None);
        assert!(matches!(
            banco
                .definir_nuvem(&ConfigNuvem {
                    endereco: "http://backup.exemplo.com".into()
                })
                .await,
            Err(ErroBanco::Regra(_))
        ));
        banco
            .definir_nuvem(&ConfigNuvem {
                endereco: "https://backup.exemplo.com/".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            banco.nuvem().await.unwrap().unwrap().endereco,
            "https://backup.exemplo.com"
        );

        assert!(!banco.tem_token_nuvem().await.unwrap());
        assert_eq!(banco.token_nuvem(&cofre).await.unwrap(), None);
        banco.definir_token_nuvem(&cofre, " token-teste ").await.unwrap();
        assert!(banco.tem_token_nuvem().await.unwrap());
        let guardado = banco.configuracao(CHAVE_TOKEN).await.unwrap().unwrap();
        assert!(!guardado.contains(&hex::encode(b"token-teste")), "não fica em claro");
        assert_eq!(
            banco.token_nuvem(&cofre).await.unwrap().unwrap().as_str(),
            "token-teste"
        );

        assert_eq!(banco.nuvem_ultimo().await.unwrap(), None);
        banco.marcar_envio_nuvem().await.unwrap();
        let ultimo = banco.nuvem_ultimo().await.unwrap().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(&ultimo).is_ok());
    }
}
