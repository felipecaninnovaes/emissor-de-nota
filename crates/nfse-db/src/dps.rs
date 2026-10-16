//! Numeração e ciclo de vida da DPS.
//!
//! ```text
//! reservada ──envio──▶ enviando ──▶ autorizada ──▶ cancelada / substituida
//!     ▲                   │  └────▶ rejeitada ──(corrigir e reenviar)──▶ enviando
//!     │                   └───────▶ incerta (timeout)
//!     └──(consulta: Sefin não tem a NFS-e)── incerta ──(consulta: tem)──▶ autorizada
//! ```
//!
//! O número é reservado numa única instrução `UPDATE ... RETURNING`, então duas emissões
//! simultâneas nunca recebem o mesmo número. Pular número não é problema: a DPS não tem
//! inutilização. Reenviar uma DPS em situação `incerta` sem reconciliar é bloqueado.

use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use nfse_core::modelo::{Ambiente, VERSAO_LEIAUTE};
use nfse_core::{Dps, DpsId, Valor};

use crate::empresas::{ambiente_de, ambiente_texto};
use crate::{Banco, ErroBanco, Resultado, agora, de_json, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SituacaoDps {
    Reservada,
    Enviando,
    Autorizada,
    Rejeitada,
    Incerta,
    Cancelada,
    Substituida,
}

impl SituacaoDps {
    pub fn texto(self) -> &'static str {
        match self {
            SituacaoDps::Reservada => "reservada",
            SituacaoDps::Enviando => "enviando",
            SituacaoDps::Autorizada => "autorizada",
            SituacaoDps::Rejeitada => "rejeitada",
            SituacaoDps::Incerta => "incerta",
            SituacaoDps::Cancelada => "cancelada",
            SituacaoDps::Substituida => "substituida",
        }
    }

    fn de(s: &str) -> Resultado<Self> {
        Ok(match s {
            "reservada" => SituacaoDps::Reservada,
            "enviando" => SituacaoDps::Enviando,
            "autorizada" => SituacaoDps::Autorizada,
            "rejeitada" => SituacaoDps::Rejeitada,
            "incerta" => SituacaoDps::Incerta,
            "cancelada" => SituacaoDps::Cancelada,
            "substituida" => SituacaoDps::Substituida,
            outro => return Err(ErroBanco::DadoInvalido(format!("situação da DPS {outro:?}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistroDps {
    pub id: i64,
    pub empresa_id: i64,
    pub ambiente: Ambiente,
    pub serie: String,
    pub numero: u64,
    pub id_dps: String,
    pub situacao: SituacaoDps,
    pub xml_assinado: Option<String>,
    pub tentativas: i64,
    pub ultimo_erro: Option<String>,
}

/// Dados da NFS-e autorizada a gravar (o XML é guardado exatamente como veio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfseRecebida {
    pub chave: String,
    pub numero: Option<String>,
    pub xml: String,
    pub valor_servico: Valor,
    pub competencia: chrono::NaiveDate,
}

/// Linha da lista de notas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoNota {
    pub dps_id: i64,
    pub serie: String,
    pub numero_dps: u64,
    pub situacao: SituacaoDps,
    pub ultimo_erro: Option<String>,
    pub criada_em: String,
    pub nfse_id: Option<i64>,
    pub chave: Option<String>,
    pub numero_nfse: Option<String>,
    pub valor: Option<Valor>,
    pub competencia: Option<String>,
    pub tomador: Option<String>,
    pub descricao: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfseGravada {
    pub id: i64,
    pub dps_id: i64,
    pub chave: String,
    pub numero: Option<String>,
    pub xml: String,
    pub xml_sha256: String,
}

const SELECT_DPS: &str =
    "SELECT id, empresa_id, ambiente, serie, numero, id_dps, situacao, xml_assinado, tentativas, ultimo_erro FROM dps";

fn dps_de(r: &SqliteRow) -> Resultado<RegistroDps> {
    Ok(RegistroDps {
        id: r.get(0),
        empresa_id: r.get(1),
        ambiente: ambiente_de(r.get(2))?,
        serie: r.get(3),
        numero: r.get::<i64, _>(4) as u64,
        id_dps: r.get(5),
        situacao: SituacaoDps::de(r.get(6))?,
        xml_assinado: r.get(7),
        tentativas: r.get(8),
        ultimo_erro: r.get(9),
    })
}

pub fn sha256_hex(dados: &[u8]) -> String {
    hex::encode(Sha256::digest(dados))
}

impl Banco {
    /// Reserva o próximo número da série da empresa no ambiente configurado e cria a DPS.
    pub async fn reservar_dps(&self, empresa_id: i64) -> Resultado<RegistroDps> {
        let empresa = self.empresa(empresa_id).await?;
        let ambiente = ambiente_texto(empresa.ambiente);
        let mut tx = self.pool().begin().await?;
        sqlx::query("INSERT INTO numeracao (empresa_id, ambiente, serie, proximo_numero) VALUES (?, ?, ?, 1) ON CONFLICT DO NOTHING")
            .bind(empresa_id)
            .bind(ambiente)
            .bind(&empresa.serie)
            .execute(&mut *tx)
            .await?;
        let numero: i64 = sqlx::query(
            "UPDATE numeracao SET proximo_numero = proximo_numero + 1
             WHERE empresa_id = ? AND ambiente = ? AND serie = ? RETURNING proximo_numero - 1",
        )
        .bind(empresa_id)
        .bind(ambiente)
        .bind(&empresa.serie)
        .fetch_one(&mut *tx)
        .await?
        .get(0);
        let inscricao = nfse_core::Inscricao::Cnpj(empresa.cnpj.clone());
        let id_dps = DpsId::novo(&empresa.municipio_emissor, &inscricao, &empresa.serie, numero as u64)
            .map_err(|e| ErroBanco::Regra(e.to_string()))?;
        let agora = agora();
        let id: i64 = sqlx::query(
            "INSERT INTO dps (empresa_id, ambiente, serie, numero, id_dps, situacao, versao_leiaute, criado_em, atualizado_em)
             VALUES (?, ?, ?, ?, ?, 'reservada', ?, ?, ?) RETURNING id",
        )
        .bind(empresa_id)
        .bind(ambiente)
        .bind(&empresa.serie)
        .bind(numero)
        .bind(id_dps.as_str())
        .bind(VERSAO_LEIAUTE)
        .bind(&agora)
        .bind(&agora)
        .fetch_one(&mut *tx)
        .await?
        .get(0);
        tx.commit().await?;
        self.dps(id).await
    }

    /// Próximo número que será reservado na série (1 se a série ainda não foi usada).
    pub async fn proximo_numero(&self, empresa_id: i64, ambiente: Ambiente, serie: &str) -> Resultado<u64> {
        let proximo: Option<i64> =
            sqlx::query("SELECT proximo_numero FROM numeracao WHERE empresa_id = ? AND ambiente = ? AND serie = ?")
                .bind(empresa_id)
                .bind(ambiente_texto(ambiente))
                .bind(serie)
                .fetch_optional(self.pool())
                .await?
                .map(|r| r.get(0));
        Ok(proximo.unwrap_or(1) as u64)
    }

    /// Ajusta o próximo número (ex.: continuar de onde outro emissor parou na mesma série).
    /// Não deixa voltar para um número já usado por este sistema.
    pub async fn definir_proximo_numero(
        &self,
        empresa_id: i64,
        ambiente: Ambiente,
        serie: &str,
        proximo: u64,
    ) -> Resultado<()> {
        let ambiente = ambiente_texto(ambiente);
        let maior: Option<i64> =
            sqlx::query("SELECT MAX(numero) FROM dps WHERE empresa_id = ? AND ambiente = ? AND serie = ?")
                .bind(empresa_id)
                .bind(ambiente)
                .bind(serie)
                .fetch_one(self.pool())
                .await?
                .get(0);
        if let Some(maior) = maior.filter(|m| proximo as i64 <= *m) {
            return Err(ErroBanco::Regra(format!(
                "o número {maior} da série {serie} já foi usado; o próximo deve ser maior"
            )));
        }
        if proximo == 0 || proximo > 999_999_999_999_999 {
            return Err(ErroBanco::Regra("número da DPS fora da faixa".into()));
        }
        sqlx::query(
            "INSERT INTO numeracao (empresa_id, ambiente, serie, proximo_numero) VALUES (?, ?, ?, ?)
             ON CONFLICT (empresa_id, ambiente, serie) DO UPDATE SET proximo_numero = excluded.proximo_numero",
        )
        .bind(empresa_id)
        .bind(ambiente)
        .bind(serie)
        .bind(proximo as i64)
        .execute(self.pool())
        .await?;
        self.auditar(
            "numeracao_ajustada",
            Some(empresa_id),
            Some(&format!("série {serie}: próximo {proximo}")),
        )
        .await
    }

    pub async fn dps(&self, id: i64) -> Resultado<RegistroDps> {
        let linha = sqlx::query(&format!("{SELECT_DPS} WHERE id = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("DPS"))?;
        dps_de(&linha)
    }

    /// Modelo da DPS como foi enviado (`None` se nunca chegou a ser enviada).
    pub async fn modelo_dps(&self, id: i64) -> Resultado<Option<Dps>> {
        let json: Option<String> = sqlx::query("SELECT modelo_json FROM dps WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("DPS"))?
            .get(0);
        json.map(|j| de_json(&j, "modelo da DPS")).transpose()
    }

    /// DPS que ficaram sem desfecho (queda durante o envio ou timeout): reconciliar ao iniciar.
    pub async fn dps_pendentes(&self) -> Resultado<Vec<RegistroDps>> {
        sqlx::query(&format!(
            "{SELECT_DPS} WHERE situacao IN ('enviando', 'incerta') ORDER BY id"
        ))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(dps_de)
        .collect()
    }

    async fn mudar_situacao(
        &self,
        dps_id: i64,
        de: &[SituacaoDps],
        para: SituacaoDps,
        erro: Option<&str>,
    ) -> Resultado<()> {
        let lista = de
            .iter()
            .map(|s| format!("'{}'", s.texto()))
            .collect::<Vec<_>>()
            .join(",");
        let afetadas = sqlx::query(&format!(
            "UPDATE dps SET situacao = ?, ultimo_erro = COALESCE(?, ultimo_erro), atualizado_em = ?
             WHERE id = ? AND situacao IN ({lista})"
        ))
        .bind(para.texto())
        .bind(erro)
        .bind(agora())
        .bind(dps_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        if afetadas == 0 {
            let atual = self.dps(dps_id).await?.situacao;
            return Err(ErroBanco::Regra(format!(
                "DPS em situação \"{}\" não pode passar para \"{}\"",
                atual.texto(),
                para.texto()
            )));
        }
        Ok(())
    }

    /// Grava o XML assinado e marca como `enviando` antes de chamar a Sefin. Se o programa cair
    /// depois daqui, a DPS aparece em [`Banco::dps_pendentes`] e é reconciliada.
    pub async fn registrar_envio(&self, dps_id: i64, modelo: &Dps, xml_assinado: &str) -> Resultado<()> {
        let registro = self.dps(dps_id).await?;
        if modelo.id().ok().as_ref().map(DpsId::as_str) != Some(registro.id_dps.as_str()) {
            return Err(ErroBanco::Regra(
                "o modelo não corresponde ao Id reservado da DPS".into(),
            ));
        }
        let afetadas = sqlx::query(
            "UPDATE dps SET situacao = 'enviando', modelo_json = ?, xml_assinado = ?, tentativas = tentativas + 1,
                 atualizado_em = ?
             WHERE id = ? AND situacao IN ('reservada', 'rejeitada')",
        )
        .bind(json(modelo))
        .bind(xml_assinado)
        .bind(agora())
        .bind(dps_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        if afetadas == 0 {
            return Err(ErroBanco::Regra(format!(
                "DPS em situação \"{}\" não pode ser enviada{}",
                registro.situacao.texto(),
                if registro.situacao == SituacaoDps::Incerta {
                    " antes da reconciliação"
                } else {
                    ""
                }
            )));
        }
        Ok(())
    }

    pub async fn registrar_rejeicao(&self, dps_id: i64, erro: &str) -> Resultado<()> {
        self.mudar_situacao(dps_id, &[SituacaoDps::Enviando], SituacaoDps::Rejeitada, Some(erro))
            .await
    }

    pub async fn registrar_incerta(&self, dps_id: i64, erro: &str) -> Resultado<()> {
        self.mudar_situacao(
            dps_id,
            &[SituacaoDps::Enviando, SituacaoDps::Incerta],
            SituacaoDps::Incerta,
            Some(erro),
        )
        .await
    }

    /// Reconciliação concluiu que a Sefin não gerou NFS-e: a DPS volta a poder ser enviada.
    pub async fn liberar_para_reenvio(&self, dps_id: i64) -> Resultado<()> {
        self.mudar_situacao(
            dps_id,
            &[SituacaoDps::Enviando, SituacaoDps::Incerta],
            SituacaoDps::Rejeitada,
            None,
        )
        .await
    }

    /// Grava a NFS-e e marca a DPS como autorizada, na mesma transação.
    pub async fn registrar_autorizacao(&self, dps_id: i64, nfse: &NfseRecebida) -> Resultado<NfseGravada> {
        let mut tx = self.pool().begin().await?;
        let agora = agora();
        let afetadas = sqlx::query(
            "UPDATE dps SET situacao = 'autorizada', ultimo_erro = NULL, atualizado_em = ?
             WHERE id = ? AND situacao IN ('enviando', 'incerta')",
        )
        .bind(&agora)
        .bind(dps_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if afetadas == 0 {
            return Err(ErroBanco::Regra("a DPS não estava aguardando autorização".into()));
        }
        let sha = sha256_hex(nfse.xml.as_bytes());
        let id: i64 = sqlx::query(
            "INSERT INTO nfse (dps_id, chave, numero, xml, xml_sha256, valor_servico, competencia, autorizada_em)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(dps_id)
        .bind(&nfse.chave)
        .bind(&nfse.numero)
        .bind(&nfse.xml)
        .bind(&sha)
        .bind(nfse.valor_servico.centesimos())
        .bind(nfse.competencia.format("%Y-%m-%d").to_string())
        .bind(&agora)
        .fetch_one(&mut *tx)
        .await?
        .get(0);
        tx.commit().await?;
        let empresa_id = self.dps(dps_id).await?.empresa_id;
        self.auditar("nfse_autorizada", Some(empresa_id), Some(&nfse.chave))
            .await?;
        self.nfse(id).await
    }

    pub async fn nfse(&self, id: i64) -> Resultado<NfseGravada> {
        let r = sqlx::query("SELECT id, dps_id, chave, numero, xml, xml_sha256 FROM nfse WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ErroBanco::NaoEncontrado("NFS-e"))?;
        let gravada = NfseGravada {
            id: r.get(0),
            dps_id: r.get(1),
            chave: r.get(2),
            numero: r.get(3),
            xml: r.get(4),
            xml_sha256: r.get(5),
        };
        // Integridade: o documento fiscal não pode ter sido alterado no banco.
        if sha256_hex(gravada.xml.as_bytes()) != gravada.xml_sha256 {
            return Err(ErroBanco::DadoInvalido(format!(
                "XML da NFS-e {} alterado (hash não confere)",
                gravada.chave
            )));
        }
        Ok(gravada)
    }

    /// Notas da empresa (DPS e, quando autorizada, a NFS-e), mais recentes primeiro.
    /// `mes` no formato `AAAA-MM` filtra pela competência (ou pela data de criação da DPS sem nota).
    pub async fn listar_notas(&self, empresa_id: i64, mes: Option<&str>) -> Resultado<Vec<ResumoNota>> {
        let linhas = sqlx::query(
            "SELECT d.id, d.serie, d.numero, d.situacao, d.ultimo_erro, d.modelo_json, d.criado_em,
                    n.id, n.chave, n.numero, n.valor_servico, n.competencia
             FROM dps d LEFT JOIN nfse n ON n.dps_id = d.id
             WHERE d.empresa_id = ? AND d.situacao <> 'reservada'
               AND (? IS NULL OR COALESCE(n.competencia, substr(d.criado_em, 1, 10)) LIKE ? || '%')
             ORDER BY d.id DESC LIMIT 500",
        )
        .bind(empresa_id)
        .bind(mes)
        .bind(mes)
        .fetch_all(self.pool())
        .await?;
        linhas
            .iter()
            .map(|r| {
                let modelo: Option<Dps> = r
                    .get::<Option<String>, _>(5)
                    .and_then(|j| serde_json::from_str(&j).ok());
                Ok(ResumoNota {
                    dps_id: r.get(0),
                    serie: r.get(1),
                    numero_dps: r.get::<i64, _>(2) as u64,
                    situacao: SituacaoDps::de(r.get(3))?,
                    ultimo_erro: r.get(4),
                    criada_em: r.get(6),
                    nfse_id: r.get(7),
                    chave: r.get(8),
                    numero_nfse: r.get(9),
                    valor: r
                        .get::<Option<i64>, _>(10)
                        .map(Valor::de_centesimos)
                        .or_else(|| modelo.as_ref().map(|m| m.valores.servico)),
                    competencia: r.get(11),
                    tomador: modelo.as_ref().and_then(|m| m.tomador.as_ref()).map(|t| t.nome.clone()),
                    descricao: modelo.map(|m| m.servico.descricao),
                })
            })
            .collect()
    }

    /// Registra o evento de cancelamento homologado e marca a DPS como cancelada.
    pub async fn registrar_cancelamento(&self, nfse_id: i64, xml_evento: &str) -> Resultado<()> {
        let nfse = self.nfse(nfse_id).await?;
        self.mudar_situacao(nfse.dps_id, &[SituacaoDps::Autorizada], SituacaoDps::Cancelada, None)
            .await?;
        sqlx::query("INSERT INTO evento (nfse_id, tipo, xml, criado_em) VALUES (?, 'cancelamento', ?, ?)")
            .bind(nfse_id)
            .bind(xml_evento)
            .bind(agora())
            .execute(self.pool())
            .await?;
        let empresa_id = self.dps(nfse.dps_id).await?.empresa_id;
        self.auditar("nfse_cancelada", Some(empresa_id), Some(&nfse.chave))
            .await
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::empresas::testes::banco_com_empresa;

    fn modelo_para(registro: &RegistroDps) -> Dps {
        let mut dps: Dps = serde_json::from_str(include_str!("../../../exemplos/dps-exemplo.json")).unwrap();
        dps.serie = registro.serie.clone();
        dps.numero = registro.numero;
        dps
    }

    fn nfse_recebida() -> NfseRecebida {
        NfseRecebida {
            chave: "35111022211222333000181000000000000126101234567890".into(),
            numero: Some("1".into()),
            xml: "<NFSe>...</NFSe>".into(),
            valor_servico: Valor::parse("1500").unwrap(),
            competencia: chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
        }
    }

    #[tokio::test]
    async fn numeracao_sequencial_sem_repetir() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let mut tarefas = Vec::new();
        for _ in 0..20 {
            let b = banco.clone();
            tarefas.push(tokio::spawn(
                async move { b.reservar_dps(empresa.id).await.unwrap().numero },
            ));
        }
        let mut numeros = Vec::new();
        for t in tarefas {
            numeros.push(t.await.unwrap());
        }
        numeros.sort();
        assert_eq!(numeros, (1..=20).collect::<Vec<_>>());
        let primeira = banco.dps(1).await.unwrap();
        assert_eq!(primeira.id_dps.len(), 45);
        assert_eq!(primeira.situacao, SituacaoDps::Reservada);
    }

    #[tokio::test]
    async fn mesma_serie_e_numero_em_cada_ambiente() {
        // Tentativa em homologação e depois a empresa passa para produção: a numeração de
        // produção começa do 1 e gera o mesmo Id de DPS, o que é válido (Sefins separadas).
        let (banco, _, empresa) = banco_com_empresa().await;
        let homologacao = banco.reservar_dps(empresa.id).await.unwrap();
        let mut dados = crate::empresas::testes::nova_empresa();
        dados.ambiente = Ambiente::Producao;
        banco.atualizar_empresa(empresa.id, &dados).await.unwrap();
        let producao = banco.reservar_dps(empresa.id).await.unwrap();
        assert_eq!(producao.numero, homologacao.numero);
        assert_eq!(producao.id_dps, homologacao.id_dps);
        assert_eq!(producao.ambiente, Ambiente::Producao);
        // No mesmo ambiente continua sem repetir.
        assert_eq!(banco.reservar_dps(empresa.id).await.unwrap().numero, 2);
    }

    #[tokio::test]
    async fn exclusao_de_empresa_depende_do_historico() {
        use crate::empresas::Exclusao;
        use crate::empresas::testes::nova_empresa;

        // Sem nota: sai tudo (inclusive DPS só reservada e serviços).
        let (banco, cofre, empresa) = banco_com_empresa().await;
        banco
            .importar_certificado(&cofre, empresa.id, crate::empresas::testes::PFX, "1234")
            .await
            .unwrap();
        banco.reservar_dps(empresa.id).await.unwrap();
        assert_eq!(banco.excluir_empresa(empresa.id).await.unwrap(), Exclusao::Excluida);
        assert!(banco.empresas().await.unwrap().is_empty());
        let certificados: i64 = sqlx::query("SELECT COUNT(*) FROM certificado")
            .fetch_one(banco.pool())
            .await
            .unwrap()
            .get(0);
        assert_eq!(certificados, 0);

        // Envio sem resposta: recusa.
        let empresa = banco.cadastrar_empresa(&nova_empresa()).await.unwrap();
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        banco.registrar_incerta(r.id, "timeout").await.unwrap();
        assert!(matches!(
            banco.excluir_empresa(empresa.id).await,
            Err(ErroBanco::Regra(_))
        ));
        // Também não troca de ambiente enquanto o envio não for resolvido.
        let mut producao = nova_empresa();
        producao.ambiente = Ambiente::Producao;
        assert!(matches!(
            banco.atualizar_empresa(empresa.id, &producao).await,
            Err(ErroBanco::Regra(m)) if m.contains("sem resposta")
        ));

        // Com nota autorizada: só arquiva, e recadastrar o CNPJ traz a empresa de volta.
        banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        assert_eq!(
            banco.excluir_empresa(empresa.id).await.unwrap(),
            Exclusao::Arquivada { notas: 1 }
        );
        assert!(banco.empresas().await.unwrap().is_empty());
        let de_volta = banco.cadastrar_empresa(&nova_empresa()).await.unwrap();
        assert_eq!(de_volta.id, empresa.id);
        assert_eq!(banco.listar_notas(empresa.id, None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn proximo_numero_nao_volta() {
        let (banco, _, empresa) = banco_com_empresa().await;
        banco
            .definir_proximo_numero(empresa.id, Ambiente::Homologacao, "1", 50)
            .await
            .unwrap();
        assert_eq!(banco.reservar_dps(empresa.id).await.unwrap().numero, 50);
        assert!(matches!(
            banco
                .definir_proximo_numero(empresa.id, Ambiente::Homologacao, "1", 50)
                .await,
            Err(ErroBanco::Regra(_))
        ));
        banco
            .definir_proximo_numero(empresa.id, Ambiente::Homologacao, "1", 51)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn ciclo_feliz_ate_cancelamento() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        assert_eq!(
            banco.dps_pendentes().await.unwrap().len(),
            1,
            "enviando conta como pendente"
        );

        let nfse = banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        assert_eq!(nfse.xml_sha256, sha256_hex(b"<NFSe>...</NFSe>"));
        assert_eq!(banco.dps(r.id).await.unwrap().situacao, SituacaoDps::Autorizada);
        assert!(banco.dps_pendentes().await.unwrap().is_empty());

        banco.registrar_cancelamento(nfse.id, "<evento/>").await.unwrap();
        assert_eq!(banco.dps(r.id).await.unwrap().situacao, SituacaoDps::Cancelada);
        assert!(banco.registrar_cancelamento(nfse.id, "<evento/>").await.is_err());
    }

    #[tokio::test]
    async fn incerta_bloqueia_reenvio_ate_reconciliar() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        let modelo = modelo_para(&r);
        banco.registrar_envio(r.id, &modelo, "<DPS/>").await.unwrap();
        banco.registrar_incerta(r.id, "timeout").await.unwrap();

        let erro = banco.registrar_envio(r.id, &modelo, "<DPS/>").await.unwrap_err();
        assert!(matches!(erro, ErroBanco::Regra(m) if m.contains("reconciliação")));

        // Consulta disse que não há NFS-e: libera; reenvio conta nova tentativa.
        banco.liberar_para_reenvio(r.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo, "<DPS/>").await.unwrap();
        assert_eq!(banco.dps(r.id).await.unwrap().tentativas, 2);
    }

    #[tokio::test]
    async fn incerta_pode_virar_autorizada() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        banco.registrar_incerta(r.id, "timeout").await.unwrap();
        banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        assert_eq!(banco.dps(r.id).await.unwrap().situacao, SituacaoDps::Autorizada);
    }

    #[tokio::test]
    async fn rejeitada_pode_ser_corrigida_e_reenviada() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        banco.registrar_rejeicao(r.id, "E0120 IM").await.unwrap();
        let rejeitada = banco.dps(r.id).await.unwrap();
        assert_eq!(rejeitada.ultimo_erro.as_deref(), Some("E0120 IM"));
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();

        let mut outro = modelo_para(&r);
        outro.numero += 1;
        banco.registrar_rejeicao(r.id, "x").await.unwrap();
        assert!(
            banco.registrar_envio(r.id, &outro, "<DPS/>").await.is_err(),
            "Id diferente do reservado"
        );
    }

    #[tokio::test]
    async fn lista_notas_por_mes() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        let r2 = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r2.id, &modelo_para(&r2), "<DPS/>").await.unwrap();
        banco.registrar_rejeicao(r2.id, "E0120").await.unwrap();
        banco.reservar_dps(empresa.id).await.unwrap(); // só reservada: não aparece

        let todas = banco.listar_notas(empresa.id, None).await.unwrap();
        assert_eq!(todas.len(), 2);
        assert_eq!(todas[0].situacao, SituacaoDps::Rejeitada);
        assert_eq!(todas[1].numero_nfse.as_deref(), Some("1"));
        assert_eq!(todas[1].tomador.as_deref(), Some("CLINICA EXEMPLO LTDA"));
        let mes_atual = chrono::Utc::now().format("%Y-%m").to_string();
        // A autorizada é de 2026-10 (competência); a rejeitada conta pelo dia em que foi criada.
        assert_eq!(
            banco.listar_notas(empresa.id, Some("2026-10")).await.unwrap().len(),
            if mes_atual == "2026-10" { 2 } else { 1 }
        );
        assert!(
            banco
                .listar_notas(empresa.id, Some("2020-01"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn detecta_xml_alterado_no_banco() {
        let (banco, _, empresa) = banco_com_empresa().await;
        let r = banco.reservar_dps(empresa.id).await.unwrap();
        banco.registrar_envio(r.id, &modelo_para(&r), "<DPS/>").await.unwrap();
        let nfse = banco.registrar_autorizacao(r.id, &nfse_recebida()).await.unwrap();
        sqlx::query("UPDATE nfse SET xml = '<NFSe>alterado</NFSe>'")
            .execute(banco.pool())
            .await
            .unwrap();
        assert!(matches!(banco.nfse(nfse.id).await, Err(ErroBanco::DadoInvalido(_))));
    }
}
