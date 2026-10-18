//! Lista de Serviços Nacional (`cTribNac`): o desdobro nacional dos subitens da LC 116, embutido a
//! partir do Anexo B da documentação técnica. A API do Sistema Nacional não lista os serviços; o
//! arquivo `dados/lista-servicos-nacional.tsv` é gerado pelo exemplo `gerar_lista_servicos`.

use std::sync::OnceLock;

use serde::Serialize;

use crate::texto::sem_acento_minusculo as normalizar;

const TSV: &str = include_str!("../dados/lista-servicos-nacional.tsv");

/// Um código da lista, com a descrição do desdobro e a do item da LC 116 a que pertence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemListaServico {
    /// 6 dígitos, sem pontos (ex.: `171901`).
    pub codigo: &'static str,
    pub descricao: &'static str,
    /// Descrição do item (ex.: "Serviços de Informática e congêneres.").
    pub grupo: &'static str,
}

struct Lista {
    fonte: &'static str,
    itens: Vec<ItemListaServico>,
    /// Texto normalizado de cada item para a busca (mesma ordem de `itens`).
    textos: Vec<(String, String)>,
}

fn lista() -> &'static Lista {
    static LISTA: OnceLock<Lista> = OnceLock::new();
    LISTA.get_or_init(|| {
        let mut fonte = "";
        let mut itens = Vec::new();
        for linha in TSV.lines().map(|l| l.trim_end_matches('\r')) {
            if let Some(f) = linha.strip_prefix("# fonte:") {
                fonte = f.trim();
                continue;
            }
            let mut campos = linha.split('\t');
            if let (Some(codigo), Some(descricao), Some(grupo)) = (campos.next(), campos.next(), campos.next()) {
                itens.push(ItemListaServico {
                    codigo,
                    descricao,
                    grupo,
                });
            }
        }
        let textos = itens
            .iter()
            .map(|i| (normalizar(i.descricao), normalizar(i.grupo)))
            .collect();
        Lista { fonte, itens, textos }
    })
}

/// Nome do arquivo oficial de onde a lista embutida foi gerada (indica a versão).
pub fn fonte() -> &'static str {
    lista().fonte
}

/// Todos os códigos, na ordem da lista.
pub fn itens() -> &'static [ItemListaServico] {
    &lista().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static ItemListaServico> {
    itens().iter().find(|i| i.codigo == codigo)
}

/// Busca por nome ou código. Só dígitos (com ou sem pontos, como em `17.19.01`): códigos que
/// começam com eles. Texto: itens que têm todas as palavras, sem diferenciar acento nem
/// maiúscula; quem casa pela descrição do desdobro vem antes de quem casa só pelo item.
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static ItemListaServico> {
    let lista = lista();
    let termo = termo.trim();
    if termo.is_empty() {
        return Vec::new();
    }
    if termo.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let digitos: String = termo.chars().filter(char::is_ascii_digit).collect();
        return lista
            .itens
            .iter()
            .filter(|i| i.codigo.starts_with(&digitos))
            .take(limite)
            .collect();
    }
    let palavras: Vec<String> = normalizar(termo).split_whitespace().map(str::to_string).collect();
    let mut pela_descricao = Vec::new();
    let mut pelo_grupo = Vec::new();
    for (item, (descricao, grupo)) in lista.itens.iter().zip(&lista.textos) {
        if palavras.iter().all(|p| descricao.contains(p.as_str())) {
            pela_descricao.push(item);
        } else if palavras
            .iter()
            .all(|p| descricao.contains(p.as_str()) || grupo.contains(p.as_str()))
        {
            pelo_grupo.push(item);
        }
    }
    pela_descricao.extend(pelo_grupo);
    pela_descricao.truncate(limite);
    pela_descricao
}

#[cfg(test)]
mod testes {
    use super::*;

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
