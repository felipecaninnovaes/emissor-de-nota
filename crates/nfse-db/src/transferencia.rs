//! Exportação e importação de empresas entre máquinas (`.nfemp`, ADR 0015).
//!
//! O arquivo leva só cadastros: a empresa, os serviços ativos, os tomadores vinculados e as
//! recorrências ativas. Notas, numeração, certificados e configurações ficam na origem. O conteúdo
//! (JSON) vai cifrado com uma senha própria da exportação (Argon2id + AES-256-GCM, pelo [`Cofre`]),
//! nunca com a senha mestra: o arquivo é para outro operador abrir.
//!
//! Formato: `NFEMP1`, tamanho do cabeçalho (u32 big-endian), cabeçalho do cofre (JSON, sem segredo)
//! e `nonce || texto cifrado` do pacote.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use nfse_core::modelo::{Ambiente, Obra, RegimeTributario, Tomador};
use nfse_core::{Cnpj, Valor};

use crate::empresas::{NovaEmpresa, ambiente_texto, validar};
use crate::servicos::{Servico, Tributacao, chave_nome, gravar_servico};
use crate::tomadores::{SELECT_TOMADOR, TipoTomador, gravar_tomador, tipo_e_numero, tomador_de};
use crate::{Banco, CabecalhoCofre, Cofre, ErroBanco, ErroCofre, Resultado, agora, auditar_em, de_json, json};

pub const EXTENSAO: &str = "nfemp";
const MAGICO: &[u8] = b"NFEMP1";
const VERSAO: u32 = 1;
const CONTEXTO: &str = "exportacao-empresas:v1";
const TETO_CABECALHO: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacoteEmpresas {
    pub versao: u32,
    pub gerado_em: String,
    pub empresas: Vec<EmpresaExportada>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmpresaExportada {
    pub cnpj: String,
    pub nome: String,
    pub inscricao_municipal: Option<String>,
    pub im_na_dps_producao: bool,
    pub im_na_dps_homologacao: bool,
    pub municipio_emissor: String,
    pub regime: RegimeTributario,
    pub ambiente: Ambiente,
    pub serie: String,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub servicos: Vec<ServicoExportado>,
    pub tomadores: Vec<TomadorExportado>,
    pub recorrencias: Vec<RecorrenciaExportada>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServicoExportado {
    pub nome: String,
    pub descricao: String,
    pub codigo_trib_nac: String,
    pub codigo_trib_mun: Option<String>,
    pub nbs: Option<String>,
    pub tributacao: Tributacao,
    /// Centésimos.
    pub valor_sugerido: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TomadorExportado {
    pub tomador: Tomador,
    pub emails: Option<String>,
    pub observacao: Option<String>,
    pub retem_federais: bool,
    pub retem_iss: bool,
    /// Pacotes antigos não têm o campo: tomador privado.
    #[serde(default)]
    pub tipo: TipoTomador,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecorrenciaExportada {
    /// Documento do tomador (um dos `tomadores` do pacote); `None` = nota sem tomador.
    pub documento_tomador: Option<String>,
    /// Nome do serviço (um dos `servicos` do pacote).
    pub servico: String,
    /// Centésimos; `None` = valor sugerido do serviço.
    pub valor: Option<i64>,
    pub descricao: Option<String>,
    pub informacoes_complementares: Option<String>,
    pub obra: Option<Obra>,
}

/// Como a empresa do arquivo entra nesta máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoImportacao {
    /// Não existe aqui: é cadastrada e a numeração precisa ser conferida na Sefin.
    Nova,
    /// Já existe: atualiza os dados cadastrais; ambiente, série e numeração ficam.
    Existente,
    /// Existe arquivada (tinha notas): volta ativa com o histórico.
    Arquivada,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviaEmpresa {
    pub cnpj: String,
    pub nome: String,
    pub situacao: SituacaoImportacao,
    pub servicos: usize,
    pub tomadores: usize,
    pub recorrencias: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ResumoImportacaoEmpresa {
    pub cnpj: String,
    pub nome: String,
    pub nova: bool,
    pub servicos_novos: usize,
    pub servicos_atualizados: usize,
    pub tomadores: usize,
    pub recorrencias_novas: usize,
    pub recorrencias_atualizadas: usize,
}

fn erro_senha(e: ErroCofre) -> ErroBanco {
    ErroBanco::Regra(match e {
        ErroCofre::SenhaCurta => "a senha da exportação precisa ter pelo menos 10 caracteres".into(),
        ErroCofre::SenhaFraca => {
            "senha da exportação fácil de adivinhar (só números, sequência ou palavra comum); use uma frase".into()
        }
        ErroCofre::SenhaIncorreta => "senha incorreta para este arquivo".into(),
        ErroCofre::Corrompido => "arquivo de exportação corrompido ou alterado".into(),
        ErroCofre::Parametros(p) => format!("arquivo de exportação inválido: {p}"),
    })
}

fn arquivo_invalido() -> ErroBanco {
    ErroBanco::Regra("o arquivo não é uma exportação de empresas deste emissor".into())
}

/// Texto aparado; vazio vira `None` (como as recorrências são gravadas).
fn vazio(t: &Option<String>) -> Option<String> {
    t.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned)
}

/// Cifra o pacote com a senha da exportação (Argon2id com os parâmetros do cofre; demora: chamar
/// fora do runtime assíncrono). Recusa senha curta ou previsível.
pub fn cifrar_pacote(pacote: &PacoteEmpresas, senha: &str) -> Resultado<Vec<u8>> {
    let (cofre, cabecalho) = Cofre::criar(senha).map_err(erro_senha)?;
    cifrar_com(pacote, &cofre, &cabecalho)
}

fn cifrar_com(pacote: &PacoteEmpresas, cofre: &Cofre, cabecalho: &CabecalhoCofre) -> Resultado<Vec<u8>> {
    let cabecalho = serde_json::to_vec(cabecalho).expect("cabeçalho serializa");
    let conteudo = zeroize::Zeroizing::new(serde_json::to_vec(pacote).expect("pacote serializa"));
    let cifrado = cofre.cifrar(&conteudo, CONTEXTO).map_err(erro_senha)?;
    let mut saida = Vec::with_capacity(MAGICO.len() + 4 + cabecalho.len() + cifrado.len());
    saida.extend_from_slice(MAGICO);
    saida.extend_from_slice(&(cabecalho.len() as u32).to_be_bytes());
    saida.extend_from_slice(&cabecalho);
    saida.extend_from_slice(&cifrado);
    Ok(saida)
}

/// Abre o arquivo com a senha da exportação (também demora, como [`cifrar_pacote`]).
pub fn decifrar_pacote(bytes: &[u8], senha: &str) -> Resultado<PacoteEmpresas> {
    let resto = bytes.strip_prefix(MAGICO).ok_or_else(arquivo_invalido)?;
    let (tamanho, resto) = resto.split_first_chunk::<4>().ok_or_else(arquivo_invalido)?;
    let tamanho = u32::from_be_bytes(*tamanho) as usize;
    if tamanho > TETO_CABECALHO || tamanho > resto.len() {
        return Err(arquivo_invalido());
    }
    let (cabecalho, cifrado) = resto.split_at(tamanho);
    let cabecalho: CabecalhoCofre = serde_json::from_slice(cabecalho).map_err(|_| arquivo_invalido())?;
    let cofre = Cofre::abrir(senha, &cabecalho).map_err(erro_senha)?;
    let conteudo = cofre.decifrar(cifrado, CONTEXTO).map_err(erro_senha)?;
    let pacote: PacoteEmpresas = serde_json::from_slice(&conteudo)
        .map_err(|e| ErroBanco::Regra(format!("conteúdo da exportação inválido: {e}")))?;
    if pacote.versao != VERSAO {
        return Err(ErroBanco::Regra(format!(
            "exportação feita por uma versão mais nova do emissor (formato {}); atualize este",
            pacote.versao
        )));
    }
    Ok(pacote)
}

impl Banco {
    /// Monta o pacote das empresas (ativas) dadas, com serviços, tomadores e recorrências ativos.
    pub async fn exportar_empresas(&self, empresa_ids: &[i64]) -> Resultado<PacoteEmpresas> {
        if empresa_ids.is_empty() {
            return Err(ErroBanco::Regra("escolha ao menos uma empresa para exportar".into()));
        }
        let mut empresas = Vec::new();
        for &id in empresa_ids {
            let e = self.empresa(id).await?;
            if !e.ativa {
                return Err(ErroBanco::NaoEncontrado("empresa"));
            }
            let servicos = self
                .servicos(id)
                .await?
                .into_iter()
                .map(|s| ServicoExportado {
                    nome: s.nome,
                    descricao: s.descricao,
                    codigo_trib_nac: s.codigo_trib_nac,
                    codigo_trib_mun: s.codigo_trib_mun,
                    nbs: s.nbs,
                    tributacao: s.tributacao,
                    valor_sugerido: s.valor_sugerido.map(|v| v.centesimos()),
                })
                .collect();
            let tomadores = sqlx::query(&format!("{SELECT_TOMADOR} WHERE v.empresa_id = ? ORDER BY p.nome"))
                .bind(id)
                .fetch_all(self.pool())
                .await?
                .iter()
                .map(|r| {
                    tomador_de(r).map(|t| TomadorExportado {
                        tomador: t.tomador,
                        emails: t.emails,
                        observacao: t.observacao,
                        retem_federais: t.retem_federais,
                        retem_iss: t.retem_iss,
                        tipo: t.tipo,
                    })
                })
                .collect::<Resultado<Vec<_>>>()?;
            // Recorrência de serviço desativado não sai: o serviço não vai no pacote.
            let recorrencias = sqlx::query(
                "SELECT p.documento, s.nome, r.valor, r.descricao, r.informacoes_complementares, r.obra_json
                 FROM recorrencia r JOIN servico s ON s.id = r.servico_id LEFT JOIN pessoa p ON p.id = r.pessoa_id
                 WHERE r.empresa_id = ? AND r.ativa = 1 AND s.ativo = 1 ORDER BY r.id",
            )
            .bind(id)
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(|r| {
                Ok(RecorrenciaExportada {
                    documento_tomador: r.get(0),
                    servico: r.get(1),
                    valor: r.get(2),
                    descricao: r.get(3),
                    informacoes_complementares: r.get(4),
                    obra: r.get::<Option<String>, _>(5).map(|o| de_json(&o, "obra")).transpose()?,
                })
            })
            .collect::<Resultado<Vec<_>>>()?;
            empresas.push(EmpresaExportada {
                cnpj: e.cnpj.as_str().to_owned(),
                nome: e.nome,
                inscricao_municipal: e.inscricao_municipal,
                im_na_dps_producao: e.im_na_dps_producao,
                im_na_dps_homologacao: e.im_na_dps_homologacao,
                municipio_emissor: e.municipio_emissor,
                regime: e.regime,
                ambiente: e.ambiente,
                serie: e.serie,
                telefone: e.telefone,
                email: e.email,
                servicos,
                tomadores,
                recorrencias,
            });
        }
        Ok(PacoteEmpresas {
            versao: VERSAO,
            gerado_em: agora(),
            empresas,
        })
    }

    /// O que a importação faria com cada empresa do pacote (sem gravar nada).
    pub async fn previa_importacao(&self, pacote: &PacoteEmpresas) -> Resultado<Vec<PreviaEmpresa>> {
        let mut previa = Vec::new();
        for e in &pacote.empresas {
            let ativa: Option<bool> =
                sqlx::query("SELECT e.ativa FROM empresa e JOIN pessoa p ON p.id = e.pessoa_id WHERE p.documento = ?")
                    .bind(&e.cnpj)
                    .fetch_optional(self.pool())
                    .await?
                    .map(|r| r.get(0));
            previa.push(PreviaEmpresa {
                cnpj: e.cnpj.clone(),
                nome: e.nome.clone(),
                situacao: match ativa {
                    None => SituacaoImportacao::Nova,
                    Some(true) => SituacaoImportacao::Existente,
                    Some(false) => SituacaoImportacao::Arquivada,
                },
                servicos: e.servicos.len(),
                tomadores: e.tomadores.len(),
                recorrencias: e.recorrencias.len(),
            });
        }
        Ok(previa)
    }

    /// Importa as empresas do pacote com os CNPJs dados, tudo ou nada (ADR 0015). Empresa nova
    /// fica marcada para conferir a numeração na Sefin; existente mantém ambiente, série,
    /// numeração, certificados e notas.
    pub async fn importar_empresas(
        &self,
        pacote: &PacoteEmpresas,
        cnpjs: &[String],
    ) -> Resultado<Vec<ResumoImportacaoEmpresa>> {
        let escolhidas: Vec<&EmpresaExportada> = pacote.empresas.iter().filter(|e| cnpjs.contains(&e.cnpj)).collect();
        if escolhidas.is_empty() {
            return Err(ErroBanco::Regra("marque ao menos uma empresa para importar".into()));
        }
        // Valida tudo antes de abrir a transação: a mensagem diz qual empresa e qual item.
        let mut validadas = Vec::new();
        for e in &escolhidas {
            let nova = nova_empresa(e)?;
            let servicos = servicos_de(e)?;
            for t in &e.tomadores {
                if t.tomador.nome.trim().is_empty() {
                    return Err(ErroBanco::Regra(format!("{}: tomador sem nome", e.nome)));
                }
            }
            validadas.push((*e, nova, servicos));
        }

        let mut tx = self.pool().begin().await?;
        let mut resumos = Vec::new();
        for (e, nova, servicos) in validadas {
            let mut resumo = ResumoImportacaoEmpresa {
                cnpj: e.cnpj.clone(),
                nome: e.nome.clone(),
                ..Default::default()
            };
            let (empresa_id, criada) = gravar_empresa(&mut tx, e, &nova).await?;
            resumo.nova = criada;

            let mut existentes: HashMap<String, (i64, Option<i64>)> =
                sqlx::query("SELECT id, nome, valor_sugerido FROM servico WHERE empresa_id = ? AND ativo = 1")
                    .bind(empresa_id)
                    .fetch_all(&mut *tx)
                    .await?
                    .iter()
                    .map(|r| (chave_nome(r.get::<&str, _>(1)), (r.get(0), r.get(2))))
                    .collect();
            for s in servicos {
                let chave = chave_nome(&s.nome);
                let id = existentes.get(&chave).map(|(id, _)| *id);
                let gravado = gravar_servico(
                    &mut *tx,
                    &Servico {
                        id,
                        empresa_id,
                        ..s.clone()
                    },
                )
                .await?;
                match id {
                    Some(_) => resumo.servicos_atualizados += 1,
                    None => resumo.servicos_novos += 1,
                }
                existentes.insert(chave, (gravado, s.valor_sugerido.map(|v| v.centesimos())));
            }

            let mut pessoas: HashMap<String, i64> = HashMap::new();
            for t in &e.tomadores {
                let pessoa_id = gravar_tomador(&mut tx, empresa_id, &t.tomador, "importacao").await?;
                sqlx::query(
                    "UPDATE empresa_tomador SET emails = ?, observacao = ?, retem_federais = ?, retem_iss = ?, tipo = ?
                     WHERE empresa_id = ? AND pessoa_id = ?",
                )
                .bind(vazio(&t.emails))
                .bind(vazio(&t.observacao))
                .bind(t.retem_federais)
                .bind(t.retem_iss)
                .bind(t.tipo.como_texto())
                .bind(empresa_id)
                .bind(pessoa_id)
                .execute(&mut *tx)
                .await?;
                pessoas.insert(tipo_e_numero(&t.tomador.documento).1.to_owned(), pessoa_id);
                resumo.tomadores += 1;
            }

            for r in &e.recorrencias {
                let (servico_id, sugerido) = *existentes.get(&chave_nome(&r.servico)).ok_or_else(|| {
                    ErroBanco::Regra(format!(
                        "{}: recorrência do serviço \"{}\", que não veio no arquivo",
                        e.nome, r.servico
                    ))
                })?;
                let pessoa_id = match &r.documento_tomador {
                    None => None,
                    Some(doc) => Some(*pessoas.get(doc).ok_or_else(|| {
                        ErroBanco::Regra(format!(
                            "{}: recorrência do tomador {doc}, que não veio no arquivo",
                            e.nome
                        ))
                    })?),
                };
                if r.valor.or(sugerido).is_none() {
                    return Err(ErroBanco::Regra(format!(
                        "{}: recorrência de \"{}\" sem valor (o serviço não tem valor sugerido)",
                        e.nome, r.servico
                    )));
                }
                let descricao = vazio(&r.descricao);
                let existente: Option<i64> = sqlx::query(
                    "SELECT id FROM recorrencia WHERE empresa_id = ? AND ativa = 1 AND pessoa_id IS ?
                         AND servico_id = ? AND descricao IS ? ORDER BY id LIMIT 1",
                )
                .bind(empresa_id)
                .bind(pessoa_id)
                .bind(servico_id)
                .bind(&descricao)
                .fetch_optional(&mut *tx)
                .await?
                .map(|l| l.get(0));
                let obra = r.obra.as_ref().map(json);
                let complementares = vazio(&r.informacoes_complementares);
                match existente {
                    Some(id) => {
                        sqlx::query(
                            "UPDATE recorrencia SET valor = ?, informacoes_complementares = ?, obra_json = ? WHERE id = ?",
                        )
                        .bind(r.valor)
                        .bind(&complementares)
                        .bind(&obra)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                        resumo.recorrencias_atualizadas += 1;
                    }
                    None => {
                        sqlx::query(
                            "INSERT INTO recorrencia (empresa_id, pessoa_id, servico_id, valor, descricao,
                                 informacoes_complementares, obra_json, criado_em) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(empresa_id)
                        .bind(pessoa_id)
                        .bind(servico_id)
                        .bind(r.valor)
                        .bind(&descricao)
                        .bind(&complementares)
                        .bind(&obra)
                        .bind(agora())
                        .execute(&mut *tx)
                        .await?;
                        resumo.recorrencias_novas += 1;
                    }
                }
            }
            resumos.push(resumo);
        }
        tx.commit().await?;
        Ok(resumos)
    }
}

fn nova_empresa(e: &EmpresaExportada) -> Resultado<NovaEmpresa> {
    let nova = NovaEmpresa {
        cnpj: Cnpj::parse(&e.cnpj).map_err(|err| ErroBanco::Regra(format!("{}: CNPJ {}: {err}", e.nome, e.cnpj)))?,
        nome: e.nome.clone(),
        inscricao_municipal: vazio(&e.inscricao_municipal),
        municipio_emissor: e.municipio_emissor.clone(),
        regime: e.regime.clone(),
        ambiente: e.ambiente,
        serie: e.serie.clone(),
        telefone: vazio(&e.telefone),
        email: vazio(&e.email),
    };
    validar(&nova).map_err(|err| ErroBanco::Regra(format!("{}: {err}", e.nome)))?;
    Ok(nova)
}

fn servicos_de(e: &EmpresaExportada) -> Resultado<Vec<Servico>> {
    e.servicos
        .iter()
        .map(|s| {
            let servico = Servico {
                id: None,
                empresa_id: 0,
                nome: s.nome.clone(),
                descricao: s.descricao.clone(),
                codigo_trib_nac: s.codigo_trib_nac.clone(),
                codigo_trib_mun: s.codigo_trib_mun.clone(),
                nbs: s.nbs.clone(),
                tributacao: s.tributacao.clone(),
                valor_sugerido: s.valor_sugerido.map(Valor::de_centesimos),
            };
            servico
                .validar()
                .map_err(|err| ErroBanco::Regra(format!("{}: serviço \"{}\": {err}", e.nome, s.nome.trim())))?;
            Ok(servico)
        })
        .collect()
}

/// Cadastra a empresa nova (marcada para conferir a numeração) ou atualiza a existente sem mexer
/// em ambiente, série e numeração. Devolve (id, criada).
async fn gravar_empresa(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    e: &EmpresaExportada,
    nova: &NovaEmpresa,
) -> Resultado<(i64, bool)> {
    let pessoa_id: i64 = sqlx::query(
        "INSERT INTO pessoa (tipo, documento, nome, telefone, email, fonte, atualizado_em)
         VALUES ('cnpj', ?, ?, ?, ?, 'importacao', ?)
         ON CONFLICT (documento) DO UPDATE SET nome = excluded.nome,
             telefone = COALESCE(excluded.telefone, telefone), email = COALESCE(excluded.email, email),
             atualizado_em = excluded.atualizado_em
         RETURNING id",
    )
    .bind(nova.cnpj.as_str())
    .bind(nova.nome.trim())
    .bind(&nova.telefone)
    .bind(&nova.email)
    .bind(agora())
    .fetch_one(&mut **tx)
    .await?
    .get(0);
    let existente: Option<i64> = sqlx::query("SELECT id FROM empresa WHERE pessoa_id = ?")
        .bind(pessoa_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(|r| r.get(0));
    if let Some(id) = existente {
        sqlx::query(
            "UPDATE empresa SET inscricao_municipal = ?, im_na_dps_producao = ?, im_na_dps_homologacao = ?,
                 municipio_emissor = ?, regime_json = ?, ativa = 1 WHERE id = ?",
        )
        .bind(&nova.inscricao_municipal)
        .bind(e.im_na_dps_producao)
        .bind(e.im_na_dps_homologacao)
        .bind(&nova.municipio_emissor)
        .bind(json(&nova.regime))
        .bind(id)
        .execute(&mut **tx)
        .await?;
        auditar_em(tx, "empresa_importada", id, "dados cadastrais atualizados").await?;
        return Ok((id, false));
    }
    let id: i64 = sqlx::query(
        "INSERT INTO empresa (pessoa_id, inscricao_municipal, im_na_dps_producao, im_na_dps_homologacao,
             municipio_emissor, regime_json, ambiente, serie, conferir_numeracao, criado_em)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?) RETURNING id",
    )
    .bind(pessoa_id)
    .bind(&nova.inscricao_municipal)
    .bind(e.im_na_dps_producao)
    .bind(e.im_na_dps_homologacao)
    .bind(&nova.municipio_emissor)
    .bind(json(&nova.regime))
    .bind(ambiente_texto(nova.ambiente))
    .bind(&nova.serie)
    .bind(agora())
    .fetch_one(&mut **tx)
    .await?
    .get(0);
    auditar_em(tx, "empresa_importada", id, "cadastrada; numeração a conferir na Sefin").await?;
    Ok((id, true))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::{banco_com_empresa, nova_empresa};
    use crate::recorrencias::Recorrencia;
    use nfse_core::modelo::{Documento, EnderecoNacional};

    fn convenio() -> Tomador {
        Tomador {
            documento: Documento::Cnpj(Cnpj::parse("12ABC34501DE35").unwrap()),
            inscricao_municipal: None,
            nome: "CONVENIO SAUDE EXEMPLO".into(),
            endereco: Some(EnderecoNacional {
                municipio: "3511102".into(),
                cep: "15800000".into(),
                logradouro: "RUA A".into(),
                numero: "1".into(),
                complemento: None,
                bairro: "CENTRO".into(),
            }),
            telefone: None,
            email: Some("faturamento@convenio.exemplo".into()),
        }
    }

    /// Banco de origem com empresa, serviço, tomador (e-mails e retenção) e recorrência.
    async fn origem() -> (Banco, i64) {
        let (banco, _, empresa) = banco_com_empresa().await;
        let servico_id = crate::recorrencias::testes::servico(&banco, empresa.id, Some("850")).await;
        let pessoa = banco
            .salvar_tomador_vinculado(
                empresa.id,
                &convenio(),
                "manual",
                Some("a@convenio.exemplo"),
                crate::tomadores::RetencaoDoVinculo {
                    federais: true,
                    tipo: TipoTomador::Federal,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        banco
            .salvar_recorrencia(&Recorrencia {
                id: None,
                empresa_id: empresa.id,
                tomador: Some(pessoa),
                servico_id,
                valor: Some(Valor::parse("1200").unwrap()),
                descricao: Some("PLANO {mes_ref}".into()),
                informacoes_complementares: None,
                obra: None,
            })
            .await
            .unwrap();
        (banco, empresa.id)
    }

    /// Cifra com Argon2 leve, para os testes não demorarem.
    fn cifrar_rapido(pacote: &PacoteEmpresas, senha: &str) -> Vec<u8> {
        let (cofre, cabecalho) = Cofre::criar_com(senha, 1024, 1, 1).unwrap();
        cifrar_com(pacote, &cofre, &cabecalho).unwrap()
    }

    #[tokio::test]
    async fn exporta_e_importa_em_banco_novo() {
        let (banco, id) = origem().await;
        let pacote = banco.exportar_empresas(&[id]).await.unwrap();
        let arquivo = cifrar_rapido(&pacote, "frase da exportacao");
        let lido = decifrar_pacote(&arquivo, "frase da exportacao").unwrap();
        assert_eq!(lido, pacote);

        let destino = Banco::em_memoria().await.unwrap();
        let previa = destino.previa_importacao(&lido).await.unwrap();
        assert_eq!(previa[0].situacao, SituacaoImportacao::Nova);
        assert_eq!(
            (previa[0].servicos, previa[0].tomadores, previa[0].recorrencias),
            (1, 1, 1)
        );

        let cnpj = vec![pacote.empresas[0].cnpj.clone()];
        let resumo = destino.importar_empresas(&lido, &cnpj).await.unwrap();
        assert!(resumo[0].nova);
        assert_eq!(
            (
                resumo[0].servicos_novos,
                resumo[0].tomadores,
                resumo[0].recorrencias_novas
            ),
            (1, 1, 1)
        );

        let empresa = &destino.empresas().await.unwrap()[0];
        assert_eq!(empresa.cnpj, nova_empresa().cnpj);
        assert!(empresa.conferir_numeracao);
        let t = &destino.buscar_tomadores(empresa.id, "convenio").await.unwrap()[0];
        assert_eq!(t.emails.as_deref(), Some("a@convenio.exemplo"));
        assert!(t.retem_federais && !t.retem_iss);
        assert_eq!(t.tipo, TipoTomador::Federal);
        let recorrencias = destino.recorrencias(Some(empresa.id), "2026-10").await.unwrap();
        assert_eq!(recorrencias.len(), 1);

        // A numeração só libera depois da consulta à Sefin.
        let erro = destino.reservar_dps(empresa.id).await.unwrap_err();
        assert!(erro.to_string().contains("Consultar na Sefin"), "{erro}");
        destino.marcar_numeracao_conferida(empresa.id).await.unwrap();
        assert_eq!(destino.reservar_dps(empresa.id).await.unwrap().numero, 1);

        // Importar de novo atualiza em vez de duplicar.
        let resumo = destino.importar_empresas(&lido, &cnpj).await.unwrap();
        assert!(!resumo[0].nova);
        assert_eq!(
            (resumo[0].servicos_atualizados, resumo[0].recorrencias_atualizadas),
            (1, 1)
        );
        assert_eq!(destino.servicos(empresa.id).await.unwrap().len(), 1);
        assert_eq!(
            destino.recorrencias(Some(empresa.id), "2026-10").await.unwrap().len(),
            1
        );
        assert!(!destino.empresa(empresa.id).await.unwrap().conferir_numeracao);
    }

    #[tokio::test]
    async fn empresa_existente_mantem_ambiente_serie_e_numeracao() {
        let (banco, id) = origem().await;
        let mut pacote = banco.exportar_empresas(&[id]).await.unwrap();
        pacote.empresas[0].nome = "NOME NOVO LTDA".into();
        pacote.empresas[0].serie = "9".into();
        pacote.empresas[0].ambiente = Ambiente::Producao;

        let (destino, _, empresa) = banco_com_empresa().await;
        destino.reservar_dps(empresa.id).await.unwrap();
        assert_eq!(
            destino.previa_importacao(&pacote).await.unwrap()[0].situacao,
            SituacaoImportacao::Existente
        );
        destino
            .importar_empresas(&pacote, &[pacote.empresas[0].cnpj.clone()])
            .await
            .unwrap();

        let depois = destino.empresa(empresa.id).await.unwrap();
        assert_eq!(depois.nome, "NOME NOVO LTDA");
        assert_eq!((depois.ambiente, depois.serie.as_str()), (Ambiente::Homologacao, "1"));
        assert!(!depois.conferir_numeracao);
        assert_eq!(
            destino
                .proximo_numero(empresa.id, Ambiente::Homologacao, "1")
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn importacao_invalida_nao_grava_nada() {
        let (banco, id) = origem().await;
        let mut pacote = banco.exportar_empresas(&[id]).await.unwrap();
        pacote.empresas[0].recorrencias[0].servico = "Serviço que não veio".into();
        let destino = Banco::em_memoria().await.unwrap();
        let erro = destino
            .importar_empresas(&pacote, &[pacote.empresas[0].cnpj.clone()])
            .await
            .unwrap_err();
        assert!(erro.to_string().contains("não veio no arquivo"), "{erro}");
        assert!(destino.empresas().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn recusa_senha_errada_e_arquivo_alterado() {
        let (banco, id) = origem().await;
        let pacote = banco.exportar_empresas(&[id]).await.unwrap();
        let mut arquivo = cifrar_rapido(&pacote, "frase da exportacao");
        assert!(
            decifrar_pacote(&arquivo, "outra frase qualquer")
                .unwrap_err()
                .to_string()
                .contains("senha incorreta")
        );
        let ultimo = arquivo.len() - 1;
        arquivo[ultimo] ^= 1;
        assert!(
            decifrar_pacote(&arquivo, "frase da exportacao")
                .unwrap_err()
                .to_string()
                .contains("corrompido")
        );
        assert!(decifrar_pacote(b"PK\x03\x04qualquer", "frase da exportacao").is_err());
        assert!(cifrar_pacote(&pacote, "curta").is_err());
    }
}
