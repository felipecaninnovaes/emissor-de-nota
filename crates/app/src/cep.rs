//! Consulta pública de CEP (ViaCEP) para preencher endereço de obra e de tomador.
//! O resultado só sugere dados; o operador confere o número e o complemento.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

const URL: &str = "https://viacep.com.br/ws/";

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct DadosCep {
    pub cep: String,
    pub logradouro: Option<String>,
    pub complemento: Option<String>,
    pub bairro: Option<String>,
    /// Código IBGE com 7 dígitos.
    pub municipio: Option<String>,
    pub municipio_nome: Option<String>,
    pub uf: Option<String>,
}

pub async fn consultar(cep: &str) -> Result<DadosCep, String> {
    let corpo = buscar(&format!("{URL}{cep}/json/")).await?;
    interpretar(&corpo).ok_or_else(|| "CEP não encontrado".into())
}

/// Busca CEPs pelo nome da rua (até 50 resultados, como o ViaCEP devolve). Serve a quem não sabe o CEP.
pub async fn buscar_por_endereco(uf: &str, cidade: &str, logradouro: &str) -> Result<Vec<DadosCep>, String> {
    let mut url = reqwest::Url::parse(URL).map_err(|e| e.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "URL da consulta de CEP inválida".to_string())?
        .pop_if_empty()
        .extend([uf, cidade, logradouro, "json"]);
    let corpo = buscar(url.as_str()).await?;
    Ok(interpretar_lista(&corpo))
}

async fn buscar(url: &str) -> Result<Value, String> {
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let resposta = cliente
        .get(url)
        .send()
        .await
        .map_err(|e| format!("sem conexão com a consulta de CEP: {e}"))?;
    let status = resposta.status();
    if !status.is_success() {
        return Err(format!("consulta de CEP falhou (HTTP {status})"));
    }
    resposta
        .json()
        .await
        .map_err(|e| format!("resposta inválida da consulta de CEP: {e}"))
}

/// Itens da busca por endereço; resposta que não é lista (ou `{"erro": true}`) vira lista vazia.
pub fn interpretar_lista(v: &Value) -> Vec<DadosCep> {
    v.as_array()
        .map(|itens| itens.iter().filter_map(interpretar).collect())
        .unwrap_or_default()
}

fn texto(v: &Value, chave: &str) -> Option<String> {
    let s = v.get(chave)?.as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

/// `None` quando o ViaCEP responde `{"erro": true}` (CEP com formato válido, mas inexistente).
pub fn interpretar(v: &Value) -> Option<DadosCep> {
    if v.get("erro").is_some_and(|e| e == true || e == "true") {
        return None;
    }
    Some(DadosCep {
        cep: texto(v, "cep")?.replace('-', ""),
        logradouro: texto(v, "logradouro"),
        complemento: texto(v, "complemento"),
        bairro: texto(v, "bairro"),
        municipio: texto(v, "ibge"),
        municipio_nome: texto(v, "localidade"),
        uf: texto(v, "uf"),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn interpreta_resposta() {
        let v: Value = serde_json::from_str(
            r#"{"cep": "15800-000", "logradouro": "", "complemento": "", "bairro": "",
                "localidade": "Catanduva", "uf": "SP", "ibge": "3511102"}"#,
        )
        .unwrap();
        let d = interpretar(&v).unwrap();
        assert_eq!(d.cep, "15800000");
        assert_eq!(d.logradouro, None);
        assert_eq!(d.municipio.as_deref(), Some("3511102"));
        assert_eq!(d.municipio_nome.as_deref(), Some("Catanduva"));
    }

    #[test]
    fn interpreta_lista_da_busca_por_endereco() {
        let v: Value = serde_json::from_str(
            r#"[{"cep": "15800-020", "logradouro": "Rua Brasil", "bairro": "Centro",
                 "localidade": "Catanduva", "uf": "SP", "ibge": "3511102"},
                {"cep": "15800-030", "logradouro": "Rua Brasil", "bairro": "Vila Nova",
                 "localidade": "Catanduva", "uf": "SP", "ibge": "3511102"}]"#,
        )
        .unwrap();
        let lista = interpretar_lista(&v);
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[1].cep, "15800030");
        assert_eq!(lista[1].bairro.as_deref(), Some("Vila Nova"));
        assert!(interpretar_lista(&serde_json::json!([])).is_empty());
        assert!(interpretar_lista(&serde_json::json!({"erro": true})).is_empty());
    }

    #[test]
    fn cep_inexistente() {
        for corpo in [r#"{"erro": true}"#, r#"{"erro": "true"}"#] {
            assert_eq!(interpretar(&serde_json::from_str(corpo).unwrap()), None);
        }
    }

    /// Busca por rua de verdade (rede), com acento e espaço no nome.
    #[tokio::test]
    #[ignore]
    async fn busca_real_por_endereco() {
        let lista = buscar_por_endereco("SP", "São Paulo", "Praça da Sé").await.unwrap();
        assert!(lista.iter().any(|d| d.cep == "01001000"));
    }

    /// Consulta de verdade (rede): `cargo test -p emissor-nfse -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn consulta_real() {
        let d = consultar("01001000").await.unwrap();
        assert_eq!(d.logradouro.as_deref(), Some("Praça da Sé"));
        assert_eq!(d.municipio.as_deref(), Some("3550308"));
    }
}
