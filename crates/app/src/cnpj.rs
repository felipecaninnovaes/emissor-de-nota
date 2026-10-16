//! Consulta pública de CNPJ (publica.cnpj.ws) para preencher cadastros de tomador e empresa.
//! A API gratuita aceita 3 consultas por minuto; o resultado só sugere dados, o operador confere.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

const URL: &str = "https://publica.cnpj.ws/cnpj/";

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct DadosCnpj {
    pub cnpj: String,
    pub razao_social: String,
    pub nome_fantasia: Option<String>,
    /// "Ativa", "Baixada", "Inapta"...
    pub situacao: Option<String>,
    pub logradouro: Option<String>,
    pub numero: Option<String>,
    pub complemento: Option<String>,
    pub bairro: Option<String>,
    pub cep: Option<String>,
    /// Código IBGE com 7 dígitos.
    pub municipio: Option<String>,
    pub municipio_nome: Option<String>,
    pub uf: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
    /// `None` quando a base não informa.
    pub optante_simples: Option<bool>,
    pub mei: Option<bool>,
}

pub async fn consultar(cnpj: &str) -> Result<DadosCnpj, String> {
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let resposta = cliente
        .get(format!("{URL}{cnpj}"))
        .send()
        .await
        .map_err(|e| format!("sem conexão com a consulta de CNPJ: {e}"))?;
    let status = resposta.status();
    let corpo: Value = resposta
        .json()
        .await
        .map_err(|e| format!("resposta inválida da consulta de CNPJ: {e}"))?;
    if !status.is_success() {
        let detalhe = texto(&corpo, &["detalhes"]).or_else(|| texto(&corpo, &["titulo"]));
        return Err(match status.as_u16() {
            404 => "CNPJ não encontrado na base da Receita".into(),
            429 => "limite de 3 consultas por minuto atingido; aguarde um pouco e tente de novo".into(),
            _ => format!(
                "consulta de CNPJ falhou (HTTP {status}){}",
                detalhe.map(|d| format!(": {d}")).unwrap_or_default()
            ),
        });
    }
    Ok(interpretar(&corpo))
}

fn texto(v: &Value, caminho: &[&str]) -> Option<String> {
    let mut atual = v;
    for chave in caminho {
        atual = atual.get(chave)?;
    }
    let s = match atual {
        Value::String(s) => s.split_whitespace().collect::<Vec<_>>().join(" "),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

fn sim_nao(v: &Value, caminho: &[&str]) -> Option<bool> {
    texto(v, caminho).map(|s| s.eq_ignore_ascii_case("sim"))
}

pub fn interpretar(v: &Value) -> DadosCnpj {
    let e = |c: &str| texto(v, &["estabelecimento", c]);
    let logradouro = match (e("tipo_logradouro"), e("logradouro")) {
        (Some(tipo), Some(l)) if !l.to_uppercase().starts_with(&tipo.to_uppercase()) => Some(format!("{tipo} {l}")),
        (_, l) => l,
    };
    let telefone = match (e("ddd1"), e("telefone1")) {
        (Some(ddd), Some(tel)) => Some(format!("{ddd}{tel}")),
        (None, tel) => tel,
        _ => None,
    };
    DadosCnpj {
        cnpj: e("cnpj").unwrap_or_default(),
        razao_social: texto(v, &["razao_social"]).unwrap_or_default(),
        nome_fantasia: e("nome_fantasia"),
        situacao: e("situacao_cadastral"),
        logradouro,
        numero: e("numero"),
        complemento: e("complemento"),
        bairro: e("bairro"),
        cep: e("cep"),
        municipio: texto(v, &["estabelecimento", "cidade", "ibge_id"]),
        municipio_nome: texto(v, &["estabelecimento", "cidade", "nome"]),
        uf: texto(v, &["estabelecimento", "estado", "sigla"]),
        telefone,
        email: e("email").map(|m| m.to_lowercase()),
        optante_simples: sim_nao(v, &["simples", "simples"]),
        mei: sim_nao(v, &["simples", "mei"]),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn interpreta_resposta() {
        // Formato da publica.cnpj.ws com dados fictícios.
        let v: Value = serde_json::from_str(
            r#"{
              "razao_social": "EMPRESA EXEMPLO LTDA",
              "simples": { "simples": "Sim", "mei": "Não" },
              "estabelecimento": {
                "cnpj": "11222333000181", "nome_fantasia": null, "situacao_cadastral": "Ativa",
                "tipo_logradouro": "RUA", "logradouro": "EXEMPLO", "numero": "100",
                "complemento": "SALA   2   ", "bairro": "CENTRO", "cep": "15800000",
                "ddd1": "17", "telefone1": "35220000", "email": "CONTATO@EXEMPLO.COM",
                "cidade": { "nome": "Catanduva", "ibge_id": 3511102 },
                "estado": { "sigla": "SP" }
              }
            }"#,
        )
        .unwrap();
        let d = interpretar(&v);
        assert_eq!(d.razao_social, "EMPRESA EXEMPLO LTDA");
        assert_eq!(d.logradouro.as_deref(), Some("RUA EXEMPLO"));
        assert_eq!(d.complemento.as_deref(), Some("SALA 2"));
        assert_eq!(d.municipio.as_deref(), Some("3511102"));
        assert_eq!(d.telefone.as_deref(), Some("1735220000"));
        assert_eq!(d.email.as_deref(), Some("contato@exemplo.com"));
        assert_eq!(d.nome_fantasia, None);
        assert_eq!(d.optante_simples, Some(true));
        assert_eq!(d.mei, Some(false));
    }

    /// Consulta de verdade (rede; limite de 3 por minuto): `cargo test -p emissor-nfse -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn consulta_real() {
        let d = consultar("00000000000191").await.unwrap();
        assert_eq!(d.razao_social, "BANCO DO BRASIL SA");
        assert_eq!(d.municipio.as_deref(), Some("5300108"));
    }

    #[test]
    fn nao_repete_tipo_do_logradouro() {
        let v: Value = serde_json::from_str(
            r#"{"razao_social": "X", "simples": null,
               "estabelecimento": {"tipo_logradouro": "QUADRA", "logradouro": "QUADRA 5 BLOCO B"}}"#,
        )
        .unwrap();
        let d = interpretar(&v);
        assert_eq!(d.logradouro.as_deref(), Some("QUADRA 5 BLOCO B"));
        assert_eq!(d.optante_simples, None);
    }
}
