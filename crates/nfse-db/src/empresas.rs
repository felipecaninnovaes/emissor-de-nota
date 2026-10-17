//! Empresas (prestadores) e seus certificados.

use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use nfse_cert::{CertificadoA1, InfoCertificado};
use nfse_core::Cnpj;
use nfse_core::modelo::{Ambiente, RegimeTributario};

use crate::{Banco, Cofre, ErroBanco, Resultado, agora, de_json, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaEmpresa {
    pub cnpj: Cnpj,
    pub nome: String,
    pub inscricao_municipal: Option<String>,
    /// Código IBGE do município onde a empresa emite (`cLocEmi`).
    pub municipio_emissor: String,
    pub regime: RegimeTributario,
    pub ambiente: Ambiente,
    /// Série da DPS deste emissor. Não usar a do Emissor Web (70000).
    pub serie: String,
    pub telefone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Empresa {
    pub id: i64,
    pub cnpj: Cnpj,
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
    pub ativa: bool,
}

impl Empresa {
    /// IM a informar na DPS no ambiente dado (a Sefin rejeita com E0120 quando o CNC não a tem).
    pub fn im_para_dps(&self, ambiente: Ambiente) -> Option<&str> {
        let informar = match ambiente {
            Ambiente::Producao => self.im_na_dps_producao,
            Ambiente::Homologacao => self.im_na_dps_homologacao,
        };
        self.inscricao_municipal.as_deref().filter(|_| informar)
    }
}

pub(crate) fn ambiente_texto(a: Ambiente) -> &'static str {
    match a {
        Ambiente::Producao => "producao",
        Ambiente::Homologacao => "homologacao",
    }
}

pub(crate) fn ambiente_de(s: &str) -> Resultado<Ambiente> {
    match s {
        "producao" => Ok(Ambiente::Producao),
        "homologacao" => Ok(Ambiente::Homologacao),
        outro => Err(ErroBanco::DadoInvalido(format!("ambiente {outro:?}"))),
    }
}

const SELECT_EMPRESA: &str = "SELECT e.id, p.documento, p.nome, e.inscricao_municipal, e.im_na_dps_producao,
    e.im_na_dps_homologacao, e.municipio_emissor, e.regime_json, e.ambiente, e.serie, p.telefone, p.email, e.ativa
    FROM empresa e JOIN pessoa p ON p.id = e.pessoa_id";

fn empresa_de(r: &SqliteRow) -> Resultado<Empresa> {
    let documento: String = r.get(1);
    Ok(Empresa {
        id: r.get(0),
        cnpj: Cnpj::parse(&documento).map_err(|e| ErroBanco::DadoInvalido(format!("CNPJ {documento}: {e}")))?,
        nome: r.get(2),
        inscricao_municipal: r.get(3),
        im_na_dps_producao: r.get(4),
        im_na_dps_homologacao: r.get(5),
        municipio_emissor: r.get(6),
        regime: de_json(r.get(7), "regime")?,
        ambiente: ambiente_de(r.get(8))?,
        serie: r.get(9),
        telefone: r.get(10),
        email: r.get(11),
        ativa: r.get(12),
    })
}

/// Contexto (AAD) do cofre para o certificado: amarra o dado cifrado à empresa e ao certificado.
fn contexto_certificado(empresa_id: i64, thumbprint: &str, parte: &str) -> String {
    format!("certificado:{empresa_id}:{thumbprint}:{parte}")
}

fn data(s: &str) -> Resultado<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| ErroBanco::DadoInvalido(format!("data {s:?}: {e}")))
}

fn validar(nova: &NovaEmpresa) -> Resultado<()> {
    let erros: Vec<String> = [
        (nova.nome.trim().is_empty(), "nome vazio"),
        (
            nova.municipio_emissor.len() != 7 || !nova.municipio_emissor.bytes().all(|b| b.is_ascii_digit()),
            "município IBGE deve ter 7 dígitos",
        ),
        (
            nova.serie == "70000",
            "a série 70000 é usada pelo Emissor Web; escolha outra",
        ),
    ]
    .into_iter()
    .filter(|(erro, _)| *erro)
    .map(|(_, msg)| msg.to_owned())
    .collect();
    if !erros.is_empty() {
        return Err(ErroBanco::Regra(erros.join("; ")));
    }
    Ok(())
}

/// O que `excluir_empresa` fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusao {
    Excluida,
    /// Tinha NFS-e emitidas: saiu da lista, mas as notas continuam no banco.
    Arquivada {
        notas: i64,
    },
}

/// Linha do painel de vencimentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VencimentoCertificado {
    pub empresa_id: i64,
    pub empresa: String,
    pub titular: String,
    pub valido_ate: DateTime<Utc>,
}

impl Banco {
    pub async fn cadastrar_empresa(&self, nova: &NovaEmpresa) -> Resultado<Empresa> {
        validar(nova)?;

        let mut tx = self.pool().begin().await?;
        let agora = agora();
        let pessoa_id: i64 = sqlx::query(
            "INSERT INTO pessoa (tipo, documento, nome, telefone, email, atualizado_em) VALUES ('cnpj', ?, ?, ?, ?, ?)
             ON CONFLICT (documento) DO UPDATE SET nome = excluded.nome,
                 telefone = COALESCE(excluded.telefone, telefone), email = COALESCE(excluded.email, email),
                 atualizado_em = excluded.atualizado_em
             RETURNING id",
        )
        .bind(nova.cnpj.as_str())
        .bind(nova.nome.trim())
        .bind(&nova.telefone)
        .bind(&nova.email)
        .bind(&agora)
        .fetch_one(&mut *tx)
        .await?
        .get(0);

        // Empresa arquivada (tinha notas) volta com o histórico e os dados novos.
        let arquivada: Option<i64> = sqlx::query("SELECT id FROM empresa WHERE pessoa_id = ? AND ativa = 0")
            .bind(pessoa_id)
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.get(0));
        if let Some(id) = arquivada {
            sqlx::query(
                "UPDATE empresa SET inscricao_municipal = ?, municipio_emissor = ?, regime_json = ?, ambiente = ?,
                     serie = ?, ativa = 1 WHERE id = ?",
            )
            .bind(&nova.inscricao_municipal)
            .bind(&nova.municipio_emissor)
            .bind(json(&nova.regime))
            .bind(ambiente_texto(nova.ambiente))
            .bind(&nova.serie)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            self.auditar("empresa_reativada", Some(id), Some(nova.cnpj.as_str()))
                .await?;
            return self.empresa(id).await;
        }

        let id: i64 = sqlx::query(
            "INSERT INTO empresa (pessoa_id, inscricao_municipal, municipio_emissor, regime_json, ambiente, serie, criado_em)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(pessoa_id)
        .bind(&nova.inscricao_municipal)
        .bind(&nova.municipio_emissor)
        .bind(json(&nova.regime))
        .bind(ambiente_texto(nova.ambiente))
        .bind(&nova.serie)
        .bind(&agora)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => {
                ErroBanco::Regra(format!("a empresa {} já está cadastrada", nova.cnpj))
            }
            outro => outro.into(),
        })?
        .get(0);
        tx.commit().await?;
        self.auditar("empresa_cadastrada", Some(id), Some(nova.cnpj.as_str()))
            .await?;
        self.empresa(id).await
    }

    pub async fn empresa(&self, id: i64) -> Resultado<Empresa> {
        let linha = sqlx::query(&format!("{SELECT_EMPRESA} WHERE e.id = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("empresa"))?;
        empresa_de(&linha)
    }

    pub async fn empresas(&self) -> Resultado<Vec<Empresa>> {
        sqlx::query(&format!("{SELECT_EMPRESA} WHERE e.ativa = 1 ORDER BY p.nome"))
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(empresa_de)
            .collect()
    }

    /// Atualiza os dados cadastrais. O CNPJ não muda (o certificado é dele). Trocar ambiente ou
    /// série só com todos os envios resolvidos; a numeração de cada ambiente/série é separada.
    pub async fn atualizar_empresa(&self, id: i64, dados: &NovaEmpresa) -> Resultado<Empresa> {
        validar(dados)?;
        let atual = self.empresa(id).await?;
        if atual.cnpj != dados.cnpj {
            return Err(ErroBanco::Regra(
                "o CNPJ não pode ser alterado; exclua e cadastre a outra empresa".into(),
            ));
        }
        let muda_emissao = atual.ambiente != dados.ambiente || atual.serie != dados.serie;
        if muda_emissao {
            let pendentes: i64 =
                sqlx::query("SELECT COUNT(*) FROM dps WHERE empresa_id = ? AND situacao IN ('enviando', 'incerta')")
                    .bind(id)
                    .fetch_one(self.pool())
                    .await?
                    .get(0);
            if pendentes > 0 {
                return Err(ErroBanco::Regra(
                    "há envio sem resposta da Sefin; verifique-o antes de trocar ambiente ou série".into(),
                ));
            }
        }
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "UPDATE pessoa SET nome = ?, telefone = ?, email = ?, atualizado_em = ?
             WHERE id = (SELECT pessoa_id FROM empresa WHERE id = ?)",
        )
        .bind(dados.nome.trim())
        .bind(&dados.telefone)
        .bind(&dados.email)
        .bind(agora())
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE empresa SET inscricao_municipal = ?, municipio_emissor = ?, regime_json = ?, ambiente = ?,
                 serie = ? WHERE id = ?",
        )
        .bind(&dados.inscricao_municipal)
        .bind(&dados.municipio_emissor)
        .bind(json(&dados.regime))
        .bind(ambiente_texto(dados.ambiente))
        .bind(&dados.serie)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let detalhe = if muda_emissao {
            format!(
                "{} série {} → {} série {}",
                ambiente_texto(atual.ambiente),
                atual.serie,
                ambiente_texto(dados.ambiente),
                dados.serie
            )
        } else {
            "dados cadastrais".into()
        };
        self.auditar("empresa_atualizada", Some(id), Some(&detalhe)).await?;
        self.empresa(id).await
    }

    /// Remove a empresa da lista. Sem NFS-e emitida, apaga o cadastro inteiro (certificado,
    /// serviços, vínculos, numeração e DPS não autorizadas); com NFS-e, só arquiva, porque as
    /// notas são documento fiscal e continuam no banco. Recusa se houver envio sem resposta.
    pub async fn excluir_empresa(&self, id: i64) -> Resultado<Exclusao> {
        let empresa = self.empresa(id).await?;
        let contar = |sql: &'static str| async move {
            sqlx::query(sql)
                .bind(id)
                .fetch_one(self.pool())
                .await
                .map(|r| r.get::<i64, _>(0))
        };
        if contar("SELECT COUNT(*) FROM dps WHERE empresa_id = ? AND situacao IN ('enviando', 'incerta')").await? > 0 {
            return Err(ErroBanco::Regra(
                "há envio sem resposta da Sefin para esta empresa; verifique-o antes de excluir".into(),
            ));
        }
        let notas = contar("SELECT COUNT(*) FROM nfse n JOIN dps d ON d.id = n.dps_id WHERE d.empresa_id = ?").await?;
        if notas > 0 {
            sqlx::query("UPDATE empresa SET ativa = 0 WHERE id = ?")
                .bind(id)
                .execute(self.pool())
                .await?;
            self.auditar("empresa_arquivada", Some(id), Some(empresa.cnpj.as_str()))
                .await?;
            return Ok(Exclusao::Arquivada { notas });
        }
        let mut tx = self.pool().begin().await?;
        sqlx::query("DELETE FROM emissao_importada WHERE dps_id IN (SELECT id FROM dps WHERE empresa_id = ?)")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "DELETE FROM emissao_recorrente WHERE recorrencia_id IN (SELECT id FROM recorrencia WHERE empresa_id = ?)",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        for tabela in [
            "recorrencia",
            "dps",
            "servico",
            "empresa_tomador",
            "numeracao",
            "certificado",
        ] {
            sqlx::query(&format!("DELETE FROM {tabela} WHERE empresa_id = ?"))
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM empresa WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.auditar("empresa_excluida", Some(id), Some(empresa.cnpj.as_str()))
            .await?;
        Ok(Exclusao::Excluida)
    }

    pub async fn definir_im_na_dps(&self, empresa_id: i64, ambiente: Ambiente, informar: bool) -> Resultado<()> {
        let coluna = match ambiente {
            Ambiente::Producao => "im_na_dps_producao",
            Ambiente::Homologacao => "im_na_dps_homologacao",
        };
        sqlx::query(&format!("UPDATE empresa SET {coluna} = ? WHERE id = ?"))
            .bind(informar)
            .bind(empresa_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Importa o .pfx: confere senha, CNPJ (mesma raiz), EKU Client Authentication e validade;
    /// guarda cifrado e passa a ser o certificado ativo (o anterior fica no histórico).
    pub async fn importar_certificado(
        &self,
        cofre: &Cofre,
        empresa_id: i64,
        pfx: &[u8],
        senha: &str,
    ) -> Resultado<InfoCertificado> {
        let empresa = self.empresa(empresa_id).await?;
        let cert = CertificadoA1::carregar(pfx, senha)?;
        let info = cert.info().clone();
        if !info.pertence_a(&empresa.cnpj) {
            let do_cert = info.cnpj.as_ref().map_or("sem CNPJ".to_owned(), |c| c.to_string());
            return Err(ErroBanco::Regra(format!(
                "o certificado é de {do_cert}, não da empresa {}",
                empresa.cnpj
            )));
        }
        if !info.client_auth {
            return Err(ErroBanco::Regra(
                "o certificado não tem uso Client Authentication; a Sefin vai recusar".into(),
            ));
        }
        if info.vencido_em(Utc::now()) {
            return Err(ErroBanco::Regra(format!(
                "certificado vencido em {}",
                info.valido_ate.format("%d/%m/%Y")
            )));
        }

        let pfx_cifrado = cofre.cifrar(pfx, &contexto_certificado(empresa_id, &info.thumbprint, "pfx"))?;
        let senha_cifrada = cofre.cifrar(
            senha.as_bytes(),
            &contexto_certificado(empresa_id, &info.thumbprint, "senha"),
        )?;

        let mut tx = self.pool().begin().await?;
        sqlx::query("UPDATE certificado SET ativo = 0 WHERE empresa_id = ?")
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO certificado (empresa_id, pfx_cifrado, senha_cifrada, thumbprint, titular, emissor, valido_de,
                 valido_ate, ativo, importado_em)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
             ON CONFLICT (empresa_id, thumbprint) DO UPDATE SET pfx_cifrado = excluded.pfx_cifrado,
                 senha_cifrada = excluded.senha_cifrada, ativo = 1, importado_em = excluded.importado_em",
        )
        .bind(empresa_id)
        .bind(pfx_cifrado)
        .bind(senha_cifrada)
        .bind(&info.thumbprint)
        .bind(&info.titular)
        .bind(&info.emissor)
        .bind(info.valido_de.to_rfc3339())
        .bind(info.valido_ate.to_rfc3339())
        .bind(agora())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.auditar("certificado_importado", Some(empresa_id), Some(&info.thumbprint))
            .await?;
        Ok(info)
    }

    /// Abre o certificado ativo da empresa (decifrado só em memória).
    pub async fn certificado_ativo(&self, cofre: &Cofre, empresa_id: i64) -> Resultado<CertificadoA1> {
        let linha = sqlx::query(
            "SELECT thumbprint, pfx_cifrado, senha_cifrada FROM certificado WHERE empresa_id = ? AND ativo = 1",
        )
        .bind(empresa_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ErroBanco::NaoEncontrado("certificado ativo da empresa"))?;
        let thumbprint: String = linha.get(0);
        let pfx = cofre.decifrar(linha.get(1), &contexto_certificado(empresa_id, &thumbprint, "pfx"))?;
        let senha = cofre.decifrar(linha.get(2), &contexto_certificado(empresa_id, &thumbprint, "senha"))?;
        let senha = std::str::from_utf8(&senha).map_err(|_| ErroBanco::DadoInvalido("senha do certificado".into()))?;
        Ok(CertificadoA1::carregar(&pfx, senha)?)
    }

    /// Titular e validade do certificado ativo (sem abrir o cofre).
    pub async fn certificado_resumo(&self, empresa_id: i64) -> Resultado<Option<(String, DateTime<Utc>)>> {
        let linha = sqlx::query("SELECT titular, valido_ate FROM certificado WHERE empresa_id = ? AND ativo = 1")
            .bind(empresa_id)
            .fetch_optional(self.pool())
            .await?;
        linha.map(|r| Ok((r.get(0), data(r.get(1))?))).transpose()
    }

    /// Certificados ativos que vencem em até `dias` (inclui os já vencidos), do mais urgente ao menos.
    pub async fn certificados_vencendo(&self, dias: i64) -> Resultado<Vec<VencimentoCertificado>> {
        let limite = (Utc::now() + chrono::Duration::days(dias)).to_rfc3339();
        let linhas = sqlx::query(
            "SELECT c.empresa_id, p.nome, c.titular, c.valido_ate FROM certificado c
             JOIN empresa e ON e.id = c.empresa_id JOIN pessoa p ON p.id = e.pessoa_id
             WHERE c.ativo = 1 AND e.ativa = 1 AND c.valido_ate <= ? ORDER BY c.valido_ate",
        )
        .bind(limite)
        .fetch_all(self.pool())
        .await?;
        linhas
            .iter()
            .map(|r| {
                Ok(VencimentoCertificado {
                    empresa_id: r.get(0),
                    empresa: r.get(1),
                    titular: r.get(2),
                    valido_ate: data(r.get(3))?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;
    use nfse_core::modelo::{ApuracaoSimples, SituacaoSimples};

    pub const PFX: &[u8] = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");

    pub fn nova_empresa() -> NovaEmpresa {
        NovaEmpresa {
            cnpj: Cnpj::parse("11222333000181").unwrap(),
            nome: "EMPRESA TESTE LTDA".into(),
            inscricao_municipal: Some("12345".into()),
            municipio_emissor: "3511102".into(),
            regime: RegimeTributario {
                simples: SituacaoSimples::MeEpp,
                apuracao_simples: Some(ApuracaoSimples::TudoNoSimples),
                regime_especial: 0,
            },
            ambiente: Ambiente::Homologacao,
            serie: "1".into(),
            telefone: Some("1733334444".into()),
            email: None,
        }
    }

    pub async fn banco_com_empresa() -> (Banco, Cofre, Empresa) {
        let banco = Banco::em_memoria().await.unwrap();
        let cofre = banco.criar_cofre_rapido("senha-mestra").await.unwrap();
        let empresa = banco.cadastrar_empresa(&nova_empresa()).await.unwrap();
        (banco, cofre, empresa)
    }

    #[tokio::test]
    async fn cadastra_e_le_empresa() {
        let (banco, _, empresa) = banco_com_empresa().await;
        assert_eq!(empresa.cnpj.as_str(), "11222333000181");
        assert_eq!(empresa.regime.simples, SituacaoSimples::MeEpp);
        assert_eq!(banco.empresas().await.unwrap(), vec![empresa.clone()]);
        // IM: por padrão vai em Produção e não em Homologação (E0120 na Produção Restrita).
        assert_eq!(empresa.im_para_dps(Ambiente::Producao), Some("12345"));
        assert_eq!(empresa.im_para_dps(Ambiente::Homologacao), None);
        banco
            .definir_im_na_dps(empresa.id, Ambiente::Homologacao, true)
            .await
            .unwrap();
        assert_eq!(
            banco
                .empresa(empresa.id)
                .await
                .unwrap()
                .im_para_dps(Ambiente::Homologacao),
            Some("12345")
        );

        assert!(matches!(
            banco.cadastrar_empresa(&nova_empresa()).await,
            Err(ErroBanco::Regra(_))
        ));
        let mut serie_web = nova_empresa();
        serie_web.serie = "70000".into();
        assert!(matches!(banco.cadastrar_empresa(&serie_web).await, Err(ErroBanco::Regra(m)) if m.contains("70000")));
    }

    #[tokio::test]
    async fn atualiza_dados_cadastrais() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let mut dados = nova_empresa();
        dados.nome = "NOME NOVO LTDA".into();
        dados.ambiente = Ambiente::Producao;
        dados.serie = "2".into();
        dados.inscricao_municipal = None;
        dados.telefone = None;
        let nova = banco.atualizar_empresa(empresa.id, &dados).await.unwrap();
        assert_eq!(nova.nome, "NOME NOVO LTDA");
        assert_eq!(nova.ambiente, Ambiente::Producao);
        assert_eq!(nova.serie, "2");
        assert_eq!(nova.inscricao_municipal, None);
        assert_eq!(nova.telefone, None);

        dados.cnpj = Cnpj::parse("12ABC34501DE35").unwrap();
        assert!(matches!(
            banco.atualizar_empresa(empresa.id, &dados).await,
            Err(ErroBanco::Regra(m)) if m.contains("CNPJ")
        ));
        let mut serie_web = nova_empresa();
        serie_web.serie = "70000".into();
        assert!(banco.atualizar_empresa(empresa.id, &serie_web).await.is_err());
    }

    #[tokio::test]
    async fn certificado_cifrado_e_reaberto() {
        let (banco, cofre, empresa) = banco_com_empresa().await;
        let info = banco
            .importar_certificado(&cofre, empresa.id, PFX, "1234")
            .await
            .unwrap();
        assert!(info.client_auth);

        // O .pfx não fica em claro no banco.
        let blob: Vec<u8> = sqlx::query("SELECT pfx_cifrado FROM certificado")
            .fetch_one(banco.pool())
            .await
            .unwrap()
            .get(0);
        assert!(!blob.windows(PFX.len().min(64)).any(|j| j == &PFX[..PFX.len().min(64)]));

        let cert = banco.certificado_ativo(&cofre, empresa.id).await.unwrap();
        assert_eq!(cert.info().thumbprint, info.thumbprint);

        // Reimportar o mesmo não duplica; continua um ativo só.
        banco
            .importar_certificado(&cofre, empresa.id, PFX, "1234")
            .await
            .unwrap();
        let ativos: i64 = sqlx::query("SELECT COUNT(*) FROM certificado WHERE ativo = 1")
            .fetch_one(banco.pool())
            .await
            .unwrap()
            .get(0);
        assert_eq!(ativos, 1);

        // Vence em ~10 anos: aparece numa janela de 4000 dias, não numa de 30.
        assert!(banco.certificados_vencendo(30).await.unwrap().is_empty());
        assert_eq!(banco.certificados_vencendo(4000).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn recusa_certificado_de_outra_empresa() {
        let banco = Banco::em_memoria().await.unwrap();
        let cofre = banco.criar_cofre_rapido("senha-mestra").await.unwrap();
        let mut outra = nova_empresa();
        outra.cnpj = Cnpj::parse("12ABC34501DE35").unwrap();
        let empresa = banco.cadastrar_empresa(&outra).await.unwrap();
        let erro = banco
            .importar_certificado(&cofre, empresa.id, PFX, "1234")
            .await
            .unwrap_err();
        assert!(matches!(erro, ErroBanco::Regra(m) if m.contains("não da empresa")));
    }

    #[tokio::test]
    async fn senha_mestra_errada_nao_abre_certificado() {
        let (banco, cofre, empresa) = banco_com_empresa().await;
        banco
            .importar_certificado(&cofre, empresa.id, PFX, "1234")
            .await
            .unwrap();
        let (outro_cofre, _) = crate::cofre::testes::cofre_rapido("outra-senha");
        assert!(matches!(
            banco.certificado_ativo(&outro_cofre, empresa.id).await,
            Err(ErroBanco::Cofre(_))
        ));
    }
}
