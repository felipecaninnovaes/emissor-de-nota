//! XML da NFS-e Nacional: geração da DPS e dos eventos, leitura da NFS-e,
//! canonicalização (C14N) e assinatura XMLDSIG.

pub mod assinatura;
pub mod c14n;
pub mod dps;
pub mod evento;
pub mod nfse;

pub use assinatura::{Assinante, ErroAssinatura, assinar, verificar};
pub use dps::{DpsInvalida, gerar_dps};
pub use evento::{MotivoCancelamento, PedidoCancelamento, PedidoInvalido, gerar_pedido_cancelamento};
pub use nfse::{DadosNfse, ErroLeituraNfse, ler_nfse};
