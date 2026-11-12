//! Prévia da importação de um arquivo PGN (#270, especificação §6.5): antes de gravar
//! qualquer coisa, mostra quantas Partidas há, os cabeçalhos das primeiras e a fonte que as
//! tags sugerem (a pessoa confirma ou troca). Quando a licença da fonte é restrita, pede a
//! confirmação de direitos. Só lê o começo do arquivo; nada é gravado até "Importar".

use crate::i18n::I18n;
use crate::library_ui::source_key;
use crate::ui_theme::{
    CardEmphasis, StatusTone, cs_card_frame, cs_checkbox, cs_status_badge, palette,
};
use bevy_egui::egui;
use chess_store::{
    DEFAULT_PREVIEW_BYTES, Estimate, ImportPreview, ImportSource, PreviewGame, preview_pgn_file,
};
use egui_phosphor::regular as icon;
use std::path::Path;

/// A prévia do arquivo escolhido e a confirmação de direitos.
#[derive(Debug, Default)]
pub struct PreviewState {
    /// O caminho a que `result` se refere.
    path: String,
    result: Option<Result<ImportPreview, String>>,
    /// "Confirmo que posso usar estas Partidas" (só vale para a fonte escolhida agora).
    rights: Option<ImportSource>,
}

impl PreviewState {
    /// Relê a prévia quando o caminho mudou; sugere a fonte detectada se a pessoa ainda não
    /// escolheu nenhuma ("Outro ou não sei").
    pub fn refresh(&mut self, path: &str, source: &mut ImportSource) -> bool {
        let path = path.trim();
        if path == self.path {
            return false;
        }
        let before = *source;
        self.path = path.to_owned();
        self.rights = None;
        self.result = (!path.is_empty() && Path::new(path).is_file()).then(|| {
            preview_pgn_file(Path::new(path), DEFAULT_PREVIEW_BYTES).map_err(|e| e.to_string())
        });
        if let Some(Ok(preview)) = &self.result
            && let Some(detected) = preview.detected
            && *source == ImportSource::Other
        {
            *source = detected;
        }
        *source != before
    }

    /// Dá para importar: há uma prévia legível e, se a fonte é restrita, os direitos foram
    /// confirmados.
    pub fn can_import(&self, source: ImportSource) -> bool {
        matches!(self.result, Some(Ok(_)))
            && (!source.is_restricted() || self.rights == Some(source))
    }

    /// Cenas de evidência e testes: uma prévia pronta, sem ler arquivo.
    pub fn with_preview(path: &str, preview: ImportPreview) -> Self {
        Self {
            path: path.to_owned(),
            result: Some(Ok(preview)),
            rights: None,
        }
    }

    pub fn confirm_rights(&mut self, source: ImportSource) {
        self.rights = Some(source);
    }
}

/// "Cerca de 2.000 Partidas", "3 Partidas" ou "Pelo menos 120 Partidas".
pub fn count_text(i18n: &I18n, estimate: Estimate) -> String {
    let (key, n) = match estimate {
        Estimate::Exact(1) => ("import-preview-count-one", 1),
        Estimate::Exact(n) => ("import-preview-count-other", n),
        Estimate::About(n) => ("import-preview-count-about", n),
        Estimate::AtLeast(n) => ("import-preview-count-atleast", n),
    };
    i18n.t_with(key, &[("count", &n.to_string())])
}

/// Uma linha de cabeçalho: "alice × bob · 2026.01.01 · 1-0" (o evento vai na dica).
pub fn header_text(game: &PreviewGame) -> String {
    format!(
        "{} \u{d7} {} \u{b7} {} \u{b7} {}",
        game.white, game.black, game.date, game.result
    )
}

/// Desenha a prévia abaixo do seletor de arquivo e da fonte.
pub fn draw(ui: &mut egui::Ui, i18n: &I18n, state: &mut PreviewState, source: ImportSource) {
    let p = palette(ui);
    match &state.result {
        None => return,
        Some(Err(message)) => {
            ui.colored_label(
                p.danger,
                i18n.t_with("import-preview-failed", &[("detail", message)]),
            );
            return;
        }
        Some(Ok(_)) => {}
    }
    let Some(Ok(preview)) = &state.result else {
        return;
    };
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(i18n.t("import-preview-title")).strong());
        ui.label(count_text(i18n, preview.estimate));
        if preview.compressed {
            ui.small(i18n.t("import-preview-compressed"));
        }
        for game in &preview.games {
            ui.add(egui::Label::new(egui::RichText::new(header_text(game)).small()).truncate())
                .on_hover_text(&game.event);
        }
        if preview.unreadable > 0 {
            ui.small(i18n.t_with(
                "import-preview-unreadable",
                &[("count", &preview.unreadable.to_string())],
            ));
        }
        ui.add_space(4.0);
        match preview.detected {
            Some(found) if found == source => {
                cs_status_badge(
                    ui,
                    StatusTone::Success,
                    icon::CHECK_CIRCLE,
                    &i18n.t_with(
                        "import-preview-detected",
                        &[("source", &i18n.t(source_key(found)))],
                    ),
                );
            }
            Some(found) => {
                cs_status_badge(
                    ui,
                    StatusTone::Warning,
                    icon::WARNING,
                    &i18n.t_with(
                        "import-preview-mismatch",
                        &[
                            ("found", &i18n.t(source_key(found))),
                            ("chosen", &i18n.t(source_key(source))),
                        ],
                    ),
                );
            }
            None => {
                ui.small(i18n.t("import-preview-undetected"));
            }
        }
        if source.is_restricted() {
            ui.add_space(4.0);
            ui.colored_label(
                p.warning,
                i18n.t_with(
                    "import-preview-restricted",
                    &[("source", &i18n.t(source_key(source)))],
                ),
            );
            let mut confirmed = state.rights == Some(source);
            cs_checkbox(ui, &mut confirmed, &i18n.t("import-preview-rights"));
            state.rights = confirmed.then_some(source);
        }
    });
}

/// Cenas de evidência da prévia (`--explorer-demo import-file*`), com dados fictícios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoScene {
    /// Arquivo do Lichess: a fonte é detectada e é livre.
    Detected,
    /// Arquivo sem marca nas tags, fonte TWIC escolhida: pede a confirmação de direitos.
    Restricted,
    /// Depois de confirmar e importar: o resumo conta as Partidas com licença restrita.
    RestrictedDone,
}

impl DemoScene {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "import-file" => Some(Self::Detected),
            "import-file-twic" => Some(Self::Restricted),
            "import-file-twic-done" => Some(Self::RestrictedDone),
            _ => None,
        }
    }

    /// Caminho, fonte, prévia, direitos confirmados e o resumo (se já importou).
    pub fn parts(
        self,
    ) -> (
        &'static str,
        ImportSource,
        ImportPreview,
        bool,
        Option<crate::library_ui::ImportOutcome>,
    ) {
        let game = |w: &str, b: &str, event: &str, date: &str, result: &str| PreviewGame {
            white: w.into(),
            black: b.into(),
            event: event.into(),
            date: date.into(),
            result: result.into(),
        };
        match self {
            Self::Detected => (
                "/home/ana/Documentos/lichess_ana_alvares_2026-10.pgn",
                ImportSource::Lichess,
                ImportPreview {
                    estimate: Estimate::About(2140),
                    games: vec![
                        game(
                            "ana_alvares",
                            "bruno_c",
                            "Rated Blitz game",
                            "2026.10.30",
                            "1-0",
                        ),
                        game(
                            "dora_x",
                            "ana_alvares",
                            "Rated Rapid game",
                            "2026.10.29",
                            "0-1",
                        ),
                        game(
                            "ana_alvares",
                            "edu77",
                            "Rated Blitz game",
                            "2026.10.29",
                            "1/2-1/2",
                        ),
                    ],
                    detected: Some(ImportSource::Lichess),
                    unreadable: 0,
                    bytes_read: 262_144,
                    compressed: false,
                },
                false,
                None,
            ),
            Self::Restricted | Self::RestrictedDone => {
                let done = self == Self::RestrictedDone;
                (
                    "/home/ana/Documentos/semana-1500.pgn",
                    ImportSource::Twic,
                    ImportPreview {
                        estimate: Estimate::About(1854),
                        games: vec![
                            game(
                                "Fulano, A",
                                "Beltrano, B",
                                "Torneio Aberto",
                                "2026.10.20",
                                "1-0",
                            ),
                            game(
                                "Ciclano, C",
                                "Dowling, D",
                                "Torneio Aberto",
                                "2026.10.20",
                                "1/2-1/2",
                            ),
                            game(
                                "Mestre, E",
                                "Exemplo, F",
                                "Torneio Aberto",
                                "2026.10.21",
                                "0-1",
                            ),
                        ],
                        detected: None,
                        unreadable: 0,
                        bytes_read: 262_144,
                        compressed: false,
                    },
                    done,
                    done.then(|| crate::library_ui::ImportOutcome {
                        file: "semana-1500.pgn".into(),
                        imported: 1842,
                        duplicates: 12,
                        error_count: 0,
                        errors: Vec::new(),
                        converted: 0,
                        cancelled: false,
                        source: ImportSource::Twic,
                        restricted: 1842,
                    }),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview(detected: Option<ImportSource>) -> ImportPreview {
        ImportPreview {
            estimate: Estimate::About(2000),
            games: vec![PreviewGame {
                white: "alice".into(),
                black: "bob".into(),
                event: "Rated Blitz game".into(),
                date: "2026.01.01".into(),
                result: "1-0".into(),
            }],
            detected,
            unreadable: 0,
            bytes_read: 1024,
            compressed: false,
        }
    }

    #[test]
    fn nothing_can_be_imported_before_there_is_a_preview() {
        let state = PreviewState::default();
        assert!(!state.can_import(ImportSource::Other));
    }

    #[test]
    fn a_free_source_imports_without_asking_and_a_restricted_one_needs_the_confirmation() {
        let mut state = PreviewState::with_preview("a.pgn", preview(None));
        assert!(state.can_import(ImportSource::Lichess));
        assert!(!state.can_import(ImportSource::Twic));
        state.confirm_rights(ImportSource::Twic);
        assert!(state.can_import(ImportSource::Twic));
        // A confirmação vale para a fonte confirmada, não para outra restrita.
        assert!(!state.can_import(ImportSource::PgnMentor));
    }

    #[test]
    fn the_count_text_says_whether_it_is_exact_an_estimate_or_a_floor() {
        let i18n = I18n::pt_br();
        assert_eq!(count_text(&i18n, Estimate::Exact(3)), "3 Partidas");
        assert_eq!(
            count_text(&i18n, Estimate::About(2000)),
            "Cerca de 2000 Partidas"
        );
        assert_eq!(
            count_text(&i18n, Estimate::AtLeast(120)),
            "Pelo menos 120 Partidas"
        );
    }

    #[test]
    fn the_header_line_shows_players_event_date_and_result() {
        let line = header_text(&preview(None).games[0]);
        assert_eq!(line, "alice \u{d7} bob \u{b7} 2026.01.01 \u{b7} 1-0");
    }

    #[test]
    fn choosing_a_file_suggests_the_detected_source_only_when_none_was_chosen() {
        let dir = std::env::temp_dir().join(format!("cs-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.pgn");
        std::fs::write(
            &file,
            "[Event \"x\"]\n[Site \"https://lichess.org/abcd1234\"]\n[White \"a\"]\n[Black \"b\"]\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n\n",
        )
        .unwrap();
        let path = file.display().to_string();
        let mut chosen = ImportSource::Other;
        let mut state = PreviewState::default();
        state.refresh(&path, &mut chosen);
        assert_eq!(chosen, ImportSource::Lichess);
        // Uma escolha da pessoa não é trocada.
        let mut twic = ImportSource::Twic;
        let mut other_state = PreviewState::default();
        other_state.refresh(&path, &mut twic);
        assert_eq!(twic, ImportSource::Twic);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
