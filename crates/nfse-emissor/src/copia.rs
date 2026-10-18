//! Dados de uma nota já enviada para preencher outra emissão: refazer a rejeitada ou copiar a autorizada.

use nfse_core::Valor;
use nfse_core::modelo::{Documento, Obra};
use nfse_db::dps::SituacaoDps;
use nfse_db::servicos::Servico;
use nfse_db::tomadores::TomadorVinculado;

use crate::{Conector, Emissor, ErroEmissao};

/// Dados de uma DPS rejeitada para preencher uma nova emissão (que recebe o próximo número:
/// pelo Anexo I, só a DPS que gerou NFS-e trava série e número — regra E0014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refazer {
    pub empresa_id: i64,
    /// `None` se não havia tomador ou se ele não está mais vinculado à empresa.
    pub tomador: Option<TomadorVinculado>,
    /// Serviço do cadastro com o mesmo código (e NBS, se houver mais de um); o operador confere.
    pub servico_id: Option<i64>,
    pub valor: Valor,
    /// Descrição como foi enviada (com o mês de referência já aplicado).
    pub descricao: String,
    pub obra: Option<Obra>,
    pub informacoes_complementares: Option<String>,
}

impl<C: Conector> Emissor<C> {
    /// Lê uma DPS rejeitada (pela Sefin ou não enviada por falta de conexão) para emitir de novo.
    pub async fn dados_para_refazer(&self, dps_id: i64) -> Result<Refazer, ErroEmissao> {
        let registro = self.banco.dps(dps_id).await?;
        if registro.situacao != SituacaoDps::Rejeitada {
            return Err(ErroEmissao::Regra(format!(
                "só nota rejeitada pode ser refeita (esta está \"{}\")",
                registro.situacao.texto()
            )));
        }
        self.ler_dados_da_dps(dps_id).await
    }

    /// Lê uma nota autorizada ou cancelada para emitir outra igual. A descrição devolvida é a da nota
    /// antiga (com o mês dela): quem copia deve refazê-la a partir do modelo do serviço.
    pub async fn dados_para_copiar(&self, dps_id: i64) -> Result<Refazer, ErroEmissao> {
        let registro = self.banco.dps(dps_id).await?;
        if !matches!(registro.situacao, SituacaoDps::Autorizada | SituacaoDps::Cancelada) {
            return Err(ErroEmissao::Regra(format!(
                "só nota autorizada ou cancelada pode ser copiada (esta está \"{}\")",
                registro.situacao.texto()
            )));
        }
        self.ler_dados_da_dps(dps_id).await
    }

    async fn ler_dados_da_dps(&self, dps_id: i64) -> Result<Refazer, ErroEmissao> {
        let registro = self.banco.dps(dps_id).await?;
        let modelo = self
            .banco
            .modelo_dps(dps_id)
            .await?
            .ok_or_else(|| ErroEmissao::Regra("a DPS não tem os dados do envio".into()))?;
        let empresa_id = registro.empresa_id;

        let mut tomador = None;
        if let Some(t) = &modelo.tomador {
            let documento = match &t.documento {
                Documento::Cnpj(c) => c.as_str(),
                Documento::Cpf(c) => c.as_str(),
            };
            if let Some(pessoa) = self.banco.pessoa_por_documento(documento).await? {
                tomador = self.banco.tomador(empresa_id, pessoa).await.ok();
            }
        }

        let codigo = &modelo.servico.codigo_tributacao_nacional;
        let candidatos: Vec<Servico> = self
            .banco
            .servicos(empresa_id)
            .await?
            .into_iter()
            .filter(|s| &s.codigo_trib_nac == codigo)
            .collect();
        let servico_id = candidatos
            .iter()
            .find(|s| s.nbs == modelo.servico.nbs)
            .or(candidatos.first())
            .and_then(|s| s.id);

        Ok(Refazer {
            empresa_id,
            tomador,
            servico_id,
            valor: modelo.valores.servico,
            descricao: modelo.servico.descricao,
            obra: modelo.servico.obra,
            informacoes_complementares: modelo.servico.informacoes_complementares,
        })
    }
}
