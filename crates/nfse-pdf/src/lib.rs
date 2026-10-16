//! DANFSe v2.0 (NT 008/2026) gerado localmente a partir do XML autorizado da NFS-e.
//!
//! Página única A4 retrato, posições e tamanhos do item 2.4.5 da NT, fontes do item 2.4,
//! QR Code de consulta pública, supressões dos itens 2.3 e marca d'água de cancelada/substituída.
//! O bloco opcional de canhoto não é impresso; o espaço vai para "Informações Complementares".

pub mod dados;
pub mod fontes;

use krilla::Document;
use krilla::color::{luma, rgb};
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Paint, Stroke};
use krilla::surface::Surface;
use krilla::text::TextDirection;

use dados::{Danfse, Destinatario, Parte};
pub use fontes::{ErroFonte, Fonte, Fontes};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Situacao {
    #[default]
    Normal,
    Cancelada,
    Substituida,
}

#[derive(Debug, Clone, Default)]
pub struct Opcoes {
    pub situacao: Situacao,
    /// Logomarca oficial da NFS-e (PNG). Sem ela, o espaço recebe o texto "NFS-e".
    pub logo_png: Option<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErroPdf {
    #[error(transparent)]
    Dados(#[from] dados::ErroDados),
    #[error(transparent)]
    Fonte(#[from] ErroFonte),
    #[error("falha ao gerar o PDF: {0}")]
    Pdf(String),
}

pub const URL_CONSULTA: &str = "https://www.nfse.gov.br/ConsultaPublica/?tpc=1&chave=";

const CM: f32 = 72.0 / 2.54;
/// Bordas das quatro colunas do formulário (cm).
const COL: [f32; 5] = [0.30, 5.41, 10.51, 15.62, 20.70];
const LARGURA: f32 = 20.40;
const RODAPE: f32 = 29.40;
const LINHA: f32 = 0.63;
const LINHA_TOTAIS: f32 = 0.67;
const MENSAGEM: f32 = 0.32;
const ENTRELINHA: f32 = 0.30;
/// 5% de densidade (sombreamento do cabeçalho e títulos).
const CINZA_CLARO: u8 = 242;
/// K35 (marca d'água).
const CINZA_MARCA: u8 = 166;

struct Celula<'a> {
    de: usize,
    ate: usize,
    rotulo: &'a str,
    valor: &'a str,
    fundo: bool,
}

fn c<'a>(de: usize, ate: usize, rotulo: &'a str, valor: &'a str) -> Celula<'a> {
    Celula {
        de,
        ate,
        rotulo,
        valor,
        fundo: false,
    }
}

struct Pintor<'s, 'f> {
    s: Surface<'s>,
    f: &'f Fontes,
}

impl Pintor<'_, '_> {
    fn preencher(&mut self, paint: Paint) {
        self.s.set_fill(Some(Fill {
            paint,
            opacity: NormalizedF32::ONE,
            rule: Default::default(),
        }));
    }

    fn preto(&mut self) {
        self.preencher(luma::Color::black().into());
    }

    fn retangulo(&mut self, x: f32, y: f32, w: f32, h: f32, fundo: Option<u8>, borda: Option<f32>) {
        let Some(rect) = Rect::from_xywh(x * CM, y * CM, w * CM, h * CM) else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.push_rect(rect);
        let Some(caminho) = pb.finish() else { return };
        if let Some(cinza) = fundo {
            self.preencher(luma::Color::new(cinza).into());
            self.s.set_stroke(None);
            self.s.draw_path(&caminho);
        }
        if let Some(espessura) = borda {
            self.s.set_fill(None);
            self.s.set_stroke(Some(Stroke {
                width: espessura,
                ..Default::default()
            }));
            self.s.draw_path(&caminho);
            self.s.set_stroke(None);
        }
        self.preto();
    }

    fn texto(&mut self, x: f32, base: f32, fonte: &Fonte, tamanho: f32, texto: &str) {
        if texto.is_empty() {
            return;
        }
        self.s.draw_text(
            Point::from_xy(x * CM, base * CM),
            fonte.krilla.clone(),
            tamanho,
            texto,
            false,
            TextDirection::Auto,
        );
    }

    fn texto_centralizado(&mut self, x: f32, largura: f32, base: f32, fonte: &Fonte, tamanho: f32, texto: &str) {
        let w = fonte.largura(texto, tamanho) / CM;
        self.texto(x + (largura - w).max(0.0) / 2.0, base, fonte, tamanho, texto);
    }

    fn rotulo_bloco(&mut self, x: f32, y: f32, w: f32, h: f32, texto: &str) {
        self.retangulo(x, y, w, h, Some(CINZA_CLARO), Some(0.5));
        let f = self.f.titulo_negrito.clone();
        let texto = caber(&f, 7.0, &texto.to_uppercase(), w - 0.16);
        self.texto(x + 0.08, y + h / 2.0 + 0.09, &f, 7.0, &texto);
    }

    #[allow(clippy::too_many_arguments)]
    fn celula(&mut self, x: f32, y: f32, w: f32, h: f32, rotulo: &str, valor: &str, fundo: bool, rotulo_tam: f32) {
        self.retangulo(x, y, w, h, fundo.then_some(CINZA_CLARO), Some(0.5));
        let negrito = self.f.titulo_negrito.clone();
        let conteudo = self.f.conteudo.clone();
        if !rotulo.is_empty() {
            let r = caber(&negrito, rotulo_tam, rotulo, w - 0.16);
            self.texto(x + 0.08, y + 0.22, &negrito, rotulo_tam, &r);
        }
        let v = caber(&conteudo, 7.0, valor, w - 0.16);
        let base = if rotulo.is_empty() {
            y + h / 2.0 + 0.09
        } else {
            y + h - 0.10
        };
        self.texto(x + 0.08, base, &conteudo, 7.0, &v);
    }

    /// Linha de 4 colunas; `titulo` ocupa a primeira coluna (sombreada).
    fn linha(&mut self, y: f32, h: f32, titulo: Option<&str>, celulas: &[Celula]) {
        if let Some(t) = titulo {
            self.rotulo_bloco(COL[0], y, COL[1] - COL[0], h, t);
        }
        for cel in celulas {
            let (x, w) = (COL[cel.de], COL[cel.ate] - COL[cel.de]);
            self.celula(x, y, w, h, cel.rotulo, cel.valor, cel.fundo, 6.0);
        }
    }

    fn mensagem(&mut self, y: f32, texto: &str) {
        self.rotulo_bloco(COL[0], y, LARGURA, MENSAGEM, texto);
    }

    /// Caixa de texto livre com quebra de linha; corta com reticências o que não couber.
    fn texto_livre(&mut self, y: f32, h: f32, rotulo: Option<&str>, texto: &str) {
        self.retangulo(COL[0], y, LARGURA, h, None, Some(0.5));
        let mut base = y + 0.24;
        if let Some(r) = rotulo {
            let f = self.f.titulo_negrito.clone();
            self.texto(COL[0] + 0.08, base, &f, 6.0, r);
            base += 0.28;
        }
        let f = self.f.conteudo.clone();
        let linhas = quebrar(&f, 7.0, texto, LARGURA - 0.16);
        let cabem = (((y + h - 0.08) - base) / ENTRELINHA).floor().max(0.0) as usize + 1;
        for (i, l) in linhas.iter().take(cabem).enumerate() {
            let l = if i + 1 == cabem && linhas.len() > cabem {
                caber(&f, 7.0, &format!("{l} ..."), LARGURA - 0.16)
            } else {
                l.clone()
            };
            self.texto(COL[0] + 0.08, base + i as f32 * ENTRELINHA, &f, 7.0, &l);
        }
    }
}

/// Corta o texto com reticências para caber em `largura` cm.
fn caber(fonte: &Fonte, tamanho: f32, texto: &str, largura: f32) -> String {
    let limite = largura * CM;
    if fonte.largura(texto, tamanho) <= limite {
        return texto.to_owned();
    }
    let mut s: String = texto.to_owned();
    while !s.is_empty() && fonte.largura(&format!("{s}..."), tamanho) > limite {
        s.pop();
    }
    format!("{}...", s.trim_end())
}

/// Quebra em linhas que cabem em `largura` cm, respeitando quebras de linha do texto.
fn quebrar(fonte: &Fonte, tamanho: f32, texto: &str, largura: f32) -> Vec<String> {
    let limite = largura * CM;
    let mut linhas = Vec::new();
    for paragrafo in texto.lines() {
        let mut atual = String::new();
        for palavra in paragrafo.split_whitespace() {
            let tentativa = if atual.is_empty() {
                palavra.to_owned()
            } else {
                format!("{atual} {palavra}")
            };
            if fonte.largura(&tentativa, tamanho) <= limite {
                atual = tentativa;
                continue;
            }
            if !atual.is_empty() {
                linhas.push(std::mem::take(&mut atual));
            }
            // Palavra maior que a linha: quebra por caractere.
            let mut pedaco = String::new();
            for ch in palavra.chars() {
                pedaco.push(ch);
                if fonte.largura(&pedaco, tamanho) > limite {
                    pedaco.pop();
                    linhas.push(std::mem::take(&mut pedaco));
                    pedaco.push(ch);
                }
            }
            atual = pedaco;
        }
        linhas.push(atual);
    }
    linhas
}

fn altura_texto(linhas: usize, com_rotulo: bool) -> f32 {
    (if com_rotulo { 0.52 } else { 0.24 }) + linhas.max(1) as f32 * ENTRELINHA
}

fn parte_completa(p: &mut Pintor, y: f32, titulo: &str, parte: &Parte, com_im: bool) -> f32 {
    let mut primeira = vec![c(1, 2, "CNPJ / CPF / NIF", &parte.documento)];
    if com_im {
        primeira.push(c(2, 3, "Indicador Municipal (Inscrição)", &parte.inscricao_municipal));
    } else {
        primeira[0].ate = 3;
    }
    primeira.push(c(3, 4, "Telefone", &parte.telefone));
    p.linha(y, LINHA, Some(titulo), &primeira);
    p.linha(
        y + LINHA,
        LINHA,
        None,
        &[
            c(0, 2, "Nome / Nome Empresarial", &parte.nome),
            c(2, 3, "Município / Sigla UF", &parte.municipio_uf),
            c(3, 4, "Código IBGE / CEP", &parte.ibge_cep),
        ],
    );
    p.linha(
        y + 2.0 * LINHA,
        LINHA,
        None,
        &[c(0, 2, "Endereço", &parte.endereco), c(2, 4, "E-mail", &parte.email)],
    );
    y + 3.0 * LINHA
}

fn desenhar_qr(p: &mut Pintor, x: f32, y: f32, lado: f32, conteudo: &str) -> Result<(), ErroPdf> {
    let qr = qrcode::QrCode::with_error_correction_level(conteudo, qrcode::EcLevel::M)
        .map_err(|e| ErroPdf::Pdf(e.to_string()))?;
    let n = qr.width();
    let modulo = lado / n as f32;
    let cores = qr.to_colors();
    let mut pb = PathBuilder::new();
    for linha in 0..n {
        let mut coluna = 0;
        while coluna < n {
            if cores[linha * n + coluna] == qrcode::Color::Dark {
                let inicio = coluna;
                while coluna < n && cores[linha * n + coluna] == qrcode::Color::Dark {
                    coluna += 1;
                }
                if let Some(r) = Rect::from_xywh(
                    (x + inicio as f32 * modulo) * CM,
                    (y + linha as f32 * modulo) * CM,
                    (coluna - inicio) as f32 * modulo * CM,
                    modulo * CM,
                ) {
                    pb.push_rect(r);
                }
            } else {
                coluna += 1;
            }
        }
    }
    if let Some(caminho) = pb.finish() {
        p.preto();
        p.s.draw_path(&caminho);
    }
    Ok(())
}

fn cabecalho(p: &mut Pintor, d: &Danfse, logo: Option<&[u8]>) {
    p.retangulo(COL[0], 0.30, LARGURA, 1.16, Some(CINZA_CLARO), Some(0.5));
    p.retangulo(5.41, 0.30, 10.19, 1.16, None, Some(0.5));

    match logo.and_then(|png| Image::from_png(png.to_vec().into(), true).ok()) {
        Some(img) => {
            let (iw, ih) = img.size();
            let escala = (4.00 / iw as f32).min(0.85 / ih as f32);
            let (w, h) = (iw as f32 * escala, ih as f32 * escala);
            if let Some(tam) = Size::from_wh(w * CM, h * CM) {
                p.s.push_transform(&Transform::from_translate(0.49 * CM, (0.44 + (0.85 - h) / 2.0) * CM));
                p.s.draw_image(img, tam);
                p.s.pop();
            }
        }
        None => {
            let f = p.f.titulo_negrito.clone();
            p.texto(0.49, 0.95, &f, 16.0, "NFS-e");
            let f = p.f.titulo.clone();
            p.texto(0.49, 1.25, &f, 6.0, "Nota Fiscal de Serviço eletrônica");
        }
    }

    let negrito = p.f.titulo_negrito.clone();
    p.texto_centralizado(5.41, 10.19, 0.68, &negrito, 9.0, "DANFSe v2.0");
    p.texto_centralizado(5.41, 10.19, 1.02, &negrito, 9.0, "Documento Auxiliar da NFS-e");
    if d.homologacao {
        p.preencher(rgb::Color::new(255, 0, 0).into());
        p.texto_centralizado(5.41, 10.19, 1.36, &negrito, 9.0, "NFS-e SEM VALIDADE JURÍDICA");
        p.preto();
    }

    let conteudo = p.f.conteudo.clone();
    if let Some(m) = &d.municipio_cabecalho {
        let m = caber(&conteudo, 8.0, m, 4.93);
        p.texto(15.70, 0.72, &conteudo, 8.0, &m);
    }
    p.texto(15.70, 1.15, &conteudo, 6.0, &d.ambiente_gerador);
    p.texto(15.70, 1.40, &conteudo, 6.0, &d.tipo_ambiente);
}

fn identificacao(p: &mut Pintor, d: &Danfse) -> Result<(), ErroPdf> {
    p.retangulo(COL[0], 1.48, LARGURA, 2.84, None, Some(0.5));
    p.celula(
        0.30,
        1.48,
        15.32,
        0.77,
        "CHAVE DE ACESSO DA NFS-E",
        &d.chave,
        false,
        7.0,
    );
    let linhas: [[(&str, &str, bool); 3]; 3] = [
        [
            ("NÚMERO DA NFS-E", &d.numero, false),
            ("COMPETÊNCIA DA NFS-E", &d.competencia, false),
            ("DATA E HORA DA EMISSÃO DA NFS-E", &d.emissao_nfse, false),
        ],
        [
            ("NÚMERO DA DPS", &d.numero_dps, false),
            ("SÉRIE DA DPS", &d.serie_dps, false),
            ("DATA E HORA DA EMISSÃO DA DPS", &d.emissao_dps, false),
        ],
        [
            ("EMITENTE DA NFS-E", &d.emitente, true),
            ("SITUAÇÃO DA NFS-E", &d.situacao, false),
            ("FINALIDADE", &d.finalidade, false),
        ],
    ];
    for (i, linha) in linhas.iter().enumerate() {
        let y = 2.25 + i as f32 * 0.69;
        for (j, (rotulo, valor, fundo)) in linha.iter().enumerate() {
            p.celula(COL[j], y, COL[j + 1] - COL[j], 0.69, rotulo, valor, *fundo, 7.0);
        }
    }
    desenhar_qr(p, 17.48, 1.67, 1.52, &format!("{URL_CONSULTA}{}", d.chave))?;
    let f = p.f.conteudo.clone();
    let aviso = "A autenticidade desta NFS-e pode ser verificada pela leitura deste código QR ou pela consulta da chave de acesso no portal nacional da NFS-e";
    for (i, l) in quebrar(&f, 6.0, aviso, 4.72).iter().take(3).enumerate() {
        p.texto_centralizado(15.80, 4.72, 3.48 + i as f32 * 0.24, &f, 6.0, l);
    }
    Ok(())
}

fn marca_dagua(p: &mut Pintor, texto: &str) {
    let f = p.f.titulo.clone();
    let tamanho = 72.0;
    let largura = f.largura(texto, tamanho);
    let (cx, cy) = (10.5 * CM, 14.85 * CM);
    p.preencher(luma::Color::new(CINZA_MARCA).into());
    p.s.push_transform(&Transform::from_rotate_at(-45.0, cx, cy));
    p.s.draw_text(
        Point::from_xy(cx - largura / 2.0, cy + tamanho * 0.35),
        f.krilla.clone(),
        tamanho,
        texto,
        false,
        TextDirection::Auto,
    );
    p.s.pop();
    p.preto();
}

fn desenhar(
    p: &mut Pintor,
    d: &Danfse,
    opcoes: &Opcoes,
    inicio: f32,
    altura_desc: f32,
    altura_info: f32,
) -> Result<(), ErroPdf> {
    // Marca d'água primeiro, para ficar atrás do conteúdo.
    match opcoes.situacao {
        Situacao::Normal => {}
        Situacao::Cancelada => marca_dagua(p, "CANCELADA"),
        Situacao::Substituida => marca_dagua(p, "SUBSTITUÍDA"),
    }
    cabecalho(p, d, opcoes.logo_png.as_deref());
    identificacao(p, d)?;

    let mut y = inicio;
    y = parte_completa(p, y, "PRESTADOR / FORNECEDOR", &d.prestador, true);
    p.linha(
        y,
        LINHA,
        None,
        &[
            c(0, 1, "Simples Nacional na Data de Competência", &d.simples_nacional),
            c(1, 4, "Regime de Apuração Tributária pelo SN", &d.regime_apuracao_sn),
        ],
    );
    y += LINHA;

    y = match &d.tomador {
        Some(t) => parte_completa(p, y, "TOMADOR / ADQUIRENTE", t, true),
        None => {
            p.mensagem(y, "TOMADOR/ADQUIRENTE DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e");
            y + MENSAGEM
        }
    };
    y = match &d.destinatario {
        Destinatario::Outro(dest) => parte_completa(p, y, "DESTINATÁRIO DA OPERAÇÃO", dest, false),
        Destinatario::ProprioTomador => {
            p.mensagem(y, "O DESTINATÁRIO É O PRÓPRIO TOMADOR/ADQUIRENTE DA OPERAÇÃO");
            y + MENSAGEM
        }
        Destinatario::NaoIdentificado => {
            p.mensagem(y, "DESTINATÁRIO DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e");
            y + MENSAGEM
        }
    };
    y = match &d.intermediario {
        Some(i) => parte_completa(p, y, "INTERMEDIÁRIO DA OPERAÇÃO", i, true),
        None => {
            p.mensagem(y, "INTERMEDIÁRIO DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e");
            y + MENSAGEM
        }
    };

    p.linha(
        y,
        LINHA,
        Some("SERVIÇO PRESTADO"),
        &[
            c(1, 2, "Código de Tributação Nacional / Municipal", &d.codigo_tributacao),
            c(2, 3, "Código da NBS", &d.nbs),
            c(3, 4, "Local da Prestação / Sigla UF / País", &d.local_prestacao),
        ],
    );
    y += LINHA;
    p.celula(COL[0], y, LARGURA, 0.38, "", &d.descricao_codigo, false, 6.0);
    y += 0.38;
    p.texto_livre(y, altura_desc, Some("Descrição do Serviço"), &d.descricao_servico);
    y += altura_desc;

    match &d.issqn {
        Some(i) => {
            p.linha(
                y,
                LINHA,
                Some("TRIBUTAÇÃO MUNICIPAL (ISSQN)"),
                &[
                    c(1, 2, "Tipo de Tributação do ISSQN", &i.tipo),
                    c(
                        2,
                        4,
                        "Município / Sigla UF / País de Incidência do ISSQN",
                        &i.municipio_incidencia,
                    ),
                ],
            );
            y += LINHA;
            if i.mostrar_regime {
                p.linha(
                    y,
                    LINHA,
                    None,
                    &[
                        c(0, 1, "Regime Especial de Tributação do ISSQN", &i.regime_especial),
                        c(1, 2, "Tipo de Imunidade do ISSQN", &i.imunidade),
                        c(2, 3, "Suspensão da Exigibilidade do ISSQN", &i.suspensao),
                        c(3, 4, "Número Processo Suspensão", &i.processo),
                    ],
                );
                y += LINHA;
            }
            if i.mostrar_beneficio {
                p.linha(
                    y,
                    LINHA,
                    None,
                    &[
                        c(0, 1, "Benefício Municipal", &i.beneficio),
                        c(1, 2, "Cálculo do BM", &i.calculo_beneficio),
                        c(2, 3, "Total Deduções/Reduções", &i.deducoes),
                        c(3, 4, "Desconto Incondicionado", &i.desconto_incondicionado),
                    ],
                );
                y += LINHA;
            }
            p.linha(
                y,
                LINHA,
                None,
                &[
                    c(0, 1, "BC ISSQN", &i.base_calculo),
                    c(1, 2, "Alíquota Aplicada", &i.aliquota),
                    c(2, 3, "Retenção do ISSQN", &i.retencao),
                    c(3, 4, "ISSQN Apurado", &i.apurado),
                ],
            );
            y += LINHA;
        }
        None => {
            p.mensagem(y, "TRIBUTAÇÃO MUNICIPAL (ISSQN) - OPERAÇÃO NÃO SUJEITA AO ISSQN");
            y += MENSAGEM;
        }
    }

    let f = &d.federal;
    p.linha(
        y,
        LINHA,
        Some("TRIBUTAÇÃO FEDERAL (EXCETO CBS)"),
        &[
            c(1, 2, "IRRF", &f.irrf),
            c(2, 3, "Contribuição Previdenciária - Retida", &f.previdenciaria),
            c(3, 4, "Contribuições Sociais - Retidas", &f.sociais),
        ],
    );
    y += LINHA;
    if f.mostrar_pis_cofins {
        p.linha(
            y,
            LINHA,
            None,
            &[
                c(0, 1, "PIS - Débito Apuração Própria", &f.pis),
                c(1, 2, "COFINS - Débito Apuração Própria", &f.cofins),
                c(2, 4, "Descrição Contrib. Sociais - Retidas", &f.descricao_retidas),
            ],
        );
        y += LINHA;
    }

    let t = &d.ibs_cbs;
    p.linha(
        y,
        LINHA,
        Some("TRIBUTAÇÃO IBS / CBS"),
        &[
            c(1, 2, "CST / cClassTrib", &t.cst_class),
            c(
                2,
                4,
                "Indicador de Operação / Código IBGE Incidência / Município Incidência / Sigla UF",
                &t.indicador_local,
            ),
        ],
    );
    y += LINHA;
    for linha in [
        [
            ("Exclusões e Reduções da Base de Cálculo", &t.exclusoes),
            ("Base de Cálculo Após Exclusões e Reduções", &t.base_calculo),
            ("Red. Alíquota IBS / Red. Alíquota CBS", &t.reducoes),
            ("Alíquota - IBS UF / IBS Mun", &t.aliquotas_ibs),
        ],
        [
            ("Alíq. Efetiva Municipal - IBS", &t.aliq_efetiva_mun),
            ("Valor Apurado Municipal - IBS", &t.valor_mun),
            ("Alíq. Efetiva Estadual - IBS", &t.aliq_efetiva_uf),
            ("Valor Apurado Estadual - IBS", &t.valor_uf),
        ],
        [
            ("Valor Total Apurado - IBS", &t.total_ibs),
            ("Alíquota - CBS", &t.aliquota_cbs),
            ("Alíquota Efetiva - CBS", &t.aliq_efetiva_cbs),
            ("Valor Total Apurado - CBS", &t.valor_cbs),
        ],
    ] {
        let celulas: Vec<Celula> = linha.iter().enumerate().map(|(i, (r, v))| c(i, i + 1, r, v)).collect();
        p.linha(y, LINHA, None, &celulas);
        y += LINHA;
    }

    let v = &d.totais;
    p.linha(
        y,
        LINHA_TOTAIS,
        Some("VALOR TOTAL DA NFS-E"),
        &[
            c(1, 2, "Valor da Operação / Serviço", &v.servico),
            c(2, 3, "Desconto Incondicionado", &v.desconto_incondicionado),
            c(3, 4, "Desconto Condicionado", &v.desconto_condicionado),
        ],
    );
    y += LINHA_TOTAIS;
    p.linha(
        y,
        LINHA_TOTAIS,
        None,
        &[
            c(0, 1, "Total das Retenções (ISSQN / Federais)", &v.retencoes),
            c(1, 2, "Valor Líquido da NFS-e", &v.liquido),
            c(2, 3, "Total do IBS/CBS", &v.total_ibs_cbs),
            Celula {
                de: 3,
                ate: 4,
                rotulo: "Valor Líquido da NFS-e + IBS/CBS",
                valor: &v.liquido_com_ibs_cbs,
                fundo: true,
            },
        ],
    );
    y += LINHA_TOTAIS;

    p.rotulo_bloco(COL[0], y, LARGURA, 0.39, "INFORMAÇÕES COMPLEMENTARES");
    y += 0.39;
    p.texto_livre(y, altura_info, None, &d.informacoes_complementares);

    // Borda da página: 1 ponto.
    p.retangulo(COL[0], 0.30, LARGURA, RODAPE - 0.30, None, Some(1.0));

    Ok(())
}

/// Gera o PDF do DANFSe a partir do XML da NFS-e autorizada.
pub fn gerar_danfse(xml: &str, opcoes: &Opcoes, fontes: &Fontes) -> Result<Vec<u8>, ErroPdf> {
    let d = dados::ler(xml)?;

    // Alturas: blocos fixos primeiro; "Descrição do Serviço" e "Informações Complementares" dividem o resto.
    let tres = 3.0 * LINHA;
    let mut fixo = 4.0 * LINHA; // prestador
    fixo += if d.tomador.is_some() { tres } else { MENSAGEM };
    fixo += if matches!(d.destinatario, Destinatario::Outro(_)) {
        tres
    } else {
        MENSAGEM
    };
    fixo += if d.intermediario.is_some() { tres } else { MENSAGEM };
    fixo += LINHA + 0.38; // serviço (sem a descrição)
    fixo += match &d.issqn {
        Some(i) => LINHA * (2 + i.mostrar_regime as u8 + i.mostrar_beneficio as u8) as f32,
        None => MENSAGEM,
    };
    fixo += LINHA * (1 + d.federal.mostrar_pis_cofins as u8) as f32;
    fixo += 4.0 * LINHA + 2.0 * LINHA_TOTAIS + 0.39;
    let inicio = 4.34;
    let disponivel = RODAPE - inicio - fixo;
    let largura_texto = LARGURA - 0.16;
    let precisa_desc = altura_texto(
        quebrar(&fontes.conteudo, 7.0, &d.descricao_servico, largura_texto).len(),
        true,
    )
    .max(LINHA);
    let precisa_info = altura_texto(
        quebrar(&fontes.conteudo, 7.0, &d.informacoes_complementares, largura_texto).len(),
        false,
    );
    let altura_desc = precisa_desc
        .min((disponivel - precisa_info).max(disponivel * 0.6))
        .max(LINHA);
    let altura_info = disponivel - altura_desc;

    let mut documento = Document::new();
    let mut pagina = documento.start_page_with(PageSettings::from_wh(595.28, 841.89).expect("A4"));
    let resultado = {
        let mut p = Pintor {
            s: pagina.surface(),
            f: fontes,
        };
        p.preto();
        let r = desenhar(&mut p, &d, opcoes, inicio, altura_desc, altura_info);
        p.s.finish();
        r
    };
    pagina.finish();
    resultado?;
    documento.finish().map_err(|e| ErroPdf::Pdf(format!("{e:?}")))
}

#[cfg(test)]
mod testes;
