//! Classificação tributária do IBS/CBS (`cClassTrib`, 6 dígitos) e o CST (3 dígitos) a que cada
//! código pertence, embutidos a partir da tabela oficial do Portal DFe (SVRS). Cada `cClassTrib`
//! tem um único CST, então o par pode ser conferido. O arquivo `dados/class-trib.tsv` é gerado pelo
//! exemplo `gerar_class_trib`; as colunas são `código`, `descrição`, `nome do CST`, `CST`,
//! `redução do IBS (%)`, `redução da CBS (%)` e `fim da vigência` (vazio se vigente).

use std::sync::OnceLock;

use serde::Serialize;

use crate::tabela_codigos::Tabela;

const TSV: &str = include_str!("../dados/class-trib.tsv");

/// Um `cClassTrib` com o CST dele e os percentuais de redução de alíquota.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemClassTrib {
    pub codigo: &'static str,
    pub descricao: &'static str,
    /// Nome do CST (ex.: "Alíquota reduzida"); é o "grupo" da busca.
    pub grupo: &'static str,
    pub cst: &'static str,
    /// Redução da alíquota do IBS em %, como veio da tabela (ex.: `60`).
    pub reducao_ibs: &'static str,
    pub reducao_cbs: &'static str,
    /// `AAAA-MM-DD` do fim da vigência, vazio se o código está vigente.
    pub fim_vigencia: &'static str,
}

struct Lista {
    tabela: Tabela,
    /// Mesma ordem de `tabela.itens`.
    itens: Vec<ItemClassTrib>,
}

fn lista() -> &'static Lista {
    static LISTA: OnceLock<Lista> = OnceLock::new();
    LISTA.get_or_init(|| {
        let itens = TSV
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.starts_with('#'))
            .filter_map(|linha| {
                let mut c = linha.split('\t');
                let (codigo, descricao, grupo) = (c.next()?, c.next()?, c.next()?);
                Some(ItemClassTrib {
                    codigo,
                    descricao,
                    grupo,
                    cst: c.next()?,
                    reducao_ibs: c.next()?,
                    reducao_cbs: c.next()?,
                    fim_vigencia: c.next().unwrap_or(""),
                })
            })
            .collect();
        Lista {
            tabela: Tabela::ler(TSV),
            itens,
        }
    })
}

/// Nome do arquivo de origem da lista embutida (indica a versão).
pub fn fonte() -> &'static str {
    lista().tabela.fonte
}

pub fn itens() -> &'static [ItemClassTrib] {
    &lista().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static ItemClassTrib> {
    let l = lista();
    l.tabela.posicao(codigo).map(|p| &l.itens[p])
}

/// Busca por nome ou código (mesma regra da Lista de Serviços Nacional).
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static ItemClassTrib> {
    let l = lista();
    l.tabela
        .posicoes(termo, limite)
        .into_iter()
        .map(|p| &l.itens[p])
        .collect()
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
        assert!(itens.len() > 150);
        assert_eq!(itens.len(), lista().tabela.itens.len());
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
            assert!(
                i.cst.len() == 3 && i.cst.bytes().all(|b| b.is_ascii_digit()),
                "{}",
                i.codigo
            );
            assert!(!i.descricao.is_empty() && !i.grupo.is_empty(), "{}", i.codigo);
            assert!(i.reducao_ibs.parse::<f64>().is_ok() && i.reducao_cbs.parse::<f64>().is_ok());
            // o primeiro dígito do cClassTrib é o do CST
            assert_eq!(&i.codigo[..1], &i.cst[..1], "{}", i.codigo);
        }
    }

    #[test]
    fn saude_humana_tem_cst_200_e_reducao_de_60() {
        let i = por_codigo("200029").expect("200029 está na tabela");
        assert_eq!(i.cst, "200");
        assert_eq!((i.reducao_ibs, i.reducao_cbs), ("60", "60"));
        assert!(i.descricao.contains("saúde humana"), "{}", i.descricao);
        assert_eq!(por_codigo("000001").unwrap().cst, "000");
        assert!(por_codigo("999999").is_none());
    }

    #[test]
    fn busca_por_codigo_e_por_nome() {
        assert_eq!(buscar("200029", 10)[0].codigo, "200029");
        assert!(buscar("saude humana", 30).iter().any(|i| i.codigo == "200029"));
        assert!(buscar("  ", 10).is_empty());
    }
}
