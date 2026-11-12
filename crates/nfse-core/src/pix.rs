//! Chave PIX da empresa, para as descrições da nota (variável `{pix}`).
//!
//! Os formatos são os do DICT do Banco Central: CPF, CNPJ, e-mail, telefone (`+55` com DDD) e
//! chave aleatória (UUID). A chave fica gravada como o banco a conhece: CPF e CNPJ sem pontuação,
//! e-mail e chave aleatória em minúsculas, telefone `+55DDNÚMERO`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::documento::{Cnpj, Cpf};

/// Tamanho máximo do e-mail como chave PIX no DICT.
const EMAIL_MAX: usize = 77;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ChavePixInvalida(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoChavePix {
    Cpf,
    Cnpj,
    Email,
    Telefone,
    Aleatoria,
}

impl TipoChavePix {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Cpf => "CPF",
            Self::Cnpj => "CNPJ",
            Self::Email => "e-mail",
            Self::Telefone => "telefone",
            Self::Aleatoria => "chave aleatória",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChavePix {
    chave: String,
    tipo: TipoChavePix,
}

impl TryFrom<String> for ChavePix {
    type Error = ChavePixInvalida;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<ChavePix> for String {
    fn from(c: ChavePix) -> String {
        c.chave
    }
}

fn invalida(msg: impl Into<String>) -> ChavePixInvalida {
    ChavePixInvalida(msg.into())
}

fn email_valido(e: &str) -> bool {
    let Some((local, dominio)) = e.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !dominio.contains('@')
        && dominio.contains('.')
        && !dominio.starts_with('.')
        && !dominio.ends_with('.')
        && !e.chars().any(|c| c.is_whitespace() || matches!(c, ';' | ','))
}

/// UUID no formato 8-4-4-4-12, em hexadecimal.
fn uuid_valido(s: &str) -> bool {
    let partes: Vec<&str> = s.split('-').collect();
    partes.len() == 5
        && partes
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl ChavePix {
    pub fn parse(entrada: &str) -> Result<Self, ChavePixInvalida> {
        let s = entrada.trim();
        if s.is_empty() {
            return Err(invalida("chave PIX vazia"));
        }
        if s.contains('@') {
            let e = s.to_lowercase();
            if !email_valido(&e) {
                return Err(invalida("chave PIX: e-mail inválido"));
            }
            if e.chars().count() > EMAIL_MAX {
                return Err(invalida(format!(
                    "chave PIX: e-mail com mais de {EMAIL_MAX} caracteres"
                )));
            }
            return Ok(Self {
                chave: e,
                tipo: TipoChavePix::Email,
            });
        }
        if uuid_valido(s) {
            return Ok(Self {
                chave: s.to_ascii_lowercase(),
                tipo: TipoChavePix::Aleatoria,
            });
        }
        if let Some(resto) = s.strip_prefix('+') {
            let digitos: String = resto
                .chars()
                .filter(|c| !matches!(c, ' ' | '(' | ')' | '-' | '.'))
                .collect();
            let valido = digitos.bytes().all(|b| b.is_ascii_digit())
                && digitos.starts_with("55")
                && matches!(digitos.len(), 12 | 13);
            if !valido {
                return Err(invalida(
                    "chave PIX: telefone no formato +55 com DDD e número (ex.: +55 17 99999-8888)",
                ));
            }
            return Ok(Self {
                chave: format!("+{digitos}"),
                tipo: TipoChavePix::Telefone,
            });
        }
        let documento: String = s.chars().filter(|c| !matches!(c, '.' | '/' | '-' | ' ')).collect();
        match documento.len() {
            11 => Cpf::parse(&documento)
                .map(|c| Self {
                    chave: c.as_str().to_owned(),
                    tipo: TipoChavePix::Cpf,
                })
                .map_err(|e| {
                    invalida(format!(
                        "chave PIX: CPF inválido ({e}); telefone vai com +55 (ex.: +55 17 99999-8888)"
                    ))
                }),
            14 => Cnpj::parse(&documento)
                .map(|c| Self {
                    chave: c.as_str().to_owned(),
                    tipo: TipoChavePix::Cnpj,
                })
                .map_err(|e| invalida(format!("chave PIX: CNPJ inválido ({e})"))),
            _ => Err(invalida(
                "chave PIX: use CPF, CNPJ, e-mail, telefone com +55 ou a chave aleatória",
            )),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.chave
    }

    pub fn tipo(&self) -> TipoChavePix {
        self.tipo
    }
}

impl fmt::Display for ChavePix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.chave)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn chave(s: &str) -> (String, TipoChavePix) {
        let c = ChavePix::parse(s).unwrap();
        (c.as_str().to_owned(), c.tipo())
    }

    #[test]
    fn aceita_os_cinco_formatos() {
        assert_eq!(chave(" 529.982.247-25 "), ("52998224725".into(), TipoChavePix::Cpf));
        assert_eq!(
            chave("11.222.333/0001-81"),
            ("11222333000181".into(), TipoChavePix::Cnpj)
        );
        assert_eq!(
            chave("12.ABC.345/01DE-35"),
            ("12ABC34501DE35".into(), TipoChavePix::Cnpj)
        );
        assert_eq!(
            chave("Financeiro@Exemplo.com.br"),
            ("financeiro@exemplo.com.br".into(), TipoChavePix::Email)
        );
        assert_eq!(
            chave("+55 (17) 99999-8888"),
            ("+5517999998888".into(), TipoChavePix::Telefone)
        );
        assert_eq!(chave("+551733334444"), ("+551733334444".into(), TipoChavePix::Telefone));
        assert_eq!(
            chave("123E4567-E89B-42D3-A456-426614174000"),
            ("123e4567-e89b-42d3-a456-426614174000".into(), TipoChavePix::Aleatoria)
        );
    }

    #[test]
    fn recusa_chave_invalida() {
        for ruim in [
            "",
            "   ",
            "529.982.247-26",     // DV do CPF
            "11222333000182",     // DV do CNPJ
            "(17) 99999-8888",    // telefone sem +55
            "+1 555 123 4567",    // fora do Brasil
            "+55 17 9999",        // curto
            "fulano@",            // e-mail sem domínio
            "fulano@exemplo",     // domínio sem ponto
            "a b@exemplo.com",    // espaço
            "123e4567-e89b-42d3", // UUID incompleto
            "qualquer coisa",
        ] {
            assert!(ChavePix::parse(ruim).is_err(), "{ruim:?}");
        }
        let longo = format!("{}@exemplo.com", "a".repeat(70));
        assert!(ChavePix::parse(&longo).unwrap_err().to_string().contains("77"));
        // O telefone digitado sem +55 cai no CPF; a mensagem lembra do formato do telefone.
        assert!(ChavePix::parse("17999998888").unwrap_err().to_string().contains("+55"));
    }

    #[test]
    fn serde_valida_e_grava_a_forma_canonica() {
        let c: ChavePix = serde_json::from_str("\"+55 17 99999-8888\"").unwrap();
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"+5517999998888\"");
        assert!(serde_json::from_str::<ChavePix>("\"nada\"").is_err());
    }
}
