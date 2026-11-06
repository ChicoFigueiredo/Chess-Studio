//! Sequências de ponteiro até Comandos e Partida (INT-01..04, UX-02, UX-09).

use chess_core::{Color, Piece, Square};
use chess_game::{Command, Game, GameSetup, PolicyKind};
use chess_input::{
    GestureEvent, GestureInterpreter, GestureState, Modifiers, MoveMethod, PointerButton,
};
use glam::Vec3;

fn sq(name: &str) -> Square {
    name.parse().unwrap()
}

fn new_game() -> Game {
    Game::new(GameSetup::standard(PolicyKind::Assisted))
}

fn down(button: PointerButton, at: &str) -> GestureEvent {
    GestureEvent::PointerDown {
        button,
        square: Some(sq(at)),
        modifiers: Modifiers::default(),
    }
}

fn press(at: &str) -> GestureEvent {
    down(PointerButton::Primary, at)
}

fn mv(at: &str) -> GestureEvent {
    GestureEvent::PointerMove {
        square: Some(sq(at)),
        world: Vec3::ZERO,
    }
}

fn up(at: &str) -> GestureEvent {
    GestureEvent::PointerUp {
        square: Some(sq(at)),
    }
}

fn release(from: &str, to: &str) -> Command {
    Command::Release {
        from: sq(from),
        to: sq(to),
    }
}

/// Alimenta os eventos em ordem, junta os Comandos emitidos e os aplica à Partida
/// (como o app faz), avisando o intérprete com `Resolved` depois de cada Release.
fn run(interp: &mut GestureInterpreter, game: &mut Game, events: &[GestureEvent]) -> Vec<Command> {
    let mut all = Vec::new();
    for (i, event) in events.iter().enumerate() {
        let snap = game.snapshot();
        let commands = interp.handle(*event, &snap);
        for c in &commands {
            game.handle(c.clone(), chess_game::Timestamp::mono(i as u64))
                .unwrap();
        }
        if commands
            .iter()
            .any(|c| matches!(c, Command::Release { .. }))
        {
            interp.handle(GestureEvent::Resolved, &game.snapshot());
        }
        all.extend(commands);
    }
    all
}

fn fresh(method: MoveMethod) -> (GestureInterpreter, Game) {
    (GestureInterpreter::new(method), new_game())
}

fn two_taps(from: &str, to: &str) -> Vec<GestureEvent> {
    vec![press(from), up(from), press(to), up(to)]
}

fn drag(from: &str, to: &str) -> Vec<GestureEvent> {
    vec![press(from), mv(from), mv(to), up(to)]
}

// --- INT-01 / UX-02 ---------------------------------------------------------

#[test]
fn two_taps_and_drag_emit_the_same_release_and_reach_the_same_position() {
    let (mut a, mut ga) = fresh(MoveMethod::Both);
    let (mut b, mut gb) = fresh(MoveMethod::Both);
    let ca = run(&mut a, &mut ga, &two_taps("e2", "e4"));
    let cb = run(&mut b, &mut gb, &drag("e2", "e4"));
    assert_eq!(ca, vec![release("e2", "e4")]);
    assert_eq!(ca, cb);
    assert_eq!(ga.snapshot().position, gb.snapshot().position);
    assert_eq!(ga.snapshot().history[0].san, "e4");
    assert_eq!(a.state(), GestureState::Idle);
}

#[test]
fn capture_by_two_taps_and_by_drag_agree() {
    for events in [two_taps("e4", "d5"), drag("e4", "d5")] {
        let (mut i, mut g) = fresh(MoveMethod::Both);
        run(&mut i, &mut g, &two_taps("e2", "e4"));
        run(&mut i, &mut g, &two_taps("d7", "d5"));
        let c = run(&mut i, &mut g, &events);
        assert_eq!(c, vec![release("e4", "d5")]);
        assert_eq!(g.snapshot().history[2].san, "exd5");
    }
}

#[test]
fn two_taps_only_accepts_taps_and_ignores_drag_release() {
    let (mut i, mut g) = fresh(MoveMethod::TwoTaps);
    // Pressionar já seleciona; arrastar e soltar em outra casa não move (é clique de destino
    // só se pressionado ali), e soltar fora do clique de destino não emite nada.
    assert_eq!(
        run(&mut i, &mut g, &two_taps("g1", "f3")),
        vec![release("g1", "f3")]
    );
    let (mut i, mut g) = fresh(MoveMethod::TwoTaps);
    let c = run(&mut i, &mut g, &[press("e2"), mv("e4"), up("e4")]);
    assert!(c.is_empty());
}

#[test]
fn drag_only_does_not_select_on_click() {
    let (mut i, mut g) = fresh(MoveMethod::Drag);
    let c = run(&mut i, &mut g, &two_taps("e2", "e4"));
    assert!(c.is_empty(), "{c:?}");
    assert_eq!(i.state(), GestureState::Idle);
    let c = run(&mut i, &mut g, &drag("e2", "e4"));
    assert_eq!(c, vec![release("e2", "e4")]);
}

#[test]
fn both_allows_click_select_then_drag_of_another_piece() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    let c = run(
        &mut i,
        &mut g,
        &[press("e2"), up("e2"), press("d2"), mv("d3"), mv("d4"), up("d4")],
    );
    assert_eq!(c, vec![release("d2", "d4")]);
}

// --- INT-02 -----------------------------------------------------------------

#[test]
fn illegal_destination_is_forwarded_verbatim_never_converted() {
    for events in [two_taps("e2", "e5"), drag("e2", "e5")] {
        let (mut i, mut g) = fresh(MoveMethod::Both);
        let c = run(&mut i, &mut g, &events);
        assert_eq!(c, vec![release("e2", "e5")]);
        let snap = g.snapshot();
        assert!(snap.history.is_empty(), "lance ilegal não entra no histórico");
        assert_eq!(snap.side_to_move, Color::White);
        assert_eq!(i.state(), GestureState::Idle);
    }
}

// --- INT-03 / UX-09 ---------------------------------------------------------

fn cancelers() -> Vec<(&'static str, GestureEvent)> {
    vec![
        ("Esc", GestureEvent::Escape),
        ("foco", GestureEvent::FocusLost),
        ("painel", GestureEvent::PanelInteraction),
        ("órbita", GestureEvent::OrbitStarted),
    ]
}

#[test]
fn cancel_events_abort_drag_without_any_command() {
    for (name, cancel) in cancelers() {
        let (mut i, mut g) = fresh(MoveMethod::Both);
        let c = run(&mut i, &mut g, &[press("e2"), mv("e3"), mv("e4"), cancel, up("e4")]);
        assert!(c.is_empty(), "{name}: {c:?}");
        assert_eq!(i.state(), GestureState::Idle, "{name}");
        assert!(g.snapshot().history.is_empty(), "{name}");
        assert!(g.events().is_empty(), "{name}: nada no log");
    }
}

#[test]
fn cancel_events_abort_selection_and_a_late_click_does_not_move() {
    for (name, cancel) in cancelers() {
        let (mut i, mut g) = fresh(MoveMethod::Both);
        let c = run(
            &mut i,
            &mut g,
            &[press("e2"), up("e2"), cancel, press("e4"), up("e4")],
        );
        assert!(c.is_empty(), "{name}: {c:?}");
        assert!(g.events().is_empty(), "{name}");
    }
}

#[test]
fn cancel_while_held_and_after_drag_start_leaves_nothing_behind() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    run(&mut i, &mut g, &[press("g1"), GestureEvent::FocusLost]);
    assert_eq!(i.state(), GestureState::Idle);
    assert!(i.visual().origin.is_none());
    assert!(!i.visual().lifted);
}

#[test]
fn escape_during_pending_promotion_retracts_in_assisted() {
    let mut i = GestureInterpreter::new(MoveMethod::Both);
    let mut g;
    let fen = "8/4P2k/8/8/8/8/8/4K3 w - - 0 1";
    g = Game::new(GameSetup::from_fen(PolicyKind::Assisted, fen).unwrap());
    let c = run(&mut i, &mut g, &two_taps("e7", "e8"));
    assert_eq!(c, vec![release("e7", "e8")]);
    assert!(g.snapshot().provisional.is_some());
    // Enquanto o seletor está aberto, ponteiro no tabuleiro é ignorado.
    assert!(run(&mut i, &mut g, &two_taps("e1", "e2")).is_empty());
    let c = run(&mut i, &mut g, &[GestureEvent::Escape]);
    assert_eq!(c, vec![Command::RetractMove]);
    assert!(g.snapshot().provisional.is_none());
    // Escolher a peça é Comando do seletor (egui), não do intérprete.
    assert!(run(&mut i, &mut g, &two_taps("e7", "e8")).len() == 1);
    g.handle(Command::ChoosePromotion { piece: Piece::Queen }, chess_game::Timestamp::mono(99)).unwrap();
}

// --- INT-04 -----------------------------------------------------------------

#[test]
fn dragging_emits_nothing_until_release_and_exposes_visual_state() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    let before = g.snapshot();
    let world = Vec3::new(1.0, 0.0, 2.0);
    let mut cmds = Vec::new();
    for e in [
        press("e2"),
        GestureEvent::PointerMove {
            square: Some(sq("e3")),
            world,
        },
        mv("e4"),
        GestureEvent::PointerMove {
            square: Some(sq("e5")),
            world,
        },
    ] {
        cmds.extend(i.handle(e, &g.snapshot()));
    }
    assert!(cmds.is_empty());
    let v = i.visual();
    assert_eq!(v.origin, Some(sq("e2")));
    assert!(v.lifted);
    assert_eq!(v.drag_world, Some(world)); // último PointerMove
    assert!(matches!(i.state(), GestureState::Dragging { .. }));
    assert_eq!(g.snapshot(), before, "preview não altera a Partida");
    assert!(g.events().is_empty());
}

// --- Cliques que não selecionam / botões ------------------------------------

#[test]
fn click_on_empty_square_does_not_select() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    let c = run(&mut i, &mut g, &two_taps("e4", "e5"));
    assert!(c.is_empty());
    assert_eq!(i.state(), GestureState::Idle);
    assert!(!i.visual().lifted);
}

#[test]
fn click_on_opponent_piece_does_not_select() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    assert!(run(&mut i, &mut g, &two_taps("e7", "e5")).is_empty());
    assert_eq!(i.state(), GestureState::Idle);
}

#[test]
fn middle_and_secondary_buttons_never_move_pieces() {
    for button in [PointerButton::Middle, PointerButton::Secondary] {
        let (mut i, mut g) = fresh(MoveMethod::Both);
        let c = run(
            &mut i,
            &mut g,
            &[down(button, "e2"), mv("e3"), mv("e4"), up("e4")],
        );
        assert!(c.is_empty());
        assert_eq!(i.state(), GestureState::Idle);
        // Nem como destino de uma seleção por clique.
        let c = run(
            &mut i,
            &mut g,
            &[press("e2"), up("e2"), down(button, "e4"), up("e4")],
        );
        assert!(c.is_empty(), "{button:?}: {c:?}");
        assert!(g.events().is_empty());
    }
}

#[test]
fn clicking_the_selected_piece_again_deselects() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    run(&mut i, &mut g, &[press("e2"), up("e2")]);
    assert_eq!(i.state(), GestureState::Selected { from: sq("e2") });
    run(&mut i, &mut g, &[press("e2"), up("e2")]);
    assert_eq!(i.state(), GestureState::Idle);
}

#[test]
fn clicking_another_own_piece_changes_selection() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    let c = run(&mut i, &mut g, &[press("e2"), up("e2"), press("d2"), up("d2")]);
    assert!(c.is_empty());
    assert_eq!(i.state(), GestureState::Selected { from: sq("d2") });
}

#[test]
fn drop_on_origin_or_off_board_cancels() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    let c = run(&mut i, &mut g, &[press("e2"), mv("e3"), up("e2")]);
    assert!(c.is_empty());
    assert_eq!(i.state(), GestureState::Idle);
    let off = GestureEvent::PointerUp { square: None };
    let c = run(&mut i, &mut g, &[press("e2"), mv("e3"), off]);
    assert!(c.is_empty());
    assert_eq!(i.state(), GestureState::Idle);
}

// --- Estado após o Lance ----------------------------------------------------

#[test]
fn pending_release_ignores_pointer_until_resolved() {
    let (mut i, g) = fresh(MoveMethod::Both);
    let snap = g.snapshot();
    for e in two_taps("e2", "e4") {
        i.handle(e, &snap);
    }
    assert_eq!(
        i.state(),
        GestureState::ReleasedPending {
            from: sq("e2"),
            to: sq("e4")
        }
    );
    assert!(i.handle(press("d2"), &snap).is_empty());
    i.handle(GestureEvent::Resolved, &snap);
    assert_eq!(i.state(), GestureState::Idle);
}

#[test]
fn selection_is_dropped_when_the_game_changes_underneath() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    run(&mut i, &mut g, &[press("e2"), up("e2")]);
    g.handle(release("d2", "d4"), chess_game::Timestamp::mono(50))
        .unwrap();
    i.handle(GestureEvent::Resync, &g.snapshot());
    assert_eq!(i.state(), GestureState::Idle);
}

#[test]
fn nothing_is_selectable_after_the_game_ends() {
    let (mut i, mut g) = fresh(MoveMethod::Both);
    g.handle(
        Command::Resign { side: Color::White },
        chess_game::Timestamp::mono(1),
    )
    .unwrap();
    assert!(run(&mut i, &mut g, &two_taps("e7", "e5")).is_empty());
    assert_eq!(i.state(), GestureState::Idle);
}
