//! Catálogo de serviços por empresa, com a tributação padrão de cada um.

use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use nfse_core::modelo::{IbsCbs, Issqn, TotalTributos, TributosFederais};
use nfse_core::{Percentual, Valor};

use crate::{Banco, ErroBanco, Resultado, de_json, json};

/// Tributação padrão do serviço; vira `valores` (e `IBSCBS`) da DPS na emissão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Tributacao {
    #[serde(default)]
    pub issqn: Issqn,
    #[serde(default)]
    pub federais: TributosFederais,
    #[serde(default)]
    pub total_tributos: TotalTributos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ibs_cbs: Option<IbsCbs>,
    /// Percentuais aplicados sobre o valor de cada nota (as retenções variam com o valor).
    #[serde(default)]
    pub retencoes: RetencoesPercentuais,
}

/// Alíquotas de retenção do serviço. Na emissão viram valores (`vRetIRRF`, `vRetCSLL`, `vRetCP`,
/// `vPis`/`vCofins` com base no valor do serviço), arredondados ao centavo.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RetencoesPercentuais {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub irrf: Option<Percentual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csll: Option<Percentual>,
    /// Contribuição previdenciária (INSS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<Percentual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pis: Option<Percentual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cofins: Option<Percentual>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Servico {
    /// `None` antes de gravar.
    pub id: Option<i64>,
    pub empresa_id: i64,
    /// Nome curto para a tela (ex.: "Honorários mensais").
    pub nome: String,
    /// Descrição padrão da nota; aceita `{mes_ref}` (ex.: "Honorários contábeis {mes_ref}").
    pub descricao: String,
    pub codigo_trib_nac: String,
    pub codigo_trib_mun: Option<String>,
    pub nbs: Option<String>,
    pub tributacao: Tributacao,
    pub valor_sugerido: Option<Valor>,
}

const SELECT_SERVICO: &str = "SELECT id, empresa_id, nome, descricao, codigo_trib_nac, codigo_trib_mun, nbs,
    tributacao_json, valor_sugerido FROM servico";

fn servico_de(r: &SqliteRow) -> Resultado<Servico> {
    Ok(Servico {
        id: Some(r.get(0)),
        empresa_id: r.get(1),
        nome: r.get(2),
        descricao: r.get(3),
        codigo_trib_nac: r.get(4),
        codigo_trib_mun: r.get(5),
        nbs: r.get(6),
        tributacao: de_json(r.get(7), "tributacao")?,
        valor_sugerido: r.get::<Option<i64>, _>(8).map(Valor::de_centesimos),
    })
}

impl Servico {
    /// Regras de cadastro (as da emissão ficam na validação da DPS).
    pub fn validar(&self) -> Resultado<()> {
        let digitos = |v: &str, n: usize| v.len() == n && v.bytes().all(|b| b.is_ascii_digit());
        if !digitos(&self.codigo_trib_nac, 6) {
            return Err(ErroBanco::Regra(
                "código de tributação nacional deve ter 6 dígitos (ex.: 171901)".into(),
            ));
        }
        if self.nbs.as_deref().is_some_and(|n| !digitos(n, 9)) {
            return Err(ErroBanco::Regra("NBS deve ter 9 dígitos".into()));
        }
        if self.nome.trim().is_empty() || self.descricao.trim().is_empty() {
            return Err(ErroBanco::Regra("nome e descrição do serviço são obrigatórios".into()));
        }
        Ok(())
    }
}

/// Quantos serviços a importação criou e quantos atualizou (mesmo nome já cadastrado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ResumoImportacaoServicos {
    pub novos: usize,
    pub atualizados: usize,
}

/// Chave de comparação de nomes: sem diferença de maiúsculas e espaços nas pontas.
fn chave_nome(nome: &str) -> String {
    nome.trim().to_lowercase()
}

impl Banco {
    /// Insere (sem `id`) ou atualiza o serviço. Devolve o id.
    pub async fn salvar_servico(&self, s: &Servico) -> Resultado<i64> {
        s.validar()?;
        let consulta = match s.id {
            None => sqlx::query(
                "INSERT INTO servico (empresa_id, nome, descricao, codigo_trib_nac, codigo_trib_mun, nbs,
                     tributacao_json, valor_sugerido) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            ),
            Some(_) => sqlx::query(
                "UPDATE servico SET empresa_id = ?, nome = ?, descricao = ?, codigo_trib_nac = ?, codigo_trib_mun = ?,
                     nbs = ?, tributacao_json = ?, valor_sugerido = ? WHERE id = ? RETURNING id",
            ),
        };
        let mut consulta = consulta
            .bind(s.empresa_id)
            .bind(s.nome.trim())
            .bind(&s.descricao)
            .bind(&s.codigo_trib_nac)
            .bind(&s.codigo_trib_mun)
            .bind(&s.nbs)
            .bind(json(&s.tributacao))
            .bind(s.valor_sugerido.map(|v| v.centesimos()));
        if let Some(id) = s.id {
            consulta = consulta.bind(id);
        }
        let id = consulta
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("serviço"))?
            .get(0);
        Ok(id)
    }

    /// Grava os serviços na empresa (o `id` e o `empresa_id` de cada um são ignorados). Um serviço
    /// ativo com o mesmo nome é atualizado; os demais são criados. Nada é gravado se algum for
    /// inválido.
    pub async fn importar_servicos(
        &self,
        empresa_id: i64,
        servicos: &[Servico],
    ) -> Resultado<ResumoImportacaoServicos> {
        for s in servicos {
            s.validar()
                .map_err(|e| ErroBanco::Regra(format!("serviço \"{}\": {e}", s.nome.trim())))?;
        }
        let mut existentes: std::collections::HashMap<String, i64> = self
            .servicos(empresa_id)
            .await?
            .into_iter()
            .filter_map(|s| Some((chave_nome(&s.nome), s.id?)))
            .collect();
        let mut resumo = ResumoImportacaoServicos::default();
        for s in servicos {
            let chave = chave_nome(&s.nome);
            let id = existentes.get(&chave).copied();
            let novo = Servico {
                id,
                empresa_id,
                ..s.clone()
            };
            let gravado = self.salvar_servico(&novo).await?;
            match id {
                Some(_) => resumo.atualizados += 1,
                None => {
                    resumo.novos += 1;
                    existentes.insert(chave, gravado);
                }
            }
        }
        Ok(resumo)
    }

    pub async fn servicos(&self, empresa_id: i64) -> Resultado<Vec<Servico>> {
        sqlx::query(&format!(
            "{SELECT_SERVICO} WHERE empresa_id = ? AND ativo = 1 ORDER BY nome"
        ))
        .bind(empresa_id)
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(servico_de)
        .collect()
    }

    pub async fn servico(&self, id: i64) -> Resultado<Servico> {
        let linha = sqlx::query(&format!("{SELECT_SERVICO} WHERE id = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("serviço"))?;
        servico_de(&linha)
    }

    pub async fn desativar_servico(&self, id: i64) -> Resultado<()> {
        sqlx::query("UPDATE servico SET ativo = 0 WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::banco_com_empresa;
    use nfse_core::Percentual;
    use nfse_core::modelo::PisCofins;

    fn honorarios(empresa_id: i64) -> Servico {
        Servico {
            id: None,
            empresa_id,
            nome: "Honorários mensais".into(),
            descricao: "SERVIÇOS CONTÁBEIS REFERENTE A {mes_ref}".into(),
            codigo_trib_nac: "171901".into(),
            codigo_trib_mun: None,
            nbs: Some("113022100".into()),
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
                total_tributos: TotalTributos::PercentualSimples(Percentual::parse("11.48").unwrap()),
                ..Default::default()
            },
            valor_sugerido: Some(Valor::parse("850").unwrap()),
        }
    }

    #[tokio::test]
    async fn salva_atualiza_e_desativa() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let mut s = honorarios(empresa.id);
        let id = banco.salvar_servico(&s).await.unwrap();
        s.id = Some(id);
        assert_eq!(banco.servico(id).await.unwrap(), s);

        s.valor_sugerido = Some(Valor::parse("900").unwrap());
        banco.salvar_servico(&s).await.unwrap();
        assert_eq!(banco.servicos(empresa.id).await.unwrap(), vec![s.clone()]);

        banco.desativar_servico(id).await.unwrap();
        assert!(banco.servicos(empresa.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn importa_atualizando_pelo_nome() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let existente = banco.salvar_servico(&honorarios(empresa.id)).await.unwrap();

        // De outra empresa (id e empresa_id são ignorados), com o nome em maiúsculas.
        let mut mesmo = honorarios(999);
        mesmo.id = Some(12345);
        mesmo.nome = "  HONORÁRIOS MENSAIS ".into();
        mesmo.valor_sugerido = Some(Valor::parse("990").unwrap());
        let mut outro = honorarios(999);
        outro.nome = "Consulta".into();
        let resumo = banco.importar_servicos(empresa.id, &[mesmo, outro]).await.unwrap();
        assert_eq!(
            resumo,
            ResumoImportacaoServicos {
                novos: 1,
                atualizados: 1
            }
        );

        let lista = banco.servicos(empresa.id).await.unwrap();
        assert_eq!(lista.len(), 2);
        let atualizado = lista.iter().find(|s| s.id == Some(existente)).unwrap();
        assert_eq!(atualizado.valor_sugerido, Some(Valor::parse("990").unwrap()));
        assert!(lista.iter().all(|s| s.empresa_id == empresa.id));

        // Um inválido impede a importação inteira.
        let mut novo = honorarios(empresa.id);
        novo.nome = "Exame".into();
        let mut ruim = honorarios(empresa.id);
        ruim.codigo_trib_nac = "123".into();
        assert!(banco.importar_servicos(empresa.id, &[novo, ruim]).await.is_err());
        assert_eq!(banco.servicos(empresa.id).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn valida_codigos() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let mut s = honorarios(empresa.id);
        s.codigo_trib_nac = "17.19.01".into();
        assert!(matches!(banco.salvar_servico(&s).await, Err(ErroBanco::Regra(_))));
    }
}
