//! Notas recorrentes: um modelo (empresa, tomador, serviço, valor, descrição com `{mes_ref}`)
//! emitido uma vez por mês de referência, sozinho ou no lote do mês.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use nfse_core::Valor;
use nfse_core::modelo::Obra;

use crate::dps::SituacaoDps;
use crate::{Banco, ErroBanco, Resultado, agora};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorrencia {
    /// `None` antes de gravar.
    pub id: Option<i64>,
    pub empresa_id: i64,
    /// `pessoa_id` do tomador vinculado à empresa.
    pub tomador: Option<i64>,
    pub servico_id: i64,
    /// `None` = valor sugerido do serviço.
    pub valor: Option<Valor>,
    /// `None` = descrição padrão do serviço.
    pub descricao: Option<String>,
    pub informacoes_complementares: Option<String>,
    /// Dados da obra, para serviço de construção civil (regra E0370).
    pub obra: Option<Obra>,
}

/// Linha das telas de recorrentes e do lote: o modelo com nomes e valores já resolvidos e a
/// situação no mês consultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRecorrente {
    pub recorrencia: Recorrencia,
    pub empresa: String,
    pub tomador: Option<String>,
    pub servico: String,
    /// `false` se o serviço foi removido do cadastro (não dá para emitir).
    pub servico_ativo: bool,
    /// Valor que será usado (o da recorrência ou o sugerido do serviço).
    pub valor_efetivo: Option<Valor>,
    /// Descrição que será usada, ainda com `{mes_ref}`.
    pub descricao_efetiva: String,
    /// Última tentativa no mês consultado.
    pub no_mes: Option<TentativaNoMes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TentativaNoMes {
    pub dps_id: i64,
    pub situacao: SituacaoDps,
    pub numero_nfse: Option<String>,
    pub nfse_id: Option<i64>,
    pub erro: Option<String>,
}

impl TentativaNoMes {
    /// Rejeitada ou cancelada: o mês pode ser emitido de novo.
    pub fn libera_o_mes(&self) -> bool {
        matches!(self.situacao, SituacaoDps::Rejeitada | SituacaoDps::Cancelada)
    }
}

fn mes_valido(mes: &str) -> Resultado<()> {
    let ok = mes.len() == 7
        && mes.as_bytes()[4] == b'-'
        && mes[..4].bytes().chain(mes[5..].bytes()).all(|b| b.is_ascii_digit())
        && (1..=12).contains(&mes[5..].parse::<u32>().unwrap_or(0));
    if ok {
        Ok(())
    } else {
        Err(ErroBanco::Regra(format!(
            "mês de referência inválido: {mes:?} (use AAAA-MM)"
        )))
    }
}

const SELECT_ITEM: &str = "SELECT r.id, r.empresa_id, r.pessoa_id, r.servico_id, r.valor, r.descricao,
        r.informacoes_complementares, pe.nome, pt.nome, s.nome, s.ativo, s.valor_sugerido, s.descricao,
        r.obra_json
    FROM recorrencia r
    JOIN empresa e ON e.id = r.empresa_id
    JOIN pessoa pe ON pe.id = e.pessoa_id
    LEFT JOIN pessoa pt ON pt.id = r.pessoa_id
    JOIN servico s ON s.id = r.servico_id";

fn item_de(r: &SqliteRow) -> ItemRecorrente {
    let valor: Option<Valor> = r.get::<Option<i64>, _>(4).map(Valor::de_centesimos);
    let descricao: Option<String> = r.get(5);
    ItemRecorrente {
        valor_efetivo: valor.or_else(|| r.get::<Option<i64>, _>(11).map(Valor::de_centesimos)),
        descricao_efetiva: descricao.clone().unwrap_or_else(|| r.get(12)),
        recorrencia: Recorrencia {
            id: Some(r.get(0)),
            empresa_id: r.get(1),
            tomador: r.get(2),
            servico_id: r.get(3),
            valor,
            descricao,
            informacoes_complementares: r.get(6),
            // Gravado por `salvar_recorrencia`; ilegível vira "sem obra" e a validação da DPS aponta.
            obra: r
                .get::<Option<String>, _>(13)
                .and_then(|j| serde_json::from_str(&j).ok()),
        },
        empresa: r.get(7),
        tomador: r.get(8),
        servico: r.get(9),
        servico_ativo: r.get(10),
        no_mes: None,
    }
}

impl Banco {
    /// Insere (sem `id`) ou atualiza a recorrência. Devolve o id.
    pub async fn salvar_recorrencia(&self, r: &Recorrencia) -> Resultado<i64> {
        let servico = self.servico(r.servico_id).await?;
        if servico.empresa_id != r.empresa_id {
            return Err(ErroBanco::Regra("o serviço escolhido é de outra empresa".into()));
        }
        if let Some(pessoa) = r.tomador {
            self.tomador(r.empresa_id, pessoa).await?;
        }
        if r.valor.or(servico.valor_sugerido).is_none() {
            return Err(ErroBanco::Regra(
                "informe o valor (o serviço não tem valor sugerido)".into(),
            ));
        }
        let vazio = |t: &Option<String>| t.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned);
        let consulta = match r.id {
            None => sqlx::query(
                "INSERT INTO recorrencia (empresa_id, pessoa_id, servico_id, valor, descricao, informacoes_complementares,
                     obra_json, criado_em) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            ),
            Some(_) => sqlx::query(
                "UPDATE recorrencia SET empresa_id = ?, pessoa_id = ?, servico_id = ?, valor = ?, descricao = ?,
                     informacoes_complementares = ?, obra_json = ? WHERE id = ? AND ativa = 1 RETURNING id",
            ),
        };
        let consulta = consulta
            .bind(r.empresa_id)
            .bind(r.tomador)
            .bind(r.servico_id)
            .bind(r.valor.map(|v| v.centesimos()))
            .bind(vazio(&r.descricao))
            .bind(vazio(&r.informacoes_complementares))
            .bind(r.obra.as_ref().map(crate::json));
        let consulta = match r.id {
            None => consulta.bind(agora()),
            Some(id) => consulta.bind(id),
        };
        Ok(consulta
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("recorrência"))?
            .get(0))
    }

    pub async fn recorrencia(&self, id: i64) -> Resultado<ItemRecorrente> {
        let linha = sqlx::query(&format!("{SELECT_ITEM} WHERE r.id = ? AND r.ativa = 1"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("recorrência"))?;
        Ok(item_de(&linha))
    }

    /// Recorrências ativas (de uma empresa ou de todas as empresas ativas) com a situação no mês.
    pub async fn recorrencias(&self, empresa_id: Option<i64>, mes: &str) -> Resultado<Vec<ItemRecorrente>> {
        mes_valido(mes)?;
        let linhas = sqlx::query(&format!(
            "{SELECT_ITEM} WHERE r.ativa = 1 AND e.ativa = 1 AND (? IS NULL OR r.empresa_id = ?)
             ORDER BY pe.nome, COALESCE(pt.nome, ''), r.id"
        ))
        .bind(empresa_id)
        .bind(empresa_id)
        .fetch_all(self.pool())
        .await?;
        let mut itens: Vec<ItemRecorrente> = linhas.iter().map(item_de).collect();
        for item in &mut itens {
            item.no_mes = self
                .tentativa_no_mes(item.recorrencia.id.expect("gravada"), mes)
                .await?;
        }
        Ok(itens)
    }

    pub async fn desativar_recorrencia(&self, id: i64) -> Resultado<()> {
        sqlx::query("UPDATE recorrencia SET ativa = 0 WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Última tentativa de emitir a recorrência no mês (`AAAA-MM`).
    pub async fn tentativa_no_mes(&self, recorrencia_id: i64, mes: &str) -> Resultado<Option<TentativaNoMes>> {
        let linha = sqlx::query(
            "SELECT d.id, d.situacao, n.numero, n.id, d.ultimo_erro
             FROM emissao_recorrente er JOIN dps d ON d.id = er.dps_id LEFT JOIN nfse n ON n.dps_id = d.id
             WHERE er.recorrencia_id = ? AND er.mes_referencia = ? ORDER BY er.id DESC LIMIT 1",
        )
        .bind(recorrencia_id)
        .bind(mes)
        .fetch_optional(self.pool())
        .await?;
        linha
            .map(|r| {
                Ok(TentativaNoMes {
                    dps_id: r.get(0),
                    situacao: SituacaoDps::de(r.get(1))?,
                    numero_nfse: r.get(2),
                    nfse_id: r.get(3),
                    erro: r.get(4),
                })
            })
            .transpose()
    }

    /// Liga a DPS recém-reservada à recorrência e ao mês. Recusa se o mês já tem nota emitida
    /// ou em andamento (a verificação e a gravação são atômicas).
    pub async fn registrar_emissao_recorrente(&self, recorrencia_id: i64, mes: &str, dps_id: i64) -> Resultado<()> {
        mes_valido(mes)?;
        let mut tx = self.pool().begin().await?;
        let ocupado: Option<String> = sqlx::query(
            "SELECT d.situacao FROM emissao_recorrente er JOIN dps d ON d.id = er.dps_id
             WHERE er.recorrencia_id = ? AND er.mes_referencia = ? AND er.dps_id <> ?
               AND d.situacao NOT IN ('rejeitada', 'cancelada') LIMIT 1",
        )
        .bind(recorrencia_id)
        .bind(mes)
        .bind(dps_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.get(0));
        if let Some(situacao) = ocupado {
            return Err(ErroBanco::Regra(format!(
                "esta recorrência já tem nota {situacao} em {mes}"
            )));
        }
        sqlx::query("INSERT INTO emissao_recorrente (recorrencia_id, mes_referencia, dps_id) VALUES (?, ?, ?)")
            .bind(recorrencia_id)
            .bind(mes)
            .bind(dps_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;
    use crate::empresas::testes::banco_com_empresa;
    use crate::servicos::{Servico, Tributacao};

    pub async fn servico(banco: &Banco, empresa_id: i64, valor: Option<&str>) -> i64 {
        banco
            .salvar_servico(&Servico {
                id: None,
                empresa_id,
                nome: "Honorários".into(),
                descricao: "HONORÁRIOS {mes_ref}".into(),
                codigo_trib_nac: "171901".into(),
                codigo_trib_mun: None,
                nbs: None,
                tributacao: Tributacao::default(),
                valor_sugerido: valor.map(|v| Valor::parse(v).unwrap()),
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn salva_lista_e_desativa() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let servico_id = servico(&banco, empresa.id, Some("850")).await;
        let mut r = Recorrencia {
            id: None,
            empresa_id: empresa.id,
            tomador: None,
            servico_id,
            valor: None,
            descricao: Some("   ".into()),
            informacoes_complementares: None,
            obra: None,
        };
        let id = banco.salvar_recorrencia(&r).await.unwrap();
        let item = banco.recorrencia(id).await.unwrap();
        assert_eq!(item.valor_efetivo, Some(Valor::parse("850").unwrap()));
        assert_eq!(
            item.descricao_efetiva, "HONORÁRIOS {mes_ref}",
            "descrição em branco usa a do serviço"
        );
        assert_eq!(item.recorrencia.descricao, None);

        r.id = Some(id);
        r.valor = Some(Valor::parse("900").unwrap());
        banco.salvar_recorrencia(&r).await.unwrap();
        let lista = banco.recorrencias(None, "2026-09").await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].valor_efetivo, Some(Valor::parse("900").unwrap()));
        assert_eq!(lista[0].no_mes, None);

        banco.desativar_recorrencia(id).await.unwrap();
        assert!(
            banco
                .recorrencias(Some(empresa.id), "2026-09")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(banco.recorrencias(None, "2026-13").await.is_err());
    }

    #[tokio::test]
    async fn exige_valor_e_servico_da_empresa() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let sem_valor = servico(&banco, empresa.id, None).await;
        let r = Recorrencia {
            id: None,
            empresa_id: empresa.id,
            tomador: None,
            servico_id: sem_valor,
            valor: None,
            descricao: None,
            informacoes_complementares: None,
            obra: None,
        };
        assert!(matches!(banco.salvar_recorrencia(&r).await, Err(ErroBanco::Regra(_))));
        assert!(matches!(
            banco
                .salvar_recorrencia(&Recorrencia {
                    empresa_id: empresa.id + 1,
                    valor: Some(Valor::parse("1").unwrap()),
                    ..r
                })
                .await,
            Err(ErroBanco::Regra(_))
        ));
    }
}
