//! XML da NFS-e Nacional: geração da DPS, canonicalização (C14N) e assinatura XMLDSIG.

pub mod assinatura;
pub mod c14n;
pub mod dps;

pub use assinatura::{Assinante, ErroAssinatura, assinar, verificar};
pub use dps::{DpsInvalida, gerar_dps};
