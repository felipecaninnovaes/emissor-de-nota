//! Municípios do IBGE embutidos no `nfse-core`, para os campos de município da interface.

use serde::Serialize;

use nfse_core::municipios::{self, Municipio};

#[derive(Serialize)]
pub struct MunicipiosDto {
    /// Arquivo da DTB de onde a tabela veio (a tela cita no aviso de código fora da tabela).
    fonte: &'static str,
    itens: Vec<&'static Municipio>,
}

/// Busca na tabela de municípios embutida (por nome, nome e UF ou código). Não lê dados do usuário:
/// dispensa o cofre.
#[tauri::command]
pub fn buscar_municipios(termo: String) -> MunicipiosDto {
    MunicipiosDto {
        fonte: municipios::fonte(),
        itens: municipios::buscar(&termo, 30),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn busca_pelo_codigo_devolve_fonte_e_itens() {
        let json = serde_json::to_value(buscar_municipios("3543402".into())).unwrap();
        assert_eq!(json["fonte"], "RELATORIO_DTB_BRASIL_2025_MUNICIPIOS.xls");
        assert_eq!(
            json["itens"],
            serde_json::json!([{ "codigo": "3543402", "nome": "Ribeirão Preto", "uf": "SP" }])
        );
        let vazio = serde_json::to_value(buscar_municipios(String::new())).unwrap();
        assert_eq!(vazio["itens"], serde_json::json!([]));
    }
}
