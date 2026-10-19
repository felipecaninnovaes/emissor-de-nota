//! Valores monetários e percentuais em inteiros (centavos / centésimos), sem ponto flutuante.
//!
//! No XML saem sempre com duas casas e ponto decimal (`150.00`), como pedem os tipos
//! `TSDec15V2`, `TSDec3V2` etc. do leiaute. Na entrada aceitam `150`, `150.5`, `150,50`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("valor decimal inválido: {0:?} (use até duas casas, ex.: 150.00)")]
pub struct DecimalInvalido(pub String);

/// Converte texto com até duas casas decimais em centésimos.
fn parse_centesimos(entrada: &str) -> Result<i64, DecimalInvalido> {
    let erro = || DecimalInvalido(entrada.to_owned());
    let s = entrada.trim().replace(',', ".");
    let (inteiro, frac) = s.split_once('.').unwrap_or((&s, ""));
    if inteiro.is_empty() || !inteiro.bytes().all(|b| b.is_ascii_digit()) || frac.len() > 2 {
        return Err(erro());
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(erro());
    }
    let inteiro: i64 = inteiro.parse().map_err(|_| erro())?;
    let frac: i64 = format!("{frac:0<2}").parse().map_err(|_| erro())?;
    inteiro
        .checked_mul(100)
        .and_then(|v| v.checked_add(frac))
        .ok_or_else(erro)
}

fn formatar_centesimos(v: i64, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let sinal = if v < 0 { "-" } else { "" };
    let v = v.unsigned_abs();
    write!(f, "{sinal}{}.{:02}", v / 100, v % 100)
}

macro_rules! decimal_2_casas {
    ($nome:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
        pub struct $nome(i64);

        impl $nome {
            pub const ZERO: Self = Self(0);

            pub fn de_centesimos(v: i64) -> Self {
                Self(v)
            }

            pub fn centesimos(self) -> i64 {
                self.0
            }

            pub fn parse(s: &str) -> Result<Self, DecimalInvalido> {
                parse_centesimos(s).map(Self)
            }

            /// Parte inteira (para conferir limites de dígitos do leiaute).
            pub fn parte_inteira(self) -> i64 {
                self.0 / 100
            }
        }

        impl fmt::Display for $nome {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatar_centesimos(self.0, f)
            }
        }

        impl Serialize for $nome {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $nome {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                // Aceita "150.00" ou 150 (inteiro). Número com casas decimais em JSON é recusado
                // de propósito, para não passar por ponto flutuante.
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Entrada {
                    Texto(String),
                    Inteiro(u64),
                }
                match Entrada::deserialize(d)? {
                    Entrada::Texto(t) => Self::parse(&t).map_err(serde::de::Error::custom),
                    Entrada::Inteiro(i) => i
                        .checked_mul(100)
                        .and_then(|v| i64::try_from(v).ok())
                        .map(Self)
                        .ok_or_else(|| serde::de::Error::custom("valor muito grande")),
                }
            }
        }
    };
}

decimal_2_casas!(Valor, "Valor em reais, guardado em centavos.");
decimal_2_casas!(
    Percentual,
    "Percentual com duas casas (ex.: alíquota 2,00%), guardado em centésimos."
);

impl Valor {
    /// `self × p%`, arredondado para o centavo mais próximo (meio para cima).
    pub fn aplicar(self, p: Percentual) -> Valor {
        // O produto passa de i64 com valores grandes (15 dígitos × 100,00%); o resultado cabe.
        let v = (i128::from(self.0) * i128::from(p.0) + 5_000) / 10_000;
        Valor(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX }))
    }

    /// `self × p%`, truncado no centavo (descarta a fração, como o Emissor Web faz em PIS e COFINS:
    /// 650,00 × 0,65% = 4,225 vira 4,22).
    pub fn aplicar_truncado(self, p: Percentual) -> Valor {
        let v = i128::from(self.0) * i128::from(p.0) / 10_000;
        Valor(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn parse_e_formatacao() {
        assert_eq!(Valor::parse("150").unwrap().to_string(), "150.00");
        assert_eq!(Valor::parse("150.5").unwrap().to_string(), "150.50");
        assert_eq!(Valor::parse("1234,56").unwrap().to_string(), "1234.56");
        assert_eq!(Valor::parse("0").unwrap().to_string(), "0.00");
        assert_eq!(Valor::parse("0.07").unwrap().to_string(), "0.07");
        for invalido in ["", "1.234", "-1", "1,2,3", "abc", ".5", "1.5x"] {
            assert!(Valor::parse(invalido).is_err(), "{invalido}");
        }
    }

    #[test]
    fn aplicar_percentual() {
        let v = Valor::parse("1000").unwrap();
        assert_eq!(v.aplicar(Percentual::parse("4.65").unwrap()).to_string(), "46.50");
        assert_eq!(
            Valor::parse("33.33")
                .unwrap()
                .aplicar(Percentual::parse("1.5").unwrap())
                .to_string(),
            "0.50"
        );
    }

    #[test]
    fn aplicar_truncado_descarta_a_fracao_do_centavo() {
        let p = |t: &str| Percentual::parse(t).unwrap();
        let v = Valor::parse("650").unwrap();
        // 4,225 e 19,50: o arredondamento comum daria 4,23
        assert_eq!(v.aplicar_truncado(p("0.65")).to_string(), "4.22");
        assert_eq!(v.aplicar(p("0.65")).to_string(), "4.23");
        assert_eq!(v.aplicar_truncado(p("3")).to_string(), "19.50");
    }

    #[test]
    fn valores_extremos() {
        let grande = Valor::parse("999999999999999.99").unwrap();
        assert_eq!(grande.aplicar(Percentual::parse("100").unwrap()), grande);
        assert_eq!(Valor::de_centesimos(-5).to_string(), "-0.05");
        assert_eq!(Valor::de_centesimos(-12345).to_string(), "-123.45");
    }

    #[test]
    fn serde() {
        let v: Valor = serde_json::from_str("\"150,00\"").unwrap();
        assert_eq!(v.centesimos(), 15000);
        let v: Valor = serde_json::from_str("150").unwrap();
        assert_eq!(v.centesimos(), 15000);
        assert!(serde_json::from_str::<Valor>("150.5").is_err());
        assert_eq!(serde_json::to_string(&v).unwrap(), "\"150.00\"");
    }
}
