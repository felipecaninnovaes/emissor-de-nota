use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use nfse_core::Percentual;
use nfse_core::modelo::Documento;
use nfse_core::modelo::{ApuracaoSimples, EnderecoNacional, PisCofins, RegimeTributario, SituacaoSimples, Tomador};
use nfse_core::modelo::{TotalTributos, TributosFederais};
use nfse_core::{Cnpj, Dps};
use nfse_db::dps::SituacaoDps;
use nfse_db::empresas::NovaEmpresa;
use nfse_db::servicos::{Servico, Tributacao};
use nfse_nacional::NfseAutorizada;
use nfse_xml::MotivoCancelamento;

use super::*;
use crate::montagem::{mes_anterior, valor_br, valor_liquido};

const PFX: &[u8] = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
const CHAVE: &str = "35111022211222333000181000000000000126101234567890";

enum Resposta {
    Emitir(Result<NfseAutorizada, ErroSefin>),
    ConsultarDps(Result<Option<String>, ErroSefin>),
    ConsultarNfse(Result<NfseAutorizada, ErroSefin>),
    Evento(Result<String, ErroSefin>),
    ConsultarEvento(Result<Option<String>, ErroSefin>),
}

/// Sefin que responde na ordem de um roteiro e guarda o que recebeu. A NFS-e do roteiro com
/// `<infDPS Id="X">` sai com o Id da última DPS enviada ou consultada, como a da Sefin.
#[derive(Clone, Default)]
struct SefinSimulada {
    roteiro: Arc<Mutex<VecDeque<Resposta>>>,
    recebidos: Arc<Mutex<Vec<String>>>,
    ambientes: Arc<Mutex<Vec<Ambiente>>>,
    ultima_dps: Arc<Mutex<Option<String>>>,
    /// Banco fechado no envio da DPS: simula a falha do banco logo depois de a Sefin responder.
    fechar_no_envio: Arc<Mutex<Option<Banco>>>,
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

    fn com_id_da_dps(&self, r: Result<NfseAutorizada, ErroSefin>) -> Result<NfseAutorizada, ErroSefin> {
        r.map(|mut n| {
            if let Some(id) = self.ultima_dps.lock().unwrap().as_deref() {
                n.xml = n.xml.replace(r#"<infDPS Id="X">"#, &format!(r#"<infDPS Id="{id}">"#));
            }
            n
        })
    }
}

impl Sefin for SefinSimulada {
    async fn emitir(&self, dps: &str) -> Result<NfseAutorizada, ErroSefin> {
        let id = dps
            .split_once(r#"<infDPS Id=""#)
            .and_then(|(_, resto)| resto.split_once('"'))
            .map(|(id, _)| id.to_owned());
        *self.ultima_dps.lock().unwrap() = id;
        let fechar = self.fechar_no_envio.lock().unwrap().take();
        if let Some(banco) = fechar {
            banco.fechar().await;
        }
        match self.proxima(dps) {
            Resposta::Emitir(r) => self.com_id_da_dps(r),
            _ => panic!("esperava emitir"),
        }
    }

    async fn consultar_dps(&self, id: &str) -> Result<Option<String>, ErroSefin> {
        *self.ultima_dps.lock().unwrap() = Some(id.to_owned());
        match self.proxima(id) {
            Resposta::ConsultarDps(r) => r,
            _ => panic!("esperava consultar_dps"),
        }
    }

    async fn consultar_nfse(&self, chave: &str) -> Result<NfseAutorizada, ErroSefin> {
        match self.proxima(chave) {
            Resposta::ConsultarNfse(r) => self.com_id_da_dps(r),
            _ => panic!("esperava consultar_nfse"),
        }
    }

    async fn registrar_evento(&self, _chave: &str, pedido: &str) -> Result<String, ErroSefin> {
        match self.proxima(pedido) {
            Resposta::Evento(r) => r,
            _ => panic!("esperava registrar_evento"),
        }
    }

    async fn consultar_evento(&self, chave: &str, tipo: &str, seq: u32) -> Result<Option<String>, ErroSefin> {
        match self.proxima(&format!("{chave}/{tipo}/{seq}")) {
            Resposta::ConsultarEvento(r) => r,
            _ => panic!("esperava consultar_evento"),
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
    nfse_com_chave(CHAVE, 1)
}

fn nfse_com_chave(chave: &str, numero: u32) -> NfseAutorizada {
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?><NFSe versao="1.01" xmlns="http://www.sped.fazenda.gov.br/nfse">"#,
            r#"<infNFSe Id="NFS{}"><nNFSe>{}</nNFSe><cStat>100</cStat><DPS versao="1.01"><infDPS Id="X">"#,
            "<dCompet>2026-10-16</dCompet><valores><vServPrest><vServ>850.00</vServ></vServPrest></valores>",
            "</infDPS></DPS></infNFSe></NFSe>"
        ),
        chave, numero
    );
    NfseAutorizada {
        id_dps: None,
        chave_acesso: chave.into(),
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
        corpo: format!(r#"{{"erros":[{{"Codigo":"{codigo}"}}]}}"#),
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
            chave_pix: None,
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
            obra: None,
            local_prestacao: None,
            informacoes_complementares: None,
            reter: Default::default(),
            origem: Origem::Manual,
            substituir: None,
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
fn variaveis_do_mes_e_do_valor() {
    let mes = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    assert_eq!(
        aplicar_mes_ref("{mes_ref_extenso} / {ano_ref}", mes),
        "setembro de 2026 / 2026"
    );
    assert_eq!(valor_br(Valor::de_centesimos(125_000)), "1.250,00");
    assert_eq!(valor_br(Valor::de_centesimos(5)), "0,05");
    assert_eq!(valor_br(Valor::de_centesimos(123_456_789)), "1.234.567,89");
    assert_eq!(valor_br(Valor::de_centesimos(-5)), "-0,05");
}

#[test]
fn variavel_desconhecida_e_apontada() {
    assert_eq!(
        variaveis_desconhecidas("REF 09/2026 {tomadr} {tomadr}"),
        vec!["tomadr".to_string()]
    );
    assert!(variaveis_desconhecidas("sem variável { espaço }").is_empty());
}

#[test]
fn retencao_so_vale_para_tomador_que_retem() {
    use nfse_core::modelo::{Documento, RetencaoIssqn, Tomador};
    use nfse_db::servicos::AplicarRetencao;
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let mut servico = Servico {
        id: None,
        empresa_id: 1,
        nome: "x".into(),
        descricao: "x".into(),
        codigo_trib_nac: "171901".into(),
        codigo_trib_mun: None,
        nbs: None,
        tributacao: Tributacao {
            retencoes: nfse_db::servicos::RetencoesPercentuais {
                irrf: p("1.5"),
                aplicar: AplicarRetencao::SoTomadorQueRetem,
                ..Default::default()
            },
            aplicar_retencao_iss: AplicarRetencao::SoTomadorQueRetem,
            ..Default::default()
        },
        valor_sugerido: None,
    };
    servico.tributacao.issqn.retencao = RetencaoIssqn::RetidoPeloTomador;
    let tomador = |federais, iss| TomadorVinculado {
        pessoa_id: 1,
        tomador: Tomador {
            documento: Documento::Cnpj(nfse_core::Cnpj::parse("12ABC34501DE35").unwrap()),
            inscricao_municipal: None,
            nome: "T".into(),
            endereco: None,
            telefone: None,
            email: None,
        },
        emails: None,
        observacao: None,
        retem_federais: federais,
        retem_iss: iss,
        tipo: nfse_db::tomadores::TipoTomador::Privado,
    };
    let nao = tributacao_da_nota(&servico, Some(&tomador(false, false)), ReterNota::default());
    assert_eq!(nao.retencoes.irrf, None);
    assert_eq!(nao.issqn.retencao, RetencaoIssqn::NaoRetido);
    let sim = tributacao_da_nota(&servico, Some(&tomador(true, true)), ReterNota::default());
    assert!(sim.retencoes.irrf.is_some());
    assert_eq!(sim.issqn.retencao, RetencaoIssqn::RetidoPeloTomador);
    // A escolha feita na emissão vale mais que o cadastro.
    let so_iss = ReterNota {
        federais: Some(false),
        iss: Some(true),
        ..Default::default()
    };
    let t = tributacao_da_nota(&servico, Some(&tomador(true, false)), so_iss);
    assert_eq!(t.retencoes.irrf, None);
    assert_eq!(t.issqn.retencao, RetencaoIssqn::RetidoPeloTomador);
    // "Sempre" não muda nada, mesmo com tomador que não retém.
    servico.tributacao.retencoes.aplicar = AplicarRetencao::Sempre;
    let t = tributacao_da_nota(&servico, None, ReterNota::default());
    assert!(t.retencoes.irrf.is_some());
}

#[test]
fn valor_liquido_da_descricao() {
    use nfse_core::modelo::RetencaoIssqn;
    let p = |v: &str| Percentual::parse(v).unwrap();
    let mut t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: Some(p("1.5")),
            ..Default::default()
        },
        ..Default::default()
    };
    let valor = Valor::parse("1000").unwrap();
    let f = federais_da_nota(&t, valor);
    assert_eq!(valor_liquido(valor, &f, &t).unwrap().to_string(), "985.00");
    // ISS retido sem alíquota: não dá para saber.
    t.issqn.retencao = RetencaoIssqn::RetidoPeloTomador;
    assert_eq!(valor_liquido(valor, &f, &t), None);
    t.issqn.aliquota = Some(p("2"));
    assert_eq!(valor_liquido(valor, &f, &t).unwrap().to_string(), "965.00");
    assert!(texto_variaveis_desconhecidas("a descrição tem", &["valor_liquido".into()]).contains("alíquota"));
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
            ..Default::default()
        },
        ..Default::default()
    };
    let f = federais_da_nota(&t, Valor::parse("1234.56").unwrap());
    assert_eq!(f.retencao_irrf.unwrap().to_string(), "18.52");
    // vRetCSLL = CSLL 12,35 + PIS 8,02 + COFINS 37,04 (NT 007/2026)
    assert_eq!(f.retencao_csll.unwrap().to_string(), "57.41");
    assert_eq!(f.retencao_cp, None);
    let pc = f.pis_cofins.unwrap();
    // retido também sai como débito próprio, com os mesmos valores somados em vRetCSLL
    assert_eq!(pc.base_calculo.unwrap().to_string(), "1234.56");
    assert_eq!(
        (pc.valor_pis.unwrap().to_string(), pc.valor_cofins.unwrap().to_string()),
        ("8.02".into(), "37.04".into())
    );
    assert_eq!((pc.aliquota_pis, pc.aliquota_cofins), (p("0.65"), p("3")));
    // PIS, COFINS e CSLL retidos: sem isso a Sefin não os desconta do valor líquido.
    assert_eq!(pc.tipo_retencao, Some(3));
}

/// Retenção de até R$ 10,00 é dispensada: IRRF de 1,5% até 666,66 e PIS/COFINS/CSLL de 4,65% até 215,05.
#[test]
fn retencao_de_ate_dez_reais_e_dispensada() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: p("1.5"),
            csll: p("1"),
            pis: p("0.65"),
            cofins: p("3"),
            ..Default::default()
        },
        ..Default::default()
    };
    let nota = |v: &str| federais_da_nota(&t, Valor::parse(v).unwrap());
    let texto = |v: Option<Valor>| v.map(|v| v.to_string());

    // 215,00: IRRF 3,23 e contribuições 10,00 (CSLL 2,15 + PIS 1,40 + COFINS 6,45), tudo dispensado
    let f = nota("215");
    assert_eq!((f.retencao_irrf, f.retencao_csll), (None, None));
    assert_eq!(f.pis_cofins, None);
    assert_eq!(
        retencoes_dispensaveis(&t, Valor::parse("215").unwrap()),
        RetencoesDispensaveis {
            irrf: Some(Valor::parse("3.23").unwrap()),
            contribuicoes: Some(Valor::parse("10").unwrap()),
        }
    );
    // 216,00: as contribuições passam de 10,00 (2,16 + 1,40 + 6,48) e são retidas; o IRRF não
    let f = nota("216");
    assert_eq!(
        (texto(f.retencao_irrf), texto(f.retencao_csll)),
        (None, Some("10.04".into()))
    );
    assert_eq!(f.pis_cofins.unwrap().tipo_retencao, Some(3));
    // 666,00: IRRF 9,99 dispensado; 667,00: IRRF 10,01 retido
    assert_eq!(texto(nota("666").retencao_irrf), None);
    assert_eq!(texto(nota("667").retencao_irrf), Some("10.01".into()));
    assert_eq!(
        retencoes_dispensaveis(&t, Valor::parse("667").unwrap()),
        RetencoesDispensaveis::default()
    );
}

/// Outras notas do período, somadas, passam do limite: a emissão pode reter mesmo abaixo de R$ 10,00,
/// escolhendo IRRF e PIS/COFINS/CSLL em separado.
#[test]
fn escolha_retem_abaixo_do_limite() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: p("1.5"),
            csll: p("1"),
            pis: p("0.65"),
            cofins: p("3"),
            ..Default::default()
        },
        ..Default::default()
    };
    let valor = Valor::parse("215").unwrap();
    let texto = |v: Option<Valor>| v.map(|v| v.to_string());
    let so_contribuicoes = ReterNota {
        contribuicoes_ate_o_limite: true,
        ..Default::default()
    };
    let f = federais_com_escolha(&t, valor, so_contribuicoes);
    assert_eq!(
        (texto(f.retencao_irrf), texto(f.retencao_csll)),
        (None, Some("10.00".into()))
    );
    assert_eq!(f.pis_cofins.unwrap().tipo_retencao, Some(3));
    let so_irrf = ReterNota {
        irrf_ate_o_limite: true,
        ..Default::default()
    };
    let f = federais_com_escolha(&t, valor, so_irrf);
    assert_eq!((texto(f.retencao_irrf), f.retencao_csll), (Some("3.23".into()), None));
}

/// Dispensada a retenção, o PIS e a COFINS de apuração própria continuam na DPS, sem tipo de retenção.
#[test]
fn retencao_dispensada_mantem_pis_cofins_proprios() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let mut t = Tributacao::default();
    t.retencoes.pis = p("0.65");
    t.retencoes.cofins = p("3");
    t.retencoes.csll = p("1");
    t.federais.pis_cofins = Some(nfse_core::modelo::PisCofins {
        cst: "01".into(),
        base_calculo: None,
        aliquota_pis: p("0.65"),
        aliquota_cofins: p("3"),
        valor_pis: None,
        valor_cofins: None,
        tipo_retencao: Some(3),
    });
    let f = federais_da_nota(&t, Valor::parse("200").unwrap());
    assert_eq!(f.retencao_csll, None);
    let pc = f.pis_cofins.unwrap();
    assert_eq!(pc.tipo_retencao, Some(0));
    assert_eq!(
        (pc.valor_pis.unwrap().to_string(), pc.valor_cofins.unwrap().to_string()),
        ("1.30".into(), "6.00".into())
    );

    // Pelo tipo do cadastro (sem percentual): a soma de 7,30 também é dispensada.
    t.retencoes = Default::default();
    let f = federais_da_nota(&t, Valor::parse("200").unwrap());
    assert_eq!(f.retencao_csll, None);
    assert_eq!(f.pis_cofins.unwrap().tipo_retencao, Some(0));
}

/// Nota real do Emissor Web (19/10/2026, 650,00, Lucro Presumido): PIS 0,65% e COFINS 3,00% de apuração
/// própria saem na DPS com base e valores truncados (4,225 vira 4,22).
#[test]
fn pis_cofins_proprios_viram_base_e_valores() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let mut t = Tributacao::default();
    t.federais.pis_cofins = Some(nfse_core::modelo::PisCofins {
        cst: "01".into(),
        base_calculo: None,
        aliquota_pis: p("0.65"),
        aliquota_cofins: p("3"),
        valor_pis: None,
        valor_cofins: None,
        tipo_retencao: Some(0),
    });
    let valor = Valor::parse("650").unwrap();
    let pc = federais_da_nota(&t, valor).pis_cofins.unwrap();
    assert_eq!(pc.base_calculo.unwrap().to_string(), "650.00");
    assert_eq!(pc.valor_pis.unwrap().to_string(), "4.22");
    assert_eq!(pc.valor_cofins.unwrap().to_string(), "19.50");
    // a base do IBS/CBS que a Sefin acha: 650,00 - 4,22 - 19,50
    let base_ibs = valor.centesimos() - pc.valor_pis.unwrap().centesimos() - pc.valor_cofins.unwrap().centesimos();
    assert_eq!(Valor::de_centesimos(base_ibs).to_string(), "626.28");
    // CST sem tributação (00, 04...) ou sem alíquota: nada é calculado
    t.federais.pis_cofins.as_mut().unwrap().cst = "04".into();
    assert_eq!(federais_da_nota(&t, valor).pis_cofins.unwrap().valor_pis, None);
    t.federais.pis_cofins.as_mut().unwrap().cst = "01".into();
    t.federais.pis_cofins.as_mut().unwrap().aliquota_pis = None;
    let pc = federais_da_nota(&t, valor).pis_cofins.unwrap();
    assert_eq!(
        (pc.valor_pis, pc.valor_cofins.unwrap().to_string()),
        (None, "19.50".into())
    );
    // PIS retido: vPis igual ao retido (arredondado, como em vRetCSLL); COFINS própria segue truncada
    t.federais.pis_cofins.as_mut().unwrap().aliquota_pis = p("0.65");
    t.retencoes.pis = p("0.65");
    let f = federais_da_nota(&t, Valor::parse("1650").unwrap());
    assert_eq!(f.retencao_csll.unwrap().to_string(), "10.73");
    let pc = f.pis_cofins.unwrap();
    assert_eq!(
        (pc.valor_pis.unwrap().to_string(), pc.valor_cofins.unwrap().to_string()),
        ("10.73".into(), "49.50".into())
    );
}

/// Nota real de 26/10/2026 (64.715,35, IR 1,5%, CSLL 1%, PIS 0,65% e COFINS 3% retidos): o DANFSe saiu com
/// "PIS/COFINS - Débito Apuração Própria" zerados porque vPis/vCofins não iam na DPS.
#[test]
fn retencao_percentual_tambem_leva_vpis_e_vcofins() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: p("1.5"),
            csll: p("1"),
            cp: None,
            pis: p("0.65"),
            cofins: p("3"),
            ..Default::default()
        },
        ..Default::default()
    };
    let f = federais_da_nota(&t, Valor::parse("64715.35").unwrap());
    assert_eq!(f.retencao_irrf.unwrap().to_string(), "970.73");
    assert_eq!(f.retencao_csll.unwrap().to_string(), "3009.26");
    let pc = f.pis_cofins.unwrap();
    assert_eq!(pc.base_calculo.unwrap().to_string(), "64715.35");
    assert_eq!(
        (pc.valor_pis.unwrap().to_string(), pc.valor_cofins.unwrap().to_string()),
        ("420.65".into(), "1941.46".into())
    );
    assert_eq!(pc.tipo_retencao, Some(3));
}

/// Nota do Portal Nacional (700,00): tipo 3 no cadastro, sem percentual de retenção; vRetCSLL = CSLL 7,00 + PIS 4,55 + COFINS 21,00.
#[test]
fn tipo_3_do_cadastro_soma_pis_cofins_em_vretcsll() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let mut t = Tributacao::default();
    t.federais.retencao_csll = Some(Valor::parse("7").unwrap());
    t.federais.pis_cofins = Some(nfse_core::modelo::PisCofins {
        cst: "01".into(),
        base_calculo: None,
        aliquota_pis: p("0.65"),
        aliquota_cofins: p("3"),
        valor_pis: None,
        valor_cofins: None,
        tipo_retencao: Some(3),
    });
    let f = federais_da_nota(&t, Valor::parse("700").unwrap());
    assert_eq!(f.retencao_csll.unwrap().to_string(), "32.55");
    let pc = f.pis_cofins.unwrap();
    assert_eq!(
        (pc.valor_pis.unwrap().to_string(), pc.valor_cofins.unwrap().to_string()),
        ("4.55".into(), "21.00".into())
    );
}

/// Nota real de 17/10/2026: 7.000,00 com IR 1,5%, PIS 0,65%, COFINS 3% e CSLL 1% deve dar líquido 6.569,50.
#[test]
fn retencoes_somam_no_valor_liquido() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let t = Tributacao {
        retencoes: nfse_db::servicos::RetencoesPercentuais {
            irrf: p("1.5"),
            csll: p("1"),
            cp: None,
            pis: p("0.65"),
            cofins: p("3"),
            ..Default::default()
        },
        ..Default::default()
    };
    let f = federais_da_nota(&t, Valor::parse("7000").unwrap());
    // vTotalRet da Sefin = vRetCP + vRetIRRF + vRetCSLL
    let total: i64 = [f.retencao_cp, f.retencao_irrf, f.retencao_csll]
        .into_iter()
        .flatten()
        .map(|v| v.centesimos())
        .sum();
    assert_eq!(f.retencao_csll.unwrap().to_string(), "325.50");
    assert_eq!(Valor::de_centesimos(700_000 - total).to_string(), "6569.50");
}

#[test]
fn tipo_de_retencao_segue_os_percentuais() {
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let mut t = Tributacao::default();
    // cadastro com "não retidos" e PIS/COFINS retidos nos percentuais
    t.federais.pis_cofins = Some(nfse_core::modelo::PisCofins {
        cst: "01".into(),
        base_calculo: None,
        aliquota_pis: None,
        aliquota_cofins: None,
        valor_pis: None,
        valor_cofins: None,
        tipo_retencao: Some(0),
    });
    t.retencoes.pis = p("0.65");
    t.retencoes.cofins = p("3");
    let tipo = |t: &Tributacao| {
        federais_da_nota(t, Valor::parse("10000").unwrap())
            .pis_cofins
            .unwrap()
            .tipo_retencao
    };
    assert_eq!(tipo(&t), Some(4));
    t.retencoes.cofins = None;
    assert_eq!(tipo(&t), Some(5));
    t.retencoes.pis = None;
    // sem retenção percentual, vale o tipo do cadastro
    assert_eq!(tipo(&t), Some(0));
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

    // O diagnóstico fica guardado: o XML enviado e o corpo da resposta da Sefin.
    assert!(dps.xml_assinado.is_some_and(|x| x.contains("<infDPS")));
    let resposta = c.emissor.banco().resposta_sefin(dps_id).await.unwrap().unwrap();
    assert!(
        resposta.starts_with("HTTP 400") && resposta.contains("E0120"),
        "{resposta}"
    );

    let texto = c.emissor.diagnostico_nota(dps_id).await.unwrap();
    assert!(
        texto.contains("Situação: rejeitada") && texto.contains("Leiaute da DPS: nt"),
        "{texto}"
    );
    assert!(texto.contains("--- Resposta da Sefin ---") && texto.contains("E0120"));
    assert!(
        texto.contains("<infDPS") && texto.contains("<CNPJ>***</CNPJ>"),
        "{texto}"
    );

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
async fn copiar_vale_para_autorizada_e_nao_para_rejeitada() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Err(rejeicao("E0120"))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let r = c.emissor.dados_para_copiar(nfse.dps_id).await.unwrap();
    assert_eq!(r.empresa_id, c.pedido.empresa_id);
    assert_eq!(r.tomador.map(|t| t.pessoa_id), c.pedido.tomador);
    assert_eq!(r.servico_id, Some(c.pedido.servico_id));
    assert_eq!(r.valor, c.pedido.valor);

    let ErroEmissao::Rejeitada {
        dps_id: Some(rejeitada),
        ..
    } = c.emissor.emitir(&c.pedido).await.unwrap_err()
    else {
        panic!()
    };
    assert!(matches!(
        c.emissor.dados_para_copiar(rejeitada).await,
        Err(ErroEmissao::Regra(_))
    ));
}

/// Serviço prestado fora do município da empresa: `cLocPrestacao` leva o município escolhido, e
/// copiar a nota o traz de volta (a nota no município da empresa não traz nada).
#[tokio::test]
async fn local_da_prestacao_fora_do_municipio() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Ok(nfse_com_chave(CHAVE_SUBSTITUTA, 2))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse: propria, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert!(c.sefin.recebidos()[0].contains("<cLocPrestacao>3511102</cLocPrestacao>"));
    assert_eq!(
        c.emissor
            .dados_para_copiar(propria.dps_id)
            .await
            .unwrap()
            .local_prestacao,
        None
    );

    let mut fora = c.pedido.clone();
    fora.local_prestacao = Some("3550308".into());
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&fora).await.unwrap() else {
        panic!()
    };
    assert!(c.sefin.recebidos()[1].contains("<cLocPrestacao>3550308</cLocPrestacao>"));
    assert_eq!(
        c.emissor.dados_para_copiar(nfse.dps_id).await.unwrap().local_prestacao,
        Some("3550308".into())
    );
}

#[tokio::test]
async fn local_da_prestacao_invalido_nao_gasta_numero() {
    let c = cenario(vec![]).await;
    let mut invalido = c.pedido.clone();
    invalido.local_prestacao = Some("355".into());
    assert!(matches!(
        c.emissor.emitir(&invalido).await,
        Err(ErroEmissao::DpsInvalida(_))
    ));
    assert!(c.sefin.recebidos().is_empty());
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
    let mut c = cenario(vec![
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        Resposta::ConsultarDps(Err(ErroSefin::SemConexao("rede caiu".into()))),
        // Ao reabrir o sistema:
        Resposta::ConsultarDps(Ok(None)),
    ])
    .await;
    let ResultadoEmissao::Incerta { dps_id, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert_eq!(
        c.emissor.banco().dps(dps_id).await.unwrap().situacao,
        SituacaoDps::Incerta
    );
    assert_eq!(c.emissor.banco().dps_pendentes().await.unwrap().len(), 1);

    // Passada a janela de reconciliação, o 404 libera a DPS.
    c.emissor.janela_reconciliacao = chrono::Duration::zero();
    let banco = c.emissor.banco();
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
                chave_pix: e.chave_pix.clone(),
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
            obra: None,
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

#[tokio::test]
async fn planilha_vincula_tomador_herdando_retencao_e_tipo() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let banco = c.emissor.banco();
    // Tomador cadastrado só pela empresa B, que marcou retenção e o tipo (município).
    let outra = banco
        .cadastrar_empresa(&NovaEmpresa {
            cnpj: Cnpj::parse("33000167000101").unwrap(),
            nome: "OUTRA EMPRESA LTDA".into(),
            inscricao_municipal: None,
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
            chave_pix: None,
        })
        .await
        .unwrap();
    let pessoa = banco
        .salvar_tomador(
            outra.id,
            &Tomador {
                documento: Documento::Cnpj(Cnpj::parse("11444777000161").unwrap()),
                inscricao_municipal: None,
                nome: "PREFEITURA EXEMPLO".into(),
                endereco: None,
                telefone: None,
                email: None,
            },
            "manual",
        )
        .await
        .unwrap();
    let retencao = nfse_db::tomadores::RetencaoDoVinculo {
        federais: true,
        iss: true,
        tipo: nfse_db::tomadores::TipoTomador::Municipal,
    };
    banco
        .definir_retencao_tomador(outra.id, pessoa, retencao)
        .await
        .unwrap();

    // A planilha da empresa A cita esse tomador: o vínculo novo herda a retenção e o tipo.
    let csv = "Tomador;Valor;Descricao\r\n11.444.777/0001-61;150,00;CONSULTAS\r\n";
    let linhas = crate::planilha::ler("convenio.csv", csv.as_bytes()).unwrap();
    let setembro = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let itens = importacao::resolver(banco, &linhas, Some(c.pedido.empresa_id), setembro)
        .await
        .unwrap();
    assert!(itens[0].emitivel(), "{:?}", itens[0].erros);
    assert!(itens[0].vincular_tomador);
    assert!(matches!(
        c.emissor.emitir_importado(&itens[0], None).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    let v = banco.tomador(c.pedido.empresa_id, pessoa).await.unwrap();
    assert_eq!(
        (v.retem_federais, v.retem_iss, v.tipo),
        (retencao.federais, retencao.iss, retencao.tipo)
    );
}

/// Carteiro que guarda as mensagens em vez de enviar (ou falha, se pedido).
#[derive(Default)]
struct CarteiroTeste {
    enviadas: Mutex<Vec<String>>,
    falhar: bool,
}

impl email::Carteiro for CarteiroTeste {
    async fn entregar(
        &self,
        _cfg: &nfse_db::emails::ConfigSmtp,
        senha: Option<&str>,
        mensagem: lettre::Message,
    ) -> Result<(), String> {
        assert_eq!(senha, Some("senha-smtp-teste"));
        if self.falhar {
            return Err("conexão recusada".into());
        }
        self.enviadas
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(&mensagem.formatted()).into_owned());
        Ok(())
    }
}

async fn configurar_email(c: &Cenario, enviar_ao_emitir: bool) {
    let banco = c.emissor.banco();
    banco
        .definir_smtp(&nfse_db::emails::ConfigSmtp {
            servidor: "smtp.exemplo.com".into(),
            porta: 587,
            seguranca: nfse_db::emails::Seguranca::StartTls,
            usuario: Some("notas@escritorio.com".into()),
            remetente: "notas@escritorio.com".into(),
            nome_remetente: None,
            assunto: nfse_db::emails::ConfigSmtp::ASSUNTO_PADRAO.into(),
            corpo: nfse_db::emails::ConfigSmtp::CORPO_PADRAO.into(),
            enviar_ao_emitir,
        })
        .await
        .unwrap();
    banco
        .definir_senha_smtp(c.emissor.cofre(), "senha-smtp-teste")
        .await
        .unwrap();
}

#[tokio::test]
async fn envia_a_nota_por_email_e_registra() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let carteiro = CarteiroTeste::default();
    let banco = c.emissor.banco();

    // Sem configuração: avisa e não envia.
    let erro = c.emissor.enviar_email(nfse.id, &[], &carteiro).await.unwrap_err();
    assert!(erro.to_string().contains("Configurações"), "{erro}");
    configurar_email(&c, false).await;
    assert_eq!(
        c.emissor.enviar_email_apos_emissao(nfse.id, &carteiro).await,
        None,
        "automático desligado"
    );

    // Tomador sem e-mail de envio: falha registrada.
    assert!(c.emissor.enviar_email(nfse.id, &[], &carteiro).await.is_err());
    banco
        .definir_emails_tomador(
            c.pedido.empresa_id,
            c.pedido.tomador.unwrap(),
            Some("financeiro@cliente.com"),
            None,
        )
        .await
        .unwrap();
    let para = c.emissor.enviar_email(nfse.id, &[], &carteiro).await.unwrap();
    assert_eq!(para, vec!["financeiro@cliente.com"]);
    let enviada = carteiro.enviadas.lock().unwrap()[0].clone();
    assert!(enviada.contains("To: financeiro@cliente.com"));
    assert!(enviada.contains("NFSe-1-"), "anexos com o nome do arquivo da nota");

    let notas = banco.listar_notas(c.pedido.empresa_id, None).await.unwrap();
    assert!(notas[0].email_enviado_em.is_some());

    // Falha do servidor: erro registrado, e o envio que deu certo continua valendo.
    let quebrado = CarteiroTeste {
        falhar: true,
        ..Default::default()
    };
    configurar_email(&c, true).await;
    let aviso = c.emissor.enviar_email_apos_emissao(nfse.id, &quebrado).await.unwrap();
    assert!(
        aviso.contains("não enviado") && aviso.contains("conexão recusada"),
        "{aviso}"
    );
    let aviso = c.emissor.enviar_email_apos_emissao(nfse.id, &carteiro).await.unwrap();
    assert!(aviso.starts_with("E-mail enviado"), "{aviso}");
}

#[tokio::test]
async fn nao_encontrada_logo_apos_timeout_continua_incerta() {
    let c = cenario(vec![
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        // A Sefin ainda não enxerga a DPS que acabou de processar.
        Resposta::ConsultarDps(Ok(None)),
        // O operador verifica depois e a nota apareceu.
        Resposta::ConsultarDps(Ok(Some(CHAVE.into()))),
        Resposta::ConsultarNfse(Ok(nfse_autorizada())),
    ])
    .await;
    let ResultadoEmissao::Incerta { dps_id, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!("404 dentro da janela não pode liberar para reenvio")
    };
    let banco = c.emissor.banco();
    assert_eq!(banco.dps(dps_id).await.unwrap().situacao, SituacaoDps::Incerta);

    let nfse = c.emissor.reconciliar(dps_id).await.unwrap().expect("nota gravada");
    assert_eq!(nfse.chave, CHAVE);
    assert_eq!(banco.dps(dps_id).await.unwrap().situacao, SituacaoDps::Autorizada);
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn verificar_envio_em_curso_nao_consulta_a_sefin() {
    let c = cenario(vec![]).await;
    let banco = c.emissor.banco();
    let reserva = banco.reservar_dps(c.pedido.empresa_id).await.unwrap();
    let mut modelo: Dps = serde_json::from_str(include_str!("../../../exemplos/dps-exemplo.json")).unwrap();
    modelo.numero = reserva.numero;
    banco.registrar_envio(reserva.id, &modelo, "<DPS/>").await.unwrap();

    assert!(matches!(
        c.emissor.reconciliar(reserva.id).await,
        Err(ErroEmissao::Regra(_))
    ));
    assert_eq!(banco.dps(reserva.id).await.unwrap().situacao, SituacaoDps::Enviando);
    assert!(c.sefin.recebidos().is_empty());
}

#[tokio::test]
async fn autorizada_e_nao_gravada_fica_incerta() {
    let mut ilegivel = nfse_autorizada();
    ilegivel.xml = "<NFSe>truncado".into();
    let c = cenario(vec![
        Resposta::Emitir(Ok(ilegivel)),
        // Reconciliação: direto pela chave recebida; a Sefin devolve o XML íntegro.
        Resposta::ConsultarNfse(Ok(nfse_autorizada())),
    ])
    .await;
    let ResultadoEmissao::Incerta { dps_id, motivo } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!("a nota existe na Sefin: não pode virar erro de emissão")
    };
    assert!(motivo.contains(CHAVE), "{motivo}");
    let banco = c.emissor.banco();
    let dps = banco.dps(dps_id).await.unwrap();
    assert_eq!(dps.situacao, SituacaoDps::Incerta);
    assert_eq!(
        dps.chave_recebida.as_deref(),
        Some(CHAVE),
        "a chave fica no banco, não só no texto"
    );

    let nfse = c.emissor.reconciliar(dps_id).await.unwrap().expect("nota gravada");
    assert_eq!(nfse.chave, CHAVE);
    assert_eq!(banco.dps(dps_id).await.unwrap().situacao, SituacaoDps::Autorizada);
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn chave_recebida_nunca_libera_a_dps() {
    let mut ilegivel = nfse_autorizada();
    ilegivel.xml = "<NFSe>truncado".into();
    let mut c = cenario(vec![
        Resposta::Emitir(Ok(ilegivel)),
        // Ao reabrir: a Sefin está fora; a DPS continua incerta, sem consulta pelo Id da DPS.
        Resposta::ConsultarNfse(Err(ErroSefin::Http {
            status: 404,
            corpo: String::new(),
        })),
    ])
    .await;
    let ResultadoEmissao::Incerta { dps_id, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    c.emissor.janela_reconciliacao = chrono::Duration::zero();
    let resultados = c.emissor.reconciliar_pendentes().await.unwrap();
    assert!(matches!(resultados.as_slice(), [(id, Err(_))] if *id == dps_id));
    let banco = c.emissor.banco();
    assert_eq!(banco.dps(dps_id).await.unwrap().situacao, SituacaoDps::Incerta);
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn nfse_de_outra_dps_nao_e_gravada() {
    let mut outra = nfse_autorizada();
    outra.xml = outra.xml.replace(
        r#"<infDPS Id="X">"#,
        r#"<infDPS Id="DPS351110221122233300018100001000000000000999">"#,
    );
    let c = cenario(vec![Resposta::Emitir(Ok(outra))]).await;
    let ResultadoEmissao::Incerta { dps_id, motivo } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!("NFS-e de outra DPS não pode dar esta como autorizada")
    };
    assert!(motivo.contains("não desta"), "{motivo}");
    let dps = c.emissor.banco().dps(dps_id).await.unwrap();
    assert_eq!(dps.situacao, SituacaoDps::Incerta);
    assert_eq!(
        dps.chave_recebida, None,
        "a chave de outra DPS não prova nada sobre esta"
    );
}

#[tokio::test]
async fn erro_do_banco_depois_do_envio_fica_incerta() {
    for resposta in [Ok(nfse_autorizada()), Err(rejeicao("E0120"))] {
        let c = cenario(vec![Resposta::Emitir(resposta)]).await;
        *c.sefin.fechar_no_envio.lock().unwrap() = Some(c.emissor.banco().clone());
        let Ok(ResultadoEmissao::Incerta { motivo, .. }) = c.emissor.emitir(&c.pedido).await else {
            panic!("depois do envio, erro do banco não pode liberar novo clique")
        };
        assert!(motivo.contains("Verificar"), "{motivo}");
    }
}

#[tokio::test]
async fn erro_5xx_da_sefin_consulta_antes_de_liberar() {
    let c = cenario(vec![
        Resposta::Emitir(Err(ErroSefin::Http {
            status: 500,
            corpo: r#"{"erros":[{"Codigo":"E9999"}]}"#.into(),
        })),
        Resposta::ConsultarDps(Ok(Some(CHAVE.into()))),
        Resposta::ConsultarNfse(Ok(nfse_autorizada())),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert_eq!(nfse.chave, CHAVE);
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn reserva_sem_envio_e_liberada_ao_abrir() {
    let c = cenario(vec![]).await;
    let banco = c.emissor.banco();
    // Simula o programa fechando entre a reserva do número e o envio.
    let reserva = banco.reservar_dps(c.pedido.empresa_id).await.unwrap();

    assert!(c.emissor.reconciliar_pendentes().await.unwrap().is_empty());
    let dps = banco.dps(reserva.id).await.unwrap();
    assert_eq!(dps.situacao, SituacaoDps::Rejeitada);
    assert!(dps.ultimo_erro.unwrap().starts_with("não enviada"));
    assert!(c.sefin.recebidos().is_empty());
}

#[tokio::test]
async fn cancelamento_sem_resposta_mantem_autorizada() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Evento(Err(ErroSefin::Incerto("timeout".into()))),
        // A consulta também não acha o evento.
        Resposta::ConsultarEvento(Ok(None)),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let Err(ErroEmissao::Regra(mensagem)) = c
        .emissor
        .cancelar(
            nfse.id,
            MotivoCancelamento::ErroNaEmissao,
            "Valor informado errado na emissão",
        )
        .await
    else {
        panic!("esperava aviso de cancelamento incerto")
    };
    assert!(mensagem.contains("Tente cancelar de novo"), "{mensagem}");
    assert_eq!(
        c.emissor.banco().dps(nfse.dps_id).await.unwrap().situacao,
        SituacaoDps::Autorizada
    );
}

/// Evento de cancelamento homologado, no formato da Sefin, para a chave do cenário.
fn evento_cancelamento() -> String {
    format!(
        concat!(
            r#"<evento xmlns="http://www.sped.fazenda.gov.br/nfse" versao="1.01"><infEvento Id="EVT{chave}1011010001">"#,
            r#"<nSeqEvento>1</nSeqEvento><pedRegEvento versao="1.01"><infPedReg Id="PRE{chave}101101">"#,
            "<chNFSe>{chave}</chNFSe><e101101><xDesc>Cancelamento de NFS-e</xDesc><cMotivo>1</cMotivo>",
            "<xMotivo>Cancelada no portal nacional</xMotivo></e101101></infPedReg></pedRegEvento></infEvento></evento>"
        ),
        chave = CHAVE
    )
}

#[tokio::test]
async fn sincroniza_cancelamento_feito_no_portal() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::ConsultarEvento(Ok(None)),
        Resposta::ConsultarEvento(Ok(Some(evento_cancelamento()))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let banco = c.emissor.banco();
    assert!(
        !c.emissor.sincronizar_cancelamento(nfse.id).await.unwrap(),
        "ainda não cancelada"
    );
    assert_eq!(banco.dps(nfse.dps_id).await.unwrap().situacao, SituacaoDps::Autorizada);

    assert!(c.emissor.sincronizar_cancelamento(nfse.id).await.unwrap());
    assert_eq!(banco.dps(nfse.dps_id).await.unwrap().situacao, SituacaoDps::Cancelada);
    assert_eq!(c.sefin.recebidos()[2], format!("{CHAVE}/101101/1"));
    // Já cancelada: não consulta de novo.
    assert!(!c.emissor.sincronizar_cancelamento(nfse.id).await.unwrap());
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn evento_de_outra_nota_nao_cancela() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::ConsultarEvento(Ok(Some(evento_cancelamento().replace(CHAVE, &"9".repeat(50))))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    assert!(matches!(
        c.emissor.sincronizar_cancelamento(nfse.id).await,
        Err(ErroEmissao::Regra(_))
    ));
    assert_eq!(
        c.emissor.banco().dps(nfse.dps_id).await.unwrap().situacao,
        SituacaoDps::Autorizada
    );
}

#[tokio::test]
async fn cancelar_nota_ja_cancelada_no_portal_registra_o_evento() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        // A Sefin recusa o pedido porque a nota já tem cancelamento.
        Resposta::Evento(Err(rejeicao("X0000"))),
        Resposta::ConsultarEvento(Ok(Some(evento_cancelamento()))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    c.emissor
        .cancelar(
            nfse.id,
            MotivoCancelamento::ErroNaEmissao,
            "Valor informado errado na emissão",
        )
        .await
        .unwrap();
    assert_eq!(
        c.emissor.banco().dps(nfse.dps_id).await.unwrap().situacao,
        SituacaoDps::Cancelada
    );
}

#[tokio::test]
async fn recorrencia_de_obra_leva_a_obra_na_nota() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let banco = c.emissor.banco();
    // Serviço de construção civil: a DPS exige o grupo obra (E0370).
    let mut servico = banco.servico(c.pedido.servico_id).await.unwrap();
    servico.codigo_trib_nac = "070201".into();
    banco.salvar_servico(&servico).await.unwrap();
    let setembro = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();

    let sem_obra = recorrencia_do_cenario(&c).await;
    let erro = c.emissor.emitir_recorrencia(sem_obra, setembro).await.unwrap_err();
    assert!(erro.to_string().contains("E0370"), "{erro:?}");
    assert!(c.sefin.recebidos().is_empty(), "barrada antes de reservar número");

    let com_obra = banco
        .salvar_recorrencia(&nfse_db::recorrencias::Recorrencia {
            id: None,
            empresa_id: c.pedido.empresa_id,
            tomador: c.pedido.tomador,
            servico_id: c.pedido.servico_id,
            valor: Some(c.pedido.valor),
            descricao: None,
            informacoes_complementares: None,
            obra: Some(Obra {
                inscricao_imobiliaria: None,
                identificacao: nfse_core::modelo::IdentificacaoObra::Cno("123456789012".into()),
            }),
        })
        .await
        .unwrap();
    assert!(
        banco.recorrencia(com_obra).await.unwrap().recorrencia.obra.is_some(),
        "obra gravada"
    );
    let r = c.emissor.emitir_recorrencia(com_obra, setembro).await.unwrap();
    assert!(matches!(r, ResultadoEmissao::Autorizada { .. }));
    assert!(
        c.sefin.recebidos()[0].contains("<cObra>123456789012</cObra>"),
        "{}",
        c.sefin.recebidos()[0]
    );
}

#[test]
fn estado_df_e_municipio_retem_so_irrf_e_inss() {
    use nfse_core::modelo::{Documento, Tomador};
    use nfse_db::tomadores::TipoTomador;
    let p = |v: &str| Some(Percentual::parse(v).unwrap());
    let servico = Servico {
        id: None,
        empresa_id: 1,
        nome: "x".into(),
        descricao: "x".into(),
        codigo_trib_nac: "040301".into(),
        codigo_trib_mun: None,
        nbs: None,
        tributacao: Tributacao {
            retencoes: nfse_db::servicos::RetencoesPercentuais {
                irrf: p("4.8"),
                csll: p("1"),
                cp: p("11"),
                pis: p("0.65"),
                cofins: p("3"),
                ..Default::default()
            },
            ..Default::default()
        },
        valor_sugerido: None,
    };
    let tomador = |tipo| TomadorVinculado {
        pessoa_id: 1,
        tomador: Tomador {
            documento: Documento::Cnpj(nfse_core::Cnpj::parse("12ABC34501DE35").unwrap()),
            inscricao_municipal: None,
            nome: "Prefeitura".into(),
            endereco: None,
            telefone: None,
            email: None,
        },
        emails: None,
        observacao: None,
        retem_federais: true,
        retem_iss: true,
        tipo,
    };
    for tipo in [TipoTomador::Privado, TipoTomador::Federal] {
        let t = tributacao_da_nota(&servico, Some(&tomador(tipo)), ReterNota::default());
        assert_eq!(t.retencoes, servico.tributacao.retencoes, "{tipo:?} retém tudo");
    }
    for tipo in [
        TipoTomador::Estadual,
        TipoTomador::DistritoFederal,
        TipoTomador::Municipal,
    ] {
        let t = tributacao_da_nota(&servico, Some(&tomador(tipo)), ReterNota::default());
        assert_eq!((t.retencoes.irrf, t.retencoes.cp), (p("4.8"), p("11")), "{tipo:?}");
        assert_eq!(
            (t.retencoes.csll, t.retencoes.pis, t.retencoes.cofins),
            (None, None, None),
            "{tipo:?}"
        );
    }
}

/// Chave da NFS-e que substitui a de `CHAVE`.
const CHAVE_SUBSTITUTA: &str = "35111022211222333000181000000000000226101234567891";

fn substituir(nfse_id: i64) -> Option<SubstituirNota> {
    Some(SubstituirNota {
        nfse_id,
        motivo: "99".into(),
        descricao_motivo: Some("Valor do serviço informado errado".into()),
    })
}

#[tokio::test]
async fn substitui_nota_autorizada() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Ok(nfse_com_chave(CHAVE_SUBSTITUTA, 2))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let r = c.emissor.dados_para_substituir(original.dps_id).await.unwrap();
    assert_eq!(r.valor, c.pedido.valor);

    let pedido = PedidoEmissao {
        substituir: substituir(original.id),
        ..c.pedido.clone()
    };
    let ResultadoEmissao::Autorizada { nfse: nova, .. } = c.emissor.emitir(&pedido).await.unwrap() else {
        panic!()
    };
    let enviado = &c.sefin.recebidos()[1];
    nfse_xml::verificar(enviado).expect("DPS enviada assinada");
    assert!(
        enviado.contains(&format!(
            "<subst><chSubstda>{CHAVE}</chSubstda><cMotivo>99</cMotivo><xMotivo>Valor do serviço informado errado</xMotivo></subst>"
        )),
        "{enviado}"
    );
    let banco = c.emissor.banco();
    assert_eq!(
        banco.dps(original.dps_id).await.unwrap().situacao,
        SituacaoDps::Substituida
    );
    assert_eq!(banco.dps(nova.dps_id).await.unwrap().situacao, SituacaoDps::Autorizada);
    let auditoria = banco.auditoria(10).await.unwrap();
    assert!(auditoria.iter().any(|(_, acao, _, _)| acao == "nfse_substituida"));

    // A substituída não volta a ser substituída nem cancelada; nada vai à Sefin.
    let erro = c.emissor.emitir(&pedido).await.unwrap_err();
    assert!(erro.to_string().contains("substituida"), "{erro}");
    assert!(matches!(
        c.emissor.dados_para_substituir(original.dps_id).await,
        Err(ErroEmissao::Regra(_))
    ));
    assert_eq!(c.sefin.recebidos().len(), 2);
}

#[tokio::test]
async fn substituta_incerta_bloqueia_outra_substituicao() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        Resposta::ConsultarDps(Err(ErroSefin::Incerto("timeout".into()))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let pedido = PedidoEmissao {
        substituir: substituir(original.id),
        ..c.pedido.clone()
    };
    let ResultadoEmissao::Incerta { .. } = c.emissor.emitir(&pedido).await.unwrap() else {
        panic!("esperava incerta")
    };
    let erro = c.emissor.emitir(&pedido).await.unwrap_err();
    assert!(erro.to_string().contains("aguardando a Sefin"), "{erro}");
    assert_eq!(
        c.emissor.banco().dps(original.dps_id).await.unwrap().situacao,
        SituacaoDps::Autorizada,
        "a original só muda quando a substituta é autorizada"
    );
    assert!(c.sefin.roteiro_vazio());
}

#[tokio::test]
async fn substituicao_com_motivo_invalido_nao_gasta_numero() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let pedido = PedidoEmissao {
        substituir: Some(SubstituirNota {
            nfse_id: original.id,
            motivo: "06".into(),
            descricao_motivo: Some("curto".into()),
        }),
        ..c.pedido.clone()
    };
    let erro = c.emissor.emitir(&pedido).await.unwrap_err();
    assert!(erro.to_string().contains("substituicao.motivo"), "{erro}");
    assert!(erro.to_string().contains("substituicao.descricao_motivo"), "{erro}");
    assert_eq!(c.sefin.recebidos().len(), 1);
}

/// NFS-e de empresa ME/EPP (`opSimpNac` = 3) com o tomador do cenário, para a regra do E0063.
fn nfse_me_epp() -> NfseAutorizada {
    let mut n = nfse_autorizada();
    n.xml = n.xml.replace(
        "<dCompet>2026-10-16</dCompet>",
        "<dCompet>2026-10-16</dCompet><prest><regTrib><opSimpNac>3</opSimpNac></regTrib></prest>\
         <toma><CNPJ>12ABC34501DE35</CNPJ></toma>",
    );
    n
}

#[tokio::test]
async fn substituta_de_me_epp_nao_muda_o_valor() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_me_epp()))]).await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let pedido = PedidoEmissao {
        substituir: substituir(original.id),
        valor: Valor::parse("1100").unwrap(),
        ..c.pedido.clone()
    };
    let erro = c.emissor.emitir(&pedido).await.unwrap_err().to_string();
    assert!(erro.contains("E0063") && erro.contains("valor do serviço"), "{erro}");
    assert!(erro.contains("cancele a nota"), "{erro}");
    assert_eq!(c.sefin.recebidos().len(), 1, "nada vai à Sefin");
    assert_eq!(
        c.emissor
            .banco()
            .proximo_numero(c.pedido.empresa_id, Ambiente::Producao, "1")
            .await
            .unwrap(),
        2,
        "a recusa não gasta número"
    );
}

#[tokio::test]
async fn substituta_fica_na_competencia_da_original() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_me_epp())),
        Resposta::Emitir(Ok(nfse_com_chave(CHAVE_SUBSTITUTA, 2))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let pedido = PedidoEmissao {
        substituir: substituir(original.id),
        descricao: Some("Descrição corrigida".into()),
        ..c.pedido.clone()
    };
    let ResultadoEmissao::Autorizada { .. } = c.emissor.emitir(&pedido).await.unwrap() else {
        panic!()
    };
    let enviado = &c.sefin.recebidos()[1];
    assert!(enviado.contains("<dCompet>2026-10-16</dCompet>"), "{enviado}");
}

#[tokio::test]
async fn substituta_achada_na_reconciliacao_marca_a_original() {
    let c = cenario(vec![
        Resposta::Emitir(Ok(nfse_autorizada())),
        Resposta::Emitir(Err(ErroSefin::Incerto("timeout".into()))),
        Resposta::ConsultarDps(Ok(Some(CHAVE_SUBSTITUTA.into()))),
        Resposta::ConsultarNfse(Ok(nfse_com_chave(CHAVE_SUBSTITUTA, 2))),
    ])
    .await;
    let ResultadoEmissao::Autorizada { nfse: original, .. } = c.emissor.emitir(&c.pedido).await.unwrap() else {
        panic!()
    };
    let pedido = PedidoEmissao {
        substituir: substituir(original.id),
        ..c.pedido.clone()
    };
    let ResultadoEmissao::Autorizada { nfse: nova, .. } = c.emissor.emitir(&pedido).await.unwrap() else {
        panic!("esperava autorizada pela consulta")
    };
    assert_eq!(nova.chave, CHAVE_SUBSTITUTA);
    assert_eq!(
        c.emissor.banco().dps(original.dps_id).await.unwrap().situacao,
        SituacaoDps::Substituida
    );
}

#[tokio::test]
async fn previa_e_emissao_conferem_o_mesmo() {
    let c = cenario(vec![]).await;
    let v = c.emissor.validar(&c.pedido).await.unwrap();
    assert!(v.erros().is_empty(), "{:?}", v.erros());
    assert_eq!(
        v.dps.versao_aplicativo, c.emissor.versao_aplicativo,
        "a prévia leva a versão do aplicativo"
    );

    // Variável errada: a prévia aponta e a emissão recusa com o mesmo texto, sem chamar a Sefin.
    let mut pedido = c.pedido.clone();
    pedido.descricao = Some("REF {tomadr}".into());
    let v = c.emissor.validar(&pedido).await.unwrap();
    assert!(v.regras.iter().any(|e| e.contains("tomadr")), "{:?}", v.regras);
    assert!(matches!(
        c.emissor.emitir(&pedido).await,
        Err(ErroEmissao::Regra(m)) if m.contains("tomadr")
    ));
    assert!(c.sefin.recebidos().is_empty());
    assert!(c.emissor.banco().dps_pendentes().await.unwrap().is_empty());
}

#[tokio::test]
async fn pix_da_empresa_na_descricao_e_nas_complementares() {
    let c = cenario(vec![]).await;
    let mut pedido = c.pedido.clone();
    pedido.descricao = Some("REPASSE {mes_ref}. PIX: {pix}".into());
    pedido.informacoes_complementares = Some("Chave PIX {pix}".into());

    // Sem chave no cadastro: barra com o motivo, uma vez só para os dois campos.
    let v = c.emissor.validar(&pedido).await.unwrap();
    let pix: Vec<_> = v.regras.iter().filter(|r| r.contains("chave PIX cadastrada")).collect();
    assert_eq!(pix.len(), 1, "{:?}", v.regras);

    let e = c.emissor.banco().empresa(pedido.empresa_id).await.unwrap();
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
                ambiente: e.ambiente,
                serie: e.serie.clone(),
                telefone: e.telefone.clone(),
                email: e.email.clone(),
                chave_pix: Some(nfse_core::ChavePix::parse("+55 17 99999-8888").unwrap()),
            },
        )
        .await
        .unwrap();
    let v = c.emissor.validar(&pedido).await.unwrap();
    assert!(v.erros().is_empty(), "{:?}", v.erros());
    assert_eq!(v.dps.servico.descricao, "REPASSE 09/2026. PIX: +5517999998888");
    assert_eq!(
        v.dps.servico.informacoes_complementares.as_deref(),
        Some("Chave PIX +5517999998888")
    );

    // Variável errada nas complementares também é apontada, com o nome do campo.
    pedido.informacoes_complementares = Some("PIX {pixx}".into());
    let v = c.emissor.validar(&pedido).await.unwrap();
    assert!(
        v.regras
            .iter()
            .any(|r| r.starts_with("as informações complementares têm") && r.contains("{pixx}")),
        "{:?}",
        v.regras
    );
}

// ---- Importação de pedidos por e-mail (spec 0001) ----

fn pedidos_do_email(texto: &str) -> Vec<crate::pedido_email::PedidoEmail> {
    crate::pedido_email::extrair(texto, &["11222333000181".to_string()]).unwrap()
}

fn endereco_do_cep(municipio: &str) -> Option<importacao_email::EnderecoDoCep> {
    Some(importacao_email::EnderecoDoCep {
        municipio: municipio.into(),
        logradouro: "RUA DO CEP".into(),
        bairro: "BAIRRO DO CEP".into(),
    })
}

async fn resolver_email(
    c: &Cenario,
    texto: &str,
    servico: Option<i64>,
    ceps: &importacao_email::CepsConsultados,
) -> Vec<importacao::ItemImportado> {
    let outubro = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    importacao_email::resolver(
        c.emissor.banco(),
        &pedidos_do_email(texto),
        Some(c.pedido.empresa_id),
        servico,
        outubro,
        ceps,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn email_tomador_da_filial_valor_e_mes() {
    let c = cenario(vec![]).await;
    let texto = "Solicitamos a nota da EMPRESA TESTE (CNPJ 11.222.333/0001-81) para a filial \
                 CNPJ 11.444.777/0002-42, valor R$ 1.234,56, atendimentos de 07/2026.";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert_eq!(itens.len(), 1);
    let i = &itens[0];
    assert_eq!(i.documento_tomador, "11444777000242");
    assert_eq!(i.valor, Some(Valor::de_centesimos(123_456)));
    assert_eq!(i.mes, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    // CNPJ sem cadastro: o mesmo erro da planilha, que a tela resolve com "Cadastrar tomador".
    assert_eq!(i.erros, ["tomador 11444777000242 não cadastrado"]);
    assert!(i.tomador_novo.is_none());
}

#[tokio::test]
async fn email_sem_periodo_usa_mes_da_tela() {
    let c = cenario(vec![]).await;
    let itens = resolver_email(
        &c,
        "Nota para 12.ABC.345/01DE-35 de R$ 100,00.",
        Some(c.pedido.servico_id),
        &Default::default(),
    )
    .await;
    assert_eq!(itens[0].mes, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
    assert!(itens[0].erros.is_empty(), "{:?}", itens[0].erros);
    assert_eq!(itens[0].tomador.as_deref(), Some("CLIENTE EXEMPLO LTDA"));
    assert_eq!(
        c.emissor.validar_importado(&itens[0]).await.unwrap(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn email_sem_servico() {
    let c = cenario(vec![]).await;
    let texto =
        "PACIENTE: Ana\nCPF: 12.ABC.345/01DE-35\nVALOR: 10,00\n\nPACIENTE: Bia\nCPF: 12ABC34501DE35\nVALOR: 20,00";
    let itens = resolver_email(&c, texto, None, &Default::default()).await;
    assert_eq!(itens.len(), 2);
    for i in &itens {
        assert!(
            i.erros.contains(&importacao_email::ESCOLHA_SERVICO.to_string()),
            "{:?}",
            i.erros
        );
        assert!(!i.emitivel());
    }
    // Serviço de outra empresa também não vale.
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id + 1000), &Default::default()).await;
    assert!(itens[0].erros.contains(&importacao_email::ESCOLHA_SERVICO.to_string()));
}

#[tokio::test]
async fn email_descricao_do_bloco_e_padrao() {
    let c = cenario(vec![]).await;
    let banco = c.emissor.banco();
    let mut servico = banco.servico(c.pedido.servico_id).await.unwrap();
    servico.descricao = "SESSÕES {mes_ref}. PIX: {pix}".into();
    banco.salvar_servico(&servico).await.unwrap();
    let e = banco.empresa(c.pedido.empresa_id).await.unwrap();
    banco
        .atualizar_empresa(
            e.id,
            &NovaEmpresa {
                cnpj: e.cnpj.clone(),
                nome: e.nome.clone(),
                inscricao_municipal: e.inscricao_municipal.clone(),
                municipio_emissor: e.municipio_emissor.clone(),
                regime: e.regime.clone(),
                ambiente: e.ambiente,
                serie: e.serie.clone(),
                telefone: e.telefone.clone(),
                email: e.email.clone(),
                chave_pix: Some(nfse_core::ChavePix::parse("+55 17 99999-8888").unwrap()),
            },
        )
        .await
        .unwrap();

    let texto = "Pedidos de 07/2026\n\nPACIENTE: Ana\nCPF: 12ABC34501DE35\nVALOR: 10,00\n\
                 colocar no corpo da nota: Sessão de {mes_ref}\n\n\
                 PACIENTE: Bia\nCPF: 12ABC34501DE35\nVALOR: 20,00";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert_eq!(itens[0].descricao, "Sessão de 07/2026");
    assert_eq!(itens[1].descricao, "SESSÕES 07/2026. PIX: {pix}");
    let v = c.emissor.validar(&itens[1].pedido(None).unwrap()).await.unwrap();
    assert_eq!(v.dps.servico.descricao, "SESSÕES 07/2026. PIX: +5517999998888");
}

#[tokio::test]
async fn email_aviso_dados_diferentes() {
    let c = cenario(vec![]).await;
    let texto = "PACIENTE: Outro Nome Qualquer\nCPF: 12ABC34501DE35\nCEP: 01001-000\nVALOR: 10,00\n\n\
                 PACIENTE: cliente  exemplo ltda\nCPF: 12ABC34501DE35\nCEP: 15800-000\nVALOR: 20,00";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert!(itens[0].erros.is_empty(), "{:?}", itens[0].erros);
    assert_eq!(itens[0].tomador.as_deref(), Some("CLIENTE EXEMPLO LTDA"));
    assert_eq!(itens[0].avisos.len(), 1, "{:?}", itens[0].avisos);
    assert!(itens[0].avisos[0].contains("nome") && itens[0].avisos[0].contains("CEP"));
    // Só diferença de maiúsculas e espaços não é aviso.
    assert!(itens[1].avisos.is_empty(), "{:?}", itens[1].avisos);
}

#[tokio::test]
async fn email_pessoa_nova_e_municipio_do_cep() {
    let c = cenario(vec![]).await;
    let texto = "PACIENTE: Ana Nova\nCPF: 529.982.247-25\nENDEREÇO: Rua das Flores, 100\nBAIRRO: Centro\n\
                 CEP: 15800-000\nVALOR: 10,00\n\n\
                 PACIENTE: Sem Cep\nCPF: 111.444.777-35\nVALOR: 10,00\n\n\
                 PACIENTE: Cep Desconhecido\nCPF: 390.533.447-05\nCEP: 99999-999\nVALOR: 10,00\n\n\
                 PACIENTE: Sem Conexao\nCPF: 714.602.380-01\nCEP: 01001000\nVALOR: 10,00\n\n\
                 PACIENTE: Cliente\nCPF: 12ABC34501DE35\nCEP: 15800-000\nVALOR: 10,00";
    let pedidos = pedidos_do_email(texto);
    let ceps = importacao_email::ceps_a_consultar(c.emissor.banco(), &pedidos)
        .await
        .unwrap();
    // Só das pessoas sem cadastro, sem repetir; quem não tem CEP não entra.
    assert_eq!(ceps, ["15800000", "99999999", "01001000"]);

    // "99999999" não encontrado; "01001000" sem resposta (sem conexão): os dois sem endereço.
    let consultados: importacao_email::CepsConsultados = [
        ("15800000".to_string(), endereco_do_cep("3511102")),
        ("99999999".to_string(), None),
    ]
    .into();
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &consultados).await;

    let novo = itens[0].tomador_novo.as_ref().expect("tomador novo");
    assert!(itens[0].erros.is_empty(), "{:?}", itens[0].erros);
    assert_eq!(novo.nome, "Ana Nova");
    let end = novo.endereco.as_ref().unwrap();
    assert_eq!(
        (
            end.municipio.as_str(),
            end.cep.as_str(),
            end.logradouro.as_str(),
            end.numero.as_str(),
            end.bairro.as_str()
        ),
        ("3511102", "15800000", "Rua das Flores", "100", "Centro")
    );
    assert_eq!(itens[0].pessoa_id, None);
    assert_eq!(
        c.emissor.validar_importado(&itens[0]).await.unwrap(),
        Vec::<String>::new()
    );
    // Antes de emitir, a pessoa não está no banco.
    assert_eq!(
        c.emissor.banco().pessoa_por_documento("52998224725").await.unwrap(),
        None
    );

    for i in &itens[1..4] {
        assert_eq!(i.erros, [importacao_email::SEM_MUNICIPIO], "linha {}", i.linha);
        assert!(!i.emitivel());
    }
    assert!(itens[4].tomador_novo.is_none() && itens[4].erros.is_empty());
}

#[tokio::test]
async fn email_valor_invalido_igual_planilha() {
    let c = cenario(vec![]).await;
    let texto = "PACIENTE: Ana\nCPF: 12ABC34501DE35\n\nPACIENTE: Bia\nCPF: 12ABC34501DE35\nVALOR: R$ 150,555";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;

    let csv = "Tomador;Valor\r\n12ABC34501DE35;\r\n12ABC34501DE35;R$ 150,555\r\n";
    let linhas = crate::planilha::ler("p.csv", csv.as_bytes()).unwrap();
    let outubro = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let planilha = importacao::resolver(c.emissor.banco(), &linhas, Some(c.pedido.empresa_id), outubro)
        .await
        .unwrap();
    assert_eq!(itens[0].erros, planilha[0].erros);
    assert_eq!(itens[1].erros, planilha[1].erros);
    assert!(!itens[1].erros.is_empty());
}

#[tokio::test]
async fn email_bloco_repetido() {
    let c = cenario(vec![]).await;
    let bloco = "PACIENTE: Ana\nCPF: 12ABC34501DE35\nVALOR: 10,00\n\n";
    let texto = format!("{bloco}PACIENTE: Bia\nCPF: 12ABC34501DE35\nVALOR: 11,00\n\n> {bloco}");
    let itens = resolver_email(&c, &texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert_eq!(itens.len(), 3);
    assert!(itens[0].erros.is_empty() && itens[1].erros.is_empty());
    assert_eq!(itens[2].erros, ["repetido (já está na linha 1)"]);
    assert_eq!(itens[2].chave, itens[0].chave);
}

#[tokio::test]
async fn email_reimportado_ja_emitido() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let texto = "PACIENTE: Ana\nCPF: 12ABC34501DE35\nVALOR: 10,00";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert!(itens[0].emitivel());
    assert!(matches!(
        c.emissor.emitir_importado(&itens[0], Some("pedido.eml")).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    let de_novo = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert_eq!(de_novo[0].anterior.as_ref().unwrap().situacao, SituacaoDps::Autorizada);
    assert!(!de_novo[0].emitivel());
    assert!(c.emissor.emitir_importado(&de_novo[0], None).await.is_err());
    assert_eq!(c.sefin.recebidos().len(), 1);
}

/// Pessoa física cadastrada pela empresa do cenário, com endereço em Catanduva.
async fn paciente_cadastrado(c: &Cenario, cpf: &str, nome: &str) -> i64 {
    c.emissor
        .banco()
        .salvar_tomador(
            c.pedido.empresa_id,
            &Tomador {
                documento: Documento::Cpf(nfse_core::Cpf::parse(cpf).unwrap()),
                inscricao_municipal: None,
                nome: nome.into(),
                endereco: Some(EnderecoNacional {
                    municipio: "3511102".into(),
                    cep: "15800000".into(),
                    logradouro: "RUA VELHA".into(),
                    numero: "1".into(),
                    complemento: None,
                    bairro: "CENTRO".into(),
                }),
                telefone: None,
                email: None,
            },
            "manual",
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn email_cpf_cadastrado_vale_o_cadastro_na_emissao() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    paciente_cadastrado(&c, "52998224725", "ANA ANTIGA").await;
    let antes = c.emissor.banco().pessoa_cadastrada("52998224725").await.unwrap();
    let texto = "PACIENTE: Ana Antiga\nCPF: 529.982.247-25\nENDEREÇO: Rua Nova, 5\nCEP: 01001-000\nVALOR: 10,00";
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &Default::default()).await;
    assert!(itens[0].emitivel(), "{:?}", itens[0].erros);
    assert!(itens[0].tomador_novo.is_none());
    assert_eq!(itens[0].avisos.len(), 1, "{:?}", itens[0].avisos);

    assert!(matches!(
        c.emissor.emitir_importado(&itens[0], Some("pedido.eml")).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    let enviado = &c.sefin.recebidos()[0];
    assert!(enviado.contains("<xLgr>RUA VELHA</xLgr>"), "{enviado}");
    assert!(!enviado.contains("Rua Nova"));
    assert_eq!(c.emissor.banco().pessoa_cadastrada("52998224725").await.unwrap(), antes);
}

#[tokio::test]
async fn email_pessoa_nova_cadastrada_so_ao_emitir() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let texto = "PACIENTE: Ana Nova\nCPF: 529.982.247-25\nENDEREÇO: Rua das Flores, 100\nBAIRRO: Centro\n\
                 CEP: 15800-000\nVALOR: 10,00";
    let ceps: importacao_email::CepsConsultados = [("15800000".to_string(), endereco_do_cep("3511102"))].into();
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &ceps).await;
    assert!(itens[0].emitivel(), "{:?}", itens[0].erros);
    let banco = c.emissor.banco();
    assert_eq!(banco.pessoa_por_documento("52998224725").await.unwrap(), None);

    assert!(matches!(
        c.emissor.emitir_importado(&itens[0], Some("pedido.eml")).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    let pessoa = banco
        .pessoa_por_documento("52998224725")
        .await
        .unwrap()
        .expect("cadastrada");
    let gravado = banco.pessoa_cadastrada("52998224725").await.unwrap().unwrap();
    assert_eq!(gravado.nome, "Ana Nova");
    let end = gravado.endereco.unwrap();
    assert_eq!(
        (
            end.municipio.as_str(),
            end.cep.as_str(),
            end.logradouro.as_str(),
            end.numero.as_str(),
            end.bairro.as_str()
        ),
        ("3511102", "15800000", "Rua das Flores", "100", "Centro")
    );
    assert!(
        banco.tomador(c.pedido.empresa_id, pessoa).await.is_ok(),
        "vinculada à empresa"
    );
    let enviado = &c.sefin.recebidos()[0];
    assert!(enviado.contains("<CPF>52998224725</CPF>"), "{enviado}");
    assert!(enviado.contains("<xNome>Ana Nova</xNome>"));
}

#[tokio::test]
async fn email_pessoa_cadastrada_entre_leitura_e_emissao() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let texto = "PACIENTE: Ana Nova\nCPF: 529.982.247-25\nCEP: 15800-000\nVALOR: 10,00";
    let ceps: importacao_email::CepsConsultados = [("15800000".to_string(), endereco_do_cep("3511102"))].into();
    let itens = resolver_email(&c, texto, Some(c.pedido.servico_id), &ceps).await;
    assert!(itens[0].tomador_novo.is_some());
    // Alguém cadastrou a pessoa pela tela depois da leitura: o cadastro dela não é sobrescrito.
    let pessoa = paciente_cadastrado(&c, "52998224725", "ANA DO CADASTRO").await;
    let antes = c.emissor.banco().pessoa_cadastrada("52998224725").await.unwrap();

    assert!(matches!(
        c.emissor.emitir_importado(&itens[0], None).await.unwrap(),
        ResultadoEmissao::Autorizada { .. }
    ));
    let banco = c.emissor.banco();
    assert_eq!(banco.pessoa_cadastrada("52998224725").await.unwrap(), antes);
    assert_eq!(banco.pessoa_por_documento("52998224725").await.unwrap(), Some(pessoa));
    assert!(c.sefin.recebidos()[0].contains("<xNome>ANA DO CADASTRO</xNome>"));
}

#[tokio::test]
async fn email_origem_guarda_so_arquivo_e_linha() {
    let c = cenario(vec![Resposta::Emitir(Ok(nfse_autorizada()))]).await;
    let sigilo = "TRECHO-QUE-NAO-PODE-SER-GUARDADO";
    let texto = format!(
        "Bom dia, {sigilo}.\n\nPACIENTE: Ana\nCPF: 12ABC34501DE35\nVALOR: 10,00\n\n\
         PACIENTE: Bia\nCPF: 12ABC34501DE35\nVALOR: 20,00\n\nAtenciosamente, {sigilo}"
    );
    let itens = resolver_email(&c, &texto, Some(c.pedido.servico_id), &Default::default()).await;
    let ResultadoEmissao::Autorizada { nfse, .. } =
        c.emissor.emitir_importado(&itens[1], Some("pedido.eml")).await.unwrap()
    else {
        panic!("não autorizada");
    };
    let banco = c.emissor.banco();
    assert_eq!(
        banco.origem_importada(nfse.dps_id).await.unwrap(),
        [(itens[1].chave.clone(), Some("pedido.eml".to_string()), Some(2))]
    );
    assert!(!itens[1].chave.contains(sigilo));
    for (_, _, _, detalhe) in banco.auditoria(100).await.unwrap() {
        assert!(!detalhe.unwrap_or_default().contains(sigilo));
    }
    assert!(!c.sefin.recebidos()[0].contains(sigilo));
    // O caminho do e-mail não registra nada em log: o corpo não tem como chegar lá.
    for fonte in [
        include_str!("leitura_email.rs"),
        include_str!("pedido_email.rs"),
        include_str!("importacao_email.rs"),
    ] {
        for proibido in ["tracing::", "log::", "println!", "eprintln!", "dbg!"] {
            assert!(!fonte.contains(proibido), "{proibido} no caminho do e-mail");
        }
    }
}
