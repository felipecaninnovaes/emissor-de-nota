//! Indicador da operação do IBS/CBS (`cIndOp`, 6 dígitos), embutido a partir da tabela "Indicadores
//! dos Locais de Operação" do Portal DFe (SVRS). A descrição é o nome do indicador e o "grupo" é o
//! local da operação que ele define (ex.: "Estabelecimento do fornecedor"). O arquivo
//! `dados/ind-operacao.tsv` é gerado pelo exemplo `gerar_ind_operacao`.

use std::sync::OnceLock;

pub use crate::tabela_codigos::ItemCodigo as ItemIndOperacao;
use crate::tabela_codigos::Tabela;

const TSV: &str = include_str!("../dados/ind-operacao.tsv");

fn lista() -> &'static Tabela {
    static LISTA: OnceLock<Tabela> = OnceLock::new();
    LISTA.get_or_init(|| Tabela::ler(TSV))
}

/// Origem e data da geração da lista embutida (indica a versão).
pub fn fonte() -> &'static str {
    lista().fonte
}

pub fn itens() -> &'static [ItemIndOperacao] {
    &lista().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static ItemIndOperacao> {
    lista().por_codigo(codigo)
}

/// Busca por nome ou código (mesma regra da Lista de Serviços Nacional).
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static ItemIndOperacao> {
    lista().buscar(termo, limite)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_embutida_e_valida() {
        assert!(
            fonte().contains("dfe-portal"),
            "primeira linha do TSV deve dizer a fonte"
        );
        let itens = itens();
        assert!(itens.len() >= 39);
        for janela in itens.windows(2) {
            assert!(
                janela[0].codigo < janela[1].codigo,
                "códigos fora de ordem ou repetidos: {}",
                janela[1].codigo
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
    fn servico_presencial_no_prestador_e_o_030101() {
        let i = por_codigo("030101").expect("030101 está na tabela");
        assert!(i.descricao.contains("fisicamente sobre a pessoa"), "{}", i.descricao);
        assert_eq!(i.grupo, "Estabelecimento do fornecedor");
        assert!(por_codigo("100301").unwrap().descricao.contains("Demais serviços"));
        assert!(por_codigo("999999").is_none());
    }

    #[test]
    fn busca_por_codigo_e_por_nome() {
        assert_eq!(buscar("030101", 10)[0].codigo, "030101");
        assert_eq!(buscar("0301", 10).len(), 4);
        assert!(
            buscar("presencialmente pessoa", 40)
                .iter()
                .any(|i| i.codigo == "030101")
        );
        assert!(buscar("  ", 10).is_empty());
    }
}
