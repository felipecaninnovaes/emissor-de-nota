//! Emissão a partir de planilha (convênios): cada linha vira um pedido, com a empresa, o tomador e
//! o serviço já encontrados no cadastro, os erros que impedem a emissão e a situação de uma
//! importação anterior da mesma linha.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};

use nfse_core::modelo::Tomador;
use nfse_core::texto::sem_acento_minusculo;
use nfse_core::{Cnpj, Cpf, Valor};
use nfse_db::Banco;
use nfse_db::recorrencias::TentativaNoMes;

use crate::planilha::{Coluna, LinhaPlanilha};
use crate::{ErroEmissao, Origem, PedidoEmissao, aplicar_mes_ref};

/// Linha resolvida, pronta para conferência e emissão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemImportado {
    pub linha: usize,
    pub empresa_id: Option<i64>,
    pub empresa: Option<String>,
    pub pessoa_id: Option<i64>,
    /// Documento do tomador como foi entendido (só dígitos/letras).
    pub documento_tomador: String,
    pub tomador: Option<String>,
    /// O tomador existe no cadastro, mas ainda não é desta empresa (é vinculado ao emitir).
    pub vincular_tomador: bool,
    pub servico_id: Option<i64>,
    pub servico: Option<String>,
    pub valor: Option<Valor>,
    pub mes: NaiveDate,
    /// Descrição que vai na nota, com o mês aplicado.
    pub descricao: String,
    pub informacoes: Option<String>,
    /// Identifica o conteúdo da linha (e a repetição dentro da planilha).
    pub chave: String,
    pub anterior: Option<TentativaNoMes>,
    pub erros: Vec<String>,
    /// Diferenças que não impedem a emissão (dados do e-mail que diferem do cadastro).
    pub avisos: Vec<String>,
    /// Pessoa física do e-mail ainda sem cadastro: é cadastrada e vinculada ao emitir.
    pub tomador_novo: Option<Tomador>,
}

impl ItemImportado {
    /// Pode ser emitida: sem erros e sem nota anterior autorizada ou em andamento.
    pub fn emitivel(&self) -> bool {
        self.erros.is_empty() && self.anterior.as_ref().is_none_or(TentativaNoMes::libera_o_mes)
    }

    pub fn pedido(&self, arquivo: Option<&str>) -> Option<PedidoEmissao> {
        Some(PedidoEmissao {
            empresa_id: self.empresa_id?,
            tomador: self.pessoa_id,
            servico_id: self.servico_id?,
            valor: self.valor?,
            descricao: Some(self.descricao.clone()),
            mes_referencia: Some(self.mes),
            obra: None,
            local_prestacao: None,
            informacoes_complementares: self.informacoes.clone(),
            reter: Default::default(),
            origem: Origem::Planilha {
                chave: self.chave.clone(),
                arquivo: arquivo.map(str::to_owned),
                linha: Some(self.linha as i64),
            },
            substituir: None,
        })
    }
}

/// Valor em reais: "1.250,00", "1250,5", "1250.50", "R$ 80".
pub fn valor_da_planilha(texto: &str) -> Option<Valor> {
    let t: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
    let t = t.trim_start_matches("R$");
    let t = if t.contains(',') {
        t.replace('.', "")
    } else {
        t.to_owned()
    };
    Valor::parse(&t).ok().filter(|v| v.centesimos() > 0)
}

/// Mês: "2026-09", "09/2026", "9/2026", "2026-09-15", "15/09/2026".
pub fn mes_da_planilha(texto: &str) -> Option<NaiveDate> {
    let t = texto.trim();
    let partes: Vec<&str> = t.split(['-', '/']).collect();
    let (ano, mes) = match partes.as_slice() {
        [a, m] if a.len() == 4 => (a.parse().ok()?, m.parse().ok()?),
        [m, a] if a.len() == 4 => (a.parse().ok()?, m.parse().ok()?),
        [a, m, _] if a.len() == 4 => (a.parse().ok()?, m.parse().ok()?),
        [_, m, a] if a.len() == 4 => (a.parse().ok()?, m.parse().ok()?),
        _ => return None,
    };
    NaiveDate::from_ymd_opt(ano, mes, 1)
}

/// CNPJ ou CPF da planilha (o mais provável; ver [`documentos_da_planilha`]).
pub fn documento_da_planilha(texto: &str) -> Option<String> {
    documentos_da_planilha(texto).into_iter().next()
}

/// Documentos válidos que o texto da planilha pode ser, do mais provável ao menos. Número
/// digitado no Excel perde zeros à esquerda, e o mesmo número pode ser CPF e CNPJ válidos
/// (`00829502378` é CPF e, completado com zeros, também CNPJ): quem chama prefere o cadastrado.
/// Com 9 a 11 dígitos o CPF vem primeiro; com menos, o CNPJ (CPF raramente perde 3 zeros).
pub fn documentos_da_planilha(texto: &str) -> Vec<String> {
    let doc: String = texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if doc.is_empty() || doc.len() > 14 {
        return Vec::new();
    }
    if !doc.bytes().all(|b| b.is_ascii_digit()) {
        return Cnpj::parse(&doc)
            .ok()
            .map(|c| c.as_str().to_owned())
            .into_iter()
            .collect();
    }
    let cnpj = Cnpj::parse(&format!("{doc:0>14}")).ok().map(|c| c.as_str().to_owned());
    let cpf = (doc.len() <= 11)
        .then(|| Cpf::parse(&format!("{doc:0>11}")).ok())
        .flatten()
        .map(|c| c.as_str().to_owned());
    if doc.len() >= 9 {
        cpf.into_iter().chain(cnpj).collect()
    } else {
        cnpj.into_iter().chain(cpf).collect()
    }
}

/// Sem acento, minúsculo e sem espaços, para achar o serviço pelo nome.
fn nome_comparavel(texto: &str) -> String {
    sem_acento_minusculo(texto)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Tomador do cadastro achado por um dos documentos candidatos.
pub(crate) struct TomadorCadastrado {
    pub documento: String,
    pub pessoa_id: i64,
    pub nome: Option<String>,
    /// Existe no cadastro, mas ainda não é da empresa (é vinculado ao emitir).
    pub vincular: bool,
}

/// O primeiro candidato cadastrado (o cadastrado ganha quando o número pode ser CPF ou CNPJ).
pub(crate) async fn tomador_cadastrado(
    banco: &Banco,
    candidatos: &[String],
    empresa_id: Option<i64>,
) -> Result<Option<TomadorCadastrado>, ErroEmissao> {
    for doc in candidatos {
        let Some(pessoa_id) = banco.pessoa_por_documento(doc).await? else {
            continue;
        };
        let vincular = match empresa_id {
            // Só "não vinculado" vira vínculo novo; outro erro do banco interrompe a leitura.
            Some(e) => match banco.tomador(e, pessoa_id).await {
                Ok(_) => false,
                Err(nfse_db::ErroBanco::NaoEncontrado(_)) => true,
                Err(outro) => return Err(outro.into()),
            },
            None => false,
        };
        return Ok(Some(TomadorCadastrado {
            documento: doc.clone(),
            pessoa_id,
            nome: banco.nome_pessoa(pessoa_id).await?,
            vincular,
        }));
    }
    Ok(None)
}

/// O que identifica o conteúdo de uma linha importada (planilha ou e-mail).
pub(crate) struct ConteudoDaLinha<'a> {
    pub empresa_id: Option<i64>,
    pub documento_tomador: &'a str,
    pub servico_id: Option<i64>,
    pub valor: Option<Valor>,
    /// Descrição final, com o mês aplicado.
    pub descricao: &'a str,
    pub mes: NaiveDate,
    pub informacoes: Option<&'a str>,
}

/// Chave de conteúdo sem a ocorrência: as linhas com a mesma base têm o mesmo conteúdo.
pub(crate) fn base_da_chave(c: &ConteudoDaLinha) -> String {
    [
        c.empresa_id.map(|id| id.to_string()).unwrap_or_default(),
        c.documento_tomador.to_owned(),
        c.servico_id.map(|id| id.to_string()).unwrap_or_default(),
        c.valor.map(|v| v.centesimos().to_string()).unwrap_or_default(),
        c.descricao.to_owned(),
        format!("{}-{:02}", c.mes.year(), c.mes.month()),
        c.informacoes.unwrap_or_default().to_owned(),
    ]
    .join("\u{1f}")
}

/// Chave gravada na origem da nota: a base e a ocorrência dela no arquivo (1, 2, …).
pub(crate) fn chave_da_ocorrencia(base: &str, ocorrencia: usize) -> String {
    format!("{base}\u{1f}#{ocorrencia}")
}

/// Resolve as linhas da planilha contra o cadastro. `empresa_padrao` vale para as linhas sem a
/// coluna da empresa; `mes_padrao`, para as sem mês.
pub async fn resolver(
    banco: &Banco,
    linhas: &[LinhaPlanilha],
    empresa_padrao: Option<i64>,
    mes_padrao: NaiveDate,
) -> Result<Vec<ItemImportado>, ErroEmissao> {
    let empresas = banco.empresas().await?;
    let mut servicos_por_empresa = HashMap::new();
    let mut ocorrencias: HashMap<String, usize> = HashMap::new();
    let mut itens = Vec::with_capacity(linhas.len());

    for l in linhas {
        let mut erros = Vec::new();

        let empresa = match l.campo(Coluna::Empresa) {
            Some(texto) => {
                let docs = documentos_da_planilha(texto);
                let achada = docs.iter().find_map(|d| empresas.iter().find(|e| e.cnpj.as_str() == d));
                if achada.is_none() {
                    erros.push(format!("empresa {texto} não cadastrada"));
                }
                achada
            }
            None => {
                let achada = empresa_padrao.and_then(|id| empresas.iter().find(|e| e.id == id));
                if achada.is_none() {
                    erros.push("informe a empresa (coluna \"CNPJ empresa\" ou escolha na tela)".into());
                }
                achada
            }
        };

        let texto_tomador = l.campo(Coluna::Tomador).unwrap_or_default();
        let candidatos = documentos_da_planilha(texto_tomador);
        let cadastrado = tomador_cadastrado(banco, &candidatos, empresa.map(|e| e.id)).await?;
        let documento_tomador = match &cadastrado {
            Some(t) => t.documento.clone(),
            None => candidatos.first().cloned().unwrap_or_default(),
        };
        let (mut pessoa_id, mut tomador, mut vincular_tomador) = (None, None, false);
        if documento_tomador.is_empty() {
            erros.push(format!("CNPJ/CPF do tomador inválido: \"{texto_tomador}\""));
        } else if let Some(t) = cadastrado {
            pessoa_id = Some(t.pessoa_id);
            tomador = t.nome;
            vincular_tomador = t.vincular;
        } else {
            erros.push(format!("tomador {texto_tomador} não cadastrado"));
        }

        let mut servico = None;
        if let Some(e) = empresa {
            if let std::collections::hash_map::Entry::Vacant(vaga) = servicos_por_empresa.entry(e.id) {
                vaga.insert(banco.servicos(e.id).await?);
            }
            let lista = &servicos_por_empresa[&e.id];
            servico = match l.campo(Coluna::Servico) {
                Some(texto) => {
                    let alvo = nome_comparavel(texto);
                    let s = lista
                        .iter()
                        .find(|s| s.codigo_trib_nac == texto.trim())
                        .or_else(|| lista.iter().find(|s| nome_comparavel(&s.nome) == alvo));
                    if s.is_none() {
                        erros.push(format!("serviço \"{texto}\" não encontrado em {}", e.nome));
                    }
                    s
                }
                None if lista.len() == 1 => lista.first(),
                None => {
                    erros.push(format!(
                        "informe o serviço (coluna \"Servico\"): {} tem {} serviços cadastrados",
                        e.nome,
                        lista.len()
                    ));
                    None
                }
            };
        }

        let valor = match l.campo(Coluna::Valor) {
            Some(texto) => {
                let v = valor_da_planilha(texto);
                if v.is_none() {
                    erros.push(format!("valor inválido: \"{texto}\""));
                }
                v
            }
            None => {
                erros.push("sem valor".into());
                None
            }
        };

        let mes = match l.campo(Coluna::Mes) {
            Some(texto) => mes_da_planilha(texto).unwrap_or_else(|| {
                erros.push(format!("mês de referência inválido: \"{texto}\" (use 09/2026)"));
                mes_padrao
            }),
            None => mes_padrao,
        };

        let modelo_descricao = l
            .campo(Coluna::Descricao)
            .map(str::to_owned)
            .or_else(|| servico.map(|s| s.descricao.clone()))
            .unwrap_or_default();
        let descricao = aplicar_mes_ref(&modelo_descricao, mes);
        let informacoes = l.campo(Coluna::Informacoes).map(str::to_owned);

        let base = base_da_chave(&ConteudoDaLinha {
            empresa_id: empresa.map(|e| e.id),
            documento_tomador: &documento_tomador,
            servico_id: servico.and_then(|s| s.id),
            valor,
            descricao: &descricao,
            mes,
            informacoes: informacoes.as_deref(),
        });
        let ocorrencia = ocorrencias.entry(base.clone()).or_insert(0);
        *ocorrencia += 1;
        let chave = chave_da_ocorrencia(&base, *ocorrencia);
        let anterior = banco.tentativa_importada(&chave).await?;

        itens.push(ItemImportado {
            linha: l.numero,
            empresa_id: empresa.map(|e| e.id),
            empresa: empresa.map(|e| e.nome.clone()),
            pessoa_id,
            documento_tomador,
            tomador,
            vincular_tomador,
            servico_id: servico.and_then(|s| s.id),
            servico: servico.map(|s| s.nome.clone()),
            valor,
            mes,
            descricao,
            informacoes,
            chave,
            anterior,
            erros,
            avisos: Vec::new(),
            tomador_novo: None,
        });
    }
    Ok(itens)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn valores() {
        let v = |t| valor_da_planilha(t).map(|v| v.to_string());
        assert_eq!(v("1.250,00").as_deref(), Some("1250.00"));
        assert_eq!(v("R$ 80,5").as_deref(), Some("80.50"));
        assert_eq!(v("1250.50").as_deref(), Some("1250.50"));
        assert_eq!(v("0,00"), None);
        assert_eq!(v("abc"), None);
    }

    #[test]
    fn meses() {
        let m = |t| mes_da_planilha(t).map(|d| d.format("%Y-%m").to_string());
        assert_eq!(m("2026-09").as_deref(), Some("2026-09"));
        assert_eq!(m("9/2026").as_deref(), Some("2026-09"));
        assert_eq!(m("15/09/2026").as_deref(), Some("2026-09"));
        assert_eq!(m("2026-09-15").as_deref(), Some("2026-09"));
        assert_eq!(m("13/2026"), None);
        assert_eq!(m("setembro"), None);
    }

    #[test]
    fn documentos() {
        assert_eq!(
            documento_da_planilha("11.222.333/0001-81").as_deref(),
            Some("11222333000181")
        );
        assert_eq!(
            documento_da_planilha("12.abc.345/01de-35").as_deref(),
            Some("12ABC34501DE35")
        );
        // Excel guardou o CNPJ como número e perdeu os zeros: 00.000.000/0001-91.
        assert_eq!(documento_da_planilha("191").as_deref(), Some("00000000000191"));
        assert_eq!(documento_da_planilha("529.982.247-25").as_deref(), Some("52998224725"));
        assert_eq!(documento_da_planilha("123"), None);
    }

    #[test]
    fn numero_que_e_cpf_e_cnpj() {
        // CPF válido que, completado com zeros, também passa como CNPJ.
        assert_eq!(
            documentos_da_planilha("00829502378"),
            vec!["00829502378".to_owned(), "00000829502378".to_owned()]
        );
        // Excel tirou os zeros do CPF: ainda é CPF primeiro.
        assert_eq!(documentos_da_planilha("829502378")[0], "00829502378");
        // Poucos dígitos: CNPJ de zeros à esquerda (00.000.000/0001-91) antes do CPF 000.000.001-91.
        assert_eq!(
            documentos_da_planilha("191"),
            vec!["00000000000191".to_owned(), "00000000191".to_owned()]
        );
    }
}
