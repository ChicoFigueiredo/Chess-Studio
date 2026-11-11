#![allow(clippy::unwrap_used)]
//! Partida online (ADR-0007, #230): a Partida é uma Partida comum cujo escritor único só
//! registra o que o Provedor confirma. O Provedor fala por retratos (o estado completo);
//! o Lance local é uma intenção enviada, fora do log, até a confirmação.

mod common;
use chess_core::{Color, Move};
use chess_game::{
    Command, CommandKind, Desync, EndReason, Event, EventKind, Game, GameResult, GameSetup,
    OnlineGame, OnlinePlayer, OnlineSetupError, Opponent, Outcome, PolicyKind, ProviderClock,
    ProviderEnd, ProviderSnapshot, Rejection, TimeControl, Timestamp, Variant,
};
use common::sq;

fn online_game(local: Color, opponent: Opponent) -> OnlineGame {
    OnlineGame {
        provider: "lichess".into(),
        remote_id: "qP8XaFtN".into(),
        local_color: local,
        opponent,
        white: OnlinePlayer {
            name: "Kenneth".into(),
            rating: Some(1566),
        },
        black: OnlinePlayer {
            name: "Salma".into(),
            rating: Some(1662),
        },
        rated: false,
        days_per_move: None,
    }
}

const FIVE_MINUTES: TimeControl = TimeControl {
    base_ms: 300_000,
    increment_ms: 0,
};

fn setup_for(local: Color, opponent: Opponent) -> GameSetup {
    GameSetup::online(online_game(local, opponent), Variant::Standard, None)
        .unwrap()
        .with_time_control(FIVE_MINUTES)
}

fn white_vs_human() -> Game {
    Game::new(setup_for(Color::White, Opponent::Human))
}

fn mv(uci: &str) -> Move {
    uci.parse().unwrap()
}

fn at(ms: u64) -> Timestamp {
    Timestamp::mono(ms)
}

/// Retrato com os Lances (UCI separados por espaço), relógios e nada mais.
fn snap(moves: &str, white_ms: u64, black_ms: u64) -> ProviderSnapshot {
    ProviderSnapshot {
        moves: moves.split_whitespace().map(str::to_owned).collect(),
        clock: Some(ProviderClock {
            white_ms,
            black_ms,
            running: moves.split_whitespace().count() >= 2,
        }),
        draw_offer: None,
        end: None,
    }
}

fn ended(
    mut snapshot: ProviderSnapshot,
    result: GameResult,
    reason: EndReason,
) -> ProviderSnapshot {
    snapshot.end = Some(ProviderEnd::Decided(Outcome { result, reason }));
    snapshot
}

fn provider(game: &mut Game, snapshot: ProviderSnapshot, ms: u64) -> Result<Vec<Event>, Rejection> {
    game.handle(Command::Provider(snapshot), at(ms))
}

fn kinds(events: &[Event]) -> Vec<&EventKind> {
    events.iter().map(|e| &e.kind).collect()
}

fn history(game: &Game) -> Vec<String> {
    game.snapshot()
        .history
        .iter()
        .map(|h| h.mv.to_string())
        .collect()
}

// ---- Começo ----------------------------------------------------------------------------

#[test]
fn variante_nao_padrao_e_recusada_antes_de_criar_a_partida() {
    let refused = GameSetup::online(
        online_game(Color::White, Opponent::Human),
        Variant::Other("chess960".into()),
        None,
    );
    assert_eq!(
        refused,
        Err(OnlineSetupError::UnsupportedVariant("chess960".into()))
    );
    let bad_fen = GameSetup::online(
        online_game(Color::White, Opponent::Human),
        Variant::Standard,
        Some("lixo"),
    );
    assert!(matches!(bad_fen, Err(OnlineSetupError::InvalidFen(_))));
}

#[test]
fn a_partida_online_e_uma_partida_comum_com_a_politica_do_provedor() {
    let game = white_vs_human();
    let setup = game.setup();
    assert_eq!(setup.policy(), PolicyKind::Online);
    assert_eq!(setup.online_game().unwrap().remote_id, "qP8XaFtN");
    let snapshot = game.snapshot();
    assert_eq!(snapshot.policy, PolicyKind::Online);
    assert!(!snapshot.can_undo);
    let online = snapshot.online.unwrap();
    assert_eq!(online.sent_move, None);
    assert_eq!(online.desync, None);
    assert!(!online.aborted);
    // Partida local não tem a parte online.
    assert!(common::assisted().snapshot().online.is_none());
}

#[test]
fn posicao_inicial_do_provedor_vale_como_a_de_qualquer_partida() {
    let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
    let setup = GameSetup::online(
        online_game(Color::White, Opponent::Human),
        Variant::Standard,
        Some(fen),
    )
    .unwrap();
    let mut game = Game::new(setup);
    provider(&mut game, snap("e2e4", 0, 0), 10).unwrap();
    assert_eq!(history(&game), ["e2e4"]);
}

// ---- Lances, relógios e o log -------------------------------------------------------------

#[test]
fn lance_confirmado_pelo_provedor_entra_no_log_com_os_relogios() {
    let mut game = white_vs_human();
    let events = provider(&mut game, snap("e2e4", 300_000, 300_000), 1_000).unwrap();
    assert!(matches!(
        kinds(&events)[..],
        [
            EventKind::MoveCompleted { side: Color::White, san, .. },
            EventKind::ClockReported { white_ms: 300_000, black_ms: 300_000, running: None },
        ] if san == "e4"
    ));
    let events = provider(&mut game, snap("e2e4 e7e5", 297_000, 298_500), 4_000).unwrap();
    assert!(matches!(
        kinds(&events)[..],
        [
            EventKind::MoveCompleted {
                side: Color::Black,
                ..
            },
            EventKind::ClockReported {
                white_ms: 297_000,
                black_ms: 298_500,
                running: Some(Color::White)
            },
        ]
    ));
    // O relógio exibido é o do Provedor, contado a partir da chegada do retrato.
    let reading = game.clock_at(at(6_000)).unwrap();
    assert_eq!(reading.white_ms, 295_000);
    assert_eq!(reading.black_ms, 298_500);
    assert_eq!(game.events().len(), 4);
}

#[test]
fn so_o_provedor_decide_o_fim_nem_mate_nem_bandeira_locais() {
    let mut game = white_vs_human();
    // Mate do louco sem o "mate" do Provedor: a Partida segue até o Provedor dizer.
    provider(&mut game, snap("f2f3 e7e5 g2g4 d8h4", 1_000, 1_000), 10).unwrap();
    assert!(game.snapshot().outcome.is_none());
    // Muito depois do tempo esgotado: nenhuma bandeira local.
    let events = provider(
        &mut game,
        snap("f2f3 e7e5 g2g4 d8h4", 1_000, 1_000),
        600_000,
    )
    .unwrap();
    assert!(events.is_empty());
    assert!(game.snapshot().outcome.is_none());
    let events = provider(
        &mut game,
        ended(
            snap("f2f3 e7e5 g2g4 d8h4", 1_000, 1_000),
            GameResult::BlackWins,
            EndReason::Checkmate,
        ),
        600_001,
    )
    .unwrap();
    assert!(matches!(
        kinds(&events)[..],
        [EventKind::GameEnded {
            outcome: Outcome {
                result: GameResult::BlackWins,
                reason: EndReason::Checkmate
            }
        }]
    ));
}

#[test]
fn comandos_locais_viram_pedidos_ao_provedor_e_nao_mudam_a_partida() {
    let mut game = white_vs_human();
    for command in [
        Command::Release {
            from: sq("e2"),
            to: sq("e4"),
        },
        Command::Resign { side: Color::White },
        Command::OfferDraw { side: Color::White },
        Command::AcceptDraw,
        Command::ClaimDraw,
        Command::Undo,
        Command::Pause,
        Command::EngineMove { uci: "e2e4".into() },
    ] {
        let kind = command.kind();
        assert_eq!(
            game.handle(command, at(5)),
            Err(Rejection::NotSupportedByPolicy(kind))
        );
    }
    assert!(game.events().is_empty());
    // Reabrir não pausa: o relógio é do Provedor.
    assert_eq!(game.reopen_paused(), Ok(Vec::new()));
    // E o Provedor não fala com Partida local.
    let mut local = common::assisted();
    assert_eq!(
        provider(&mut local, snap("e2e4", 0, 0), 1),
        Err(Rejection::NotSupportedByPolicy(CommandKind::Provider))
    );
}

// ---- Intenção enviada ---------------------------------------------------------------------

#[test]
fn intencao_enviada_fica_fora_do_log_ate_a_confirmacao() {
    let mut game = white_vs_human();
    game.send_move(mv("e2e4")).unwrap();
    assert!(game.events().is_empty(), "a intenção nunca vai para o log");
    let snapshot = game.snapshot();
    assert!(snapshot.history.is_empty());
    let sent = snapshot.online.unwrap().sent_move.unwrap();
    assert_eq!(sent.mv, mv("e2e4"));
    assert_eq!(sent.san, "e4");
    assert_eq!(sent.side, Color::White);
    // A interface desenha a peça no destino.
    assert_eq!(
        sent.position_after.piece_at(sq("e4")),
        Some((Color::White, chess_core::Piece::Pawn))
    );

    let events = provider(&mut game, snap("e2e4", 300_000, 300_000), 200).unwrap();
    assert!(matches!(kinds(&events)[0], EventKind::MoveCompleted { .. }));
    assert_eq!(history(&game), ["e2e4"]);
    assert_eq!(game.snapshot().online.unwrap().sent_move, None);
}

#[test]
fn intencao_e_descartada_se_o_provedor_confirma_outro_lance() {
    let mut game = white_vs_human();
    game.send_move(mv("e2e4")).unwrap();
    provider(&mut game, snap("d2d4", 300_000, 300_000), 200).unwrap();
    assert_eq!(history(&game), ["d2d4"]);
    assert_eq!(game.snapshot().online.unwrap().sent_move, None);
}

#[test]
fn intencao_recusada_pelo_provedor_some_sem_rastro() {
    let mut game = white_vs_human();
    game.send_move(mv("e2e4")).unwrap();
    assert_eq!(game.drop_sent_move().map(|s| s.mv), Some(mv("e2e4")));
    assert_eq!(game.drop_sent_move(), None);
    assert!(game.events().is_empty());
    assert_eq!(game.snapshot().online.unwrap().sent_move, None);
    // E o jogador pode tentar de novo.
    game.send_move(mv("d2d4")).unwrap();
}

#[test]
fn intencao_so_uma_so_na_vez_do_jogador_local_e_so_legal() {
    let mut game = white_vs_human();
    assert_eq!(
        game.send_move(mv("e2e5")),
        Err(Rejection::MoveNotLegal(mv("e2e5")))
    );
    game.send_move(mv("e2e4")).unwrap();
    assert_eq!(game.send_move(mv("d2d4")), Err(Rejection::MoveAlreadySent));
    provider(&mut game, snap("e2e4", 0, 0), 1).unwrap();
    assert_eq!(game.send_move(mv("e7e5")), Err(Rejection::NotLocalTurn));
    // Partida local não tem intenção enviada.
    assert_eq!(
        common::assisted().send_move(mv("e2e4")),
        Err(Rejection::NotOnline)
    );
}

#[test]
fn partida_terminada_descarta_a_intencao_e_recusa_outra() {
    let mut game = white_vs_human();
    game.send_move(mv("e2e4")).unwrap();
    provider(
        &mut game,
        ended(
            snap("", 0, 0),
            GameResult::BlackWins,
            EndReason::Resignation,
        ),
        5,
    )
    .unwrap();
    assert_eq!(game.snapshot().online.unwrap().sent_move, None);
    assert_eq!(game.send_move(mv("d2d4")), Err(Rejection::GameOver));
}

// ---- Reconciliação por retrato ------------------------------------------------------------

#[test]
fn retrato_repetido_nao_gera_eventos() {
    let mut game = white_vs_human();
    provider(&mut game, snap("e2e4 e7e5", 299_000, 298_000), 100).unwrap();
    let log = game.events().to_vec();
    for t in [200, 300, 400] {
        assert!(
            provider(&mut game, snap("e2e4 e7e5", 299_000, 298_000), t)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(game.events(), &log[..]);
}

#[test]
fn retrato_atrasado_nunca_regride_a_partida() {
    let mut game = white_vs_human();
    provider(&mut game, snap("e2e4 e7e5 g1f3", 290_000, 295_000), 100).unwrap();
    let log = game.events().to_vec();
    // Fora de ordem: o retrato de dois Lances atrás chega depois.
    assert!(
        provider(&mut game, snap("e2e4", 300_000, 300_000), 200)
            .unwrap()
            .is_empty()
    );
    assert!(
        provider(&mut game, snap("", 300_000, 300_000), 300)
            .unwrap()
            .is_empty()
    );
    assert_eq!(game.events(), &log[..]);
    assert_eq!(history(&game), ["e2e4", "e7e5", "g1f3"]);
}

#[test]
fn retrato_que_pula_lances_aplica_so_o_que_falta() {
    let mut game = white_vs_human();
    provider(&mut game, snap("e2e4", 300_000, 300_000), 100).unwrap();
    let events = provider(
        &mut game,
        snap("e2e4 e7e5 g1f3 b8c6", 290_000, 280_000),
        900,
    )
    .unwrap();
    let moves: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::MoveCompleted { .. }))
        .collect();
    assert_eq!(moves.len(), 3);
    assert_eq!(history(&game), ["e2e4", "e7e5", "g1f3", "b8c6"]);
}

#[test]
fn retrato_divergente_e_erro_tipado_e_o_log_fica_intacto() {
    let mut game = white_vs_human();
    provider(&mut game, snap("e2e4 e7e5", 0, 0), 100).unwrap();
    let log = game.events().to_vec();
    // Prefixo diferente: o Provedor diz que o primeiro Lance foi d4.
    assert_eq!(
        provider(&mut game, snap("d2d4 d7d5 c2c4", 0, 0), 200),
        Err(Rejection::Desynced(Desync::Diverged { ply: 1 }))
    );
    // Retrato menor que não é prefixo também diverge (não é só atraso).
    assert_eq!(
        provider(&mut game, snap("d2d4", 0, 0), 250),
        Err(Rejection::Desynced(Desync::Diverged { ply: 1 }))
    );
    assert_eq!(game.events(), &log[..]);
    assert_eq!(
        game.snapshot().online.unwrap().desync,
        Some(Desync::Diverged { ply: 1 })
    );
    // Dessincronizada: nada de Lance local até um retrato coerente.
    assert_eq!(
        game.send_move(mv("g1f3")),
        Err(Rejection::Desynced(Desync::Diverged { ply: 1 }))
    );
    provider(&mut game, snap("e2e4 e7e5", 0, 0), 300).unwrap();
    assert_eq!(game.snapshot().online.unwrap().desync, None);
    game.send_move(mv("g1f3")).unwrap();
}

#[test]
fn lance_ilegal_do_provedor_dessincroniza_sem_corromper_o_log() {
    let mut game = white_vs_human();
    provider(&mut game, snap("e2e4", 0, 0), 100).unwrap();
    let log = game.events().to_vec();
    for bad in ["e2e4 e7e5 e1e3", "e2e4 e7e5 zz99"] {
        assert_eq!(
            provider(&mut game, snap(bad, 0, 0), 200),
            Err(Rejection::Desynced(Desync::IllegalMove {
                ply: 3,
                uci: bad.rsplit(' ').next().unwrap().into()
            }))
        );
        // Tudo ou nada: nem o e7e5 legal entrou.
        assert_eq!(game.events(), &log[..]);
    }
    assert!(game.snapshot().online.unwrap().desync.is_some());
}

#[test]
fn roque_na_notacao_rei_para_torre_e_aceito() {
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    let setup = GameSetup::online(
        online_game(Color::White, Opponent::Human),
        Variant::Standard,
        Some(fen),
    )
    .unwrap();
    let mut game = Game::new(setup);
    provider(&mut game, snap("e1h1 e8a8", 0, 0), 1).unwrap();
    let snapshot = game.snapshot();
    assert_eq!(snapshot.history[0].mv, mv("e1g1"));
    assert_eq!(snapshot.history[0].san, "O-O");
    assert_eq!(snapshot.history[1].mv, mv("e8c8"));
    // A forma padrão também.
    let mut game = Game::new(
        GameSetup::online(
            online_game(Color::White, Opponent::Human),
            Variant::Standard,
            Some(fen),
        )
        .unwrap(),
    );
    provider(&mut game, snap("e1g1", 0, 0), 1).unwrap();
    assert_eq!(game.snapshot().history[0].san, "O-O");
}

// ---- Empate, fim e trava de fair play -------------------------------------------------------

#[test]
fn proposta_de_empate_recebida_e_retirada() {
    let mut game = white_vs_human();
    let mut offered = snap("e2e4 e7e5", 0, 0);
    offered.draw_offer = Some(Color::Black);
    let events = provider(&mut game, offered.clone(), 10).unwrap();
    assert!(
        kinds(&events).contains(&&EventKind::DrawOffered { by: Color::Black }),
        "{events:?}"
    );
    assert_eq!(game.snapshot().pending_draw_offer, Some(Color::Black));
    assert!(provider(&mut game, offered, 11).unwrap().is_empty());
    let events = provider(&mut game, snap("e2e4 e7e5", 0, 0), 12).unwrap();
    assert_eq!(
        kinds(&events),
        [&EventKind::DrawOfferWithdrawn { by: Color::Black }]
    );
    assert_eq!(game.snapshot().pending_draw_offer, None);
}

#[test]
fn abortada_termina_sem_resultado() {
    let mut game = white_vs_human();
    let mut aborted = snap("e2e4", 0, 0);
    aborted.end = Some(ProviderEnd::Aborted);
    let events = provider(&mut game, aborted, 10).unwrap();
    assert_eq!(kinds(&events).last(), Some(&&EventKind::GameAborted));
    let snapshot = game.snapshot();
    assert_eq!(snapshot.outcome, None);
    assert!(snapshot.online.as_ref().unwrap().aborted);
    assert!(snapshot.is_over());
    assert!(!game.online_human_in_progress());
    // Depois do fim, retratos (atrasados ou repetidos) não mexem em nada.
    assert!(
        provider(&mut game, snap("e2e4 e7e5", 0, 0), 20)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn trava_fica_ligada_do_inicio_ate_a_confirmacao_do_fim() {
    let mut game = white_vs_human();
    assert!(
        game.online_human_in_progress(),
        "liga antes do primeiro Lance"
    );
    provider(&mut game, snap("e2e4 e7e5", 0, 0), 10).unwrap();
    assert!(game.online_human_in_progress());
    // Pedir abandono não é fim: só o retrato com o resultado solta a trava.
    assert!(
        game.handle(Command::Resign { side: Color::White }, at(11))
            .is_err()
    );
    assert!(game.online_human_in_progress());
    provider(
        &mut game,
        ended(
            snap("e2e4 e7e5", 0, 0),
            GameResult::BlackWins,
            EndReason::Resignation,
        ),
        12,
    )
    .unwrap();
    assert!(!game.online_human_in_progress());
}

#[test]
fn trava_nao_liga_contra_a_engine_do_provedor_nem_em_partida_local() {
    let game = Game::new(setup_for(
        Color::White,
        Opponent::ProviderEngine { level: 3 },
    ));
    assert!(!game.online_human_in_progress());
    assert!(!common::assisted().online_human_in_progress());
}

#[test]
fn trava_continua_na_correspondencia_e_dessincronizada() {
    let mut info = online_game(Color::Black, Opponent::Human);
    info.days_per_move = Some(3);
    let mut game = Game::new(GameSetup::online(info, Variant::Standard, None).unwrap());
    let correspondence = ProviderSnapshot {
        moves: vec!["e2e4".into()],
        clock: None,
        draw_offer: None,
        end: None,
    };
    let events = provider(&mut game, correspondence, 10).unwrap();
    assert_eq!(events.len(), 1, "sem relógio de xadrez na correspondência");
    assert!(game.online_human_in_progress());
    let _ = provider(&mut game, snap("d2d4", 0, 0), 11);
    assert!(game.snapshot().online.unwrap().desync.is_some());
    assert!(game.online_human_in_progress());
}

// ---- O log reabre igual ---------------------------------------------------------------------

#[test]
fn partida_online_reabre_do_log_exatamente_como_estava() {
    let mut game = white_vs_human();
    let mut offered = snap("e2e4 e7e5 g1f3", 296_000, 297_000);
    offered.draw_offer = Some(Color::White);
    provider(&mut game, snap("e2e4", 300_000, 300_000), 1_000).unwrap();
    provider(&mut game, offered, 5_000).unwrap();

    // Em andamento (para a correspondência): setup e Eventos em JSON, e de volta.
    let setup_json = serde_json::to_string(game.setup()).unwrap();
    let events_json = serde_json::to_string(game.events()).unwrap();
    let setup: GameSetup = serde_json::from_str(&setup_json).unwrap();
    let events: Vec<Event> = serde_json::from_str(&events_json).unwrap();
    let reopened = Game::try_replay(setup.clone(), &events).unwrap();
    assert_eq!(reopened.snapshot(), game.snapshot());
    assert_eq!(reopened.setup(), game.setup());
    assert!(reopened.online_human_in_progress());

    // E terminada.
    provider(
        &mut game,
        ended(
            snap("e2e4 e7e5 g1f3", 290_000, 0),
            GameResult::WhiteWins,
            EndReason::Timeout,
        ),
        9_000,
    )
    .unwrap();
    let reopened = Game::try_replay(setup, game.events()).unwrap();
    assert_eq!(reopened.snapshot(), game.snapshot());
    assert!(!reopened.online_human_in_progress());
}

#[test]
fn eventos_novos_tem_tipo_estavel_na_versao_atual_do_payload() {
    let samples = [
        (
            EventKind::ClockReported {
                white_ms: 1,
                black_ms: 2,
                running: Some(Color::Black),
            },
            "clock_reported",
        ),
        (
            EventKind::DrawOfferWithdrawn { by: Color::White },
            "draw_offer_withdrawn",
        ),
        (EventKind::GameAborted, "game_aborted"),
    ];
    for (kind, name) in samples {
        let event = Event::new(Timestamp::with_wall(9, 1_794_398_400_000), kind);
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["kind"]["type"], name);
        assert_eq!(value["version"], chess_game::PAYLOAD_VERSION);
        let back: Event = serde_json::from_value(value).unwrap();
        assert_eq!(back, event);
    }
    for reason in [EndReason::Disconnection, EndReason::EndedByProvider] {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(serde_json::from_str::<EndReason>(&json).unwrap(), reason);
        assert!(!reason.is_positional());
    }
}

#[test]
fn setup_online_faz_ida_e_volta_e_setup_adulterado_e_recusado() {
    let setup = setup_for(Color::Black, Opponent::ProviderEngine { level: 8 });
    let json = serde_json::to_string(&setup).unwrap();
    assert!(json.contains("\"policy\":\"online\""), "{json}");
    assert_eq!(serde_json::from_str::<GameSetup>(&json).unwrap(), setup);
    // Dados do Provedor numa Partida local: recusado (ninguém escreve isso).
    let tampered = json.replace("\"policy\":\"online\"", "\"policy\":\"assisted\"");
    assert!(serde_json::from_str::<GameSetup>(&tampered).is_err());
}

#[test]
fn pgn_da_partida_online_diz_o_provedor_e_abortada_sai_sem_resultado() {
    let mut game = white_vs_human();
    let mut aborted = snap("e2e4", 0, 0);
    aborted.end = Some(ProviderEnd::Aborted);
    provider(&mut game, aborted, 10).unwrap();
    let pgn = game.to_pgn(&chess_game::PgnOptions::default());
    assert!(pgn.contains("Online (lichess)"), "{pgn}");
    assert!(pgn.contains("[Result \"*\"]"), "{pgn}");
    assert!(pgn.contains("1. e4"), "{pgn}");
}
