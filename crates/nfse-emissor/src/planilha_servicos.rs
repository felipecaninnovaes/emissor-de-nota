//! Planilha de serviços: importa o catálogo de uma empresa (ou de várias) de CSV/XLSX/XLS/ODS e
//! exporta no mesmo formato, para levar os serviços de uma empresa para outras.
//!
//! Uma linha por serviço; a primeira é o cabeçalho, comparado como na planilha de notas (sem
//! acento, maiúsculas, espaços ou pontuação). Só Nome, Descrição e o código de tributação nacional
//! são obrigatórios; o resto tem o mesmo padrão do formulário de serviço.

use std::collections::{BTreeMap, HashMap};

use nfse_core::modelo::{IbsCbs, Issqn, PisCofins, RetencaoIssqn, TotalTributos, TributacaoIssqn, TributosFederais};
use nfse_core::{Percentual, Valor};
use nfse_db::servicos::{RetencoesPercentuais, Servico, Tributacao};

use crate::importacao::valor_da_planilha;
use crate::planilha::{ler_tabela, normalizar};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColunaServico {
    Nome,
    Descricao,
    CodigoNacional,
    CodigoMunicipal,
    Nbs,
    ValorSugerido,
    TributacaoIss,
    RetencaoIss,
    AliquotaIss,
    TotalSimples,
    TotalFederal,
    TotalEstadual,
    TotalMunicipal,
    CstPisCofins,
    RetencaoPisCofins,
    Irrf,
    Csll,
    Inss,
    Pis,
    Cofins,
    IbsIndicador,
    IbsCst,
    IbsClassificacao,
    IbsConsumoPessoal,
}

use ColunaServico as C;

impl ColunaServico {
    pub const TODAS: [ColunaServico; 24] = [
        C::Nome,
        C::Descricao,
        C::CodigoNacional,
        C::CodigoMunicipal,
        C::Nbs,
        C::ValorSugerido,
        C::TributacaoIss,
        C::RetencaoIss,
        C::AliquotaIss,
        C::TotalSimples,
        C::TotalFederal,
        C::TotalEstadual,
        C::TotalMunicipal,
        C::CstPisCofins,
        C::RetencaoPisCofins,
        C::Irrf,
        C::Csll,
        C::Inss,
        C::Pis,
        C::Cofins,
        C::IbsIndicador,
        C::IbsCst,
        C::IbsClassificacao,
        C::IbsConsumoPessoal,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            C::Nome => "Nome",
            C::Descricao => "Descricao",
            C::CodigoNacional => "Codigo tributacao nacional",
            C::CodigoMunicipal => "Codigo municipal",
            C::Nbs => "NBS",
            C::ValorSugerido => "Valor sugerido",
            C::TributacaoIss => "Tributacao ISS",
            C::RetencaoIss => "ISS retido",
            C::AliquotaIss => "Aliquota ISS %",
            C::TotalSimples => "Total tributos Simples %",
            C::TotalFederal => "Total tributos federal %",
            C::TotalEstadual => "Total tributos estadual %",
            C::TotalMunicipal => "Total tributos municipal %",
            C::CstPisCofins => "CST PIS/COFINS",
            C::RetencaoPisCofins => "Retencao PIS/COFINS/CSLL",
            C::Irrf => "IRRF %",
            C::Csll => "CSLL %",
            C::Inss => "INSS %",
            C::Pis => "PIS %",
            C::Cofins => "COFINS %",
            C::IbsIndicador => "IBS/CBS cIndOp",
            C::IbsCst => "IBS/CBS CST",
            C::IbsClassificacao => "IBS/CBS cClassTrib",
            C::IbsConsumoPessoal => "IBS/CBS consumo pessoal",
        }
    }

    fn do_cabecalho(nome: &str) -> Option<ColunaServico> {
        let n = normalizar(nome);
        if let Some(c) = Self::TODAS.into_iter().find(|c| normalizar(c.titulo()) == n) {
            return Some(c);
        }
        let c = match n.as_str() {
            "nomenalista" | "nomedoservico" | "servico" => C::Nome,
            "descricaopadrao" | "discriminacao" | "descricaodoservico" => C::Descricao,
            "codigonacional"
            | "codtributacaonacional"
            | "codigotributacao"
            | "ctribnac"
            | "codigo"
            | "codigoservico"
            | "lc116"
            | "itemlc116" => C::CodigoNacional,
            "codtributacaomunicipal" | "codigotributacaomunicipal" | "ctribmun" => C::CodigoMunicipal,
            "codigonbs" => C::Nbs,
            "valor" | "valorpadrao" => C::ValorSugerido,
            "tributacaoissqn" | "tribiss" => C::TributacaoIss,
            "retencaoiss" | "retencaoissqn" | "issqnretido" => C::RetencaoIss,
            "aliquotaiss" | "aliquotaissqn" | "aliquota" => C::AliquotaIss,
            "simples" | "percentualsimples" | "totalsimples" | "ptottribsn" => C::TotalSimples,
            "federal" | "totalfederal" => C::TotalFederal,
            "estadual" | "totalestadual" => C::TotalEstadual,
            "municipal" | "totalmunicipal" => C::TotalMunicipal,
            "cst" | "cstpis" => C::CstPisCofins,
            "retencaopiscofins" | "tiporetencaopiscofins" | "tpretpiscofins" => C::RetencaoPisCofins,
            "cp" | "contribuicaoprevidenciaria" => C::Inss,
            "cindop" | "indicadoroperacao" | "ibscbsindicador" => C::IbsIndicador,
            "cstibscbs" | "cstibs" => C::IbsCst,
            "cclasstrib" | "classtrib" | "classificacaotributaria" => C::IbsClassificacao,
            "indfinal" | "consumopessoal" => C::IbsConsumoPessoal,
            _ => return None,
        };
        Some(c)
    }
}

/// Serviço lido de uma linha; com `erros`, o serviço é só o que deu para aproveitar (para a tela).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaServico {
    /// Número da linha na planilha (o cabeçalho é a 1).
    pub numero: usize,
    pub servico: Servico,
    pub erros: Vec<String>,
}

/// Lê a planilha de serviços. O `empresa_id` dos serviços sai 0: quem grava escolhe a empresa.
pub fn ler(nome_arquivo: &str, bytes: &[u8]) -> Result<Vec<LinhaServico>, String> {
    let mut linhas = ler_tabela(nome_arquivo, bytes)?.into_iter().enumerate();
    let (_, cabecalho) = linhas.next().ok_or("a planilha está vazia")?;
    let colunas: Vec<Option<ColunaServico>> = cabecalho.iter().map(|c| ColunaServico::do_cabecalho(c)).collect();
    for obrigatoria in [C::Nome, C::Descricao, C::CodigoNacional] {
        if !colunas.contains(&Some(obrigatoria)) {
            return Err(format!(
                "a primeira linha precisa ter a coluna \"{}\" (baixe o modelo para ver os nomes aceitos)",
                obrigatoria.titulo()
            ));
        }
    }
    let mut lidas: Vec<LinhaServico> = linhas
        .filter_map(|(i, celulas)| {
            let campos: BTreeMap<ColunaServico, String> = colunas
                .iter()
                .zip(celulas)
                .filter_map(|(coluna, valor)| Some(((*coluna)?, valor.trim().to_owned())))
                .filter(|(_, v)| !v.is_empty())
                .collect();
            (!campos.is_empty()).then(|| linha(i + 1, &campos))
        })
        .collect();
    let mut vistos: HashMap<String, usize> = HashMap::new();
    for l in &mut lidas {
        let chave = l.servico.nome.trim().to_lowercase();
        if chave.is_empty() {
            continue;
        }
        match vistos.get(&chave) {
            Some(primeira) => l.erros.push(format!("nome repetido (já está na linha {primeira})")),
            None => {
                vistos.insert(chave, l.numero);
            }
        }
    }
    Ok(lidas)
}

fn linha(numero: usize, campos: &BTreeMap<ColunaServico, String>) -> LinhaServico {
    let texto = |c: ColunaServico| campos.get(&c).map(String::as_str);
    let mut erros = Vec::new();
    let mut percentual = |c: ColunaServico| -> Option<Percentual> {
        let t = texto(c)?;
        let limpo: String = t.chars().filter(|c| !c.is_whitespace() && *c != '%').collect();
        // No XLSX, célula formatada como % chega como fração (2% = 0.02, 2,75% = 0.0275).
        // Nenhuma alíquota usada aqui fica abaixo de 0,5% (a menor comum é o PIS, 0,65%).
        let fracao = limpo.replace(',', ".").parse::<f64>().is_ok_and(|v| v > 0.0 && v < 0.5);
        match Percentual::parse(&limpo) {
            _ if fracao => {
                erros.push(format!(
                    "{}: \"{t}\" parece uma célula formatada como porcentagem no Excel; \
                     digite o número sem % (ex.: 2 para 2%) ou formate a coluna como número",
                    c.titulo()
                ));
                None
            }
            Ok(p) => Some(p),
            Err(_) => {
                erros.push(format!("{}: \"{t}\" não é um percentual (use 11,48)", c.titulo()));
                None
            }
        }
    };

    let aliquota_iss = percentual(C::AliquotaIss);
    let simples = percentual(C::TotalSimples);
    let federal = percentual(C::TotalFederal);
    let estadual = percentual(C::TotalEstadual);
    let municipal = percentual(C::TotalMunicipal);
    let retencoes = RetencoesPercentuais {
        irrf: percentual(C::Irrf),
        csll: percentual(C::Csll),
        cp: percentual(C::Inss),
        pis: percentual(C::Pis),
        cofins: percentual(C::Cofins),
        ..Default::default()
    };

    let total_tributos = match (simples, federal, estadual, municipal) {
        (Some(p), None, None, None) => TotalTributos::PercentualSimples(p),
        (Some(_), ..) => {
            erros.push(
                "informe o total de tributos pelo % do Simples ou pelos % federal/estadual/municipal, não os dois"
                    .into(),
            );
            TotalTributos::NaoInformado
        }
        (None, None, None, None) => TotalTributos::NaoInformado,
        (None, f, e, m) => TotalTributos::Percentuais {
            federal: f.unwrap_or_default(),
            estadual: e.unwrap_or_default(),
            municipal: m.unwrap_or_default(),
        },
    };

    let tributacao_iss = match texto(C::TributacaoIss).map(normalizar).as_deref() {
        None | Some("tributavel" | "operacaotributavel" | "1") => TributacaoIssqn::Tributavel,
        Some("imunidade" | "imune" | "2") => TributacaoIssqn::Imunidade,
        Some("exportacao" | "3") => TributacaoIssqn::Exportacao,
        Some("naoincidencia" | "4") => TributacaoIssqn::NaoIncidencia,
        Some(_) => {
            erros.push(format!(
                "{}: use Tributável, Imunidade, Exportação ou Não incidência",
                C::TributacaoIss.titulo()
            ));
            TributacaoIssqn::Tributavel
        }
    };
    let retencao_iss = match texto(C::RetencaoIss).map(normalizar).as_deref() {
        None | Some("nao" | "n" | "naoretido" | "1" | "false") => RetencaoIssqn::NaoRetido,
        Some("sim" | "s" | "retido" | "tomador" | "retidopelotomador" | "2" | "true") => {
            RetencaoIssqn::RetidoPeloTomador
        }
        Some("intermediario" | "retidopelointermediario" | "3") => RetencaoIssqn::RetidoPeloIntermediario,
        Some(_) => {
            erros.push(format!("{}: use Sim, Não ou Intermediário", C::RetencaoIss.titulo()));
            RetencaoIssqn::NaoRetido
        }
    };

    let cst = match texto(C::CstPisCofins).map(digitos) {
        None => "00".to_owned(),
        Some(d) if (1..=2).contains(&d.len()) => format!("{d:0>2}"),
        Some(_) => {
            erros.push(format!("{}: 2 dígitos (ex.: 00)", C::CstPisCofins.titulo()));
            "00".to_owned()
        }
    };
    let tipo_retencao = match texto(C::RetencaoPisCofins) {
        None => 0,
        Some(t) => match t.trim().parse::<u8>() {
            Ok(n) if n <= 9 => n,
            _ => {
                erros.push(format!("{}: código de 0 a 9", C::RetencaoPisCofins.titulo()));
                0
            }
        },
    };

    // IBS/CBS: o grupo existe se vier qualquer uma das colunas; CST e cClassTrib têm o padrão do formulário.
    let ibs_cbs = if [C::IbsIndicador, C::IbsCst, C::IbsClassificacao, C::IbsConsumoPessoal]
        .iter()
        .any(|c| texto(*c).is_some())
    {
        let indicador = completar(texto(C::IbsIndicador), 6);
        if indicador.is_none() {
            erros.push(format!(
                "{}: informe o código de 6 dígitos (ex.: 100301)",
                C::IbsIndicador.titulo()
            ));
        }
        let consumo_pessoal = match texto(C::IbsConsumoPessoal).map(normalizar).as_deref() {
            None | Some("nao" | "n" | "0" | "false") => None,
            Some("sim" | "s" | "1" | "true") => Some(true),
            Some(_) => {
                erros.push(format!("{}: use Sim ou Não", C::IbsConsumoPessoal.titulo()));
                None
            }
        };
        Some(IbsCbs {
            consumo_pessoal,
            indicador_operacao: indicador.unwrap_or_default(),
            destinatario_diferente_do_tomador: false,
            cst: completar(texto(C::IbsCst), 3).unwrap_or_else(|| "000".into()),
            classificacao_tributaria: completar(texto(C::IbsClassificacao), 6).unwrap_or_else(|| "000001".into()),
        })
    } else {
        None
    };

    let valor_sugerido = texto(C::ValorSugerido).and_then(|t| {
        let v = valor_da_planilha(t);
        if v.is_none() {
            erros.push(format!(
                "{}: \"{t}\" não é um valor (use 1.250,00)",
                C::ValorSugerido.titulo()
            ));
        }
        v
    });

    let servico = Servico {
        id: None,
        empresa_id: 0,
        nome: texto(C::Nome).unwrap_or_default().to_owned(),
        descricao: texto(C::Descricao).unwrap_or_default().to_owned(),
        // O Excel apaga os zeros à esquerda de códigos digitados como número (010101 → 10101).
        codigo_trib_nac: completar(texto(C::CodigoNacional), 6).unwrap_or_default(),
        codigo_trib_mun: completar(texto(C::CodigoMunicipal), 3),
        nbs: completar(texto(C::Nbs), 9),
        tributacao: Tributacao {
            issqn: Issqn {
                tributacao: tributacao_iss,
                retencao: retencao_iss,
                aliquota: aliquota_iss,
            },
            federais: TributosFederais {
                pis_cofins: Some(PisCofins {
                    cst,
                    base_calculo: None,
                    aliquota_pis: None,
                    aliquota_cofins: None,
                    valor_pis: None,
                    valor_cofins: None,
                    tipo_retencao: Some(tipo_retencao),
                }),
                ..Default::default()
            },
            total_tributos,
            ibs_cbs,
            retencoes,
            ..Default::default()
        },
        valor_sugerido,
    };
    if let Err(e) = servico.validar() {
        erros.push(e.to_string());
    }
    LinhaServico { numero, servico, erros }
}

fn digitos(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

/// Só os dígitos, com zeros à esquerda até `n` (se vier menor).
fn completar(texto: Option<&str>, n: usize) -> Option<String> {
    let d = digitos(texto?);
    if d.is_empty() {
        return None;
    }
    Some(if d.len() < n { format!("{d:0>n$}") } else { d })
}

/// Texto do decimal com vírgula, como o Excel brasileiro lê.
fn decimal(texto: impl ToString) -> String {
    texto.to_string().replace('.', ",")
}

/// Os serviços em CSV (separado por `;`), no formato que [`ler`] aceita.
pub fn para_csv(servicos: &[Servico]) -> String {
    let mut escritor = csv::WriterBuilder::new()
        .delimiter(b';')
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    let cabecalho: Vec<&str> = ColunaServico::TODAS.iter().map(|c| c.titulo()).collect();
    escritor.write_record(&cabecalho).expect("escrita em memória");
    for s in servicos {
        let t = &s.tributacao;
        let p = |v: Option<Percentual>| v.map(decimal).unwrap_or_default();
        let (simples, federal, estadual, municipal) = match &t.total_tributos {
            TotalTributos::PercentualSimples(v) => (Some(*v), None, None, None),
            TotalTributos::Percentuais {
                federal,
                estadual,
                municipal,
            } => (None, Some(*federal), Some(*estadual), Some(*municipal)),
            TotalTributos::NaoInformado | TotalTributos::Valores { .. } => (None, None, None, None),
        };
        let pc = t.federais.pis_cofins.as_ref();
        let ibs = t.ibs_cbs.as_ref();
        let linha: Vec<String> = ColunaServico::TODAS
            .iter()
            .map(|c| match c {
                C::Nome => s.nome.clone(),
                C::Descricao => s.descricao.clone(),
                C::CodigoNacional => s.codigo_trib_nac.clone(),
                C::CodigoMunicipal => s.codigo_trib_mun.clone().unwrap_or_default(),
                C::Nbs => s.nbs.clone().unwrap_or_default(),
                C::ValorSugerido => s.valor_sugerido.map(decimal).unwrap_or_default(),
                C::TributacaoIss => match t.issqn.tributacao {
                    TributacaoIssqn::Tributavel => "Tributável",
                    TributacaoIssqn::Imunidade => "Imunidade",
                    TributacaoIssqn::Exportacao => "Exportação",
                    TributacaoIssqn::NaoIncidencia => "Não incidência",
                }
                .to_owned(),
                C::RetencaoIss => match t.issqn.retencao {
                    RetencaoIssqn::NaoRetido => "Não",
                    RetencaoIssqn::RetidoPeloTomador => "Sim",
                    RetencaoIssqn::RetidoPeloIntermediario => "Intermediário",
                }
                .to_owned(),
                C::AliquotaIss => p(t.issqn.aliquota),
                C::TotalSimples => p(simples),
                C::TotalFederal => p(federal),
                C::TotalEstadual => p(estadual),
                C::TotalMunicipal => p(municipal),
                C::CstPisCofins => pc.map(|pc| pc.cst.clone()).unwrap_or_else(|| "00".into()),
                C::RetencaoPisCofins => pc.and_then(|pc| pc.tipo_retencao).unwrap_or(0).to_string(),
                C::Irrf => p(t.retencoes.irrf),
                C::Csll => p(t.retencoes.csll),
                C::Inss => p(t.retencoes.cp),
                C::Pis => p(t.retencoes.pis),
                C::Cofins => p(t.retencoes.cofins),
                C::IbsIndicador => ibs.map(|i| i.indicador_operacao.clone()).unwrap_or_default(),
                C::IbsCst => ibs.map(|i| i.cst.clone()).unwrap_or_default(),
                C::IbsClassificacao => ibs.map(|i| i.classificacao_tributaria.clone()).unwrap_or_default(),
                C::IbsConsumoPessoal => match ibs.map(|i| i.consumo_pessoal) {
                    Some(Some(true)) => "Sim".into(),
                    Some(_) => "Não".into(),
                    None => String::new(),
                },
            })
            .collect();
        escritor.write_record(&linha).expect("escrita em memória");
    }
    String::from_utf8(escritor.into_inner().expect("escrita em memória")).expect("CSV em UTF-8")
}

/// Modelo com um serviço de exemplo.
pub fn modelo_csv() -> String {
    let exemplo = Servico {
        id: None,
        empresa_id: 0,
        nome: "Consulta médica".into(),
        descricao: "CONSULTAS MÉDICAS REFERENTE A {mes_ref}".into(),
        codigo_trib_nac: "040101".into(),
        codigo_trib_mun: None,
        nbs: Some("123012100".into()),
        tributacao: Tributacao {
            federais: TributosFederais {
                pis_cofins: Some(PisCofins {
                    cst: "00".into(),
                    base_calculo: None,
                    aliquota_pis: None,
                    aliquota_cofins: None,
                    valor_pis: None,
                    valor_cofins: None,
                    tipo_retencao: Some(0),
                }),
                ..Default::default()
            },
            total_tributos: TotalTributos::PercentualSimples(Percentual::parse("6").expect("percentual")),
            ..Default::default()
        },
        valor_sugerido: Some(Valor::parse("150").expect("valor")),
    };
    para_csv(&[exemplo])
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn exporta_e_le_de_volta() {
        let mut s = ler("modelo.csv", modelo_csv().as_bytes()).unwrap().remove(0);
        assert!(s.erros.is_empty(), "{:?}", s.erros);
        s.servico.tributacao.issqn.aliquota = Some(Percentual::parse("2").unwrap());
        s.servico.tributacao.retencoes.pis = Some(Percentual::parse("0.65").unwrap());
        s.servico.tributacao.ibs_cbs = Some(IbsCbs {
            consumo_pessoal: Some(true),
            indicador_operacao: "030101".into(),
            destinatario_diferente_do_tomador: false,
            cst: "000".into(),
            classificacao_tributaria: "000001".into(),
        });
        s.servico.tributacao.total_tributos = TotalTributos::Percentuais {
            federal: Percentual::parse("13.45").unwrap(),
            estadual: Percentual::ZERO,
            municipal: Percentual::parse("5").unwrap(),
        };
        let csv = para_csv(std::slice::from_ref(&s.servico));
        assert!(csv.contains(";0,65;"), "{csv}");
        let lida = ler("servicos.csv", csv.as_bytes()).unwrap().remove(0);
        assert_eq!(lida.servico, s.servico);
        assert!(lida.erros.is_empty());
    }

    #[test]
    fn aceita_o_minimo_e_completa_zeros() {
        // Excel: código e NBS digitados como número; cabeçalhos com acento e sinônimos.
        let csv = "Nome;Descrição;Código;NBS;ISS retido;Simples\r\n\
                   Exame;EXAMES {mes_ref};10101;123012100;sim;6,5%\r\n\
                   Exame;outro;040101;;;\r\n\
                   Sem código;x;;;talvez;abc\r\n";
        let linhas = ler("servicos.csv", csv.as_bytes()).unwrap();
        assert_eq!(linhas.len(), 3);

        let a = &linhas[0];
        assert!(a.erros.is_empty(), "{:?}", a.erros);
        assert_eq!(a.servico.codigo_trib_nac, "010101");
        assert_eq!(a.servico.tributacao.issqn.retencao, RetencaoIssqn::RetidoPeloTomador);
        assert_eq!(
            a.servico.tributacao.total_tributos,
            TotalTributos::PercentualSimples(Percentual::parse("6.5").unwrap())
        );

        assert_eq!(linhas[1].erros, vec!["nome repetido (já está na linha 2)".to_owned()]);

        let c = &linhas[2];
        assert_eq!(c.numero, 4);
        assert_eq!(c.erros.len(), 3, "{:?}", c.erros);
    }

    #[test]
    fn ibs_cbs_com_padroes() {
        // Excel tira os zeros de 030101; CST e cClassTrib ficam no padrão.
        let csv = "Nome;Descricao;Codigo;cIndOp;CST IBS\r\n\
                   A;x;171901;30101;\r\n\
                   B;y;171901;;\r\n\
                   C;z;171901;;010\r\n";
        let linhas = ler("s.csv", csv.as_bytes()).unwrap();
        let a = linhas[0].servico.tributacao.ibs_cbs.as_ref().unwrap();
        assert_eq!(
            (
                a.indicador_operacao.as_str(),
                a.cst.as_str(),
                a.classificacao_tributaria.as_str()
            ),
            ("030101", "000", "000001")
        );
        assert_eq!(linhas[1].servico.tributacao.ibs_cbs, None);
        assert!(
            linhas[2].erros.iter().any(|e| e.contains("cIndOp")),
            "{:?}",
            linhas[2].erros
        );
    }

    #[test]
    fn percentual_em_fracao_do_excel_e_apontado() {
        // Como o XLSX chega depois de `celula`: 2% vira 0.02 e 2,75% vira 0.0275.
        let csv = "Nome;Descrição;Código;Aliquota ISS %;PIS %\r\n\
                   A;x;010101;0.02;0,65\r\n\
                   B;x;010101;0.0275;\r\n\
                   C;x;010101;2;0.65\r\n";
        let linhas = ler("servicos.csv", csv.as_bytes()).unwrap();
        assert!(
            linhas[0].erros.iter().any(|e| e.contains("porcentagem")),
            "{:?}",
            linhas[0].erros
        );
        assert_eq!(linhas[0].erros.len(), 1, "PIS 0,65 é válido: {:?}", linhas[0].erros);
        assert!(
            linhas[1].erros.iter().any(|e| e.contains("porcentagem")),
            "{:?}",
            linhas[1].erros
        );
        assert!(linhas[2].erros.is_empty(), "{:?}", linhas[2].erros);
        assert_eq!(
            linhas[2].servico.tributacao.issqn.aliquota,
            Some(Percentual::parse("2").unwrap())
        );
    }

    #[test]
    fn sem_coluna_obrigatoria() {
        let erro = ler("s.csv", b"Nome;Valor\r\nX;10\r\n").unwrap_err();
        assert!(erro.contains("Descricao"), "{erro}");
    }
}
