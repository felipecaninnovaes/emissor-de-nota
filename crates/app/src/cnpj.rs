//! Consulta pública de CNPJ (BrasilAPI, dados da Receita) para preencher cadastros de tomador e empresa.
//! O resultado só sugere dados, o operador confere.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

const URL: &str = "https://brasilapi.com.br/api/cnpj/v1/";

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
        let detalhe = texto(&corpo, &["message"]);
        return Err(match status.as_u16() {
            404 => "CNPJ não encontrado na base da Receita".into(),
            429 => "limite de consultas atingido; aguarde um pouco e tente de novo".into(),
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

fn booleano(v: &Value, chave: &str) -> Option<bool> {
    v.get(chave)?.as_bool()
}

pub fn interpretar(v: &Value) -> DadosCnpj {
    let t = |c: &str| texto(v, &[c]);
    let logradouro = match (t("descricao_tipo_de_logradouro"), t("logradouro")) {
        (Some(tipo), Some(l)) if !l.to_uppercase().starts_with(&tipo.to_uppercase()) => Some(format!("{tipo} {l}")),
        (_, l) => l,
    };
    DadosCnpj {
        cnpj: t("cnpj").unwrap_or_default(),
        razao_social: t("razao_social").unwrap_or_default(),
        nome_fantasia: t("nome_fantasia"),
        situacao: t("descricao_situacao_cadastral"),
        logradouro,
        numero: t("numero"),
        complemento: t("complemento"),
        bairro: t("bairro"),
        cep: t("cep"),
        municipio: t("codigo_municipio_ibge"),
        municipio_nome: t("municipio"),
        uf: t("uf"),
        telefone: t("ddd_telefone_1"),
        email: t("email").map(|m| m.to_lowercase()),
        optante_simples: booleano(v, "opcao_pelo_simples"),
        mei: booleano(v, "opcao_pelo_mei"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn interpreta_resposta() {
        // Formato da BrasilAPI com dados fictícios.
        let v: Value = serde_json::from_str(
            r#"{
              "cnpj": "11222333000181", "razao_social": "EMPRESA EXEMPLO LTDA", "nome_fantasia": "",
              "descricao_situacao_cadastral": "ATIVA", "descricao_tipo_de_logradouro": "RUA",
              "logradouro": "EXEMPLO", "numero": "100", "complemento": "SALA   2   ",
              "bairro": "CENTRO", "cep": "15800000", "uf": "SP", "municipio": "CATANDUVA",
              "codigo_municipio_ibge": 3511102, "ddd_telefone_1": "1735220000",
              "email": "CONTATO@EXEMPLO.COM", "opcao_pelo_simples": true, "opcao_pelo_mei": false
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
        assert_eq!(d.situacao.as_deref(), Some("ATIVA"));
        assert_eq!(d.optante_simples, Some(true));
        assert_eq!(d.mei, Some(false));
    }

    /// Consulta de verdade (rede): `cargo test -p emissor-nfse -- --ignored`.
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
            r#"{"razao_social": "X", "opcao_pelo_simples": null,
               "descricao_tipo_de_logradouro": "QUADRA", "logradouro": "QUADRA 5 BLOCO B"}"#,
        )
        .unwrap();
        let d = interpretar(&v);
        assert_eq!(d.logradouro.as_deref(), Some("QUADRA 5 BLOCO B"));
        assert_eq!(d.optante_simples, None);
    }
}
