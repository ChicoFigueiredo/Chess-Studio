//! Painel Súmula manual (V2-SUM-01): o jogador anota os Lances à mão, como numa competição
//! física, sem ver a notação automática.
//!
//! A Súmula ([`chess_training::ManualScoresheet`]) é um registro **separado** da Partida
//! (ADR-0001/0002): este módulo só *lê* o histórico para saber quantos Lances há e nunca
//! emite Comando. Ela fica num recurso próprio, gravada pelo `PersistencePlugin` na tabela
//! `manual_scoresheets` (por perfil local e por Partida).
//!
//! # O que fica oculto enquanto a Súmula está em anotação
//!
//! Tudo que mostra a notação (SAN) da Partida em curso, ou a entrega pronta:
//!
//! - painel **Notação** (lista de Lances) e painel **Anotações** (títulos "Lance N: SAN");
//! - **Último lance** no painel Partida e a SAN do Lance na **Revisão**;
//! - **exportar PGN** (o arquivo traria a notação inteira);
//! - painel **Análise** (linhas da Engine em SAN) e **Biblioteca** (abriria a própria Partida).
//!
//! Não é ocultado o que não é notação: o tabuleiro, as peças, o relógio e os Auxílios
//! visuais, que o jogador controla no painel Auxílios. Ao **entregar** a Súmula (ou
//! descartá-la) a notação volta. Ver [`ScoresheetUi::hides_notation`].

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::Color as Side;
use chess_training::{
    HistoryPeriod, MAX_ENTRY_CHARS, ManualScoresheet, ScoresheetCheck, ScoresheetHistory,
    SheetContent, SheetHeader, SheetLabels, SheetSpec, Verdict, render_scoresheet_pdf,
};
use std::path::{Path, PathBuf};

/// Noto Sans (OFL, já registrada em `assets/LICENSES.md`): a mesma fonte da interface.
const PDF_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

/// O que gravar no banco na próxima passada do `PersistencePlugin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    Save(ManualScoresheet),
    Delete,
}

/// Estado da Súmula da Partida atual.
#[derive(Resource, Debug, Default)]
pub struct ScoresheetUi {
    /// Época da `Session` a que a Súmula pertence (muda com Nova Partida/FEN/perfil).
    epoch: Option<u64>,
    sheet: Option<ManualScoresheet>,
    /// A Súmula gravada ainda não foi lida do banco: por segurança a notação fica oculta.
    loading: bool,
    pending: Option<PendingKind>,
    /// Conferência da Súmula entregue (V2-SUM-02): registro à parte, só leitura da Partida.
    check: Option<ScoresheetCheck>,
    pending_check: Option<ScoresheetCheck>,
    /// Sessão de Exercício no lugar da Partida: a Súmula não se aplica.
    suspended: bool,
    /// Histórico de erros do perfil (V2-SUM-03): período escolhido e o último resultado.
    history_period: HistoryPeriod,
    history: Option<ScoresheetHistory>,
    /// O `history` reflete o banco? (some ao trocar de período, de Partida ou conferir.)
    history_fresh: bool,
    /// Exportação em PDF: pasta digitada (vazia = Documentos), pedido e último resultado.
    pdf_dir: String,
    pdf_request: Option<PdfKind>,
    pdf_result: Option<Result<PathBuf, String>>,
}

/// Qual PDF o usuário pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfKind {
    Annotated,
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Save,
    Delete,
}

impl ScoresheetUi {
    pub fn sheet(&self) -> Option<&ManualScoresheet> {
        self.sheet.as_ref()
    }

    /// A notação automática deve ficar oculta agora?
    pub fn hides_notation(&self) -> bool {
        !self.suspended
            && (self.loading
                || self
                    .sheet
                    .as_ref()
                    .is_some_and(ManualScoresheet::hides_automatic_notation))
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Acompanha a `Session`: a Súmula pertence a uma Partida, então Nova Partida/FEN/troca
    /// de perfil começam sem ela até o banco responder. `persistent` é falso sem banco.
    pub fn follow(&mut self, epoch: u64, suspended: bool, persistent: bool) {
        self.suspended = suspended;
        if suspended || self.epoch == Some(epoch) {
            return;
        }
        self.epoch = Some(epoch);
        self.sheet = None;
        self.pending = None;
        self.check = None;
        self.pending_check = None;
        self.history_fresh = false;
        self.loading = persistent;
    }

    /// O histórico precisa ser (re)lido do banco?
    pub fn needs_history(&self) -> bool {
        !self.suspended && !self.history_fresh
    }

    /// O histórico lido do banco para o período atual.
    pub fn loaded_history(&mut self, history: Option<ScoresheetHistory>) {
        self.history = history;
        self.history_fresh = true;
    }

    pub fn history(&self) -> Option<&ScoresheetHistory> {
        self.history.as_ref()
    }

    pub fn history_period(&self) -> HistoryPeriod {
        self.history_period
    }

    /// Troca o período do histórico (relê do banco).
    pub fn set_history_period(&mut self, period: HistoryPeriod) {
        if self.history_period != period {
            self.history_period = period;
            self.history_fresh = false;
        }
    }

    /// Uma conferência nova foi gravada: o histórico mudou.
    pub fn invalidate_history(&mut self) {
        self.history_fresh = false;
    }

    /// Pede a exportação em PDF (atendida pelo painel, que conhece a Partida).
    pub fn request_pdf(&mut self, kind: PdfKind) {
        self.pdf_request = Some(kind);
    }

    pub fn take_pdf_request(&mut self) -> Option<PdfKind> {
        self.pdf_request.take()
    }

    pub fn pdf_result(&self) -> Option<&Result<PathBuf, String>> {
        self.pdf_result.as_ref()
    }

    /// A Súmula lida do banco para a Partida atual.
    pub fn loaded(&mut self, sheet: Option<ManualScoresheet>) {
        self.sheet = sheet;
        self.loading = false;
    }

    /// A conferência gravada para a Súmula da Partida atual.
    pub fn loaded_check(&mut self, check: Option<ScoresheetCheck>) {
        self.check = check;
    }

    pub fn check(&self) -> Option<&ScoresheetCheck> {
        self.check.as_ref()
    }

    /// A Súmula está entregue e ainda não foi conferida (nem lida do banco)?
    pub fn needs_check(&self) -> bool {
        !self.suspended
            && !self.loading
            && self.check.is_none()
            && self
                .sheet
                .as_ref()
                .is_some_and(ManualScoresheet::is_delivered)
    }

    /// Guarda a conferência recém-feita e agenda a gravação.
    pub fn record_check(&mut self, check: ScoresheetCheck) {
        self.check = Some(check.clone());
        self.pending_check = Some(check);
    }

    /// A conferência a gravar, se houver.
    pub fn take_pending_check(&mut self) -> Option<ScoresheetCheck> {
        self.pending_check.take()
    }

    /// Começa uma Súmula em branco (e passa a ocultar a notação).
    pub fn start(&mut self) {
        if self.sheet.is_none() {
            self.sheet = Some(ManualScoresheet::new());
            self.pending = Some(PendingKind::Save);
        }
    }

    /// Escreve a anotação do meio-lance. Recusas (Súmula entregue, texto inválido) não
    /// mudam nada.
    pub fn write(&mut self, ply: usize, text: &str) {
        let Some(sheet) = self.sheet.as_mut() else {
            return;
        };
        let Ok(ply) = u16::try_from(ply) else { return };
        if sheet.text(ply).unwrap_or_default() == text.trim() {
            return;
        }
        if sheet.write(ply, text).is_ok() {
            self.pending = Some(PendingKind::Save);
        }
    }

    /// Entrega a Súmula: congela o texto e libera a notação.
    pub fn deliver(&mut self, at_ms: i64) {
        if let Some(sheet) = self.sheet.as_mut()
            && sheet.deliver(at_ms).is_ok()
        {
            self.pending = Some(PendingKind::Save);
        }
    }

    /// Descarta a Súmula (apaga o registro; a Partida não muda).
    pub fn discard(&mut self) {
        if self.sheet.take().is_some() {
            self.check = None;
            self.pending_check = None;
            self.pending = Some(PendingKind::Delete);
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some() || self.pending_check.is_some()
    }

    /// O que gravar, se algo mudou desde a última chamada.
    pub fn take_pending(&mut self) -> Option<Pending> {
        match self.pending.take()? {
            PendingKind::Save => self.sheet.clone().map(Pending::Save),
            PendingKind::Delete => Some(Pending::Delete),
        }
    }
}

/// Uma linha da Súmula: número do lance completo e os meios-lances (`ply`, a partir de 1)
/// das brancas e das pretas. `white` é `None` quando a Posição inicial é das pretas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetRow {
    pub number: u16,
    pub white: Option<u16>,
    pub black: Option<u16>,
}

/// Linhas para `played` meios-lances, a partir do lance completo `first_number` com
/// `first_side` a jogar (Partida normal: 1 e brancas).
pub fn sheet_rows(first_number: u16, first_side: Side, played: usize) -> Vec<SheetRow> {
    let mut rows: Vec<SheetRow> = Vec::new();
    let mut number = first_number;
    let mut side = first_side;
    for ply in 1..=played.min(usize::from(chess_training::MAX_PLY)) {
        #[allow(clippy::cast_possible_truncation)] // limitado a MAX_PLY acima
        let ply = ply as u16;
        match side {
            Side::White => rows.push(SheetRow {
                number,
                white: Some(ply),
                black: None,
            }),
            Side::Black => {
                match rows.last_mut() {
                    Some(row) if row.number == number && row.black.is_none() => {
                        row.black = Some(ply);
                    }
                    _ => rows.push(SheetRow {
                        number,
                        white: None,
                        black: Some(ply),
                    }),
                }
                number = number.saturating_add(1);
            }
        }
        side = side.opposite();
    }
    rows
}

pub(crate) fn wall_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or_default()
}

/// Aviso padrão dos painéis que mostrariam a notação enquanto a Súmula está em anotação.
pub fn hidden_notice(ui: &mut egui::Ui, i18n: &I18n) {
    ui.weak(i18n.t("scoresheet-notation-hidden"));
}

fn panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    world.resource_scope(|world, mut state: Mut<ScoresheetUi>| {
        let Some(session) = world.get_resource::<Session>() else {
            return;
        };
        if state.suspended {
            ui.weak(i18n.t("scoresheet-unavailable-training"));
            return;
        }
        if state.loading {
            ui.weak(i18n.t("scoresheet-loading"));
            return;
        }
        if state.needs_check()
            && let Some(sheet) = state.sheet()
            && let Ok(check) = chess_training::check_against_game(sheet, session.game(), wall_ms())
        {
            state.record_check(check);
        }
        let played = session.snapshot().history.len();
        let (first_number, first_side) = session
            .game()
            .review_at_ply(0)
            .map_or((1, Side::White), |v| {
                (v.position.fullmove_number(), v.position.side_to_move())
            });
        draw(ui, i18n, &mut state, played, first_number, first_side);
        let result_token = session
            .snapshot()
            .outcome
            .map(|o| o.result.pgn_token().to_owned());
        draw_pdf(ui, i18n, &mut state);
        if let Some(kind) = state.take_pdf_request() {
            let sheet = state.sheet().cloned();
            let wall = wall_ms();
            state.pdf_result = Some(export_pdf(
                i18n,
                kind,
                sheet.as_ref(),
                &PdfGame {
                    played,
                    first_number,
                    first_side,
                    result: result_token.unwrap_or_default(),
                },
                &pdf_folder(&state.pdf_dir),
                wall,
            ));
        }
        draw_history(ui, i18n, &mut state);
    });
}

/// O que o PDF precisa saber da Partida.
struct PdfGame {
    played: usize,
    first_number: u16,
    first_side: Side,
    /// Token PGN do resultado (`1-0`...) ou vazio se a Partida não terminou.
    result: String,
}

/// Pasta de destino: a digitada, ou Documentos.
fn pdf_folder(typed: &str) -> PathBuf {
    let typed = typed.trim();
    if typed.is_empty() {
        crate::paths::documents_dir(
            crate::paths::Platform::current(),
            &crate::paths::process_env,
        )
    } else {
        PathBuf::from(typed)
    }
}

/// `07/11/2026` a partir de milissegundos Unix (UTC).
fn br_date(ms: i64) -> String {
    let d = chess_game::pgn_date_from_unix_ms(ms);
    let mut parts = d.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(day)) => format!("{day}/{m}/{y}"),
        _ => d,
    }
}

fn pdf_labels(i18n: &I18n) -> SheetLabels {
    SheetLabels {
        title: i18n.t("scoresheet-pdf-title"),
        event: i18n.t("scoresheet-pdf-event"),
        date: i18n.t("scoresheet-pdf-date"),
        white: i18n.t("scoresheet-pdf-white"),
        black: i18n.t("scoresheet-pdf-black"),
        result: i18n.t("scoresheet-pdf-result"),
        move_number: i18n.t("scoresheet-pdf-number"),
        signature_white: i18n.t("scoresheet-pdf-sign-white"),
        signature_black: i18n.t("scoresheet-pdf-sign-black"),
        signature_arbiter: i18n.t("scoresheet-pdf-sign-arbiter"),
        page: i18n.t("scoresheet-pdf-page"),
    }
}

/// Gera o PDF (anotado ou em branco) e grava na pasta, sem sobrescrever nada. Sem rede e
/// sem tocar a impressora; a Súmula e a Partida só são lidas.
fn export_pdf(
    i18n: &I18n,
    kind: PdfKind,
    sheet: Option<&ManualScoresheet>,
    game: &PdfGame,
    dir: &Path,
    wall_ms: i64,
) -> Result<PathBuf, String> {
    let blank = kind == PdfKind::Blank || sheet.is_none();
    let header = if blank {
        SheetHeader::default()
    } else {
        SheetHeader {
            date: br_date(wall_ms),
            result: game.result.clone(),
            ..SheetHeader::default()
        }
    };
    let labels = pdf_labels(i18n);
    let content = match sheet {
        Some(sheet) if !blank => SheetContent::Annotated {
            sheet,
            played_plies: game.played,
        },
        _ => SheetContent::Blank { pages: 1 },
    };
    let spec = SheetSpec {
        header: &header,
        labels: &labels,
        first_move_number: if blank { 1 } else { game.first_number },
        first_side: if blank { Side::White } else { game.first_side },
        content,
    };
    let bytes = render_scoresheet_pdf(PDF_FONT, &spec).map_err(|e| e.to_string())?;
    save_pdf(&bytes, dir, wall_ms, blank).map_err(|e| format!("{}: {e}", dir.display()))
}

/// `sumula-AAAA-MM-DD-HHMMSS.pdf` (ou `sumula-em-branco-...`), em UTC; nunca sobrescreve.
fn save_pdf(bytes: &[u8], dir: &Path, wall_ms: i64, blank: bool) -> std::io::Result<PathBuf> {
    use std::io::{Error, ErrorKind, Write};
    std::fs::create_dir_all(dir)?;
    let date = chess_game::pgn_date_from_unix_ms(wall_ms).replace('.', "-");
    let secs = wall_ms.div_euclid(1000).rem_euclid(86_400);
    let stem = format!(
        "{}-{date}-{:02}{:02}{:02}",
        if blank { "sumula-em-branco" } else { "sumula" },
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    );
    for n in 0..1000 {
        let name = if n == 0 {
            format!("{stem}.pdf")
        } else {
            format!("{stem}-{n}.pdf")
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                f.write_all(bytes)?;
                return Ok(path);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
    Err(Error::new(
        ErrorKind::AlreadyExists,
        "muitos arquivos com o mesmo nome",
    ))
}

fn draw_pdf(ui: &mut egui::Ui, i18n: &I18n, state: &mut ScoresheetUi) {
    ui.separator();
    ui.strong(i18n.t("scoresheet-pdf-heading"));
    ui.small(i18n.t("scoresheet-pdf-hint"));
    ui.horizontal(|ui| {
        ui.label(i18n.t("scoresheet-pdf-folder"));
        ui.add(
            egui::TextEdit::singleline(&mut state.pdf_dir)
                .hint_text(pdf_folder("").display().to_string())
                .desired_width(f32::INFINITY),
        );
    });
    let has_sheet = state.sheet().is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                has_sheet,
                egui::Button::new(i18n.t("scoresheet-pdf-annotated")),
            )
            .on_hover_text(i18n.t("scoresheet-pdf-annotated-hint"))
            .clicked()
        {
            state.request_pdf(PdfKind::Annotated);
        }
        if ui
            .button(i18n.t("scoresheet-pdf-blank"))
            .on_hover_text(i18n.t("scoresheet-pdf-blank-hint"))
            .clicked()
        {
            state.request_pdf(PdfKind::Blank);
        }
    });
    match state.pdf_result() {
        Some(Ok(path)) => {
            ui.colored_label(
                egui::Color32::LIGHT_GREEN,
                i18n.t_with(
                    "scoresheet-pdf-saved",
                    &[("path", &path.display().to_string())],
                ),
            );
        }
        Some(Err(detail)) => {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                i18n.t_with("scoresheet-pdf-error", &[("detail", detail)]),
            );
        }
        None => {}
    }
}

/// Chave Fluent do rótulo do período.
fn period_key(p: HistoryPeriod) -> &'static str {
    match p {
        HistoryPeriod::Week => "scoresheet-history-period-week",
        HistoryPeriod::Month => "scoresheet-history-period-month",
        HistoryPeriod::All => "scoresheet-history-period-all",
    }
}

fn draw_history(ui: &mut egui::Ui, i18n: &I18n, state: &mut ScoresheetUi) {
    ui.separator();
    ui.strong(i18n.t("scoresheet-history-heading"));
    ui.horizontal(|ui| {
        for p in HistoryPeriod::ALL {
            if ui
                .selectable_label(state.history_period() == p, i18n.t(period_key(p)))
                .clicked()
            {
                state.set_history_period(p);
            }
        }
    });
    let Some(history) = state.history() else {
        return;
    };
    if history.sheets == 0 {
        ui.weak(i18n.t("scoresheet-history-empty"));
        return;
    }
    ui.label(i18n.t_with(
        "scoresheet-history-sheets",
        &[("count", &history.sheets.to_string())],
    ));
    for verdict in Verdict::ALL {
        let n = history.totals.of(verdict);
        if n > 0 {
            ui.colored_label(
                verdict_color(verdict),
                format!("{}: {n}", i18n.t(verdict_key(verdict))),
            );
        }
    }
    ui.small(i18n.t("scoresheet-history-evolution"));
    for point in &history.points {
        let total = point.counts.total();
        let right = point.counts.correct + point.counts.accepted;
        ui.monospace(i18n.t_with(
            "scoresheet-history-point",
            &[
                ("date", &br_date(point.bucket_start_ms)),
                ("right", &right.to_string()),
                ("total", &total.to_string()),
                ("sheets", &point.sheets.to_string()),
            ],
        ));
    }
}

fn draw(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut ScoresheetUi,
    played: usize,
    first_number: u16,
    first_side: Side,
) {
    let Some(sheet) = state.sheet().cloned() else {
        ui.weak(i18n.t("scoresheet-intro"));
        if ui.button(i18n.t("scoresheet-start")).clicked() {
            state.start();
        }
        return;
    };
    let delivered = sheet.is_delivered();
    ui.strong(i18n.t(if delivered {
        "scoresheet-delivered"
    } else {
        "scoresheet-writing"
    }));
    ui.small(i18n.t("scoresheet-separate"));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 36.0)
        .show(ui, |ui| {
            if played == 0 {
                ui.weak(i18n.t("scoresheet-no-moves"));
            }
            for row in sheet_rows(first_number, first_side, played) {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{:>3}.", row.number));
                    for (slot, ply) in [(0, row.white), (1, row.black)] {
                        let Some(ply) = ply else {
                            if slot == 0 {
                                ui.weak("…");
                            }
                            continue;
                        };
                        let mut text = sheet.text(ply).unwrap_or_default().to_owned();
                        let edit = egui::TextEdit::singleline(&mut text)
                            .id_salt(("sumula-lance", ply))
                            .char_limit(MAX_ENTRY_CHARS)
                            .desired_width(70.0);
                        if ui.add_enabled(!delivered, edit).changed() {
                            state.write(usize::from(ply), &text);
                        }
                    }
                });
            }
        });
    ui.horizontal(|ui| {
        if !delivered && ui.button(i18n.t("scoresheet-deliver")).clicked() {
            state.deliver(wall_ms());
        }
        if ui.button(i18n.t("scoresheet-discard")).clicked() {
            state.discard();
        }
    });
    if !delivered {
        ui.small(i18n.t("scoresheet-deliver-hint"));
    }
    if let Some(check) = state.check() {
        draw_check(ui, i18n, check);
    }
}

/// Chave Fluent do rótulo da classe.
fn verdict_key(v: Verdict) -> &'static str {
    match v {
        Verdict::Correct => "scoresheet-verdict-correct",
        Verdict::AcceptedWithCaveat => "scoresheet-verdict-accepted",
        Verdict::WrongNotation => "scoresheet-verdict-wrong-notation",
        Verdict::Swapped => "scoresheet-verdict-swapped",
        Verdict::Missing => "scoresheet-verdict-missing",
        Verdict::Extra => "scoresheet-verdict-extra",
    }
}

fn verdict_color(v: Verdict) -> egui::Color32 {
    match v {
        Verdict::Correct => egui::Color32::from_rgb(90, 180, 110),
        Verdict::AcceptedWithCaveat => egui::Color32::from_rgb(220, 190, 80),
        _ => egui::Color32::from_rgb(220, 100, 90),
    }
}

/// Resultado da conferência: resumo por classe e, abaixo, só o que divergiu do SAN exato.
fn draw_check(ui: &mut egui::Ui, i18n: &I18n, check: &ScoresheetCheck) {
    ui.separator();
    ui.strong(i18n.t("scoresheet-check-title"));
    ui.small(i18n.t("scoresheet-check-readonly"));
    if check.is_clean() {
        ui.label(i18n.t("scoresheet-check-clean"));
    }
    let counts = check.counts();
    for (verdict, n) in [
        (Verdict::Correct, counts.correct),
        (Verdict::AcceptedWithCaveat, counts.accepted),
        (Verdict::WrongNotation, counts.wrong_notation),
        (Verdict::Swapped, counts.swapped),
        (Verdict::Missing, counts.missing),
        (Verdict::Extra, counts.extra),
    ] {
        if n > 0 {
            ui.colored_label(
                verdict_color(verdict),
                format!("{}: {n}", i18n.t(verdict_key(verdict))),
            );
        }
    }
    for e in check
        .entries()
        .iter()
        .filter(|e| e.verdict != Verdict::Correct)
    {
        let none = "—";
        let ply = e.ply.to_string();
        let verdict = i18n.t(verdict_key(e.verdict));
        let line = i18n.t_with(
            "scoresheet-check-line",
            &[
                ("ply", &ply),
                ("written", e.written.as_deref().unwrap_or(none)),
                ("expected", e.expected.as_deref().unwrap_or(none)),
                ("verdict", &verdict),
            ],
        );
        ui.colored_label(verdict_color(e.verdict), line);
    }
}

/// Acompanha a Partida atual (Treino à parte: a `Session` trocada é a de um Exercício).
pub(crate) fn follow_game(
    session: Res<Session>,
    training: Res<crate::training_ui::TrainingUi>,
    handle: Option<Res<crate::persistence::PersistenceHandle>>,
    mut state: ResMut<ScoresheetUi>,
) {
    let (epoch, suspended, persistent) = (
        session.epoch(),
        training.session_swapped(),
        handle.is_some(),
    );
    // Sem mudança real, não marca o recurso como alterado todo quadro.
    if state.suspended != suspended || state.epoch != Some(epoch) {
        state.follow(epoch, suspended, persistent);
    }
}

pub struct ScoresheetPlugin;

impl Plugin for ScoresheetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScoresheetUi>()
            .add_systems(Update, follow_game.before(crate::play::PlaySet));
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::SCORESHEET,
                label_key: "dock-panel-scoresheet",
                access: PanelAccess::Always,
                ui: panel_ui,
            },
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn active() -> ScoresheetUi {
        let mut s = ScoresheetUi::default();
        s.follow(1, false, false);
        s.start();
        s
    }

    #[test]
    fn notation_is_hidden_exactly_while_the_sheet_is_being_written() {
        let mut s = ScoresheetUi::default();
        s.follow(1, false, false);
        assert!(!s.hides_notation(), "sem Súmula, a notação aparece");
        s.start();
        assert!(s.hides_notation(), "em anotação: oculta");
        s.write(1, "e4");
        assert!(s.hides_notation());
        s.deliver(10);
        assert!(!s.hides_notation(), "entregue: liberada");
        s.discard();
        assert!(!s.hides_notation());
    }

    #[test]
    fn discarding_while_writing_releases_the_notation_and_deletes_the_record() {
        let mut s = active();
        s.take_pending();
        s.discard();
        assert!(!s.hides_notation());
        assert_eq!(s.take_pending(), Some(Pending::Delete));
        assert_eq!(s.take_pending(), None);
    }

    #[test]
    fn writes_are_saved_once_and_unchanged_text_saves_nothing() {
        let mut s = active();
        assert!(matches!(s.take_pending(), Some(Pending::Save(_))));
        s.write(1, "e4");
        let Some(Pending::Save(sheet)) = s.take_pending() else {
            panic!("esperava gravar");
        };
        assert_eq!(sheet.text(1), Some("e4"));
        s.write(1, "e4");
        s.write(1, "e4 ");
        assert_eq!(s.take_pending(), None, "texto igual: nada a gravar");
        s.deliver(5);
        s.take_pending();
        s.write(2, "e5");
        assert_eq!(s.take_pending(), None, "entregue: imutável");
        assert_eq!(s.sheet().unwrap().text(2), None);
    }

    #[test]
    fn a_new_game_starts_without_a_sheet_and_waits_for_the_database() {
        let mut s = active();
        s.follow(2, false, true);
        assert!(s.sheet().is_none());
        assert!(s.hides_notation(), "carregando: oculta por segurança");
        s.loaded(None);
        assert!(!s.hides_notation());
        let mut stored = ManualScoresheet::new();
        stored.write(1, "d4").unwrap();
        s.follow(3, false, true);
        s.loaded(Some(stored));
        assert!(s.hides_notation(), "Súmula em anotação retomada do banco");
    }

    #[test]
    fn an_exercise_session_suspends_the_hiding() {
        let mut s = active();
        s.follow(9, true, true);
        assert!(!s.hides_notation());
        assert!(s.sheet().is_some(), "a Súmula da Partida continua guardada");
        s.follow(1, false, true);
        assert!(s.hides_notation());
    }

    fn check_for(sheet: &ManualScoresheet) -> ScoresheetCheck {
        let game = chess_game::Game::new(chess_game::GameSetup::standard(
            chess_game::PolicyKind::Assisted,
        ));
        chess_training::check_against_game(sheet, &game, 3).unwrap()
    }

    #[test]
    fn a_delivered_sheet_is_checked_once_and_the_result_is_saved_once() {
        let mut s = active();
        s.take_pending();
        assert!(!s.needs_check(), "em anotação: sem conferência");
        s.deliver(10);
        assert!(s.needs_check());
        let check = check_for(s.sheet().unwrap());
        s.record_check(check.clone());
        assert!(!s.needs_check());
        assert_eq!(s.check(), Some(&check));
        assert_eq!(s.take_pending_check(), Some(check));
        assert_eq!(s.take_pending_check(), None, "grava uma vez só");
    }

    #[test]
    fn the_check_follows_the_sheet_and_the_game() {
        let mut s = active();
        s.deliver(10);
        s.record_check(check_for(s.sheet().unwrap()));
        s.discard();
        assert!(s.check().is_none(), "descartar a Súmula leva a conferência");
        let mut s = active();
        s.deliver(10);
        s.record_check(check_for(s.sheet().unwrap()));
        s.follow(2, false, true);
        assert!(s.check().is_none(), "Nova Partida começa sem conferência");
    }

    #[test]
    fn a_stored_check_is_not_recomputed() {
        let mut s = ScoresheetUi::default();
        s.follow(1, false, true);
        let mut stored = ManualScoresheet::new();
        stored.deliver(1).unwrap();
        let check = check_for(&stored);
        s.loaded(Some(stored));
        s.loaded_check(Some(check));
        assert!(!s.needs_check());
        assert!(!s.has_pending());
    }

    #[test]
    fn rows_follow_the_standard_layout() {
        assert!(sheet_rows(1, Side::White, 0).is_empty());
        let rows = sheet_rows(1, Side::White, 3);
        assert_eq!(
            rows,
            vec![
                SheetRow {
                    number: 1,
                    white: Some(1),
                    black: Some(2)
                },
                SheetRow {
                    number: 2,
                    white: Some(3),
                    black: None
                },
            ]
        );
    }

    #[test]
    fn rows_start_with_black_after_a_fen_with_black_to_move() {
        let rows = sheet_rows(7, Side::Black, 2);
        assert_eq!(
            rows,
            vec![
                SheetRow {
                    number: 7,
                    white: None,
                    black: Some(1)
                },
                SheetRow {
                    number: 8,
                    white: Some(2),
                    black: None
                },
            ]
        );
    }

    #[test]
    fn history_is_refreshed_on_period_change_new_game_and_new_check() {
        let mut s = ScoresheetUi::default();
        s.follow(1, false, true);
        assert!(s.needs_history());
        s.loaded_history(None);
        assert!(!s.needs_history());
        s.set_history_period(HistoryPeriod::Week);
        assert!(!s.needs_history(), "mesmo período: nada a reler");
        s.set_history_period(HistoryPeriod::Month);
        assert!(s.needs_history());
        s.loaded_history(None);
        s.invalidate_history();
        assert!(s.needs_history());
        s.loaded_history(None);
        s.follow(2, false, true);
        assert!(s.needs_history(), "outra Partida/perfil: relê");
    }

    #[test]
    fn pdf_is_saved_in_the_folder_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("chess-studio-pdf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = save_pdf(b"%PDF-1", &dir, 1_700_000_000_000, false).unwrap();
        let second = save_pdf(b"%PDF-2", &dir, 1_700_000_000_000, false).unwrap();
        let blank = save_pdf(b"%PDF-3", &dir, 1_700_000_000_000, true).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"%PDF-1");
        assert!(
            first
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".pdf")
        );
        assert!(
            blank
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("em-branco")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_writes_a_real_pdf_for_annotated_and_blank() {
        let dir = std::env::temp_dir().join(format!("chess-studio-pdf2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let i18n = I18n::pt_br();
        let mut sheet = ManualScoresheet::new();
        sheet.write(1, "e4").unwrap();
        let game = PdfGame {
            played: 1,
            first_number: 1,
            first_side: Side::White,
            result: String::new(),
        };
        let a = export_pdf(&i18n, PdfKind::Annotated, Some(&sheet), &game, &dir, 5).unwrap();
        let b = export_pdf(&i18n, PdfKind::Blank, None, &game, &dir, 5).unwrap();
        for path in [a, b] {
            assert!(std::fs::read(path).unwrap().starts_with(b"%PDF-"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn br_date_is_day_month_year() {
        assert_eq!(br_date(0), "01/01/1970");
    }
}
