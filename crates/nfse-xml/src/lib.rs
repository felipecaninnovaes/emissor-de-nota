//! XML da NFS-e Nacional: canonicalização (C14N) e assinatura XMLDSIG.

pub mod assinatura;
pub mod c14n;

pub use assinatura::{Assinante, ErroAssinatura, assinar, verificar};
