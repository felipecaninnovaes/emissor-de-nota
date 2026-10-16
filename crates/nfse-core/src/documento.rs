//! CNPJ (numérico e alfanumérico) e CPF.
//!
//! O CNPJ alfanumérico (IN RFB 2.229/2024) passou a ser emitido em julho de 2026:
//! as 12 primeiras posições aceitam `0-9A-Z` e os dois dígitos verificadores
//! continuam numéricos. O cálculo do DV usa o valor ASCII do caractere menos 48,
//! o que mantém o resultado idêntico para CNPJs só com números.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentoInvalido {
    #[error("tamanho inválido: esperado {esperado} caracteres, recebido {recebido}")]
    Tamanho { esperado: usize, recebido: usize },
    #[error("caractere inválido: {0:?}")]
    Caractere(char),
    #[error("dígito verificador não confere")]
    DigitoVerificador,
    #[error("documento com todos os caracteres iguais")]
    Repetido,
}

/// Remove pontuação comum (`.`, `/`, `-`, espaços) e coloca em maiúsculas.
fn normalizar(entrada: &str) -> String {
    entrada
        .chars()
        .filter(|c| !matches!(c, '.' | '/' | '-' | ' '))
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn todos_iguais(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(primeiro) => it.all(|c| c == primeiro),
        None => true,
    }
}

/// Dígito verificador módulo 11 com pesos cíclicos de 2 a `peso_max`, da direita para a esquerda.
fn dv_mod11(valores: &[u32], peso_max: u32) -> u32 {
    let mut peso = 2;
    let mut soma = 0;
    for v in valores.iter().rev() {
        soma += v * peso;
        peso = if peso == peso_max { 2 } else { peso + 1 };
    }
    let resto = soma % 11;
    if resto < 2 { 0 } else { 11 - resto }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cnpj(String);

impl Cnpj {
    pub fn parse(entrada: &str) -> Result<Self, DocumentoInvalido> {
        let s = normalizar(entrada);
        if s.len() != 14 {
            return Err(DocumentoInvalido::Tamanho {
                esperado: 14,
                recebido: s.len(),
            });
        }
        let mut valores = Vec::with_capacity(14);
        for (i, c) in s.chars().enumerate() {
            let valido = if i < 12 {
                c.is_ascii_digit() || c.is_ascii_uppercase()
            } else {
                c.is_ascii_digit()
            };
            if !valido {
                return Err(DocumentoInvalido::Caractere(c));
            }
            valores.push(c as u32 - '0' as u32);
        }
        if todos_iguais(&s) {
            return Err(DocumentoInvalido::Repetido);
        }
        let dv1 = dv_mod11(&valores[..12], 9);
        let dv2 = dv_mod11(&valores[..13], 9);
        if valores[12] != dv1 || valores[13] != dv2 {
            return Err(DocumentoInvalido::DigitoVerificador);
        }
        Ok(Self(s))
    }

    /// Os 14 caracteres, sem pontuação.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Raiz (8 primeiras posições), que identifica a empresa independentemente da filial.
    pub fn raiz(&self) -> &str {
        &self.0[..8]
    }
}

impl fmt::Display for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.0;
        write!(f, "{}.{}.{}/{}-{}", &s[..2], &s[2..5], &s[5..8], &s[8..12], &s[12..])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cpf(String);

impl Cpf {
    pub fn parse(entrada: &str) -> Result<Self, DocumentoInvalido> {
        let s = normalizar(entrada);
        if s.len() != 11 {
            return Err(DocumentoInvalido::Tamanho {
                esperado: 11,
                recebido: s.len(),
            });
        }
        if let Some(c) = s.chars().find(|c| !c.is_ascii_digit()) {
            return Err(DocumentoInvalido::Caractere(c));
        }
        if todos_iguais(&s) {
            return Err(DocumentoInvalido::Repetido);
        }
        let valores: Vec<u32> = s.chars().map(|c| c as u32 - '0' as u32).collect();
        let dv1 = dv_mod11(&valores[..9], 11);
        let dv2 = dv_mod11(&valores[..10], 11);
        if valores[9] != dv1 || valores[10] != dv2 {
            return Err(DocumentoInvalido::DigitoVerificador);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Cpf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.0;
        write!(f, "{}.{}.{}-{}", &s[..3], &s[3..6], &s[6..9], &s[9..])
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cnpj_numerico_valido() {
        let c = Cnpj::parse("11.222.333/0001-81").unwrap();
        assert_eq!(c.as_str(), "11222333000181");
        assert_eq!(c.to_string(), "11.222.333/0001-81");
        assert_eq!(c.raiz(), "11222333");
    }

    #[test]
    fn cnpj_alfanumerico_valido() {
        // Exemplo publicado pela Receita Federal.
        let c = Cnpj::parse("12.ABC.345/01DE-35").unwrap();
        assert_eq!(c.as_str(), "12ABC34501DE35");
    }

    #[test]
    fn cnpj_invalido() {
        assert_eq!(Cnpj::parse("11222333000182"), Err(DocumentoInvalido::DigitoVerificador));
        assert_eq!(Cnpj::parse("00000000000000"), Err(DocumentoInvalido::Repetido));
        assert!(matches!(Cnpj::parse("123"), Err(DocumentoInvalido::Tamanho { .. })));
        assert_eq!(Cnpj::parse("112223330001A1"), Err(DocumentoInvalido::Caractere('A')));
    }

    #[test]
    fn cpf() {
        assert_eq!(Cpf::parse("529.982.247-25").unwrap().to_string(), "529.982.247-25");
        assert_eq!(Cpf::parse("52998224724"), Err(DocumentoInvalido::DigitoVerificador));
        assert_eq!(Cpf::parse("111.111.111-11"), Err(DocumentoInvalido::Repetido));
    }
}
