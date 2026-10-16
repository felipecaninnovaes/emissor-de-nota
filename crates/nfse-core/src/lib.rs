//! Tipos de domínio e regras do emissor de NFS-e, sem I/O.

pub mod documento;
pub mod dps;

pub use documento::{Cnpj, Cpf, DocumentoInvalido};
pub use dps::{DpsId, Inscricao};
