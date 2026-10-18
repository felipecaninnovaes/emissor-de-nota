//! Tipos de domínio e regras do emissor de NFS-e, sem I/O.

pub mod documento;
pub mod dps;
pub mod lista_servicos;
pub mod modelo;
pub mod valor;

pub use documento::{Cnpj, Cpf, DocumentoInvalido};
pub use dps::{DpsId, Inscricao};
pub use modelo::Dps;
pub use valor::{Percentual, Valor};
