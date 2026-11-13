//! Controle das notas emitidas a partir de planilha: cada linha é identificada por uma chave de
//! conteúdo, e uma chave com nota autorizada ou em andamento não é emitida de novo.

use sqlx::Row;

use crate::dps::SituacaoDps;
use crate::recorrencias::TentativaNoMes;
use crate::{Banco, ErroBanco, Resultado, agora};

impl Banco {
    /// Última tentativa de emitir a linha com esta chave (em qualquer importação).
    pub async fn tentativa_importada(&self, chave: &str) -> Resultado<Option<TentativaNoMes>> {
        let linha = sqlx::query(
            "SELECT d.id, d.situacao, n.numero, n.id, d.ultimo_erro
             FROM emissao_importada ei JOIN dps d ON d.id = ei.dps_id LEFT JOIN nfse n ON n.dps_id = d.id
             WHERE ei.chave = ? ORDER BY ei.id DESC LIMIT 1",
        )
        .bind(chave)
        .fetch_optional(self.pool())
        .await?;
        linha
            .map(|r| {
                Ok(TentativaNoMes {
                    dps_id: r.get(0),
                    situacao: r.get::<&str, _>(1).parse::<SituacaoDps>()?,
                    numero_nfse: r.get(2),
                    nfse_id: r.get(3),
                    erro: r.get(4),
                })
            })
            .transpose()
    }

    /// Liga a DPS recém-reservada à linha da planilha. Recusa se a chave já tem nota emitida ou
    /// em andamento (conferência e gravação atômicas).
    pub async fn registrar_emissao_importada(
        &self,
        chave: &str,
        arquivo: Option<&str>,
        linha: Option<i64>,
        dps_id: i64,
    ) -> Resultado<()> {
        let mut tx = self.transacao().await?;
        let ocupado: Option<String> = sqlx::query(
            "SELECT d.situacao FROM emissao_importada ei JOIN dps d ON d.id = ei.dps_id
             WHERE ei.chave = ? AND ei.dps_id <> ? AND d.situacao NOT IN ('rejeitada', 'cancelada') LIMIT 1",
        )
        .bind(chave)
        .bind(dps_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.get(0));
        if let Some(situacao) = ocupado {
            return Err(ErroBanco::Regra(format!(
                "esta linha da planilha já tem nota {situacao}"
            )));
        }
        sqlx::query("INSERT INTO emissao_importada (chave, arquivo, linha, dps_id, criado_em) VALUES (?, ?, ?, ?, ?)")
            .bind(chave)
            .bind(arquivo)
            .bind(linha)
            .bind(dps_id)
            .bind(agora())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Origem gravada da DPS importada: as colunas de [`Banco::registrar_emissao_importada`]
    /// (chave, arquivo, linha), para os testes conferirem que nada além disso é guardado.
    #[cfg(any(test, feature = "teste"))]
    pub async fn origem_importada(&self, dps_id: i64) -> Resultado<Vec<(String, Option<String>, Option<i64>)>> {
        Ok(
            sqlx::query("SELECT chave, arquivo, linha FROM emissao_importada WHERE dps_id = ?")
                .bind(dps_id)
                .fetch_all(self.pool())
                .await?
                .iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect(),
        )
    }

    /// Nome da pessoa (tomador) pelo id, cadastrada por qualquer empresa.
    pub async fn nome_pessoa(&self, pessoa_id: i64) -> Resultado<Option<String>> {
        Ok(sqlx::query("SELECT nome FROM pessoa WHERE id = ?")
            .bind(pessoa_id)
            .fetch_optional(self.pool())
            .await?
            .map(|r| r.get(0)))
    }
}
