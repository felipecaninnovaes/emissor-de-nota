//! Nomenclatura Brasileira de Serviços (NBS 2.0), código de 9 dígitos do `cNBS`, embutida a partir
//! do Anexo B da documentação técnica (aba `LISTA.NBS_v2.0`). Só os códigos completos (9 dígitos)
//! entram; o grupo de cada um é a descrição do nível imediatamente acima (vazio na posição sem
//! subdivisões, que já tem 9 dígitos). O arquivo
//! `dados/nbs.tsv` é gerado pelo exemplo `gerar_lista_servicos`, junto com a lista de serviços.

use std::sync::OnceLock;

pub use crate::tabela_codigos::ItemCodigo as ItemNbs;
use crate::tabela_codigos::Tabela;

const TSV: &str = include_str!("../dados/nbs.tsv");

fn lista() -> &'static Tabela {
    static LISTA: OnceLock<Tabela> = OnceLock::new();
    LISTA.get_or_init(|| Tabela::ler(TSV))
}

/// Nome do arquivo oficial de onde a lista embutida foi gerada (indica a versão).
pub fn fonte() -> &'static str {
    lista().fonte
}

/// Todos os códigos (9 dígitos), na ordem da NBS.
pub fn itens() -> &'static [ItemNbs] {
    &lista().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static ItemNbs> {
    lista().por_codigo(codigo)
}

/// Busca por nome ou código. Só dígitos (com ou sem pontos, como em `1.0101.11.00`): códigos que
/// começam com eles. Texto: itens que têm todas as palavras, sem diferenciar acento nem
/// maiúscula; quem casa pela descrição vem antes de quem casa só pelo grupo.
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static ItemNbs> {
    lista().buscar(termo, limite)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_embutida_e_valida() {
        assert!(fonte().ends_with(".xlsx"), "primeira linha do TSV deve dizer a fonte");
        let itens = itens();
        assert!(itens.len() > 800);
        for janela in itens.windows(2) {
            assert!(
                janela[0].codigo < janela[1].codigo,
                "códigos fora de ordem ou repetidos: {}",
                janela[1].codigo
            );
        }
        for i in itens {
            assert!(
                i.codigo.len() == 9 && i.codigo.bytes().all(|b| b.is_ascii_digit()),
                "{}",
                i.codigo
            );
            assert!(!i.descricao.is_empty(), "{}", i.codigo);
            // só a posição sem subdivisões (X.XXXX.00.00) fica sem grupo
            assert!(!i.grupo.is_empty() || i.codigo.ends_with("0000"), "{}", i.codigo);
        }
    }

    #[test]
    fn busca_por_codigo_com_ou_sem_pontos() {
        assert_eq!(buscar("1.0101.11.00", 10)[0].codigo, "101011100");
        assert_eq!(buscar("1.0101.1", 10).len(), 2);
        assert_eq!(buscar("101011100", 10).len(), 1);
        assert!(buscar("  ", 10).is_empty());
        assert!(por_codigo("000000000").is_none());
    }

    #[test]
    fn busca_por_nome_sem_acento() {
        let achados = buscar("edificacoes RESIDENCIAIS", 10);
        assert_eq!(achados.first().map(|i| i.codigo), Some("101011100"));
    }

    #[test]
    fn grupo_e_o_nivel_de_cima() {
        // 1.0102.35.10 fica sob 1.0102.35 (adutoras, irrigação e cursos de água)
        let item = por_codigo("101023510").unwrap();
        assert!(item.grupo.contains("adutoras"), "{}", item.grupo);
    }
}
