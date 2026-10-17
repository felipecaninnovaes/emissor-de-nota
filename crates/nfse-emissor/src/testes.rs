use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use nfse_core::Cnpj;
use nfse_core::Percentual;
use nfse_core::modelo::{ApuracaoSimples, EnderecoNacional, PisCofins, RegimeTributario, SituacaoSimples, Tomador};
use nfse_core::modelo::{TotalTributos, TributosFederais};
use nfse_db::empresas::NovaEmpresa;
use nfse_db::servicos::Tributacao;
use nfse_nacional::NfseAutorizada;

use super::*;

const PFX: &[u8] = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
const CHAVE: &str = "35111022211222333000181000000000000126101234567890";

enum Resposta {
    Emitir(Result<NfseAutorizada, ErroSefin>),
    ConsultarDps(Result<Option<String>, ErroSefin>),
    ConsultarNfse(Result<NfseAutorizada, ErroSefin>),
    Evento(Result<String, ErroSefin>),
}

/// Sefin que responde na ordem de um roteiro e guarda o que recebeu.
#[derive(Clone, Default)]
struct SefinSimulada {
    roteiro: Arc<Mutex<VecDeque<Resposta>>>,
    recebidos: Arc<Mutex<Vec<String>>>,
    ambientes: Arc<Mutex<Vec<Ambiente>>>,
}

impl SefinSimulada {
    fn com(respostas: Vec<Resposta>) -> Self {
        let s = Self::default();
        s.roteiro.lock().unwrap().extend(respostas);
        s
    }

    fn proxima(&self, recebido: &str) -> Resposta {
        self.recebidos.lock().unwrap().push(recebido.to_owned());
        self.roteiro
            .lock()
            .unwrap()
            .pop_front()
            .expect("chamada à Sefin fora do roteiro")
    }

    fn recebidos(&self) -> Vec<String> {
        self.recebidos.lock().unwrap().clone()
    }

    fn roteiro_vazio(&self) -> bool {
        self.roteiro.lock().unwrap().is_empty()
    }
}

impl Sefin for SefinSimulada {
    async fn emitir(&self, dps: &str) -> Result<NfseAutorizada, ErroSefin> {
        match self.proxima(dps) {
            Resposta::Emitir(r) => r,
            _ => panic!("esperava emitir"),
        }
    }

    async fn consultar_dps(&self, id: &str) -> Result<Option<String>, ErroSefin> {
        match self.proxima(id) {
            Resposta::ConsultarDps(r) => r,
            _ => panic!("esperava consultar_dps"),
        }
    }

    async fn consultar_nfse(&self, chave: &str) -> Result<NfseAutorizada, ErroSefin> {
        match self.proxima(chave) {
            Resposta::ConsultarNfse(r) => r,
            _ => panic!("esperava consultar_nfse"),
        }
    }

    async fn registrar_evento(&self, _chave: &str, pedido: &str) -> Result<String, ErroSefin> {
        match self.proxima(pedido) {
            Resposta::Evento(r) => r,
            _ => panic!("esperava registrar_evento"),
        }
    }
}

impl Conector for SefinSimulada {
    type S = SefinSimulada;

    fn conectar(&self, _: &CertificadoA1, ambiente: Ambiente) -> Result<SefinSimulada, ErroSefin> {
        self.ambientes.lock().unwrap().push(ambiente);
        Ok(self.clone())
    }
}

fn nfse_autorizada() -> NfseAutorizada {
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?><NFSe versao="1.01" xmlns="http://www.sped.fazenda.gov.br/nfse">"#,
            r#"<infNFSe Id="NFS{}"><nNFSe>1</nNFSe><cStat>100</cStat><DPS versao="1.01"><infDPS Id="X">"#,
            "<dCompet>2026-10-16</dCompet><valores><vServPrest><vServ>850.00</vServ></vServPrest></valores>",
            "</infDPS></DPS></infNFSe></NFSe>"
        ),
        CHAVE
    );
    NfseAutorizada {
        id_dps: None,
        chave_acesso: CHAVE.into(),
        xml,
        alertas: vec![],
    }
}

fn rejeicao(codigo: &str) -> ErroSefin {
    ErroSefin::Rejeitada {
        status: 400,
        erros: vec![MensagemSefin {
            codigo: Some(codigo.into()),
            descricao: Some("descrição da Sefin".into()),
            complemento: None,
        }],
        corpo: String::new(),
    }
}

struct Cenario {
    emissor: Emissor<SefinSimulada>,
    sefin: SefinSimulada,
    pedido: PedidoEmissao,
}

async fn cenario(respostas: Vec<Resposta>) -> Cenario {
    let banco = Banco::em_memoria().await.unwrap();
    let cofre = banco.criar_cofre_rapido("senha-mestra").await.unwrap();
    let empresa = banco
        .cadastrar_empresa(&NovaEmpresa {
            cnpj: Cnpj::parse("11222333000181").unwrap(),
            nome: "EMPRESA TESTE LTDA".into(),
            inscricao_municipal: Some("12345".into()),
            municipio_emissor: "3511102".into(),
            regime: RegimeTributario {
                simples: SituacaoSimples::MeEpp,
                apuracao_simples: Some(ApuracaoSimples::TudoNoSimples),
                regime_especial: 0,
            },
            ambiente: Ambiente::Producao,
            serie: "1".into(),
            telefone: None,
            email: None,
        })
        .await
        .unwrap();
    banco
        .importar_certificado(&cofre, empresa.id, PFX, "1234")
        .await
        .unwrap();
    let tomador = banco
        .salvar_tomador(
            empresa.id,
            &Tomador {
                documento: Documento::Cnpj(Cnpj::parse("12ABC34501DE35").unwrap()),
                inscricao_municipal: None,
                nome: "CLIENTE EXEMPLO LTDA".into(),
                endereco: Some(EnderecoNacional {
                    municipio: "3511102".into(),
                    cep: "15800000".into(),
                    logradouro: "RUA EXEMPLO".into(),
                    numero: "632".into(),
                    complemento: None,
                    bairro: "CENTRO".into(),
                }),
                telefone: None,
                email: None,
            },
            "manual",
        )
        .await
        .unwrap();
    let servico_id = banco
        .salvar_servico(&Servico {
            id: None,
            empresa_id: empresa.id,
            nome: "Honorários".into(),
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
            valor_sugerido: None,
        })
        .await
        .unwrap();
    let sefin = SefinSimulada::com(respostas);
    Cenario {
        emissor: Emissor::novo(banco, cofre, sefin.clone()),
        sefin,
        pedido: PedidoEmissao {
            empresa_id: empresa.id,
            tomador: Some(tomador),
            servico_id,
            valor: Valor::parse("850").unwrap(),
            descricao: None,
            mes_referencia: Some(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()),
            informacoes_complementares: None,
            origem: Origem::Manual,
        },
    }
}

#[test]
fn mes_ref_e_mes_anterior() {
    let d = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    assert_eq!(aplicar_mes_ref("REF {mes_ref}", mes_anterior(d)), "REF 12/2025");
    assert_eq!(aplicar_mes_ref("sem variável", d), "sem variável");
}

#[test]
fn retencoes_percentuais_viram_valores() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: p("1.5"),
            csll: p("1"),
            cp: None,
            pis: p("0.65"),
            cofins: p("3"),
        },
        ..Default::default()
    };
    let f = federais_da_nota(&t, Valor::parse("1234.56").unwrap());
    assert_eq!(f.retencao_irrf.unwrap().to_string(), "18.52");
    assert_eq!(f.retencao_csll.unwrap().to_string(), "12.35");
    assert_eq!(f.retencao_cp, None);
    let pc = f.pis_cofins.unwrap();
    assert_eq!(pc.base_calculo.unwrap().to_string(), "1234.56");
    assert_eq!(pc.valor_pis.unwrap().to_string(), "8.02");
    assert_eq!(pc.valor_cofins.unwrap().to_string(), "37.04");
}

#[tokio::test]
async fn emite_e_grava_a_nota() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!("esperava autorizada")
    };
    assert_eq!(nfse.chave, CHAVE);
    assert_eq!(nfse.numero.as_deref(), Some("1"));
    assert_eq!(
        c.emissor.banco().dps(nfse.dps_id).await.unwrap().situacao,
        SituacaoDps::Autorizada
    );

    let enviado = &c.sefin.recebidos()[0];
    nfse_xml::verificar(enviado).expect("DPS enviada assinada");
    assert!(enviado.contains("<xDescServ>SERVIÇOS CONTÁBEIS REFERENTE A 09/2026</xDescServ>"));
    assert!(enviado.contains("<IM>12345</IM>"), "IM vai em Produção por padrão");
    let hoje = agora_brasilia()
        .date_naive()
        .format("<dCompet>%Y-%m-%d</dCompet>")
        .to_string();
    assert!(enviado.contains(&hoje), "competência é a data da emissão");
}

#[tokio::test]
async fn rejeicao_traz_orientacao_e_nao_bloqueia() {
    let c = cenario(vec![Resposta::Emitir(Err(rejeicao("E0120")))]).await;
    let erro = c.emissor.emitir(&c.pedido).await.unwrap_err();
    let ErroEmissao::Rejeitada {
        dps_id: Some(dps_id),
        mensagem,
    } = erro
    else {
        panic!("{erro:?}")
    };
    assert!(
        mensagem.starts_with("E0120 descrição da Sefin → A inscrição municipal"),
        "{mensagem}"
    );
    let dps = c.emissor.banco().dps(dps_id).await.unwrap();
    assert_eq!(dps.situacao, SituacaoDps::Rejeitada);
    assert!(c.emissor.banco().dps_pendentes().await.unwrap().is_empty());

    // Refazer traz os dados de volta para o formulário, com o mês já aplicado na descrição.
    let r = c.emissor.dados_para_refazer(dps_id).await.unwrap();
    assert_eq!(r.empresa_id, c.pedido.empresa_id);
    assert_eq!(r.tomador.map(|t| t.pessoa_id), c.pedido.tomador);
    assert_eq!(r.servico_id, Some(c.pedido.servico_id));
    assert_eq!(r.valor, c.pedido.valor);
    assert_eq!(r.descricao, "SERVIÇOS CONTÁBEIS REFERENTE A 09/2026");
}

#[tokio::test]
async fn acha_na_sefin_o_primeiro_numero_livre() {
    // Outro emissor já usou a série 1 do nº 1 ao 5. Consultas: 1, 2, 3, 5, 9 (livre), 7, 6.
    let usado = |sim: bool| Resposta::ConsultarDps(Ok(sim.then(|| CHAVE.to_owned())));
    let c = cenario(vec![
        usado(true),
        usado(true),
        usado(true),
        usado(true),
        usado(false),
        usado(false),
        usado(false),
    ])
    .await;
    let (ultimo, proximo) = c
        .emissor
        .ajustar_numeracao_pela_sefin(c.pedido.empresa_id)
        .await
        .unwrap();
    assert_eq!((ultimo, proximo), (Some(5), 6));
    assert!(c.sefin.roteiro_vazio());
    let numeros: Vec<u64> = c
        .sefin
        .recebidos()
        .iter()
        .map(|id| id[30..].parse().unwrap()) // Id: DPS + município(7) + tipo(1) + CNPJ(14) + série(5) + número(15)
        .collect();
    assert_eq!(numeros, vec![1, 2, 3, 5, 9, 7, 6]);
    let banco = c.emissor.banco();
    assert_eq!(
        banco
            .proximo_numero(c.pedido.empresa_id, Ambiente::Producao, "1")
            .await
            .unwrap(),
        6
    );
}

#[tokio::test]
async fn numero_local_ja_livre_nao_muda_nada() {
    let c = cenario(vec![Resposta::ConsultarDps(Ok(None))]).await;
    let r = c
        .emissor
        .ajustar_numeracao_pela_sefin(c.pedido.empresa_id)
        .await
        .unwrap();
    assert_eq!(r, (None, 1));
}

#[tokio::test]
async fn refazer_so_vale_para_rejeitada() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert!(matches!(
        c.emissor.dados_para_refazer(nfse.dps_id).await,
        Err(ErroEmissao::Regra(_))
    ));
}

#[tokio::test]
async fn dps_invalida_nao_gasta_numero_nem_chama_a_sefin() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let mut invalido = c.pedido.clone();
    invalido.descricao = Some("   ".into());
    assert!(matches!(
        c.emissor.emitir(&invalido).await,
        Err(ErroEmissao::DpsInvalida(_))
    ));
    assert!(c.sefin.recebidos().is_empty());

    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert_eq!(c.emissor.banco().dps(nfse.dps_id).await.unwrap().numero, 1);
}

#[tokio::test]
async fn timeout_reconcilia_sem_reenviar() {
    let c = cenario(vec![
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        Resposta::ConsultarDps(Ok(Some(CHAVE.into()))),
        Resposta::ConsultarNfse(Ok(nfse_autorizada())),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert_eq!(nfse.chave, CHAVE);
    let recebidos = c.sefin.recebidos();
    assert_eq!(recebidos.len(), 3, "um envio, uma consulta da DPS, uma da NFS-e");
    assert!(
        recebidos[1].starts_with("DPS3511102211222333000181"),
        "consulta pelo Id da DPS"
    );
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn timeout_sem_consulta_fica_pendente_ate_reconciliar() {
    let c = cenario(vec![
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        Resposta::ConsultarDps(Err(ErroSefin::SemConexao("rede caiu".into()))),
        // Ao reabrir o sistema:
        Resposta::ConsultarDps(Ok(None)),
    ])
    .await;
    let ResultadoEmissao::Incerta { dps_id, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let banco = c.emissor.banco();
    assert_eq!(banco.dps(dps_id).await.unwrap().situacao, SituacaoDps::Incerta);
    assert_eq!(banco.dps_pendentes().await.unwrap().len(), 1);

    let resultados = c.emissor.reconciliar_pendentes().await.unwrap();
    assert!(matches!(resultados.as_slice(), [(id, Ok(None))] if *id == dps_id));
    assert_eq!(
        banco.dps(dps_id).await.unwrap().situacao,
        SituacaoDps::Rejeitada,
        "liberada para reenvio"
    );
    assert!(banco.dps_pendentes().await.unwrap().is_empty());
}

#[tokio::test]
async fn sem_conexao_nao_emite_e_libera() {
    let c = cenario(vec![Resposta::Emitir(Err(ErroSefin::SemConexao("dns".into())))]).await;
    assert!(matches!(
        c.emissor.emitir(&c.pedido).await,
        Err(ErroEmissao::SemConexao { .. })
    ));
    assert!(c.emissor.banco().dps_pendentes().await.unwrap().is_empty());
}

#[tokio::test]
async fn queda_durante_envio_e_retomada() {
    let c = cenario(vec![
        Resposta::ConsultarDps(Ok(Some(CHAVE.into()))),
        Resposta::ConsultarNfse(Ok(nfse_autorizada())),
    ])
    .await;
    let banco = c.emissor.banco();
    // Simula o programa fechando depois de gravar "enviando" e antes da resposta.
    let reserva = banco.reservar_dps(c.pedido.empresa_id).await.unwrap();
    let mut modelo: Dps = serde_json::from_str(include_str!("../../../exemplos/dps-exemplo.json")).unwrap();
    modelo.numero = reserva.numero;
    banco.registrar_envio(reserva.id, &modelo, "<DPS/>").await.unwrap();

    let resultados = c.emissor.reconciliar_pendentes().await.unwrap();
    assert!(matches!(resultados.as_slice(), [(_, Ok(Some(_)))]));
    assert_eq!(banco.dps(reserva.id).await.unwrap().situacao, SituacaoDps::Autorizada);
}

/// NFS-e fictícia assinada no esquema da Sefin (C14N exclusivo + RSA-SHA256), mesma chave do cenário.
const NFSE_ASSINADA: &str = include_str!("../../nfse-xml/tests/fixtures/nfse-sefin-simulada.xml");

#[tokio::test]
async fn confere_assinatura_e_arquiva_por_empresa_e_mes() {
    let dir = std::env::temp_dir().join(format!("nfse-emissor-pasta-{}", std::process::id()));
    let Cenario { emissor, pedido, .. } = cenario(vec![
        Resposta::Emitir(Ok(NfseAutorizada {
            id_dps: None,
            chave_acesso: CHAVE.into(),
            xml: NFSE_ASSINADA.into(),
            alertas: vec![],
        })),
        Resposta::Evento(Ok("<evento>homologado</evento>".into())),
    ])
    .await;
    let emissor = emissor.com_pasta(&dir);

    let ResultadoEmissao::Autorizada { nfse, avisos } = emissor.emitir(&pedido).await.unwrap() else {
        panic!()
    };
    assert!(avisos.is_empty(), "{avisos:?}");
    let mes = dir.join("EMPRESA TESTE LTDA - 11222333000181").join("2026-10");
    let xml = std::fs::read_to_string(mes.join(format!("NFSe-1-{CHAVE}.xml"))).unwrap();
    assert_eq!(xml, NFSE_ASSINADA, "arquivo idêntico ao devolvido pela Sefin");
    let pdf = mes.join(format!("NFSe-1-{CHAVE}.pdf"));
    assert!(
        std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"),
        "DANFSe ao lado do XML"
    );

    emissor
        .cancelar(
            nfse.id,
            MotivoCancelamento::ErroNaEmissao,
            "Valor informado errado na emissão",
        )
        .await
        .unwrap();
    assert!(mes.join(format!("NFSe-1-{CHAVE}-cancelamento.xml")).exists());
    assert!(
        mes.join(format!("NFSe-1-{CHAVE}.pdf")).exists(),
        "PDF regravado com a marca CANCELADA"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn assinatura_da_sefin_que_nao_confere_vira_aviso() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let ResultadoEmissao::Autorizada { avisos, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert!(avisos[0].contains("assinatura da Sefin"), "{avisos:?}");
    let auditoria = c.emissor.banco().auditoria(5).await.unwrap();
    assert!(auditoria.iter().any(|(_, acao, _, _)| acao == "aviso"));
}

#[tokio::test]
async fn cancela_nota_autorizada() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Evento(Ok("<evento>homologado</evento>".into())),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    // A empresa passa para homologação depois de emitir: o cancelamento continua em Produção.
    let e = c.emissor.banco().empresa(c.pedido.empresa_id).await.unwrap();
    c.emissor
        .banco()
        .atualizar_empresa(
            e.id,
            &NovaEmpresa {
                cnpj: e.cnpj.clone(),
                nome: e.nome.clone(),
                inscricao_municipal: e.inscricao_municipal.clone(),
                municipio_emissor: e.municipio_emissor.clone(),
                regime: e.regime.clone(),
                ambiente: Ambiente::Homologacao,
                serie: e.serie.clone(),
                telefone: e.telefone.clone(),
                email: e.email.clone(),
            },
        )
        .await
        .unwrap();
    c.emissor
        .cancelar(
            nfse.id,
            MotivoCancelamento::ErroNaEmissao,
            "Valor informado errado na emissão",
        )
        .await
        .unwrap();
    let pedido = &c.sefin.recebidos()[1];
    nfse_xml::verificar(pedido).expect("pedido de cancelamento assinado");
    assert!(pedido.contains(&format!(r#"Id="PRE{CHAVE}101101""#)));
    assert!(pedido.contains("<tpAmb>1</tpAmb>"));
    assert_eq!(
        *c.sefin.ambientes.lock().unwrap(),
        vec![Ambiente::Producao, Ambiente::Producao]
    );
    assert_eq!(
        c.emissor.banco().dps(nfse.dps_id).await.unwrap().situacao,
        SituacaoDps::Cancelada
    );

    let erro = c
        .emissor
        .cancelar(nfse.id, MotivoCancelamento::Outros, "Segunda tentativa de cancelar")
        .await;
    assert!(matches!(erro, Err(ErroEmissao::Regra(_))));
}

async fn recorrencia_do_cenario(c: &Cenario) -> i64 {
    c.emissor
        .banco()
        .salvar_recorrencia(&nfse_db::recorrencias::Recorrencia {
            id: None,
            empresa_id: c.pedido.empresa_id,
            tomador: c.pedido.tomador,
            servico_id: c.pedido.servico_id,
            valor: Some(c.pedido.valor),
            descricao: None,
            informacoes_complementares: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn recorrencia_emite_uma_vez_por_mes() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let rec = recorrencia_do_cenario(&c).await;
    let setembro = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();

    let r = c.emissor.emitir_recorrencia(rec, setembro).await.unwrap();
    assert!(matches!(r, ResultadoEmissao::Autorizada { .. }));
    let enviado = &c.sefin.recebidos()[0];
    assert!(enviado.contains("SERVIÇOS CONTÁBEIS REFERENTE A 09/2026"), "{enviado}");

    // Mesmo mês de novo: recusa sem reservar número nem chamar a Sefin.
    let erro = c.emissor.emitir_recorrencia(rec, setembro).await.unwrap_err();
    assert!(
        matches!(&erro, ErroEmissao::Regra(m) if m.contains("autorizada")),
        "{erro:?}"
    );
    assert_eq!(c.sefin.recebidos().len(), 1);
    let banco = c.emissor.banco();
    let lote = banco.recorrencias(None, "2026-09").await.unwrap();
    let t = lote[0].no_mes.as_ref().unwrap();
    assert_eq!(t.situacao, SituacaoDps::Autorizada);
    assert_eq!(t.numero_nfse.as_deref(), Some("1"));
    assert_eq!(banco.recorrencias(None, "2026-10").await.unwrap()[0].no_mes, None);
}

#[tokio::test]
async fn recorrencia_rejeitada_libera_o_mes() {
    let c = cenario(vec![
        Resposta::Emitir(Err(rejeicao("E0712"))),
        Resposta::Emitir(Ok(nfse_autorizada())),
    ])
    .await;
    let rec = recorrencia_do_cenario(&c).await;
    let setembro = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    assert!(matches!(
        c.emissor.emitir_recorrencia(rec, setembro).await,
        Err(ErroEmissao::Rejeitada { .. })
    ));
    assert!(matches!(
        c.emissor.emitir_recorrencia(rec, setembro).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
}

#[tokio::test]
async fn planilha_emite_e_nao_repete_ao_reimportar() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Err(rejeicao("E0116"))),
    ])
    .await;
    // Duas linhas idênticas (duas guias iguais) e uma com tomador desconhecido.
    let csv = "Tomador;Valor;Descricao\r\n\
               12.ABC.345/01DE-35;150,00;CONSULTAS {mes_ref}\r\n\
               12.ABC.345/01DE-35;150,00;CONSULTAS {mes_ref}\r\n\
               529.982.247-25;80,00;CONSULTAS\r\n";
    let linhas = crate::planilha::ler("convenio.csv", csv.as_bytes()).unwrap();
    let setembro = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let banco = c.emissor.banco();
    let itens = importacao::resolver(banco, &linhas, Some(c.pedido.empresa_id), setembro)
        .await
        .unwrap();
    assert_eq!(itens.len(), 3);
    assert!(itens[0].emitivel() && itens[1].emitivel(), "{:?}", itens[0].erros);
    assert_ne!(itens[0].chave, itens[1].chave, "linhas iguais contam como duas notas");
    assert_eq!(itens[0].descricao, "CONSULTAS 09/2026");
    assert_eq!(
        itens[0].servico_id,
        Some(c.pedido.servico_id),
        "empresa com um serviço só"
    );
    assert!(itens[2].erros[0].contains("não cadastrado"), "{:?}", itens[2].erros);
    assert!(c.emissor.emitir_importado(&itens[2], None).await.is_err());

    assert!(matches!(
        c.emissor
            .emitir_importado(&itens[0], Some("convenio.csv"))
            .await
            .unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    assert!(
        c.emissor
            .emitir_importado(&itens[1], Some("convenio.csv"))
            .await
            .is_err()
    );

    // Reimportar a mesma planilha: a primeira já foi emitida, a segunda (rejeitada) pode de novo.
    let de_novo = importacao::resolver(banco, &linhas, Some(c.pedido.empresa_id), setembro)
        .await
        .unwrap();
    assert!(!de_novo[0].emitivel());
    assert_eq!(de_novo[0].anterior.as_ref().unwrap().situacao, SituacaoDps::Autorizada);
    assert!(de_novo[1].emitivel());
    let erro = c.emissor.emitir_importado(&de_novo[0], None).await.unwrap_err();
    assert!(
        matches!(&erro, ErroEmissao::Regra(m) if m.contains("linha da planilha")),
        "{erro:?}"
    );
    assert_eq!(c.sefin.recebidos().len(), 2);
}
