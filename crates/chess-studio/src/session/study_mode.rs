//! Modo de estudo e Linha de estudo (#322): o usuário move peças a partir de uma posição da
//! Partida, só Lances legais, **sem alterar a Partida**.
//!
//! - Estado só de leitura e **fora do log** (ADR-0001, ADR-0002): nenhum Comando, nenhum
//!   Evento, nada no PGN nem nas Anotações de estudo. A Linha de estudo vive só na `Session`
//!   e some ao voltar à Partida, ao trocar a Partida (Nova Partida, Retomar, Perfil local,
//!   Biblioteca) ou ao navegar pela Revisão comum ([`Session::set_review_ply`]).
//! - Uma só linha, sem ramos: um Lance novo no meio dela descarta o resto.
//! - A posição mostrada passa pelo mesmo mecanismo da Revisão e das variantes da Análise
//!   ([`Session::review`]), então peças, material capturado e marcações seguem a linha.
//! - Cada Lance é validado pelo núcleo de regras (`chess-core`); o intérprete de gestos é o
//!   mesmo do jogo normal, alimentado com a posição do estudo.

use super::{Session, SettleHint, lifted};
use crate::study::{StudyRefusal, study_mode_available};
use bevy::prelude::Vec3;
use chess_core::{CanonicalPosition, Color, Move, Square};
use chess_game::{HistoryEntry, ReviewView};
use chess_input::GestureEvent;

/// Um Lance da Linha de estudo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudyMove {
    /// Meio-lance na numeração da Partida (o ponto de partida mais a posição na linha).
    pub ply: usize,
    pub side: Color,
    pub mv: Move,
    pub san: String,
    /// A posição depois do Lance.
    pub position_after: CanonicalPosition,
}

/// A Linha de estudo: uma só sequência de Lances legais a partir de uma posição da Partida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudyLine {
    /// A posição de onde a linha partiu (a Revisão no momento da entrada).
    start: ReviewView,
    moves: Vec<StudyMove>,
    /// Quantos Lances da linha estão feitos na posição mostrada: `0` é o ponto de partida.
    cursor: usize,
    /// Meios-lance da Partida quando o estudo começou.
    game_plies_at_start: usize,
}

impl StudyLine {
    /// Meio-lance da Partida de onde a linha partiu (`0` é a posição inicial).
    pub fn base_ply(&self) -> usize {
        self.start.ply
    }

    /// A posição de partida, como Revisão.
    pub fn start(&self) -> &ReviewView {
        &self.start
    }

    /// Os Lances da linha, do primeiro ao último.
    pub fn moves(&self) -> &[StudyMove] {
        &self.moves
    }

    /// Quantos Lances da linha estão feitos na posição mostrada (`0`: ponto de partida;
    /// `moves().len()`: fim da linha).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_at_start(&self) -> bool {
        self.cursor == 0
    }

    pub fn is_at_end(&self) -> bool {
        self.cursor == self.moves.len()
    }

    /// A posição mostrada agora.
    pub fn position(&self) -> &CanonicalPosition {
        match self.cursor.checked_sub(1) {
            None => &self.start.position,
            Some(i) => &self.moves[i].position_after,
        }
    }

    /// A posição mostrada agora, como Revisão (o que o tabuleiro desenha).
    fn view(&self) -> ReviewView {
        let Some(i) = self.cursor.checked_sub(1) else {
            return self.start.clone();
        };
        let played = &self.moves[i];
        ReviewView {
            ply: self.start.ply,
            total_plies: self.start.total_plies,
            side_to_move: played.position_after.side_to_move(),
            in_check: played.position_after.is_check(),
            position: played.position_after.clone(),
            last_move: Some(HistoryEntry {
                ply: played.ply,
                side: played.side,
                mv: played.mv,
                san: played.san.clone(),
            }),
            is_start: false,
            is_end: false,
        }
    }
}

impl Session {
    /// Linha de estudo em curso, se o Modo de estudo está ligado.
    pub fn study_line(&self) -> Option<&StudyLine> {
        self.study.as_ref()
    }

    /// O Modo de estudo está ligado?
    pub fn studying(&self) -> bool {
        self.study.is_some()
    }

    /// Por que o Modo de estudo não liga agora, ou `None` se liga. A regra é a das Anotações de
    /// estudo ([`crate::study::quick_notes_available`]) mais Partida online e modo espectador,
    /// testável sozinha em [`study_mode_available`].
    pub fn study_refusal(&self) -> Option<StudyRefusal> {
        study_mode_available(
            self.competitive_in_progress(),
            self.is_online(),
            self.is_spectator(),
            self.exercise,
        )
        .err()
    }

    /// Marca esta `Session` como a de um Exercício do Treino (que ocupa o lugar da Partida do
    /// usuário): não há Modo de estudo nela, e a apresentação e a revisão do Exercício
    /// continuam só de leitura.
    pub fn mark_exercise(&mut self) {
        self.exercise = true;
        self.study = None;
    }

    /// Liga o Modo de estudo a partir da posição mostrada (a ao vivo, ou a da Revisão em
    /// curso): a Ação "Estudar aqui". Já em estudo, não muda nada.
    pub fn study_here(&mut self) -> Result<(), StudyRefusal> {
        if let Some(refusal) = self.study_refusal() {
            return Err(refusal);
        }
        if self.study.is_some() {
            return Ok(());
        }
        let start = match &self.review {
            Some(view) if !self.variation => view.clone(),
            // O fim do histórico sempre existe: é a posição ao vivo.
            _ => self
                .game
                .review_at_ply(self.game.snapshot().history.len())
                .expect("o fim do histórico é uma posição válida"),
        };
        self.study = Some(StudyLine {
            game_plies_at_start: self.game.snapshot().history.len(),
            start,
            moves: Vec::new(),
            cursor: 0,
        });
        self.show_study();
        Ok(())
    }

    /// A posição em que um gesto joga agora: a da Linha de estudo ou, numa Revisão, a
    /// posição revisada (o gesto inicia o estudo). `None`: o gesto segue o jogo normal.
    pub(super) fn study_position(&self) -> Option<CanonicalPosition> {
        match (&self.study, &self.review) {
            (Some(line), Some(shown)) if *shown == line.view() => Some(line.position().clone()),
            (None, Some(review)) if !self.variation && self.study_refusal().is_none() => {
                Some(review.position.clone())
            }
            _ => None,
        }
    }

    /// Gesto numa posição de estudo: o intérprete decide, o núcleo de regras valida.
    pub(super) fn study_gesture(&mut self, event: GestureEvent, position: &CanonicalPosition) {
        let visual = self.interpreter.visual();
        let snapshot = self.study_snapshot(position);
        let commands = self.interpreter.handle(event, &snapshot);
        for command in commands {
            if let chess_game::Command::Release { from, to } = command {
                let start = visual
                    .drag_world
                    .map_or_else(|| lifted(chess_input::square_center(from)), lifted);
                self.study_release(position, from, to, start);
                let snapshot = self.study_snapshot(position);
                self.interpreter
                    .handle(GestureEvent::Resolved, &snapshot);
            }
        }
        let snapshot = self.study_snapshot(self.study_position().as_ref().unwrap_or(position));
        self.interpreter.handle(GestureEvent::Resync, &snapshot);
    }

    /// A Partida vista pelo intérprete de gestos: a posição do estudo, sempre como Assistido
    /// (sem relógio, sem Toque intencional), sem alterar a `Session`.
    fn study_snapshot(&self, position: &CanonicalPosition) -> chess_game::Snapshot {
        let mut snapshot = self.game.snapshot();
        snapshot.policy = chess_game::PolicyKind::Assisted;
        snapshot.position = position.clone();
        snapshot.side_to_move = position.side_to_move();
        snapshot.in_check = position.is_check();
        snapshot.outcome = None;
        snapshot.provisional = None;
        snapshot.paused = false;
        snapshot.touched.clear();
        snapshot.must_move_from.clear();
        snapshot.made_move = None;
        snapshot.provisional_release = None;
        snapshot
    }

    /// Peça solta numa posição de estudo: Lance legal entra na linha; ilegal volta à origem,
    /// sem sair do modo.
    fn study_release(
        &mut self,
        position: &CanonicalPosition,
        from: Square,
        to: Square,
        start: Vec3,
    ) {
        let legal = position
            .legal_moves()
            .into_iter()
            .any(|m| m.from() == from && m.to() == to);
        if !legal {
            self.hint = Some(SettleHint {
                square: from,
                from: start,
            });
            self.revision = self.revision.wrapping_add(1);
            return;
        }
        self.hint = Some(SettleHint {
            square: to,
            from: start,
        });
        self.study_push(Move::new(from, to, None));
    }

    /// Acrescenta `mv` à linha, no ponto mostrado: o que vinha depois dele é descartado (uma
    /// só linha). Sem estudo em curso, a linha parte da Revisão mostrada.
    fn study_push(&mut self, mv: Move) {
        let Some(shown) = self.review.clone() else {
            return;
        };
        let mut line = self.study.take().unwrap_or_else(|| StudyLine {
            game_plies_at_start: self.game.snapshot().history.len(),
            start: shown,
            moves: Vec::new(),
            cursor: 0,
        });
        line.moves.truncate(line.cursor);
        let before = line.position().clone();
        let (Ok(san), Ok(after)) = (before.san(mv), before.play(mv)) else {
            self.study = Some(line);
            return;
        };
        line.moves.push(StudyMove {
            ply: line.start.ply + line.moves.len() + 1,
            side: before.side_to_move(),
            mv,
            san,
            position_after: after,
        });
        line.cursor = line.moves.len();
        self.study = Some(line);
        self.show_study();
    }

    /// Mostra no tabuleiro a posição da linha (a mesma via da Revisão e das variantes).
    fn show_study(&mut self) {
        let Some(line) = &self.study else {
            return;
        };
        self.review = Some(line.view());
        self.variation = !line.is_at_start();
        self.interpreter
            .handle(GestureEvent::PanelInteraction, &self.snapshot);
        self.revision = self.revision.wrapping_add(1);
    }
}
