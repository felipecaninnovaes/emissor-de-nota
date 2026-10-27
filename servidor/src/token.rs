//! Tokens dos clientes e identificadores de sessão: só o SHA-256 vai para o banco.

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Prefixo legível: ajuda a reconhecer o token colado no aplicativo e em varreduras de segredos.
pub const PREFIXO: &str = "nfb_";

/// 32 bytes aleatórios em base64url, com o prefixo.
pub fn gerar() -> String {
    format!("{PREFIXO}{}", aleatorio())
}

/// 32 bytes aleatórios em base64url, sem prefixo (identificador da sessão do painel).
pub fn aleatorio() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.trim().as_bytes()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn token_tem_prefixo_e_nao_se_repete() {
        let a = gerar();
        assert!(a.starts_with(PREFIXO));
        assert_eq!(a.len(), PREFIXO.len() + 43);
        assert_ne!(a, gerar());
    }

    #[test]
    fn hash_ignora_espacos_colados_junto() {
        let t = gerar();
        assert_eq!(hash(&t), hash(&format!("  {t}\n")));
        assert_eq!(hash(&t).len(), 64);
    }
}
