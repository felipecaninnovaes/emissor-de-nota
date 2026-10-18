//! Mudanças de esquema que as migrações do sqlx não conseguem fazer. O sqlx 0.8 sempre roda a
//! migração dentro de uma transação, e o SQLite só deixa desligar as chaves estrangeiras fora
//! dela; sem isso não dá para recriar uma tabela referenciada (como `dps`, por `nfse`).
//! Cada ajuste confere o esquema antes e é idempotente.

use sqlx::{Row, SqlitePool};

use crate::{ErroBanco, Resultado};

pub(crate) async fn aplicar(pool: &SqlitePool) -> Resultado<()> {
    dps_unica_por_ambiente(pool).await?;
    dps_enviado_em(pool).await
}

/// Coluna `dps.enviado_em` e o índice da migração 0005. Ficam aqui, e não numa migração, porque
/// [`dps_unica_por_ambiente`] recria a tabela depois das migrações num banco novo e perderia os dois.
async fn dps_enviado_em(pool: &SqlitePool) -> Resultado<()> {
    let tem_coluna = sqlx::query("SELECT 1 FROM pragma_table_info('dps') WHERE name = 'enviado_em'")
        .fetch_optional(pool)
        .await?
        .is_some();
    if !tem_coluna {
        sqlx::query("ALTER TABLE dps ADD COLUMN enviado_em TEXT")
            .execute(pool)
            .await?;
    }
    sqlx::query("CREATE INDEX IF NOT EXISTS dps_empresa_id ON dps (empresa_id, id DESC)")
        .execute(pool)
        .await?;
    Ok(())
}

/// `dps.id_dps` deixa de ser único no banco todo e passa a ser único por ambiente.
async fn dps_unica_por_ambiente(pool: &SqlitePool) -> Resultado<()> {
    let sql: String = sqlx::query("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'dps'")
        .fetch_one(pool)
        .await?
        .get(0);
    if sql.contains("UNIQUE (ambiente, id_dps)") {
        return Ok(());
    }
    let mut conexao = pool.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conexao).await?;
    let resultado = async {
        let mut tx = sqlx::Connection::begin(&mut *conexao).await?;
        sqlx::raw_sql(include_str!("../ajustes/dps_unica_por_ambiente.sql"))
            .execute(&mut *tx)
            .await?;
        let problemas = sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut *tx).await?;
        if !problemas.is_empty() {
            // Sai sem commit: a transação é desfeita e o esquema antigo continua.
            return Err(ErroBanco::DadoInvalido(format!(
                "{} referência(s) quebrada(s) ao recriar a tabela dps",
                problemas.len()
            )));
        }
        tx.commit().await?;
        Ok(())
    }
    .await;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conexao).await?;
    resultado
}

#[cfg(test)]
mod testes {
    use std::str::FromStr;

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::*;

    #[tokio::test]
    async fn recria_dps_sem_perder_notas_nem_eventos() {
        let opcoes = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opcoes)
            .await
            .unwrap();
        // Esquema antigo (só a migração inicial) com uma nota autorizada e um evento.
        sqlx::raw_sql(include_str!("../migrations/0001_inicial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            "INSERT INTO pessoa (id, tipo, documento, nome, atualizado_em) VALUES (1, 'cnpj', '11222333000181', 'X', 'a');
             INSERT INTO empresa (id, pessoa_id, municipio_emissor, regime_json, ambiente, serie, criado_em)
                 VALUES (1, 1, '3511102', '{}', 'producao', '1', 'a');
             INSERT INTO dps (id, empresa_id, ambiente, serie, numero, id_dps, situacao, versao_leiaute, criado_em, atualizado_em)
                 VALUES (7, 1, 'homologacao', '1', 1, 'DPS-1', 'rejeitada', '1.01', 'a', 'a'),
                        (8, 1, 'producao', '1', 2, 'DPS-2', 'autorizada', '1.01', 'a', 'a');
             INSERT INTO nfse (id, dps_id, chave, xml, xml_sha256, valor_servico, competencia, autorizada_em)
                 VALUES (3, 8, 'CHAVE', '<NFSe/>', 'h', 100, '2026-10-16', 'a');
             INSERT INTO evento (nfse_id, tipo, xml, criado_em) VALUES (3, 'cancelamento', '<e/>', 'a');",
        )
        .execute(&pool)
        .await
        .unwrap();

        aplicar(&pool).await.unwrap();
        aplicar(&pool).await.unwrap(); // idempotente

        let ids: Vec<i64> = sqlx::query("SELECT id FROM dps ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|r| r.get(0))
            .collect();
        assert_eq!(ids, vec![7, 8]);
        let dps_da_nota: i64 = sqlx::query("SELECT d.id FROM nfse n JOIN dps d ON d.id = n.dps_id")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(dps_da_nota, 8);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap()
                .is_empty()
        );
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(fk, 1, "chaves estrangeiras religadas");

        // Mesmo Id em outro ambiente agora é aceito; no mesmo ambiente, não.
        let inserir = |id: i64, ambiente: &'static str, numero: i64| {
            sqlx::query(
                "INSERT INTO dps (id, empresa_id, ambiente, serie, numero, id_dps, situacao, versao_leiaute, criado_em, atualizado_em)
                 VALUES (?, 1, ?, '1', ?, 'DPS-1', 'reservada', '1.01', 'a', 'a')",
            )
            .bind(id)
            .bind(ambiente)
            .bind(numero)
        };
        inserir(9, "producao", 1).execute(&pool).await.unwrap();
        assert!(inserir(10, "homologacao", 5).execute(&pool).await.is_err());
        // E a nota continua protegida pela chave estrangeira.
        assert!(
            sqlx::query("DELETE FROM dps WHERE id = 8")
                .execute(&pool)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn banco_novo_tem_enviado_em_e_indice_por_empresa() {
        let banco = crate::Banco::em_memoria().await.unwrap();
        let pool = banco.pool();
        aplicar(pool).await.unwrap(); // idempotente
        let tem = |sql: &'static str| async move { sqlx::query(sql).fetch_optional(pool).await.unwrap().is_some() };
        assert!(tem("SELECT 1 FROM pragma_table_info('dps') WHERE name = 'enviado_em'").await);
        assert!(tem("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'dps_empresa_id'").await);
    }
}
