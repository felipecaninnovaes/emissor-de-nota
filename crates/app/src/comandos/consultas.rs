//! Consultas públicas de CNPJ e CEP para sugerir dados no cadastro.

use nfse_core::Cnpj;

use super::comum::R;

/// Consulta pública do CNPJ para sugerir nome, endereço e contato no cadastro.
#[tauri::command]
pub async fn consultar_cnpj(cnpj: String) -> R<crate::cnpj::DadosCnpj> {
    let cnpj = Cnpj::parse(&cnpj).map_err(|e| format!("CNPJ: {e}"))?;
    crate::cnpj::consultar(cnpj.as_str()).await
}

/// Consulta pública do CEP para sugerir logradouro, bairro e município.
#[tauri::command]
pub async fn consultar_cep(cep: String) -> R<crate::cep::DadosCep> {
    let cep: String = cep.chars().filter(char::is_ascii_digit).collect();
    if cep.len() != 8 {
        return Err("CEP com 8 dígitos".into());
    }
    crate::cep::consultar(&cep).await
}

/// Busca de CEP pelo nome da rua, para quem não sabe o CEP. A UF e a cidade são obrigatórias no ViaCEP.
#[tauri::command]
pub async fn buscar_cep_por_endereco(uf: String, cidade: String, logradouro: String) -> R<Vec<crate::cep::DadosCep>> {
    let (uf, cidade, logradouro) = (uf.trim(), cidade.trim(), logradouro.trim());
    if uf.len() != 2 || !uf.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Escolha a UF.".into());
    }
    if cidade.chars().count() < 3 {
        return Err("Informe a cidade.".into());
    }
    if logradouro.chars().count() < 3 {
        return Err("Informe ao menos 3 letras do nome da rua.".into());
    }
    crate::cep::buscar_por_endereco(uf, cidade, logradouro).await
}
