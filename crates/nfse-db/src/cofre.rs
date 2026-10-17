//! Cofre dos certificados: chave AES-256 derivada da senha mestra do escritório com Argon2id.
//!
//! A senha mestra nunca é gravada. O banco guarda só o sal, os parâmetros do Argon2 e um
//! verificador (um texto conhecido cifrado), para dizer "senha incorreta" sem tentar abrir
//! certificado. Cada valor cifrado leva um contexto (AAD) que o amarra à linha de origem:
//! um .pfx copiado para outra empresa não decifra.
//!
//! A mesma chave dá origem à chave do SQLCipher que cifra o banco inteiro (SHA-256 com um rótulo,
//! para a chave dos certificados e a do arquivo serem diferentes). Como o banco só abre com essa
//! chave, o cabeçalho (sal, parâmetros e verificador, sem segredo) fica num arquivo ao lado do
//! banco (`emissor.cofre`), não dentro dele.

use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, Generate, Key, KeyInit, Nonce, Payload};
use argon2::{Algorithm, Argon2, Params, Version};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const TAM_NONCE: usize = 12;
const VERIFICADOR: &[u8] = b"emissor-nfse:cofre:v1";
const ROTULO_BANCO: &[u8] = b"emissor-nfse:banco:v1";

#[derive(Debug, thiserror::Error)]
pub enum ErroCofre {
    #[error("senha mestra incorreta")]
    SenhaIncorreta,
    #[error("senha mestra muito curta (mínimo 8 caracteres)")]
    SenhaCurta,
    #[error("dado cifrado corrompido ou de outro registro")]
    Corrompido,
    #[error("parâmetros do cofre inválidos: {0}")]
    Parametros(String),
}

/// O que o banco guarda sobre o cofre (nenhum segredo).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CabecalhoCofre {
    pub sal: Vec<u8>,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub verificador: Vec<u8>,
}

pub struct Cofre {
    chave: Zeroizing<[u8; 32]>,
    cabecalho: CabecalhoCofre,
}

impl std::fmt::Debug for Cofre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cofre(..)")
    }
}

fn derivar(senha: &str, sal: &[u8], m: u32, t: u32, p: u32) -> Result<Zeroizing<[u8; 32]>, ErroCofre> {
    let params = Params::new(m, t, p, Some(32)).map_err(|e| ErroCofre::Parametros(e.to_string()))?;
    let mut chave = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(senha.as_bytes(), sal, chave.as_mut())
        .map_err(|e| ErroCofre::Parametros(e.to_string()))?;
    Ok(chave)
}

impl Cofre {
    /// Cria um cofre novo. Parâmetros do Argon2id: os padrões recomendados (19 MiB, 2 passadas).
    pub fn criar(senha: &str) -> Result<(Self, CabecalhoCofre), ErroCofre> {
        Self::criar_com(
            senha,
            Params::DEFAULT_M_COST,
            Params::DEFAULT_T_COST,
            Params::DEFAULT_P_COST,
        )
    }

    pub(crate) fn criar_com(senha: &str, m: u32, t: u32, p: u32) -> Result<(Self, CabecalhoCofre), ErroCofre> {
        if senha.chars().count() < 8 {
            return Err(ErroCofre::SenhaCurta);
        }
        let sal = Key::<Aes256Gcm>::generate().to_vec();
        let chave = derivar(senha, &sal, m, t, p)?;
        let mut cofre = Self {
            chave,
            cabecalho: CabecalhoCofre {
                sal,
                m_cost: m,
                t_cost: t,
                p_cost: p,
                verificador: Vec::new(),
            },
        };
        cofre.cabecalho.verificador = cofre.cifrar(VERIFICADOR, "verificador")?;
        let cabecalho = cofre.cabecalho.clone();
        Ok((cofre, cabecalho))
    }

    pub fn abrir(senha: &str, cabecalho: &CabecalhoCofre) -> Result<Self, ErroCofre> {
        let c = cabecalho;
        let cofre = Self {
            chave: derivar(senha, &c.sal, c.m_cost, c.t_cost, c.p_cost)?,
            cabecalho: c.clone(),
        };
        match cofre.decifrar(&c.verificador, "verificador") {
            Ok(v) if v.as_slice() == VERIFICADOR => Ok(cofre),
            _ => Err(ErroCofre::SenhaIncorreta),
        }
    }

    /// O que o banco guarda sobre o cofre (sem segredo); vai no arquivo ao lado do banco e nos backups.
    pub fn cabecalho(&self) -> &CabecalhoCofre {
        &self.cabecalho
    }

    /// Chave do SQLCipher em hexadecimal (64 caracteres), para `PRAGMA key = "x'...'"`.
    pub(crate) fn chave_banco_hex(&self) -> Zeroizing<String> {
        let mut h = Sha256::new();
        h.update(ROTULO_BANCO);
        h.update(self.chave.as_ref());
        Zeroizing::new(hex::encode(h.finalize()))
    }

    fn cifra(&self) -> Aes256Gcm {
        Aes256Gcm::new_from_slice(self.chave.as_ref()).expect("chave de 32 bytes")
    }

    /// Devolve `nonce || texto cifrado`. `contexto` precisa ser o mesmo na hora de decifrar.
    pub fn cifrar(&self, dados: &[u8], contexto: &str) -> Result<Vec<u8>, ErroCofre> {
        let nonce = Nonce::<Aes256Gcm>::generate();
        let cifrado = self
            .cifra()
            .encrypt(
                &nonce,
                Payload {
                    msg: dados,
                    aad: contexto.as_bytes(),
                },
            )
            .map_err(|_| ErroCofre::Corrompido)?;
        let mut saida = nonce.to_vec();
        saida.extend_from_slice(&cifrado);
        Ok(saida)
    }

    pub fn decifrar(&self, blob: &[u8], contexto: &str) -> Result<Zeroizing<Vec<u8>>, ErroCofre> {
        if blob.len() < TAM_NONCE {
            return Err(ErroCofre::Corrompido);
        }
        let (nonce, cifrado) = blob.split_at(TAM_NONCE);
        let nonce = Nonce::<Aes256Gcm>::try_from(nonce).map_err(|_| ErroCofre::Corrompido)?;
        self.cifra()
            .decrypt(
                &nonce,
                Payload {
                    msg: cifrado,
                    aad: contexto.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| ErroCofre::Corrompido)
    }
}

/// Onde fica o cabeçalho do cofre de um banco: `emissor.db` -> `emissor.cofre`.
pub fn caminho_cabecalho(banco: &Path) -> PathBuf {
    banco.with_extension("cofre")
}

pub fn ler_cabecalho(banco: &Path) -> crate::Resultado<Option<CabecalhoCofre>> {
    match std::fs::read(caminho_cabecalho(banco)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| crate::ErroBanco::DadoInvalido(format!("emissor.cofre: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(crate::ErroBanco::Regra(format!(
            "não foi possível ler o cabeçalho do cofre: {e}"
        ))),
    }
}

/// Grava pelo temporário + renomear: um corte de energia nunca deixa o cabeçalho pela metade
/// (sem ele o banco não abre).
pub fn gravar_cabecalho(banco: &Path, cabecalho: &CabecalhoCofre) -> crate::Resultado<()> {
    let destino = caminho_cabecalho(banco);
    let temporario = destino.with_extension("cofre.tmp");
    let erro =
        |e: std::io::Error| crate::ErroBanco::Regra(format!("não foi possível gravar o cabeçalho do cofre: {e}"));
    {
        let mut f = std::fs::File::create(&temporario).map_err(erro)?;
        f.write_all(&serde_json::to_vec_pretty(cabecalho).expect("cabeçalho serializa"))
            .map_err(erro)?;
        f.sync_all().map_err(erro)?;
    }
    std::fs::rename(&temporario, &destino).map_err(erro)
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;

    /// Argon2 leve para os testes não demorarem.
    pub fn cofre_rapido(senha: &str) -> (Cofre, CabecalhoCofre) {
        Cofre::criar_com(senha, 1024, 1, 1).unwrap()
    }

    #[test]
    fn cifra_e_decifra_com_contexto() {
        let (cofre, _) = cofre_rapido("senha-mestra");
        let blob = cofre.cifrar(b"segredo", "certificado:1").unwrap();
        assert_eq!(cofre.decifrar(&blob, "certificado:1").unwrap().as_slice(), b"segredo");
        assert!(matches!(
            cofre.decifrar(&blob, "certificado:2"),
            Err(ErroCofre::Corrompido)
        ));
        let mut adulterado = blob.clone();
        *adulterado.last_mut().unwrap() ^= 1;
        assert!(cofre.decifrar(&adulterado, "certificado:1").is_err());
    }

    #[test]
    fn reabre_com_a_senha_certa() {
        let (cofre, cab) = cofre_rapido("senha-mestra");
        let blob = cofre.cifrar(b"x", "c").unwrap();
        let reaberto = Cofre::abrir("senha-mestra", &cab).unwrap();
        assert_eq!(reaberto.decifrar(&blob, "c").unwrap().as_slice(), b"x");
        assert!(matches!(
            Cofre::abrir("outra-senha", &cab),
            Err(ErroCofre::SenhaIncorreta)
        ));
    }

    #[test]
    fn recusa_senha_curta() {
        assert!(matches!(Cofre::criar("1234567"), Err(ErroCofre::SenhaCurta)));
    }
}
