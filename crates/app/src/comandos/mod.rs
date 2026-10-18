//! Comandos chamados pela interface. Erros voltam como texto pronto para mostrar ao operador.
//!
//! Um módulo por assunto; o que é usado por mais de um fica em `comum`.

pub(crate) mod aparencia;
pub(crate) mod atualizacao;
pub(crate) mod backup;
pub(crate) mod cofre;
mod comum;
pub(crate) mod consultas;
pub(crate) mod dialogo;
pub(crate) mod email;
pub(crate) mod emissao;
pub(crate) mod empresas;
pub(crate) mod notas;
pub(crate) mod planilha;
pub(crate) mod recorrencias;
pub(crate) mod servicos;
pub(crate) mod tomadores;
