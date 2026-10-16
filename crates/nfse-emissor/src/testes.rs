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

    fn conectar(&self, _: &CertificadoA1, _: Ambiente) -> Result<SefinSimulada, ErroSefin> {
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
