//! Pedidos de nota vindos de e-mail (spec 0001): cada pedido extraído do texto vira uma linha
//! como a da planilha ([`ItemImportado`]), para conferir e emitir pelo mesmo fluxo.
//!
//! Diferenças da planilha:
//! - o serviço vem da tela e vale para todas as linhas;
//! - pessoa física sem cadastro vira [`ItemImportado::tomador_novo`], com o município (IBGE)
//!   tirado da consulta do CEP, feita antes por quem chama ([`ceps_a_consultar`]);
//! - tomador cadastrado com nome ou CEP diferentes no e-mail gera aviso, e o cadastro não muda;
//! - pedido repetido no e-mail (histórico citado) é erro, não uma segunda nota.

use std::collections::HashMap;

use chrono::NaiveDate;

use nfse_core::Cpf;
use nfse_core::modelo::{Documento, EnderecoNacional, Tomador};
use nfse_core::texto::sem_acento_minusculo;
use nfse_db::Banco;

use crate::importacao::{
    ConteudoDaLinha, ItemImportado, base_da_chave, chave_da_ocorrencia, documentos_da_planilha, mes_da_planilha,
    tomador_cadastrado, valor_da_planilha,
};
use crate::pedido_email::PedidoEmail;
use crate::{ErroEmissao, aplicar_mes_ref};

/// Erro das linhas sem serviço escolhido na tela.
pub const ESCOLHA_SERVICO: &str = "escolha o serviço";
/// Erro do tomador novo sem município.
pub const SEM_MUNICIPIO: &str = "endereço sem município: confira o CEP";

/// Endereço devolvido pela consulta do CEP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnderecoDoCep {
    /// Código IBGE do município (7 dígitos).
    pub municipio: String,
    pub logradouro: String,
    pub bairro: String,
}

/// Resultado da consulta de cada CEP (8 dígitos). `None`: não encontrado ou a consulta falhou;
/// CEP fora do mapa conta como não consultado.
pub type CepsConsultados = HashMap<String, Option<EnderecoDoCep>>;

/// CEP com 8 dígitos, sem pontuação.
fn cep(texto: &str) -> Option<String> {
    let d: String = texto.chars().filter(char::is_ascii_digit).collect();
    (d.len() == 8).then_some(d)
}

/// Candidatos a documento do tomador do pedido (ver [`documentos_da_planilha`]).
fn candidatos(p: &PedidoEmail) -> Vec<String> {
    p.documento.as_deref().map(documentos_da_planilha).unwrap_or_default()
}

/// O pedido é de pessoa física sem cadastro (CPF válido que não está no banco).
async fn cpf_sem_cadastro(banco: &Banco, candidatos: &[String]) -> Result<Option<String>, ErroEmissao> {
    for doc in candidatos {
        if banco.pessoa_por_documento(doc).await?.is_some() {
            return Ok(None);
        }
    }
    Ok(candidatos.iter().find(|d| d.len() == 11).cloned())
}

/// CEPs a consultar antes de [`resolver`]: os das pessoas físicas sem cadastro, sem repetir, na
/// ordem do e-mail.
pub async fn ceps_a_consultar(banco: &Banco, pedidos: &[PedidoEmail]) -> Result<Vec<String>, ErroEmissao> {
    let mut ceps = Vec::new();
    for p in pedidos {
        if cpf_sem_cadastro(banco, &candidatos(p)).await?.is_some()
            && let Some(c) = p.cep.as_deref().and_then(cep)
            && !ceps.contains(&c)
        {
            ceps.push(c);
        }
    }
    Ok(ceps)
}

/// Resolve os pedidos do e-mail contra o cadastro, como [`crate::importacao::resolver`] faz com a
/// planilha. `servico_id`: o escolhido na tela; `mes_padrao`: para o e-mail sem período.
pub async fn resolver(
    banco: &Banco,
    pedidos: &[PedidoEmail],
    empresa_id: Option<i64>,
    servico_id: Option<i64>,
    mes_padrao: NaiveDate,
    ceps: &CepsConsultados,
) -> Result<Vec<ItemImportado>, ErroEmissao> {
    let empresa = match empresa_id {
        Some(id) => banco.empresas().await?.into_iter().find(|e| e.id == id),
        None => None,
    };
    let servico = match (&empresa, servico_id) {
        (Some(e), Some(id)) => banco.servicos(e.id).await?.into_iter().find(|s| s.id == Some(id)),
        _ => None,
    };
    let mut bases: HashMap<String, usize> = HashMap::new();
    let mut itens = Vec::with_capacity(pedidos.len());

    for p in pedidos {
        let mut erros = Vec::new();
        let mut avisos = Vec::new();
        if empresa.is_none() {
            erros.push("escolha a empresa na tela".to_owned());
        }
        if servico.is_none() {
            erros.push(ESCOLHA_SERVICO.to_owned());
        }

        let candidatos = candidatos(p);
        let cadastrado = tomador_cadastrado(banco, &candidatos, empresa.as_ref().map(|e| e.id)).await?;
        let documento_tomador = match &cadastrado {
            Some(t) => t.documento.clone(),
            None => candidatos.first().cloned().unwrap_or_default(),
        };
        let (mut pessoa_id, mut tomador, mut vincular_tomador, mut tomador_novo) = (None, None, false, None);
        if documento_tomador.is_empty() {
            erros.push(match &p.documento {
                Some(texto) => format!("CNPJ/CPF do tomador inválido: \"{texto}\""),
                None => "sem CPF/CNPJ do tomador".to_owned(),
            });
        } else if let Some(t) = cadastrado {
            if let Some(gravado) = banco.pessoa_cadastrada(&t.documento).await? {
                avisos.extend(aviso_de_diferenca(p, &gravado));
            }
            pessoa_id = Some(t.pessoa_id);
            tomador = t.nome;
            vincular_tomador = t.vincular;
        } else if documento_tomador.len() == 11 {
            match tomador_do_email(p, &documento_tomador, ceps) {
                Ok(t) => {
                    tomador = Some(t.nome.clone());
                    tomador_novo = Some(t);
                }
                Err(e) => erros.push(e),
            }
        } else {
            erros.push(format!("tomador {documento_tomador} não cadastrado"));
        }

        let valor = match &p.valor {
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
        let mes = p.mes.as_deref().and_then(mes_da_planilha).unwrap_or(mes_padrao);
        let modelo_descricao = p
            .descricao
            .clone()
            .or_else(|| servico.as_ref().map(|s| s.descricao.clone()))
            .unwrap_or_default();
        let descricao = aplicar_mes_ref(&modelo_descricao, mes);

        let base = base_da_chave(&ConteudoDaLinha {
            empresa_id: empresa.as_ref().map(|e| e.id),
            documento_tomador: &documento_tomador,
            servico_id: servico.as_ref().and_then(|s| s.id),
            valor,
            descricao: &descricao,
            mes,
            informacoes: None,
        });
        // O mesmo pedido de novo no e-mail (resposta com o histórico citado) não é outra nota.
        // Só entre linhas completas: as com erro já não emitem e mostram o próprio erro.
        if erros.is_empty() {
            match bases.get(&base) {
                Some(primeira) => erros.push(format!("repetido (já está na linha {primeira})")),
                None => {
                    bases.insert(base.clone(), p.numero);
                }
            }
        }
        let chave = chave_da_ocorrencia(&base, 1);
        let anterior = banco.tentativa_importada(&chave).await?;

        itens.push(ItemImportado {
            linha: p.numero,
            empresa_id: empresa.as_ref().map(|e| e.id),
            empresa: empresa.as_ref().map(|e| e.nome.clone()),
            pessoa_id,
            documento_tomador,
            tomador,
            vincular_tomador,
            servico_id: servico.as_ref().and_then(|s| s.id),
            servico: servico.as_ref().map(|s| s.nome.clone()),
            valor,
            mes,
            descricao,
            informacoes: None,
            chave,
            anterior,
            erros,
            avisos,
            tomador_novo,
        });
    }
    Ok(itens)
}

/// Pessoa física do e-mail, com o município do CEP consultado. Logradouro e bairro que faltam no
/// e-mail vêm da consulta.
fn tomador_do_email(p: &PedidoEmail, cpf: &str, ceps: &CepsConsultados) -> Result<Tomador, String> {
    let nome = p
        .nome
        .clone()
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| "sem o nome do paciente".to_owned())?;
    let cep = p.cep.as_deref().and_then(cep).ok_or_else(|| SEM_MUNICIPIO.to_owned())?;
    let consultado = ceps
        .get(&cep)
        .cloned()
        .flatten()
        .ok_or_else(|| SEM_MUNICIPIO.to_owned())?;
    let (logradouro, numero) = match p.endereco.as_deref() {
        Some(e) => separar_numero(e),
        None => (consultado.logradouro.clone(), "S/N".to_owned()),
    };
    let logradouro = if logradouro.is_empty() {
        consultado.logradouro
    } else {
        logradouro
    };
    let bairro = p.bairro.clone().unwrap_or(consultado.bairro);
    let documento = Cpf::parse(cpf).map_err(|e| format!("CPF do tomador inválido: {e}"))?;
    Ok(Tomador {
        documento: Documento::Cpf(documento),
        inscricao_municipal: None,
        nome,
        endereco: Some(EnderecoNacional {
            municipio: consultado.municipio,
            cep,
            logradouro,
            numero,
            complemento: None,
            bairro,
        }),
        telefone: None,
        email: None,
    })
}

/// "Rua das Flores, 100" → ("Rua das Flores", "100"); "Av. Brasil 2000" → ("Av. Brasil",
/// "2000"); sem número → "S/N".
fn separar_numero(endereco: &str) -> (String, String) {
    let endereco = endereco.trim();
    if let Some((rua, resto)) = endereco.rsplit_once(',') {
        let numero = resto.trim();
        if !numero.is_empty() {
            return (rua.trim().to_owned(), numero.to_owned());
        }
    }
    if let Some((rua, ultimo)) = endereco.rsplit_once(' ')
        && ultimo.starts_with(|c: char| c.is_ascii_digit())
    {
        return (rua.trim().to_owned(), ultimo.to_owned());
    }
    (endereco.to_owned(), "S/N".to_owned())
}

/// Aviso quando o nome ou o CEP do e-mail diferem do cadastro (que é o que vale).
fn aviso_de_diferenca(p: &PedidoEmail, gravado: &Tomador) -> Option<String> {
    let comparavel = |t: &str| sem_acento_minusculo(t).split_whitespace().collect::<Vec<_>>().join(" ");
    let mut campos = Vec::new();
    if let Some(nome) = &p.nome
        && comparavel(nome) != comparavel(&gravado.nome)
    {
        campos.push("nome");
    }
    if let Some(c) = p.cep.as_deref().and_then(cep)
        && gravado
            .endereco
            .as_ref()
            .is_none_or(|e| cep(&e.cep).as_deref() != Some(c.as_str()))
    {
        campos.push("CEP");
    }
    (!campos.is_empty()).then(|| {
        format!(
            "o {} do e-mail difere do cadastro; vale o cadastro",
            campos.join(" e o ")
        )
    })
}
