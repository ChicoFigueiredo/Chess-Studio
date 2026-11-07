#![allow(clippy::unwrap_used)]
//! Histórico de erros de Súmula por período (V2-SUM-03): consulta sobre as conferências
//! gravadas (V2-SUM-02), por Perfil local, com instantes injetados.

mod common;

use chess_game::{GameSetup, PolicyKind};
use chess_store::{GameMetadata, ProfileId, Store};
use chess_training::{
    EntryCheck, HistoryFingerprint, HistoryPeriod, ManualScoresheet, ScoresheetCheck,
    ScoresheetHistory, Verdict,
};
use common::{TempDb, play};

const DAY: i64 = 86_400_000;
/// "Agora" injetado: o dia 100 desde a época (nunca o relógio do sistema).
const NOW: i64 = 100 * DAY;

fn setup() -> (TempDb, Store, ProfileId, ProfileId) {
    let db = TempDb::new("scoresheet-history");
    let store = Store::open(db.path()).unwrap();
    let ana = store.default_profile().unwrap();
    let beto = store.create_profile("Beto").unwrap();
    (db, store, ana, beto)
}

/// Uma Partida do perfil com Súmula entregue e conferida em `checked_at`, com o
/// resultado `verdicts` (um por meio-lance).
fn conferred(
    store: &mut Store,
    profile: ProfileId,
    checked_at: i64,
    verdicts: &[Verdict],
) -> chess_store::GameId {
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("e2", "e4"), ("e7", "e5")],
    );
    let id = store
        .create_game_for(profile, game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    let mut sheet = ManualScoresheet::new();
    sheet.write(1, "e4").unwrap();
    sheet.deliver(checked_at - 1).unwrap();
    store.save_scoresheet(profile, id, &sheet).unwrap();
    recheck(store, profile, id, checked_at, verdicts);
    id
}

/// Grava (ou regrava) a conferência vigente da Súmula já entregue de `id`.
fn recheck(
    store: &mut Store,
    profile: ProfileId,
    id: chess_store::GameId,
    checked_at: i64,
    verdicts: &[Verdict],
) {
    let entries = verdicts
        .iter()
        .enumerate()
        .map(|(i, v)| EntryCheck {
            ply: u16::try_from(i + 1).unwrap(),
            written: Some("x".into()),
            expected: Some("y".into()),
            verdict: *v,
        })
        .collect::<Vec<_>>();
    store
        .save_scoresheet_check(
            profile,
            id,
            &ScoresheetCheck::from_parts(checked_at, HistoryFingerprint::from_parts(2, 0), entries),
        )
        .unwrap();
}

#[test]
fn recalculated_check_replaces_the_previous_one_and_counts_once() {
    let (_db, mut store, ana, _) = setup();
    let id = conferred(&mut store, ana, NOW - 2 * DAY, &[Correct, Swapped, Missing]);
    recheck(&mut store, ana, id, NOW - DAY, &[Correct, Correct]);
    let h = store
        .scoresheet_history(ana, HistoryPeriod::Week, NOW)
        .unwrap();
    assert_eq!(h.sheets, 1);
    assert_eq!(h.totals.total(), 2);
    assert_eq!(h.totals.swapped, 0);
    assert_eq!(h.points.len(), 1);
}

use Verdict::{AcceptedWithCaveat as Caveat, Correct, Extra, Missing, Swapped, WrongNotation};

#[test]
fn empty_history_has_no_sheets() {
    let (_db, store, ana, _) = setup();
    for p in HistoryPeriod::ALL {
        let h = store.scoresheet_history(ana, p, NOW).unwrap();
        assert_eq!(h.sheets, 0);
        assert_eq!(h.totals.total(), 0);
        assert!(h.points.is_empty());
    }
}

#[test]
fn totals_per_class_and_evolution_for_one_profile() {
    let (_db, mut store, ana, _) = setup();
    conferred(&mut store, ana, NOW - 2 * DAY, &[Correct, Swapped, Missing]);
    conferred(
        &mut store,
        ana,
        NOW - DAY,
        &[Correct, Correct, WrongNotation, Extra],
    );
    conferred(&mut store, ana, NOW - DAY + 10, &[Caveat, Correct]);
    let h = store
        .scoresheet_history(ana, HistoryPeriod::Week, NOW)
        .unwrap();
    assert_eq!(h.sheets, 3);
    assert_eq!(h.totals.correct, 4);
    assert_eq!(h.totals.accepted, 1);
    assert_eq!(h.totals.wrong_notation, 1);
    assert_eq!(h.totals.swapped, 1);
    assert_eq!(h.totals.missing, 1);
    assert_eq!(h.totals.extra, 1);
    // Evolução: dois dias, em ordem cronológica; o último junta duas Súmulas.
    assert_eq!(h.points.len(), 2);
    assert_eq!(h.points[0].sheets, 1);
    assert_eq!(h.points[1].sheets, 2);
    assert_eq!(h.points[0].counts.swapped, 1);
    assert_eq!(h.points[1].counts.correct, 3);
}

#[test]
fn periods_cut_by_the_injected_instant() {
    let (_db, mut store, ana, _) = setup();
    conferred(&mut store, ana, NOW - 3 * DAY, &[Swapped]); // semana, mês, tudo
    conferred(&mut store, ana, NOW - 20 * DAY, &[Missing, Missing]); // mês, tudo
    conferred(&mut store, ana, NOW - 90 * DAY, &[WrongNotation]); // só tudo
    conferred(&mut store, ana, NOW + DAY, &[Extra]); // futuro: nunca entra
    let h = |p| store.scoresheet_history(ana, p, NOW).unwrap();
    let (week, month, all) = (
        h(HistoryPeriod::Week),
        h(HistoryPeriod::Month),
        h(HistoryPeriod::All),
    );
    assert_eq!((week.sheets, month.sheets, all.sheets), (1, 2, 3));
    assert_eq!(week.totals.swapped, 1);
    assert_eq!(week.totals.missing, 0);
    assert_eq!(month.totals.missing, 2);
    assert_eq!(month.totals.wrong_notation, 0);
    assert_eq!(all.totals.wrong_notation, 1);
    assert_eq!(all.totals.extra, 0, "o futuro não entra");
    // Em "tudo" a evolução é mensal: ao menos dois meses distintos.
    assert!(all.points.len() >= 2);
    // Mudar o "agora" muda o recorte (nada de relógio do sistema).
    let later = store
        .scoresheet_history(ana, HistoryPeriod::Week, NOW + 5 * DAY)
        .unwrap();
    assert_eq!(
        later.sheets, 1,
        "a conferência futura entra quando o agora avança"
    );
}

#[test]
fn two_profiles_have_separate_histories() {
    let (_db, mut store, ana, beto) = setup();
    conferred(&mut store, ana, NOW - DAY, &[Swapped, Swapped]);
    conferred(&mut store, beto, NOW - DAY, &[Correct]);
    conferred(&mut store, beto, NOW - 2 * DAY, &[Missing]);
    let a: ScoresheetHistory = store
        .scoresheet_history(ana, HistoryPeriod::Month, NOW)
        .unwrap();
    let b = store
        .scoresheet_history(beto, HistoryPeriod::Month, NOW)
        .unwrap();
    assert_eq!((a.sheets, a.totals.swapped, a.totals.correct), (1, 2, 0));
    assert_eq!(
        (
            b.sheets,
            b.totals.swapped,
            b.totals.correct,
            b.totals.missing
        ),
        (2, 0, 1, 1)
    );
}

#[test]
fn unchecked_or_undelivered_sheets_do_not_count_and_history_is_read_only() {
    let (_db, mut store, ana, _) = setup();
    let (game, events) = play(GameSetup::standard(PolicyKind::Assisted), &[("e2", "e4")]);
    let id = store
        .create_game_for(ana, game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    let mut sheet = ManualScoresheet::new();
    sheet.write(1, "e4").unwrap();
    store.save_scoresheet(ana, id, &sheet).unwrap(); // em anotação, sem conferência
    let before = store.load_scoresheet(ana, id).unwrap();
    let h = store
        .scoresheet_history(ana, HistoryPeriod::All, NOW)
        .unwrap();
    assert_eq!(h.sheets, 0);
    assert_eq!(store.load_scoresheet(ana, id).unwrap(), before);
}

#[test]
fn unknown_profile_is_an_error() {
    let (_db, store, _, _) = setup();
    assert!(
        store
            .scoresheet_history(ProfileId(9_999), HistoryPeriod::All, NOW)
            .is_err()
    );
}
