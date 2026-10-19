//! Tabela de códigos oficiais embutida em TSV (`código\tdescrição\tgrupo`, com `# fonte: ...` na
//! primeira linha), com busca por nome ou código. Base da Lista de Serviços Nacional e da NBS.

use serde::Serialize;

use crate::texto::sem_acento_minusculo as normalizar;

/// Um código da tabela, com a descrição dele e a do grupo a que pertence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemCodigo {
    /// Só dígitos, sem pontos (ex.: `171901`).
    pub codigo: &'static str,
    pub descricao: &'static str,
    /// Descrição do grupo (ex.: "Serviços de Informática e congêneres.").
    pub grupo: &'static str,
}

pub(crate) struct Tabela {
    pub(crate) fonte: &'static str,
    pub(crate) itens: Vec<ItemCodigo>,
    /// Texto normalizado de cada item para a busca (mesma ordem de `itens`).
    textos: Vec<(String, String)>,
}

impl Tabela {
    pub(crate) fn ler(tsv: &'static str) -> Self {
        let mut fonte = "";
        let mut itens = Vec::new();
        for linha in tsv.lines().map(|l| l.trim_end_matches('\r')) {
            if let Some(f) = linha.strip_prefix("# fonte:") {
                fonte = f.trim();
                continue;
            }
            let mut campos = linha.split('\t');
            if let (Some(codigo), Some(descricao), Some(grupo)) = (campos.next(), campos.next(), campos.next()) {
                itens.push(ItemCodigo {
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
        Tabela { fonte, itens, textos }
    }

    pub(crate) fn por_codigo(&self, codigo: &str) -> Option<&ItemCodigo> {
        self.itens.iter().find(|i| i.codigo == codigo)
    }

    /// Busca por nome ou código. Só dígitos (com ou sem pontos, como em `17.19.01`): códigos que
    /// começam com eles. Texto: itens que têm todas as palavras, sem diferenciar acento nem
    /// maiúscula; quem casa pela descrição vem antes de quem casa só pelo grupo.
    pub(crate) fn buscar(&self, termo: &str, limite: usize) -> Vec<&ItemCodigo> {
        self.posicoes(termo, limite)
            .into_iter()
            .map(|p| &self.itens[p])
            .collect()
    }

    /// Posição de `codigo` em `itens`.
    pub(crate) fn posicao(&self, codigo: &str) -> Option<usize> {
        self.itens.iter().position(|i| i.codigo == codigo)
    }

    /// Mesma busca de `buscar`, devolvendo a posição em `itens` (para tabelas com colunas a mais).
    pub(crate) fn posicoes(&self, termo: &str, limite: usize) -> Vec<usize> {
        let termo = termo.trim();
        if termo.is_empty() {
            return Vec::new();
        }
        if termo.chars().all(|c| c.is_ascii_digit() || c == '.') {
            let digitos: String = termo.chars().filter(char::is_ascii_digit).collect();
            return (0..self.itens.len())
                .filter(|&p| self.itens[p].codigo.starts_with(&digitos))
                .take(limite)
                .collect();
        }
        let palavras: Vec<String> = normalizar(termo).split_whitespace().map(str::to_string).collect();
        let mut pela_descricao = Vec::new();
        let mut pelo_grupo = Vec::new();
        for (p, (descricao, grupo)) in self.textos.iter().enumerate() {
            if palavras.iter().all(|w| descricao.contains(w.as_str())) {
                pela_descricao.push(p);
            } else if palavras
                .iter()
                .all(|w| descricao.contains(w.as_str()) || grupo.contains(w.as_str()))
            {
                pelo_grupo.push(p);
            }
        }
        pela_descricao.extend(pelo_grupo);
        pela_descricao.truncate(limite);
        pela_descricao
    }
}
