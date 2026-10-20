use super::*;
use dados::TRACO;

/// NFS-e fictícia no formato da Sefin, com tomador e Simples Nacional (como uma nota real).
const NFSE: &str = include_str!("../tests/fixtures/nfse-exemplo.xml");

fn fontes() -> Fontes {
    Fontes::do_sistema().expect("Arial/Microsoft Sans Serif (Windows) ou Liberation Sans (Linux)")
}

#[test]
fn le_os_campos_da_nfse() {
    let d = dados::ler(NFSE).unwrap();
    assert_eq!(d.chave, "35111022211222333000181000000000007726107413634280");
    assert_eq!(d.numero, "77");
    assert_eq!(d.competencia, "16/10/2026");
    assert_eq!(d.emissao_nfse, "16/10/2026 16:12:30");
    assert_eq!(d.emitente, "Prestador");
    assert_eq!(d.situacao, "NFS-e Gerada");
    assert_eq!(d.municipio_cabecalho.as_deref(), Some("Município: Catanduva / SP"));
    assert_eq!(d.tipo_ambiente, "Produção");
    assert_eq!(d.prestador.documento, "11.222.333/0001-81");
    assert_eq!(d.prestador.nome, "EMPRESA TESTE LTDA", "nome vem do grupo emit");
    assert_eq!(d.prestador.endereco, "AVENIDA EXEMPLO, 100, CENTRO");
    assert_eq!(d.prestador.municipio_uf, "Catanduva / SP");
    assert_eq!(d.prestador.ibge_cep, "3511102 / 15.800-000");
    assert!(d.simples_nacional.starts_with("Optante - Microempresa"));
    let tomador = d.tomador.as_ref().unwrap();
    assert_eq!(tomador.documento, "12.ABC.345/01DE-35");
    assert_eq!(tomador.municipio_uf, "Catanduva / SP");
    assert_eq!(d.destinatario, Destinatario::ProprioTomador);
    assert_eq!(d.codigo_tributacao, "17.19.01");
    assert_eq!(d.nbs, "1.1302.21.00");
    assert_eq!(d.local_prestacao, "Catanduva / SP / BR");
    assert_eq!(d.totais.servico, "R$ 1,00");
    assert_eq!(d.totais.liquido, "R$ 1,00");
    assert_eq!(d.totais.liquido_com_ibs_cbs, TRACO);
    assert_eq!(d.federal.descricao_retidas, "PIS/COFINS/CSLL Não Retidos");
    assert!(d.federal.mostrar_pis_cofins);
    assert!(
        d.informacoes_complementares
            .ends_with("Totais Aproximados dos Tributos cfe. Lei nº 12.741/2012: Simples Nacional: 11,48%")
    );
}

#[test]
fn gera_pdf_de_uma_pagina() {
    let pdf = gerar_danfse(NFSE, &Opcoes::default(), &fontes()).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let paginas = pdf
        .windows(b"/Type /Page\n".len())
        .filter(|j| *j == b"/Type /Page\n")
        .count()
        + pdf
            .windows(b"/Type /Page/".len())
            .filter(|j| *j == b"/Type /Page/")
            .count()
        + pdf
            .windows(b"/Type /Page ".len())
            .filter(|j| *j == b"/Type /Page ")
            .count();
    assert!(paginas <= 1, "DANFSe deve ter página única");
}

#[test]
fn gera_pdf_com_a_logomarca_da_nfse() {
    let logo = include_bytes!("../../app/assets/logo-nfse.png").to_vec();
    let sem = gerar_danfse(NFSE, &Opcoes::default(), &fontes()).unwrap();
    let com = gerar_danfse(
        NFSE,
        &Opcoes {
            logo_png: Some(logo),
            ..Opcoes::default()
        },
        &fontes(),
    )
    .unwrap();
    assert!(com.starts_with(b"%PDF-"));
    assert!(com.len() > sem.len(), "a imagem deve entrar no PDF");
}

#[test]
fn cancelada_homologacao_e_textos_longos() {
    let longo = NFSE.replace("<tpAmb>1</tpAmb>", "<tpAmb>2</tpAmb>").replace(
        "SERVIÇOS CONTÁBEIS REFERENTE A 09/2026",
        &"Descrição muito longa do serviço prestado. ".repeat(40),
    );
    let pdf = gerar_danfse(
        &longo,
        &Opcoes {
            situacao: Situacao::Cancelada,
            logo_png: None,
        },
        &fontes(),
    )
    .unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn corta_e_quebra_pelo_tamanho_real() {
    let f = fontes();
    let cortado = caber(&f.conteudo, 7.0, &"X".repeat(200), 5.0);
    assert!(cortado.ends_with("..."));
    assert!(f.conteudo.largura(&cortado, 7.0) <= 5.0 * CM);
    let linhas = quebrar(&f.conteudo, 7.0, &"palavra ".repeat(100), 10.0);
    assert!(linhas.len() > 3);
    assert!(linhas.iter().all(|l| f.conteudo.largura(l, 7.0) <= 10.0 * CM));
}
