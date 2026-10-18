//! Lista de Serviços Nacional (`cTribNac`): o desdobro nacional dos subitens da LC 116, embutido a
//! partir do Anexo B da documentação técnica. A API do Sistema Nacional não lista os serviços; o
//! arquivo `dados/lista-servicos-nacional.tsv` é gerado pelo exemplo `gerar_lista_servicos`.

use std::sync::OnceLock;

pub use crate::tabela_codigos::ItemCodigo as ItemListaServico;
use crate::tabela_codigos::Tabela;

const TSV: &str = include_str!("../dados/lista-servicos-nacional.tsv");

fn lista() -> &'static Tabela {
    static LISTA: OnceLock<Tabela> = OnceLock::new();
    LISTA.get_or_init(|| Tabela::ler(TSV))
}

/// Nome do arquivo oficial de onde a lista embutida foi gerada (indica a versão).
pub fn fonte() -> &'static str {
    lista().fonte
}

/// Todos os códigos (6 dígitos), na ordem da lista; o grupo é o item da LC 116.
pub fn itens() -> &'static [ItemListaServico] {
    &lista().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static ItemListaServico> {
    lista().por_codigo(codigo)
}

/// Busca por nome ou código. Só dígitos (com ou sem pontos, como em `17.19.01`): códigos que
/// começam com eles. Texto: itens que têm todas as palavras, sem diferenciar acento nem
/// maiúscula; quem casa pela descrição do desdobro vem antes de quem casa só pelo item.
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static ItemListaServico> {
    lista().buscar(termo, limite)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::texto::sem_acento_minusculo as normalizar;

    #[test]
    fn lista_embutida_e_valida() {
        assert!(fonte().ends_with(".xlsx"), "primeira linha do TSV deve dizer a fonte");
        let itens = itens();
        assert!(itens.len() > 300);
        for janela in itens.windows(2) {
            assert!(
                janela[0].codigo < janela[1].codigo,
                "códigos fora de ordem ou repetidos"
            );
        }
        for i in itens {
            assert!(
                i.codigo.len() == 6 && i.codigo.bytes().all(|b| b.is_ascii_digit()),
                "{}",
                i.codigo
            );
            assert!(!i.descricao.is_empty() && !i.grupo.is_empty(), "{}", i.codigo);
        }
    }

    #[test]
    fn busca_por_nome_sem_acento() {
        let achados = buscar("contabil", 10);
        assert_eq!(achados.first().map(|i| i.codigo), Some("171901"));
        assert_eq!(buscar("ANÁLISE desenvolvimento", 10)[0].codigo, "010101");
    }

    #[test]
    fn descricao_vem_antes_do_grupo() {
        // "informática" está no grupo de todo o item 01, mas só na descrição de alguns desdobros
        let achados = buscar("informatica", 50);
        let primeiro_so_grupo = achados
            .iter()
            .position(|i| !normalizar(i.descricao).contains("informatica"));
        let ultimo_descricao = achados
            .iter()
            .rposition(|i| normalizar(i.descricao).contains("informatica"));
        assert!(ultimo_descricao.unwrap() < primeiro_so_grupo.unwrap());
    }

    #[test]
    fn busca_por_codigo() {
        assert_eq!(buscar("17.19", 10)[0].codigo, "171901");
        assert_eq!(buscar("171901", 10).len(), 1);
        assert!(buscar("  ", 10).is_empty());
        assert_eq!(por_codigo("010201").map(|i| i.descricao), Some("Programação."));
        assert!(por_codigo("000000").is_none());
    }
}
