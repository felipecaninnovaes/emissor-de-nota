//! Identificador da DPS, que também é a chave de idempotência na Sefin Nacional.
//!
//! Formato (45 caracteres): `DPS` + código IBGE do município emissor (7)
//! + tipo de inscrição (1: `1` = CPF, `2` = CNPJ) + inscrição federal (14, CPF com zeros à esquerda)
//! + série (5) + número da DPS (15).
//!
//! Conferir contra o leiaute vigente (anexo I do manual de integração) a cada nova versão.

use std::fmt;

use crate::{Cnpj, Cpf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inscricao {
    Cnpj(Cnpj),
    Cpf(Cpf),
}

impl Inscricao {
    fn tipo(&self) -> char {
        match self {
            Inscricao::Cpf(_) => '1',
            Inscricao::Cnpj(_) => '2',
        }
    }

    fn com_14_posicoes(&self) -> String {
        match self {
            Inscricao::Cnpj(c) => c.as_str().to_owned(),
            Inscricao::Cpf(c) => format!("000{}", c.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DpsIdInvalido {
    #[error("código IBGE do município deve ter 7 dígitos")]
    Municipio,
    #[error("série deve ter de 1 a 5 dígitos")]
    Serie,
    #[error("número da DPS deve estar entre 1 e 999999999999999")]
    Numero,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DpsId(String);

impl DpsId {
    pub fn novo(cod_municipio: &str, emitente: &Inscricao, serie: &str, numero: u64) -> Result<Self, DpsIdInvalido> {
        if cod_municipio.len() != 7 || !cod_municipio.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DpsIdInvalido::Municipio);
        }
        if serie.is_empty() || serie.len() > 5 || !serie.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DpsIdInvalido::Serie);
        }
        if numero == 0 || numero > 999_999_999_999_999 {
            return Err(DpsIdInvalido::Numero);
        }
        Ok(Self(format!(
            "DPS{}{}{}{:0>5}{:015}",
            cod_municipio,
            emitente.tipo(),
            emitente.com_14_posicoes(),
            serie,
            numero
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DpsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn id_cnpj() {
        let emitente = Inscricao::Cnpj(Cnpj::parse("11222333000181").unwrap());
        let id = DpsId::novo("3511102", &emitente, "1", 42).unwrap();
        assert_eq!(id.as_str(), "DPS351110221122233300018100001000000000000042");
        assert_eq!(id.as_str().len(), 45);
    }

    #[test]
    fn id_cpf() {
        let emitente = Inscricao::Cpf(Cpf::parse("52998224725").unwrap());
        let id = DpsId::novo("3511102", &emitente, "900", 1).unwrap();
        assert_eq!(id.as_str(), "DPS351110210005299822472500900000000000000001");
    }

    #[test]
    fn id_invalido() {
        let emitente = Inscricao::Cnpj(Cnpj::parse("11222333000181").unwrap());
        assert_eq!(DpsId::novo("351110", &emitente, "1", 1), Err(DpsIdInvalido::Municipio));
        assert_eq!(
            DpsId::novo("3511102", &emitente, "123456", 1),
            Err(DpsIdInvalido::Serie)
        );
        assert_eq!(DpsId::novo("3511102", &emitente, "1", 0), Err(DpsIdInvalido::Numero));
    }
}
