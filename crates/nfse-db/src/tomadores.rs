//! Tomadores: cadastro global por documento (`pessoa`) e vínculo com cada empresa.
//!
//! Um convênio que atende várias clínicas é cadastrado uma vez; cada clínica só cria o vínculo.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use nfse_core::modelo::{Documento, EnderecoNacional, Tomador};
use nfse_core::{Cnpj, Cpf};

use crate::{Banco, ErroBanco, Resultado, agora};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomadorVinculado {
    pub pessoa_id: i64,
    pub tomador: Tomador,
    /// E-mails de envio da nota, específicos do vínculo com a empresa (separados por `;`).
    pub emails: Option<String>,
    pub observacao: Option<String>,
    /// Este tomador retém IRRF, CSLL, PIS, COFINS e INSS nas notas da empresa.
    pub retem_federais: bool,
    /// Este tomador retém o ISS (`tpRetISSQN` = 2).
    pub retem_iss: bool,
}

fn documento_de(tipo: &str, doc: &str) -> Resultado<Documento> {
    let invalido = |e| ErroBanco::DadoInvalido(format!("documento {doc}: {e}"));
    match tipo {
        "cnpj" => Ok(Documento::Cnpj(Cnpj::parse(doc).map_err(invalido)?)),
        "cpf" => Ok(Documento::Cpf(Cpf::parse(doc).map_err(invalido)?)),
        outro => Err(ErroBanco::DadoInvalido(format!("tipo de documento {outro:?}"))),
    }
}

pub(crate) fn tipo_e_numero(d: &Documento) -> (&'static str, &str) {
    match d {
        Documento::Cnpj(c) => ("cnpj", c.as_str()),
        Documento::Cpf(c) => ("cpf", c.as_str()),
    }
}

pub(crate) const SELECT_TOMADOR: &str =
    "SELECT p.id, p.tipo, p.documento, p.inscricao_municipal, p.nome, p.municipio, p.cep,
    p.logradouro, p.numero, p.complemento, p.bairro, p.telefone, p.email, v.emails, v.observacao,
    v.retem_federais, v.retem_iss
    FROM empresa_tomador v JOIN pessoa p ON p.id = v.pessoa_id";

pub(crate) fn tomador_de(r: &SqliteRow) -> Resultado<TomadorVinculado> {
    let municipio: Option<String> = r.get(5);
    let endereco = match (municipio, r.get::<Option<String>, _>(6), r.get::<Option<String>, _>(7)) {
        (Some(municipio), Some(cep), Some(logradouro)) => Some(EnderecoNacional {
            municipio,
            cep,
            logradouro,
            numero: r.get::<Option<String>, _>(8).unwrap_or_else(|| "S/N".into()),
            complemento: r.get(9),
            bairro: r.get::<Option<String>, _>(10).unwrap_or_default(),
        }),
        _ => None,
    };
    Ok(TomadorVinculado {
        pessoa_id: r.get(0),
        tomador: Tomador {
            documento: documento_de(r.get(1), r.get(2))?,
            inscricao_municipal: r.get(3),
            nome: r.get(4),
            endereco,
            telefone: r.get(11),
            email: r.get(12),
        },
        emails: r.get(13),
        observacao: r.get(14),
        retem_federais: r.get(15),
        retem_iss: r.get(16),
    })
}

impl Banco {
    /// Cria ou atualiza a pessoa pelo documento e garante o vínculo com a empresa.
    /// `fonte` registra de onde vieram os dados (ex.: "manual", "brasilapi").
    pub async fn salvar_tomador(&self, empresa_id: i64, t: &Tomador, fonte: &str) -> Resultado<i64> {
        let mut tx = self.pool().begin().await?;
        let pessoa_id = gravar_tomador(&mut tx, empresa_id, t, fonte).await?;
        tx.commit().await?;
        Ok(pessoa_id)
    }

    /// [`Banco::salvar_tomador`] com os e-mails de envio e a retenção do vínculo, tudo ou nada.
    pub async fn salvar_tomador_vinculado(
        &self,
        empresa_id: i64,
        t: &Tomador,
        fonte: &str,
        emails: Option<&str>,
        retem_federais: bool,
        retem_iss: bool,
    ) -> Resultado<i64> {
        let mut tx = self.pool().begin().await?;
        let pessoa_id = gravar_tomador(&mut tx, empresa_id, t, fonte).await?;
        sqlx::query(
            "UPDATE empresa_tomador SET emails = ?, observacao = NULL, retem_federais = ?, retem_iss = ?
             WHERE empresa_id = ? AND pessoa_id = ?",
        )
        .bind(emails)
        .bind(retem_federais)
        .bind(retem_iss)
        .bind(empresa_id)
        .bind(pessoa_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(pessoa_id)
    }
}

/// Cria ou atualiza a pessoa e garante o vínculo com a empresa, dentro da transação.
pub(crate) async fn gravar_tomador(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    empresa_id: i64,
    t: &Tomador,
    fonte: &str,
) -> Resultado<i64> {
    if t.nome.trim().is_empty() {
        return Err(ErroBanco::Regra("nome do tomador vazio".into()));
    }
    let (tipo, numero) = tipo_e_numero(&t.documento);
    let e = t.endereco.as_ref();
    let pessoa_id: i64 = sqlx::query(
        "INSERT INTO pessoa (tipo, documento, nome, inscricao_municipal, municipio, cep, logradouro, numero,
             complemento, bairro, telefone, email, fonte, atualizado_em)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (documento) DO UPDATE SET nome = excluded.nome,
             inscricao_municipal = excluded.inscricao_municipal, municipio = excluded.municipio, cep = excluded.cep,
             logradouro = excluded.logradouro, numero = excluded.numero, complemento = excluded.complemento,
             bairro = excluded.bairro, telefone = excluded.telefone, email = excluded.email,
             fonte = excluded.fonte, atualizado_em = excluded.atualizado_em
         RETURNING id",
    )
    .bind(tipo)
    .bind(numero)
    .bind(t.nome.trim())
    .bind(&t.inscricao_municipal)
    .bind(e.map(|e| &e.municipio))
    .bind(e.map(|e| &e.cep))
    .bind(e.map(|e| e.logradouro.trim()))
    .bind(e.map(|e| e.numero.trim()))
    .bind(e.and_then(|e| e.complemento.as_deref()).map(str::trim))
    .bind(e.map(|e| e.bairro.trim()))
    .bind(&t.telefone)
    .bind(&t.email)
    .bind(fonte)
    .bind(agora())
    .fetch_one(&mut **tx)
    .await?
    .get(0);
    sqlx::query("INSERT INTO empresa_tomador (empresa_id, pessoa_id) VALUES (?, ?) ON CONFLICT DO NOTHING")
        .bind(empresa_id)
        .bind(pessoa_id)
        .execute(&mut **tx)
        .await?;
    Ok(pessoa_id)
}

impl Banco {
    pub async fn definir_emails_tomador(
        &self,
        empresa_id: i64,
        pessoa_id: i64,
        emails: Option<&str>,
        observacao: Option<&str>,
    ) -> Resultado<()> {
        let afetadas =
            sqlx::query("UPDATE empresa_tomador SET emails = ?, observacao = ? WHERE empresa_id = ? AND pessoa_id = ?")
                .bind(emails)
                .bind(observacao)
                .bind(empresa_id)
                .bind(pessoa_id)
                .execute(self.pool())
                .await?
                .rows_affected();
        if afetadas == 0 {
            return Err(ErroBanco::NaoEncontrado("vínculo do tomador com a empresa"));
        }
        Ok(())
    }

    pub async fn definir_retencao_tomador(
        &self,
        empresa_id: i64,
        pessoa_id: i64,
        retem_federais: bool,
        retem_iss: bool,
    ) -> Resultado<()> {
        let afetadas = sqlx::query(
            "UPDATE empresa_tomador SET retem_federais = ?, retem_iss = ? WHERE empresa_id = ? AND pessoa_id = ?",
        )
        .bind(retem_federais)
        .bind(retem_iss)
        .bind(empresa_id)
        .bind(pessoa_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        if afetadas == 0 {
            return Err(ErroBanco::NaoEncontrado("vínculo do tomador com a empresa"));
        }
        Ok(())
    }

    pub async fn tomador(&self, empresa_id: i64, pessoa_id: i64) -> Resultado<TomadorVinculado> {
        let linha = sqlx::query(&format!("{SELECT_TOMADOR} WHERE v.empresa_id = ? AND v.pessoa_id = ?"))
            .bind(empresa_id)
            .bind(pessoa_id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("tomador da empresa"))?;
        tomador_de(&linha)
    }

    /// Busca por parte do nome ou do documento (com ou sem pontuação), até 20 resultados.
    pub async fn buscar_tomadores(&self, empresa_id: i64, termo: &str) -> Resultado<Vec<TomadorVinculado>> {
        let termo = termo.trim();
        let so_documento: String = termo.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let linhas = sqlx::query(&format!(
            "{SELECT_TOMADOR} WHERE v.empresa_id = ?
               AND (p.nome LIKE '%' || ? || '%' COLLATE NOCASE OR (? <> '' AND p.documento LIKE ? || '%'))
             ORDER BY p.nome LIMIT 20"
        ))
        .bind(empresa_id)
        .bind(termo)
        .bind(&so_documento)
        .bind(so_documento.to_uppercase())
        .fetch_all(self.pool())
        .await?;
        linhas.iter().map(tomador_de).collect()
    }

    /// Tomadores cadastrados por outras empresas e ainda não vinculados a esta (mesma busca de
    /// `buscar_tomadores`), para vincular sem cadastrar de novo. E-mails e retenção vêm zerados: são do vínculo.
    pub async fn buscar_tomadores_de_outras_empresas(
        &self,
        empresa_id: i64,
        termo: &str,
    ) -> Resultado<Vec<TomadorVinculado>> {
        let termo = termo.trim();
        let so_documento: String = termo.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let linhas = sqlx::query(
            "SELECT p.id, p.tipo, p.documento, p.inscricao_municipal, p.nome, p.municipio, p.cep,
                 p.logradouro, p.numero, p.complemento, p.bairro, p.telefone, p.email, NULL, NULL, 0, 0
             FROM pessoa p
             WHERE p.id IN (SELECT pessoa_id FROM empresa_tomador)
               AND p.id NOT IN (SELECT pessoa_id FROM empresa_tomador WHERE empresa_id = ?)
               AND (p.nome LIKE '%' || ? || '%' COLLATE NOCASE OR (? <> '' AND p.documento LIKE ? || '%'))
             ORDER BY p.nome LIMIT 20",
        )
        .bind(empresa_id)
        .bind(termo)
        .bind(&so_documento)
        .bind(so_documento.to_uppercase())
        .fetch_all(self.pool())
        .await?;
        linhas.iter().map(tomador_de).collect()
    }

    /// Vincula a empresa a um tomador já cadastrado por outra (sem e-mails, observação nem retenção).
    pub async fn vincular_tomador_cadastrado(&self, empresa_id: i64, pessoa_id: i64) -> Resultado<TomadorVinculado> {
        let existe = sqlx::query("SELECT 1 FROM empresa_tomador WHERE pessoa_id = ?")
            .bind(pessoa_id)
            .fetch_optional(self.pool())
            .await?
            .is_some();
        if !existe {
            return Err(ErroBanco::NaoEncontrado("tomador"));
        }
        sqlx::query("INSERT INTO empresa_tomador (empresa_id, pessoa_id) VALUES (?, ?) ON CONFLICT DO NOTHING")
            .bind(empresa_id)
            .bind(pessoa_id)
            .execute(self.pool())
            .await?;
        self.tomador(empresa_id, pessoa_id).await
    }

    /// Pessoa já cadastrada (por qualquer empresa), para reaproveitar sem nova consulta de CNPJ.
    pub async fn pessoa_por_documento(&self, documento: &str) -> Resultado<Option<i64>> {
        Ok(sqlx::query("SELECT id FROM pessoa WHERE documento = ?")
            .bind(documento)
            .fetch_optional(self.pool())
            .await?
            .map(|r| r.get(0)))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::{banco_com_empresa, nova_empresa};

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

    #[tokio::test]
    async fn tomador_reutilizado_entre_empresas() {
        let (banco, _, clinica1) = banco_com_empresa().await;
        let mut outra = nova_empresa();
        outra.cnpj = Cnpj::parse("11222333000262").unwrap();
        let clinica2 = banco.cadastrar_empresa(&outra).await.unwrap();

        let id1 = banco.salvar_tomador(clinica1.id, &convenio(), "manual").await.unwrap();
        let id2 = banco.salvar_tomador(clinica2.id, &convenio(), "manual").await.unwrap();
        assert_eq!(id1, id2, "mesma pessoa para as duas clínicas");

        banco
            .definir_emails_tomador(clinica1.id, id1, Some("a@x.com; b@x.com"), None)
            .await
            .unwrap();
        assert_eq!(
            banco.tomador(clinica1.id, id1).await.unwrap().emails.as_deref(),
            Some("a@x.com; b@x.com")
        );
        assert_eq!(
            banco.tomador(clinica2.id, id1).await.unwrap().emails,
            None,
            "e-mail é do vínculo"
        );
        assert_eq!(banco.tomador(clinica1.id, id1).await.unwrap().tomador, convenio());

        banco
            .definir_retencao_tomador(clinica1.id, id1, true, false)
            .await
            .unwrap();
        let c1 = banco.tomador(clinica1.id, id1).await.unwrap();
        assert!(c1.retem_federais && !c1.retem_iss);
        let c2 = banco.tomador(clinica2.id, id1).await.unwrap();
        assert!(!c2.retem_federais && !c2.retem_iss, "retenção é do vínculo");
    }

    #[tokio::test]
    async fn salva_tomador_com_vinculo() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let id = banco
            .salvar_tomador_vinculado(empresa.id, &convenio(), "manual", Some("a@x.com"), true, false)
            .await
            .unwrap();
        let t = banco.tomador(empresa.id, id).await.unwrap();
        assert_eq!(t.emails.as_deref(), Some("a@x.com"));
        assert!(t.retem_federais && !t.retem_iss);

        // Sem nome nada é gravado, nem a pessoa nem o vínculo.
        let mut sem_nome = convenio();
        sem_nome.nome = " ".into();
        sem_nome.documento = nfse_core::modelo::Documento::Cpf(nfse_core::Cpf::parse("52998224725").unwrap());
        assert!(
            banco
                .salvar_tomador_vinculado(empresa.id, &sem_nome, "manual", None, false, false)
                .await
                .is_err()
        );
        assert_eq!(banco.pessoa_por_documento("52998224725").await.unwrap(), None);
    }

    #[tokio::test]
    async fn busca_por_nome_ou_documento() {
        let (banco, _, empresa) = banco_com_empresa().await;
        banco.salvar_tomador(empresa.id, &convenio(), "manual").await.unwrap();
        for termo in ["saude", "CONVENIO", "12.ABC", "12abc345"] {
            assert_eq!(
                banco.buscar_tomadores(empresa.id, termo).await.unwrap().len(),
                1,
                "{termo}"
            );
        }
        assert!(banco.buscar_tomadores(empresa.id, "farmacia").await.unwrap().is_empty());
        // Outra empresa sem vínculo não enxerga o tomador.
        let mut outra = nova_empresa();
        outra.cnpj = Cnpj::parse("11222333000262").unwrap();
        let outra = banco.cadastrar_empresa(&outra).await.unwrap();
        assert!(banco.buscar_tomadores(outra.id, "saude").await.unwrap().is_empty());
        assert!(banco.pessoa_por_documento("12ABC34501DE35").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn vincula_tomador_de_outra_empresa() {
        let (banco, _, clinica1) = banco_com_empresa().await;
        let mut outra = nova_empresa();
        outra.cnpj = Cnpj::parse("11222333000262").unwrap();
        let clinica2 = banco.cadastrar_empresa(&outra).await.unwrap();
        let id = banco.salvar_tomador(clinica1.id, &convenio(), "manual").await.unwrap();
        banco
            .definir_retencao_tomador(clinica1.id, id, true, true)
            .await
            .unwrap();

        assert!(
            banco
                .buscar_tomadores_de_outras_empresas(clinica1.id, "convenio")
                .await
                .unwrap()
                .is_empty()
        );
        let outros = banco
            .buscar_tomadores_de_outras_empresas(clinica2.id, "convenio")
            .await
            .unwrap();
        assert_eq!(outros.len(), 1);
        assert!(!outros[0].retem_federais && outros[0].emails.is_none());

        let v = banco.vincular_tomador_cadastrado(clinica2.id, id).await.unwrap();
        assert_eq!(v.pessoa_id, id);
        assert!(!v.retem_federais, "a retenção não é herdada da outra empresa");
        assert!(
            banco
                .buscar_tomadores_de_outras_empresas(clinica2.id, "convenio")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(banco.vincular_tomador_cadastrado(clinica2.id, 9_999).await.is_err());
    }
}
