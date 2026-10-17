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
