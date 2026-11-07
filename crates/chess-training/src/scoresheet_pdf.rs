//! PDF A4 da Súmula no formato de torneio (V2-SUM-03), gerado localmente, sem rede.
//!
//! Duas variantes:
//!
//! - **anotada** ([`SheetContent::Annotated`]): o que o jogador escreveu na
//!   [`ManualScoresheet`], exatamente como foi escrito (a conferência não entra aqui);
//! - **em branco** ([`SheetContent::Blank`]): a folha pronta para imprimir e anotar no
//!   tabuleiro físico.
//!
//! # Layout (A4 retrato, 595 x 842 pt, margem 36 pt)
//!
//! 1. título e cabeçalho: Evento, Data, Resultado, Brancas, Pretas (linhas para escrever);
//! 2. dois blocos lado a lado de 30 lances cada (60 lances por folha), cada bloco com as
//!    colunas numeradas `N.` | Brancas | Pretas; as folhas seguintes continuam a numeração;
//! 3. rodapé com três linhas de assinatura (Brancas, Pretas, Árbitro) e "Página n / N".
//!
//! O módulo é puro: recebe a fonte TrueType (Noto Sans, embutida no PDF como subconjunto
//! CID com `ToUnicode`, para o texto ser selecionável/pesquisável) e devolve os bytes. Não
//! há relógio, data de criação nem identificador aleatório: mesma entrada, mesmos bytes.
//! Quem chama escolhe onde gravar; nada aqui abre impressora.

use crate::scoresheet::ManualScoresheet;
use chess_core::Color as Side;
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use subsetter::GlyphRemapper;

/// Largura do A4 em pontos.
pub const A4_WIDTH_PT: f32 = 595.0;
/// Altura do A4 em pontos.
pub const A4_HEIGHT_PT: f32 = 842.0;
/// Lances por bloco.
pub const ROWS_PER_BLOCK: usize = 30;
/// Lances por folha (dois blocos).
pub const ROWS_PER_PAGE: usize = ROWS_PER_BLOCK * 2;
/// Máximo de folhas em branco numa só exportação.
pub const MAX_BLANK_PAGES: u16 = 20;

const MARGIN: f32 = 36.0;
const TABLE_TOP: f32 = 690.0;
const ROW_H: f32 = 18.0;
const BLOCK_W: f32 = 253.5;
const BLOCK_GAP: f32 = 16.0;
const NUM_W: f32 = 24.0;
const CELL_PAD: f32 = 4.0;
const TEXT_SIZE: f32 = 10.5;

/// Por que o PDF não pôde ser gerado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfError {
    /// A fonte não é um TrueType legível.
    BadFont,
    /// Falha ao reduzir a fonte aos glifos usados.
    Subset(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadFont => f.write_str("fonte inválida para o PDF"),
            Self::Subset(e) => write!(f, "falha ao preparar a fonte do PDF: {e}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Dados do cabeçalho (todos opcionais: vazio vira linha em branco para escrever).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SheetHeader {
    pub event: String,
    pub date: String,
    pub white: String,
    pub black: String,
    pub result: String,
}

/// Rótulos da folha, já traduzidos por quem chama (a interface é pt-BR via Fluent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetLabels {
    pub title: String,
    pub event: String,
    pub date: String,
    pub white: String,
    pub black: String,
    pub result: String,
    pub move_number: String,
    pub signature_white: String,
    pub signature_black: String,
    pub signature_arbiter: String,
    pub page: String,
}

impl SheetLabels {
    /// Rótulos em pt-BR (os mesmos do catálogo Fluent), úteis em testes e como padrão.
    pub fn pt_br() -> Self {
        Self {
            title: "Súmula".into(),
            event: "Evento".into(),
            date: "Data".into(),
            white: "Brancas".into(),
            black: "Pretas".into(),
            result: "Resultado".into(),
            move_number: "Nº".into(),
            signature_white: "Assinatura das Brancas".into(),
            signature_black: "Assinatura das Pretas".into(),
            signature_arbiter: "Árbitro".into(),
            page: "Página".into(),
        }
    }
}

/// O que vai nas linhas dos Lances.
#[derive(Clone, Copy, Debug)]
pub enum SheetContent<'a> {
    /// O texto escrito pelo jogador; `played_plies` é quantos meios-lances a Partida tem
    /// (a folha cobre ao menos até o último meio-lance anotado).
    Annotated {
        sheet: &'a ManualScoresheet,
        played_plies: usize,
    },
    /// Folha(s) com as linhas vazias, para anotar à mão.
    Blank { pages: u16 },
}

/// Tudo o que define a folha.
#[derive(Clone, Copy, Debug)]
pub struct SheetSpec<'a> {
    pub header: &'a SheetHeader,
    pub labels: &'a SheetLabels,
    /// Número do primeiro lance completo (1 numa Partida normal).
    pub first_move_number: u16,
    /// Quem joga primeiro (brancas numa Partida normal).
    pub first_side: Side,
    pub content: SheetContent<'a>,
}

/// Linhas (lances completos) necessárias para `plies` meios-lances.
pub fn rows_for(first_side: Side, plies: usize) -> usize {
    match first_side {
        Side::White => plies.div_ceil(2),
        Side::Black => (plies + 1).div_ceil(2),
    }
}

/// Meios-lances cobertos pela folha anotada (a Partida ou o último anotado, o maior).
fn covered_plies(sheet: &ManualScoresheet, played_plies: usize) -> usize {
    let last = sheet.entries().map(|(p, _)| usize::from(p)).max();
    played_plies.max(last.unwrap_or(0))
}

/// Folhas A4 necessárias para a especificação.
pub fn page_count(spec: &SheetSpec<'_>) -> usize {
    match spec.content {
        SheetContent::Blank { pages } => usize::from(pages.clamp(1, MAX_BLANK_PAGES)),
        SheetContent::Annotated {
            sheet,
            played_plies,
        } => rows_for(spec.first_side, covered_plies(sheet, played_plies))
            .div_ceil(ROWS_PER_PAGE)
            .max(1),
    }
}

/// Meios-lances (branca, preta) da linha `row` (0-based).
fn plies_of_row(first_side: Side, row: usize) -> (Option<usize>, Option<usize>) {
    match first_side {
        Side::White => (Some(2 * row + 1), Some(2 * row + 2)),
        Side::Black if row == 0 => (None, Some(1)),
        Side::Black => (Some(2 * row), Some(2 * row + 1)),
    }
}

/// Uma operação de desenho já posicionada; o texto fica como `String` até a fonte ser
/// reduzida aos glifos realmente usados.
enum Op {
    Text {
        x: f32,
        y: f32,
        size: f32,
        text: String,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
    },
}

struct Fnt<'a> {
    face: ttf_parser::Face<'a>,
}

impl Fnt<'_> {
    fn gid(&self, c: char) -> u16 {
        self.face.glyph_index(c).map_or(0, |g| g.0)
    }

    fn advance_1000(&self, gid: u16) -> f32 {
        let upem = f32::from(self.face.units_per_em());
        self.face
            .glyph_hor_advance(ttf_parser::GlyphId(gid))
            .map_or(0.0, |a| f32::from(a) * 1000.0 / upem)
    }

    /// Largura do texto em pontos no tamanho `size`.
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| self.advance_1000(self.gid(c)))
            .sum::<f32>()
            * size
            / 1000.0
    }
}

struct Page(Vec<Op>);

impl Page {
    fn text(&mut self, x: f32, y: f32, size: f32, text: &str) {
        if !text.is_empty() {
            self.0.push(Op::Text {
                x,
                y,
                size,
                text: text.to_owned(),
            });
        }
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
        self.0.push(Op::Line {
            x1,
            y1,
            x2,
            y2,
            width,
        });
    }
}

/// Menor tamanho (até `max`) em que o texto cabe na largura.
fn fit(font: &Fnt<'_>, text: &str, width: f32, max: f32) -> f32 {
    let w = font.width(text, max);
    if w <= width || w <= 0.0 {
        max
    } else {
        (max * width / w).max(5.0)
    }
}

/// Campo do cabeçalho: rótulo, valor e a linha para escrever embaixo.
fn field(page: &mut Page, font: &Fnt<'_>, (x, y, w): (f32, f32, f32), label: &str, value: &str) {
    const LABEL_W: f32 = 52.0;
    page.text(x, y, 8.5, label);
    let vx = x + LABEL_W;
    let size = fit(font, value, w - LABEL_W - 2.0, 11.0);
    page.text(vx, y, size, value);
    page.line(vx, y - 3.0, x + w, y - 3.0, 0.5);
}

#[allow(clippy::cast_precision_loss)] // índices minúsculos
fn draw_page(
    font: &Fnt<'_>,
    spec: &SheetSpec<'_>,
    index: usize,
    total: usize,
    plies: Option<usize>,
) -> Page {
    let l = spec.labels;
    let h = spec.header;
    let mut p = Page(Vec::new());
    let full = A4_WIDTH_PT - 2.0 * MARGIN;
    let half = (full - BLOCK_GAP) / 2.0;
    let right = MARGIN + half + BLOCK_GAP;

    p.text(MARGIN, 800.0, 16.0, &l.title);
    field(&mut p, font, (MARGIN, 772.0, full), &l.event, &h.event);
    field(&mut p, font, (MARGIN, 748.0, half), &l.date, &h.date);
    field(&mut p, font, (right, 748.0, half), &l.result, &h.result);
    field(&mut p, font, (MARGIN, 724.0, half), &l.white, &h.white);
    field(&mut p, font, (right, 724.0, half), &l.black, &h.black);

    // Tabela: dois blocos de 30 lances.
    let col_w = (BLOCK_W - NUM_W) / 2.0;
    let top = TABLE_TOP;
    let bottom = top - ROW_H * (ROWS_PER_BLOCK as f32 + 1.0);
    for block in 0..2 {
        let x0 = MARGIN + block as f32 * (BLOCK_W + BLOCK_GAP);
        p.text(x0 + CELL_PAD, top - 13.0, 8.5, &l.move_number);
        p.text(x0 + NUM_W + CELL_PAD, top - 13.0, 8.5, &l.white);
        p.text(x0 + NUM_W + col_w + CELL_PAD, top - 13.0, 8.5, &l.black);
        for i in 0..=(ROWS_PER_BLOCK + 1) {
            let y = top - ROW_H * i as f32;
            p.line(x0, y, x0 + BLOCK_W, y, if i <= 1 { 0.9 } else { 0.4 });
        }
        for dx in [0.0, NUM_W, NUM_W + col_w, BLOCK_W] {
            p.line(x0 + dx, top, x0 + dx, bottom, 0.6);
        }
        for r in 0..ROWS_PER_BLOCK {
            let row = index * ROWS_PER_PAGE + block * ROWS_PER_BLOCK + r;
            let y = top - ROW_H * (r as f32 + 1.0) - 13.0;
            let number = usize::from(spec.first_move_number) + row;
            p.text(x0 + CELL_PAD, y, 9.0, &format!("{number}."));
            let (SheetContent::Annotated { sheet, .. }, Some(plies)) = (spec.content, plies) else {
                continue;
            };
            let (w, b) = plies_of_row(spec.first_side, row);
            for (ply, cx) in [(w, x0 + NUM_W), (b, x0 + NUM_W + col_w)] {
                let Some(ply) = ply.filter(|q| *q <= plies) else {
                    continue;
                };
                let Some(text) = u16::try_from(ply).ok().and_then(|q| sheet.text(q)) else {
                    continue;
                };
                let size = fit(font, text, col_w - 2.0 * CELL_PAD, TEXT_SIZE);
                p.text(cx + CELL_PAD, y, size, text);
            }
        }
    }

    // Assinaturas.
    let sig_w = (full - 2.0 * BLOCK_GAP) / 3.0;
    for (i, label) in [&l.signature_white, &l.signature_black, &l.signature_arbiter]
        .into_iter()
        .enumerate()
    {
        let x = MARGIN + i as f32 * (sig_w + BLOCK_GAP);
        p.line(x, 96.0, x + sig_w, 96.0, 0.6);
        p.text(x, 84.0, 8.5, label);
    }
    // Rodapé.
    let footer = format!("{} {} / {}", l.page, index + 1, total);
    let fw = font.width(&footer, 8.5);
    p.text(A4_WIDTH_PT - MARGIN - fw, 40.0, 8.5, &footer);
    p
}

/// Gera o PDF. Determinístico: não usa relógio nem aleatoriedade.
pub fn render_scoresheet_pdf(font_data: &[u8], spec: &SheetSpec<'_>) -> Result<Vec<u8>, PdfError> {
    let face = ttf_parser::Face::parse(font_data, 0).map_err(|_| PdfError::BadFont)?;
    let font = Fnt { face };
    let total = page_count(spec);
    let plies = match spec.content {
        SheetContent::Blank { .. } => None,
        SheetContent::Annotated {
            sheet,
            played_plies,
        } => Some(covered_plies(sheet, played_plies)),
    };
    let pages: Vec<Page> = (0..total)
        .map(|i| draw_page(&font, spec, i, total, plies))
        .collect();

    // Glifos usados -> subconjunto + mapa para o ToUnicode.
    let mut remapper = GlyphRemapper::new();
    let mut to_unicode: BTreeMap<u16, char> = BTreeMap::new();
    for op in pages.iter().flat_map(|p| &p.0) {
        if let Op::Text { text, .. } = op {
            for c in text.chars() {
                let old = font.gid(c);
                let new = remapper.remap(old);
                if old != 0 {
                    to_unicode.entry(new).or_insert(c);
                }
            }
        }
    }
    let subset = subsetter::subset(font_data, 0, &remapper)
        .map_err(|e| PdfError::Subset(format!("{e:?}")))?;
    let widths: Vec<f32> = remapper
        .remapped_gids()
        .map(|old| font.advance_1000(old))
        .collect();

    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let (catalog, tree, info) = (alloc(), alloc(), alloc());
    let (type0, cid, descriptor, font_file, cmap) = (alloc(), alloc(), alloc(), alloc(), alloc());
    let page_refs: Vec<(Ref, Ref)> = pages.iter().map(|_| (alloc(), alloc())).collect();

    let base_font = Name(b"AAAAAA+NotoSans-Regular");
    let sys = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };

    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_refs.iter().map(|(p, _)| *p))
        .count(i32::try_from(total).unwrap_or(i32::MAX));
    pdf.document_info(info).title(TextStr(&spec.labels.title));

    pdf.type0_font(type0)
        .base_font(base_font)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid)
        .to_unicode(cmap);
    {
        let mut c = pdf.cid_font(cid);
        c.subtype(CidFontType::Type2)
            .base_font(base_font)
            .system_info(sys)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        c.widths().consecutive(0, widths.iter().copied());
    }
    let upem = f32::from(font.face.units_per_em());
    let scale = |v: i16| f32::from(v) * 1000.0 / upem;
    let bb = font.face.global_bounding_box();
    pdf.font_descriptor(descriptor)
        .name(base_font)
        .flags(FontFlags::NON_SYMBOLIC)
        .bbox(Rect::new(
            scale(bb.x_min),
            scale(bb.y_min),
            scale(bb.x_max),
            scale(bb.y_max),
        ))
        .italic_angle(0.0)
        .ascent(scale(font.face.ascender()))
        .descent(scale(font.face.descender()))
        .cap_height(scale(
            font.face.capital_height().unwrap_or(font.face.ascender()),
        ))
        .stem_v(80.0)
        .font_file2(font_file);
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&subset, 6);
    pdf.stream(font_file, &compressed)
        .filter(Filter::FlateDecode)
        .pair(
            Name(b"Length1"),
            i32::try_from(subset.len()).unwrap_or(i32::MAX),
        );
    let mut cm = UnicodeCmap::new(Name(b"Custom"), sys);
    for (gid, c) in &to_unicode {
        cm.pair(*gid, *c);
    }
    pdf.cmap(cmap, &cm.finish());

    for (page, (page_ref, content_ref)) in pages.iter().zip(&page_refs) {
        let mut pg = pdf.page(*page_ref);
        pg.parent(tree)
            .media_box(Rect::new(0.0, 0.0, A4_WIDTH_PT, A4_HEIGHT_PT))
            .contents(*content_ref);
        pg.resources().fonts().pair(Name(b"F1"), type0);
        pg.finish();
        pdf.stream(*content_ref, page_stream(page, &font, &remapper).as_bytes());
    }
    Ok(pdf.finish())
}

/// O fluxo de conteúdo da folha, em texto (sem compressão: facilita a verificação).
fn page_stream(page: &Page, font: &Fnt<'_>, remapper: &GlyphRemapper) -> String {
    let mut s = String::new();
    for op in &page.0 {
        match op {
            Op::Line {
                x1,
                y1,
                x2,
                y2,
                width,
            } => {
                let _ = writeln!(s, "{width:.2} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S");
            }
            Op::Text { x, y, size, text } => {
                let mut hex = String::new();
                for c in text.chars() {
                    let new = remapper.get(font.gid(c)).unwrap_or(0);
                    let _ = write!(hex, "{new:04X}");
                }
                let _ = writeln!(s, "BT /F1 {size:.2} Tf {x:.2} {y:.2} Td <{hex}> Tj ET");
            }
        }
    }
    s
}
