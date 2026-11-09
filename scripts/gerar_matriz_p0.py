#!/usr/bin/env python3
"""Gera docs/homologacao/matriz-p0.md (B18, issue #42) e valida as referências.

Cada teste citado precisa existir como `fn <nome>` no arquivo indicado (caminho relativo a
`crates/`); cada evidência citada precisa existir no disco. Se algo não existir, o script
falha e não escreve a matriz: a matriz nunca cita teste ou arquivo inventado.

Uso (na raiz do repositório):
    python3 scripts/gerar_matriz_p0.py            # valida e reescreve a matriz
    python3 scripts/gerar_matriz_p0.py --check    # só valida (sem escrever)
"""
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
SAIDA = RAIZ / "docs" / "homologacao" / "matriz-p0.md"

# Atalhos de caminho (relativos a crates/).
CORE = "chess-core/tests/"
GAME = "chess-game/tests/"
INP = "chess-input/"
STO = "chess-store/tests/"
STU = "chess-studio/src/"
TRN = "chess-training/tests/"
UCI = "chess-uci/tests/"


def t(arquivo, *nomes):
    return [(arquivo, n) for n in nomes]


# (id, requisito, onde está implementado, testes, evidência, status, nota)
LINHAS = [
    # ---- 14.1 Regras e estado ----
    ("CORE-01", "Partida completa de dois lados executa todos os movimentos e encerra em mate/empate com resultado correto.",
     "chess-core (position, status); chess-game (game, outcome)",
     t(GAME + "full_games.rs", "mate_do_pastor_encerra_com_vitoria_das_brancas", "mate_do_louco_encerra_com_vitoria_das_pretas",
       "partida_real_da_opera_ate_o_mate_com_roque_longo_e_xeques", "afogamento_empata_e_nao_e_mate", "abandono_encerra_com_vitoria_do_adversario")
     + t(STU + "session.rs", "scholars_mate_is_playable_by_gestures"),
     "PRs #63, #65, #69, #76; `docs/evidencias/b3/mate-do-pastor-2d-tecnico.png`", "PASS", ""),
    ("CORE-02", "Perft da posição inicial: 20, 400, 8.902 e 197.281 (profundidades 1 a 4).",
     "chess-core (position, perft)",
     t(CORE + "perft.rs", "perft_start_position_depth_1_to_4", "perft_start_position_via_start_constructor", "perft_kiwipete",
       "perft_position_3", "perft_position_4", "perft_position_5", "perft_deep"),
     "PR #63; d5 e d6 e posições profundas em `perft_deep` (`#[ignore]`, executado: ok)", "PASS", ""),
    ("CORE-03", "Roques ilegais (xeque, casas atacadas, peça movida) são recusados; direitos persistem corretamente.",
     "chess-core (position)",
     t(CORE + "moves.rs", "castling_refused_when_king_in_check", "castling_refused_when_king_passes_through_attacked_square",
       "castling_refused_when_king_lands_on_attacked_square", "castling_refused_after_king_or_rook_moved",
       "castling_refused_when_pieces_are_between", "capturing_a_rook_removes_the_castling_right")
     + t(GAME + "assisted_illegal.rs", "roque_atravessando_casa_atacada_e_ilegal")
     + t(STO + "store.rs", "custom_fen_start_survives_reopen"),
     "PR #63; Exercícios de roque (B14, PR #89)", "PASS", ""),
    ("CORE-04", "En passant legal e ilegal coberto por testes e persistência.",
     "chess-core (position, repetition)",
     t(CORE + "moves.rs", "en_passant_is_legal_immediately_after_double_push", "en_passant_expires_after_one_move",
       "en_passant_white_captures_too", "en_passant_refused_if_it_exposes_own_king")
     + t(CORE + "fen.rs", "valid_fens_round_trip_identically")
     + t(GAME + "replay_property.rs", "replay_reproduz_o_estado_de_partidas_aleatorias_da_posicao_inicial")
     + t(STO + "store.rs", "save_close_reopen_restores_identical_snapshot"),
     "PR #63, PR #73", "PASS",
     "A persistência do en passant é coberta pelo replay do log (partidas aleatórias) e pelo FEN ida e volta; não há teste de store dedicado só a en passant."),
    ("CORE-05", "Promoção permite D/T/B/C, inclui promoção com captura e escolha pendente.",
     "chess-core; chess-game (promoção pendente); chess-input; chess-studio (seletor)",
     t(CORE + "moves.rs", "promotion_offers_all_four_pieces", "promotion_with_capture_for_each_piece", "black_promotion_works")
     + t(GAME + "full_games.rs", "promocao_pendente_so_conclui_com_a_peca_escolhida", "promocao_com_captura_e_subpromocao_a_cavalo",
         "retratar_cancela_a_promocao_pendente")
     + t(STU + "session.rs", "promotion_with_capture_asks_for_the_piece_and_esc_cancels", "always_queen_promotes_without_the_selector"),
     "PR #76; `docs/evidencias/b3/`; Exercícios de promoção (PR #89)", "PASS", ""),
    ("CORE-06", "Afogamento é empate: lado da vez sem lances legais e fora de xeque; não é mate.",
     "chess-core (status)",
     t(CORE + "status.rs", "stalemate_is_never_checkmate", "fools_mate_is_checkmate_won_by_black")
     + t(GAME + "full_games.rs", "afogamento_empata_e_nao_e_mate")
     + t(TRN + "runs.rs", "stalemate_exercise_ends_in_a_draw_and_review_explains_it_is_not_checkmate"),
     "PRs #65, #87", "PASS", ""),
    ("CORE-07", "Repetição, 50/75 movimentos e posição morta aplicam reclamação/encerramento de modo regulamentar.",
     "chess-core (repetition, status); chess-game (policy, draws, fide)",
     t(CORE + "repetition.rs", "same_position_reached_again_has_equal_key_regardless_of_clocks", "castling_rights_are_part_of_the_key",
       "capturable_en_passant_is_part_of_the_key")
     + t(CORE + "status.rs", "fifty_move_counter_thresholds", "seventy_five_move_counter_thresholds", "insufficient_material_fide_cases",
       "dead_position_by_pawn_blockade_is_detected", "dead_position_makes_checkmate_impossible_for_both_sides",
       "blockade_with_possible_capture_is_not_dead", "king_that_reaches_an_enemy_pawn_is_not_dead",
       "pieces_other_than_kings_and_pawns_are_never_declared_dead_by_blockade", "king_in_check_by_pawn_or_en_passant_is_not_dead",
       "wall_is_independently_verified_by_exhaustive_search")
     + t(GAME + "draws.rs", "tripla_repeticao_empata_automaticamente_no_assistido", "cinquenta_lances_empatam_automaticamente_no_assistido",
         "setenta_e_cinco_lances_encerram_automaticamente", "material_insuficiente_empata_ao_ficar_rei_contra_rei",
         "posicao_morta_por_bloqueio_de_pecoes_empata_automaticamente")
     + t(GAME + "fide_claims.rs", "tripla_repeticao_nao_encerra_sozinha_exige_reclamacao_dec_38", "cinco_repeticoes_encerram_automaticamente_art_9_6_1",
         "material_insuficiente_encerra_automaticamente_art_5_2_2", "posicao_morta_por_bloqueio_encerra_automaticamente_art_5_2_2", "reclamacao_incorreta_da_dois_minutos_ao_adversario_e_a_partida_continua_art_9_5_3"),
     "PRs #65, #69, #74 e #93 (detector de posição morta). `CanonicalPosition::is_dead_position` e a prova do muro por busca exaustiva (`wall_is_independently_verified_by_exhaustive_search`)",
     "PASS",
     "Repetição, 50/75, material insuficiente e posição morta por bloqueio total de peões (art. 5.2.2) cobertos; a bandeira e as Infrações usam o detector via `can_checkmate` (arts. 6.9 e 7.5.5). O detector é conservador: não cobre bispos presos atrás de cadeias nem fortalezas (falso negativo permitido, falso positivo nunca)."),
    ("CORE-08", "Engine UCI não pode alterar a posição canônica com lance ilegal.",
     "chess-uci (engine, player, controller); chess-game (EngineMove)",
     t(UCI + "adapter.rs", "illegal_move_is_a_recoverable_error_and_leaves_the_position_unchanged", "malformed_move_text_is_a_recoverable_error")
     + t(UCI + "player.rs", "illegal_move_is_recoverable_and_never_changes_the_position")
     + t(GAME + "clock.rs", "lance_ilegal_da_engine_e_logado_e_nao_muda_a_partida"),
     "PRs #64, #74, #79", "PASS", ""),
    ("CORE-09", "Estado de turno, contadores e direitos sobrevive à reconstrução a partir da persistência.",
     "chess-store (store); chess-game (replay)",
     t(STO + "store.rs", "save_close_reopen_restores_identical_snapshot", "custom_fen_start_survives_reopen")
     + t(GAME + "replay_property.rs", "replay_reproduz_o_estado_de_partidas_aleatorias_da_posicao_inicial",
         "replay_reproduz_o_estado_da_simulacao_fide_com_e_sem_relogio")
     + t(STU + "persistence.rs", "closing_and_reopening_resumes_the_game_paused_at_the_same_move"),
     "PRs #73, #84", "PASS", ""),
    ("CORE-10", "Desfazer/refazer permitido em estudo e bloqueado para alterar resultado de torneio.",
     "chess-game (undo, policy)",
     t(GAME + "undo_redo.rs", "desfazer_volta_a_posicao_registra_evento_e_marca_a_partida", "refazer_reaplica_o_lance_desfeito")
     + t(GAME + "fide_touch.rs", "desfazer_e_refazer_nao_existem_nas_politicas_competitivas")
     + t(GAME + "elimination.rs", "nao_ha_desfazer_nem_refazer")
     + t(GAME + "pgn_export.rs", "revisao_nas_politicas_competitivas_nao_e_desfazer"),
     "PRs #69, #74, #75, #77", "PASS", ""),

    # ---- 14.2 Visualizações, câmera e interação ----
    ("VIS-01", "3D profissional com materiais, peças e iluminação coerentes com a referência de qualidade.",
     "chess-studio (board, pieces, presentation); assets CC0",
     [], "`docs/evidencias/b19/3d-profissional-*.png` (peças de buxo e ébano); PR #78/#88", "UNVERIFIED",
     "Juízo visual: depende da aprovação do usuário (checklist de homologação). Sem IBL nem desfoque."),
    ("VIS-02", "Mesa imersiva discreta sem sala 3D completa, com desempenho medido.",
     "chess-studio (board, presentation)",
     [], "`docs/evidencias/b19/mesa-imersiva-*.png`; `docs/evidencias/b19/desempenho.txt` (indicativo)", "UNVERIFIED",
     "Aprovação visual do usuário e medição formal de desempenho (B16, #40); a linha de base do A6 está contaminada."),
    ("VIS-03", "2D técnico com tabuleiro correto e coordenadas legíveis.",
     "chess-studio (board2d)",
     t(STU + "board2d.rs", "corners_map_to_a1_and_h8", "window_center_is_between_d_and_e_files")
     + t(STU + "pieces.rs", "square_pos_of_matches_the_2d_board_layout"),
     "`docs/evidencias/b19/2d-tecnico.png`; PR #78", "PASS",
     "Orientação provada por teste; a legibilidade das coordenadas é conferida na revisão visual do usuário."),
    ("VIS-05", "Alternar 3D/mesa/2D preserva posição, turno, direitos, histórico, relógios e resultado.",
     "chess-studio (presentation, pieces, session)",
     t(STU + "pieces.rs", "switching_presentation_keeps_the_game_and_the_scene_position", "each_presentation_remembers_its_own_camera")
     + t(STU + "session.rs", "same_policy_through_the_same_commands_regardless_of_presentation"),
     "PRs #78, #86, #88", "PASS", "O teste compara o `Snapshot` inteiro antes e depois da troca (inclui relógio e resultado)."),
    ("CAM-01", "Arrastar o botão central orbita a câmera 360° e não move peças.",
     "chess-studio (camera); chess-input (view, gesture)",
     t(INP + "tests/gestures.rs", "middle_and_secondary_buttons_never_move_pieces")
     + t(INP + "src/view.rs", "orbit_wraps_yaw_and_clamps_pitch"),
     "PR #62 (A4); homologação do usuário 09/11", "PASS",
     "Homologação do usuário em 09/11 (checklist do artefato, janela real): órbita com o botão do meio confirmada, sem mover peças. O intérprete ignora o botão do meio (testado) e a matemática da órbita é testada."),
    ("CAM-02", "Pan, zoom, reset e Vistas superior/oblíqua/invertida funcionam conforme o mapeamento de controles.",
     "chess-studio (camera); chess-input (view)",
     t(INP + "src/view.rs", "zoom_is_limited", "pan_moves_with_screen_axes_and_never_loses_the_board", "standard_view_looks_from_white_side_and_inverted_from_black_side",
       "fit_distance_keeps_the_target_and_fits_the_points")
     + t(STU + "pieces.rs", "each_presentation_remembers_its_own_camera")
     + t(STU + "framing.rs", "board_fits_in_every_view_with_and_without_dock", "every_view_targets_the_board_center_and_the_center_is_on_screen_center",
         "the_clock_does_not_enter_the_framing"),
     "PRs #62, #78, #137; homologação do usuário 09/11 (FAIL, corrigido)", "UNVERIFIED",
     "Homologação do usuário em 09/11: FAIL, o relógio entrava no enquadramento e descentralizava as Vistas e o reset. Corrigido no PR #137 (issue #135): as Vistas e o reset centralizam no tabuleiro e o relógio fica fora do enquadramento. Aguarda reteste do usuário. Pinça e dois dedos só em macOS físico."),
    ("CAM-03", "Rotação/inversão não modifica a interpretação lógica de a1...h8.",
     "chess-input (view, board)",
     t(INP + "src/view.rs", "inverting_the_view_does_not_change_logical_interpretation", "every_square_is_pickable_after_any_rotation")
     + t(INP + "src/board.rs", "a1_and_h8_are_opposite_corners"),
     "PR #62; homologação do usuário 09/11", "PASS", "Homologação do usuário em 09/11: as 64 casas respondem em qualquer Vista."),
    ("CAM-04", "As 64 casas são selecionáveis nas Vistas superior, oblíqua, padrão e invertida.",
     "chess-input (board, view)",
     t(INP + "src/view.rs", "every_square_is_pickable_in_every_view", "every_square_is_pickable_after_any_rotation")
     + t(INP + "src/board.rs", "vertical_ray_over_each_center_returns_that_square", "all_squares_cover_a1_to_h8"),
     "PR #62; homologação do usuário 09/11", "PASS",
     "Prova por raio em geometria pura (4 pontos por casa); homologação do usuário em 09/11: clique real nas 64 casas em qualquer Vista."),
    ("INT-01", "Dois toques e drag-and-drop chegam à mesma jogada canônica quando válida.",
     "chess-input (gesture); chess-studio (session, play)",
     t(INP + "tests/gestures.rs", "two_taps_and_drag_emit_the_same_release_and_reach_the_same_position", "capture_by_two_taps_and_by_drag_agree")
     + t(STU + "session.rs", "scholars_mate_is_playable_by_gestures")
     + t(INP + "src/board.rs", "clicking_the_upper_body_of_a_tall_piece_picks_its_square_not_the_floor_behind",
         "the_nearest_piece_wins_when_two_overlap_on_the_ray")
     + t(STU + "play.rs", "a_3px_wobble_across_a_square_edge_is_still_a_tap_that_selects", "a_20px_move_is_a_normal_drag"),
     "PR #76; `docs/evidencias/b3/`; PRs #138 (correção); homologação do usuário 09/11 (FAIL, corrigido)", "UNVERIFIED",
     "Homologação do usuário em 09/11: FAIL em janela real, clicar na parte alta de uma peça não selecionava a casa dela e um tremor do mouse virava arrasto. Corrigido no PR #138 (issue #136): picking considera o corpo das peças e o arrasto só começa após 6 px. Aguarda reteste do usuário; antes era PASS só por teste do intérprete."),
    ("INT-02", "Destino ilegal não vira jogada válida nem é convertido silenciosamente em outra.",
     "chess-input (gesture); chess-game (rejeição)",
     t(INP + "tests/gestures.rs", "illegal_destination_is_forwarded_verbatim_never_converted")
     + t(STU + "session.rs", "illegal_drop_returns_to_origin_and_leaves_the_game_unchanged")
     + t(GAME + "assisted_illegal.rs", "e2_e5_e_recusado_sem_virar_e4"),
     "PR #76; homologação do usuário 09/11", "PASS", "Homologação do usuário em 09/11: e2→e5 recusado em janela real, a peça volta à origem."),
    ("INT-03", "Esc, perda de foco, painel ou mudança para órbita não geram lances fantasmas.",
     "chess-input (gesture); chess-studio (play: tradução de eventos do Bevy)",
     t(INP + "tests/gestures.rs", "cancel_events_abort_drag_without_any_command", "cancel_events_abort_selection_and_a_late_click_does_not_move",
       "cancel_while_held_and_after_drag_start_leaves_nothing_behind", "escape_or_focus_loss_while_dragging_keeps_the_touch_obligation"),
     "PR #76; homologação do usuário 09/11", "PASS",
     "Homologação do usuário em 09/11 (checklist do artefato, janela real): cancelar o arrasto não move peça. Camada do intérprete com PASS automatizado; a tradução dos eventos reais do Bevy/egui (`WindowFocused`, painel, órbita) segue sem teste automatizado, coberta pela verificação manual."),
    ("INT-04", "Preview de peça durante o arrasto não altera o estado canônico.",
     "chess-input (gesture); chess-studio (session)",
     t(INP + "tests/gestures.rs", "dragging_emits_nothing_until_release_and_exposes_visual_state")
     + t(STU + "session.rs", "dragging_does_not_touch_the_game_until_release"),
     "PR #76; ADR-0001", "PASS", ""),
    ("INT-05", "Animação de soltura/encaixe respeita o destino sem deslocar a peça para a casa errada.",
     "chess-input (settle); chess-studio (pieces)",
     t(INP + "src/settle.rs", "ends_exactly_on_the_canonical_square_from_any_start", "approaches_the_target_monotonically_and_stays_above_the_board")
     + t(STU + "session.rs", "legal_drop_hint_targets_the_canonical_destination")
     + t(STU + "pieces.rs", "nearest_source_picks_the_closest_vanished_piece_of_the_same_kind"),
     "PR #76", "PASS", "Trajetória provada como função pura para as 64 casas; a suavidade visual ao vivo entra no roteiro manual (UX-10)."),

    # ---- 14.3 Arbitragem ----
    ("AC-01", "Seleção de Assistido, FIDE e Eliminação Imediata antes da partida.",
     "chess-studio (new_game)",
     t(STU + "new_game.rs", "each_policy_is_selectable_and_reaches_the_setup", "elimination_stage_is_recorded_in_the_setup", "free_time_control_reaches_the_setup"),
     "`docs/evidencias/b7/nova-partida.png`; PRs #86, #88", "PASS", ""),
    ("AC-02", "A Política escolhida funciona identicamente em 2D e 3D.",
     "chess-studio (session); arquitetura ADR-0001",
     t(STU + "session.rs", "same_policy_through_the_same_commands_regardless_of_presentation")
     + t(INP + "tests/gestures.rs", "sudden_elimination_uses_the_same_competitive_gestures"),
     "`docs/evidencias/b7/relogio-2d-tecnico.png`, `relogio-3d-profissional.png`, `relogio-mesa-imersiva.png`; PR #86", "PASS",
     "Todas as Apresentações enviam os mesmos Comandos à mesma `Session`."),
    ("AC-03", "Assistido rejeita lance ilegal sem mudar a posição e explica somente se configurado.",
     "chess-game (policy Assisted, rejection); chess-studio (aids)",
     t(GAME + "assisted_illegal.rs", "e2_e5_e_recusado_sem_virar_e4", "motivos_de_recusa_sao_explicaveis", "em_xeque_um_lance_que_nao_resolve_e_recusado_e_informa_o_xeque")
     + t(STU + "aids.rs", "notice_follows_the_explanation_policy")
     + t(STU + "session.rs", "illegal_drop_returns_to_origin_and_leaves_the_game_unchanged"),
     "PRs #69, #84", "PASS", ""),
    ("AC-04", "Competição separa movimentar, soltar e concluir pelo relógio, com exceções regulamentares.",
     "chess-game (fide, touch, policy)",
     t(GAME + "fide_touch.rs", "ac_04_mover_soltar_e_concluir_sao_etapas_separadas", "lance_feito_nao_pode_ser_retraido_nem_trocado_art_4_7",
       "mate_encerra_sem_acionar_o_relogio_art_6_2_1_1"),
     "PR #74 (tabela artigo a artigo)", "PASS", ""),
    ("AC-05", "FIDE clássico: primeira infração restaura a posição e concede +2 min ao oponente; a segunda perde ou empata na exceção de impossibilidade de mate.",
     "chess-game (fide)",
     t(GAME + "fide_infractions.rs", "bdd_03_primeira_infracao_restaura_a_posicao_e_da_dois_minutos_ao_adversario", "bdd_04_segunda_infracao_do_mesmo_jogador_perde_a_partida",
       "bdd_04_excecao_adversario_sem_possibilidade_de_mate_e_empate", "bdd_04_sem_a_excecao_quando_o_adversario_tem_material_de_mate"),
     "PR #74; `docs/evidencias/b7/peca-ilegal-provisoria.png`", "PASS",
     "Desvio documentado (PR #74): na promoção sem troca (art. 7.5.2) o peão vira dama em vez de restaurar a posição."),
    ("AC-06", "Eliminação Imediata encerra na primeira infração concluída, rotulada como regra personalizada.",
     "chess-game (policy SuddenElimination)",
     t(GAME + "elimination.rs", "ac_06_etapa_relogio_primeira_infracao_encerra_com_derrota_de_quem_cometeu", "ac_06_etapa_soltura_encerra_ao_soltar_a_peca",
       "snapshot_rotula_a_regra_como_personalizada_e_nunca_como_fide", "excecao_de_material_adversario_sem_como_dar_mate_resulta_em_empate_nas_duas_etapas")
     + t(GAME + "pgn_export.rs", "eliminacao_imediata_nunca_se_apresenta_como_fide"),
     "PR #75; PR #86", "PASS", ""),
    ("AC-07", "SQLite guarda modo, infrações, penalidades, resultado, motivos e histórico.",
     "chess-store (store, migrations)",
     t(STO + "store.rs", "save_close_reopen_restores_identical_snapshot", "game_end_is_recorded_and_unfinished_lookup_skips_it")
     + t(GAME + "event_serde.rs", "partida_fide_com_infracao_pausa_e_bandeira_faz_ida_e_volta")
     + t(STU + "persistence.rs", "a_new_game_screen_fide_game_with_engine_is_saved_and_resumes_paused"),
     "PRs #73, #86", "PASS", "Resultado e motivo em `games.outcome_json`; infrações e penalidades no log de eventos."),
    ("AC-08", "Tentativas ilegais não entram no PGN como lances legais, mas persistem no log de incidentes.",
     "chess-game (pgn, event); chess-store",
     t(GAME + "pgn_export.rs", "infracao_vira_comentario_no_ponto_certo_e_nao_vira_lance", "sem_comentarios_omite_infracoes")
     + t(GAME + "assisted_illegal.rs", "o_evento_de_lance_ilegal_fica_no_log_e_distingue_do_legal")
     + t(STU + "session.rs", "illegal_drop_is_recorded_with_its_reason_outside_the_legal_history")
     + t(STO + "store.rs", "pgn_after_reopen_is_identical_and_has_only_legal_moves"),
     "PRs #73, #77", "PASS", ""),
    ("AC-09", "Testes cobrem xeque, roque ilegal, movimentos impossíveis, promoção, relógio e penalidades.",
     "chess-core; chess-game",
     t(GAME + "assisted_illegal.rs", "em_xeque_um_lance_que_nao_resolve_e_recusado_e_informa_o_xeque", "roque_atravessando_casa_atacada_e_ilegal")
     + t(CORE + "moves.rs", "castling_refused_when_king_in_check", "promotion_piece_on_non_promoting_move_is_illegal")
     + t(GAME + "clock.rs", "comando_depois_do_tempo_registra_a_bandeira_e_encerra_com_derrota")
     + t(GAME + "fide_infractions.rs", "infracoes_sao_contadas_por_jogador", "promocao_sem_escolha_e_lance_ilegal_e_o_peao_vira_dama_art_7_5_2"),
     "PRs #63, #69, #74", "PASS", ""),
    ("RULE-01", "Política e critérios rastreáveis, com versão de regulamento/explicação no produto.",
     "chess-game (snapshot.rules_version, FIDE_LAWS_VERSION); chess-studio (fide_panel)",
     t(GAME + "clock.rs", "versao_das_leis_e_exposta_so_na_simulacao_fide")
     + t(GAME + "elimination.rs", "snapshot_rotula_a_regra_como_personalizada_e_nunca_como_fide")
     + t(STU + "fide_panel.rs", "the_state_line_follows_touch_release_and_clock"),
     "PRs #74, #75, #86 (versão: \"FIDE Laws of Chess, 1 Jan 2023\")", "PASS", ""),
    ("RULE-02", "Clique acidental não vira automaticamente peça tocada intencionalmente.",
     "chess-game (touch); chess-input (gesture)",
     t(GAME + "fide_touch.rs", "rule_02_selecao_nao_e_toque_so_touch_cria_obrigacao")
     + t(INP + "tests/gestures.rs", "empty_squares_other_buttons_and_hover_never_touch", "alt_click_adjusts_without_creating_an_obligation"),
     "PRs #74, #86", "PASS", "DEC-31: toque = botão esquerdo sobre peça; Alt+clique = Ajuste."),
    ("RULE-03", "A Obrigação da peça tocada persiste apesar do cancelamento de gesto de UI.",
     "chess-game (touch)",
     t(GAME + "fide_touch.rs", "bdd_08_obrigacao_persiste_apos_cancelar_o_gesto", "retratar_devolve_a_peca_e_mantem_a_obrigacao_rule_03")
     + t(INP + "tests/gestures.rs", "escape_or_focus_loss_while_dragging_keeps_the_touch_obligation")
     + t(STU + "session.rs", "touch_obligation_rejects_the_drop_of_another_piece_and_the_piece_returns"),
     "PRs #74, #86", "PASS", ""),
    ("RULE-04", "Pressionar o relógio sem jogar e situações equivalentes seguem as regras regulamentares.",
     "chess-game (fide)",
     t(GAME + "fide_infractions.rs", "rule_04_acionar_o_relogio_sem_jogar_e_lance_ilegal_art_7_5_3", "acionar_o_relogio_depois_de_so_tocar_mantem_a_obrigacao")
     + t(GAME + "elimination.rs", "acionar_o_relogio_sem_lance_tambem_elimina_na_etapa_relogio"),
     "PR #74", "PASS", ""),
    ("RULE-05", "Regras rápido/blitz só são anunciadas como homologadas após testes dos apêndices.",
     "chess-game (non_classical_notice); chess-studio (new_game, fide_panel)",
     t(GAME + "clock.rs", "aviso_de_controle_nao_classico_abaixo_de_60_minutos")
     + t(STU + "new_game.rs", "rule_05_notice_only_below_sixty_minutes_in_fide_simulation"),
     "PRs #74, #86 (DEC-37)", "PASS",
     "O P0 não declara rápido/blitz homologados: abaixo de 60 min aparece o aviso. Apêndices de rápido/blitz ficam para a V1."),

    # ---- 14.4 UX ----
    ("UX-01", "Desligar todos os auxílios remove pontos, fantasmas, setas e destinos legais.",
     "chess-studio (aids, aids_panel, highlight)",
     t(STU + "aids.rs", "physical_profile_removes_every_mark", "each_aid_works_independently", "bdd_01_physical_game_shows_no_destinations_and_e2_e5_is_illegal"),
     "`docs/evidencias/b4/jogo-fisico-3d-profissional.png`, `jogo-fisico-2d-tecnico.png`; PR #84", "PASS", ""),
    ("UX-02", "Dois toques e arrastar/soltar produzem a mesma jogada válida.",
     "chess-input (gesture)",
     t(INP + "tests/gestures.rs", "two_taps_and_drag_emit_the_same_release_and_reach_the_same_position", "capture_by_two_taps_and_by_drag_agree")
     + t(INP + "src/board.rs", "clicking_the_upper_body_of_a_tall_piece_picks_its_square_not_the_floor_behind")
     + t(STU + "play.rs", "a_3px_wobble_across_a_square_edge_is_still_a_tap_that_selects", "a_20px_move_is_a_normal_drag"),
     "PR #76; PR #138 (correção); homologação do usuário 09/11 (FAIL, corrigido)", "UNVERIFIED",
     "Igual a INT-01: FAIL na homologação de 09/11, corrigido no PR #138 (picking pelo corpo das peças e limiar de arrasto de 6 px); aguarda reteste do usuário."),
    ("UX-03", "Configurações visuais não alteram legalidade nem penalidades.",
     "chess-studio (aids, presentation); chess-input",
     t(STU + "aids.rs", "aids_never_change_the_events_of_a_command_sequence", "competitive_policies_never_show_destinations_or_ghosts")
     + t(STU + "presentation.rs", "all_window_cameras_share_one_msaa_setting"),
     "PRs #76, #78, #84; picking é geometria pura, independente de MSAA/sombras", "PASS", ""),
    ("UX-04", "e2–e5 inicial é identificado como ilegal e não entra no histórico legal.",
     "chess-game (assisted); chess-studio (session)",
     t(GAME + "assisted_illegal.rs", "e2_e5_e_recusado_sem_virar_e4", "o_evento_de_lance_ilegal_fica_no_log_e_distingue_do_legal")
     + t(STU + "aids.rs", "bdd_01_physical_game_shows_no_destinations_and_e2_e5_is_illegal"),
     "PRs #69, #84; homologação do usuário 09/11", "PASS", "Homologação do usuário em 09/11: e2→e5 recusado em janela real."),
    ("UX-05", "FIDE distingue seleção, toque intencional, soltura e relógio.",
     "chess-game (touch, fide); chess-studio (fide_panel)",
     t(GAME + "fide_touch.rs", "rule_02_selecao_nao_e_toque_so_touch_cria_obrigacao", "ac_04_mover_soltar_e_concluir_sao_etapas_separadas")
     + t(STU + "fide_panel.rs", "the_state_line_follows_touch_release_and_clock", "the_panel_does_not_reveal_whether_a_loose_piece_is_legal"),
     "PRs #74, #86", "PASS", ""),
    ("UX-06", "Picking correto das 64 casas após qualquer rotação da câmera.",
     "chess-input (view, board)",
     t(INP + "src/view.rs", "every_square_is_pickable_after_any_rotation", "every_square_is_pickable_in_every_view"),
     "PR #62 (72 yaws x 4 pitches); homologação do usuário 09/11", "PASS", "Homologação do usuário em 09/11: 64 casas em qualquer Vista (ver CAM-04)."),
    ("UX-07", "Trocar 2D/3D conserva posição, histórico e preferências aplicáveis.",
     "chess-studio (pieces, presentation, persistence)",
     t(STU + "pieces.rs", "switching_presentation_keeps_the_game_and_the_scene_position")
     + t(STU + "persistence.rs", "preferences_round_trip_and_default_to_training"),
     "PRs #78, #84", "PASS", ""),
    ("UX-08", "SQLite recupera preferências depois de reiniciar.",
     "chess-store (set_pref/get_pref); chess-studio (persistence)",
     t(STO + "store.rs", "preferences_persist_across_opens", "preferences_are_per_profile")
     + t(STU + "persistence.rs", "preferences_round_trip_and_default_to_training", "unreadable_preferences_fall_back_to_defaults"),
     "PRs #73, #84", "PASS", ""),
    ("UX-09", "Cancelamento/perda de foco não move peça acidentalmente.",
     "chess-input (gesture); chess-studio (play)",
     t(INP + "tests/gestures.rs", "cancel_events_abort_drag_without_any_command", "escape_or_focus_loss_while_dragging_keeps_the_touch_obligation"),
     "PR #76; homologação do usuário 09/11", "PASS", "Homologação do usuário em 09/11: cancelar o arrasto não move peça (ver INT-03)."),
    ("UX-10", "Animações suaves preservam precisão e as metas documentadas de performance.",
     "chess-studio (pieces); chess-input (settle)",
     t(INP + "src/settle.rs", "ends_exactly_on_the_canonical_square_from_any_start"),
     "`docs/evidencias/b19/desempenho.txt` (indicativo, 3 a 4 ms/quadro, sem vsync)", "UNVERIFIED",
     "Precisão provada; suavidade ao vivo e metas de FPS dependem do B16 (#40) e do olhar do usuário."),

    # ---- 14.6 Persistência, motores, treino, plataforma ----
    ("DB-01", "Salvar/reabrir partida restaura posição, histórico, modo, relógio e incidentes.",
     "chess-store; chess-studio (persistence)",
     t(STO + "store.rs", "save_close_reopen_restores_identical_snapshot", "reopen_reports_clean_or_abrupt_close_and_returns_paused")
     + t(STU + "persistence.rs", "closing_and_reopening_resumes_the_game_paused_at_the_same_move", "moves_after_resume_continue_the_same_log_with_monotonic_time",
         "a_new_game_screen_fide_game_with_engine_is_saved_and_resumes_paused")
     + t(GAME + "clock.rs", "reabrir_congela_o_relogio_no_ultimo_evento_gravado"),
     "PRs #73, #84, #86", "PASS", ""),
    ("DB-02", "FEN válido carrega posição; FEN inválido gera erro sem corromper o banco.",
     "chess-core (fen); chess-store; chess-studio (fen_pgn)",
     t(CORE + "fen.rs", "valid_fens_round_trip_identically", "invalid_fens_return_typed_errors_without_panicking")
     + t(STO + "store.rs", "invalid_fen_does_not_create_a_game_or_touch_the_db")
     + t(STU + "fen_pgn.rs", "invalid_fen_reports_a_clear_error_and_changes_nothing", "valid_fen_starts_a_new_game_from_that_position")
     + t(STU + "persistence.rs", "invalid_fen_changes_nothing_in_session_or_database"),
     "PRs #63, #73, #77, #84", "PASS", ""),
    ("DB-03", "Exportação PGN pode ser lida/validada por ferramenta independente.",
     "chess-game (pgn); chess-studio (fen_pgn); scripts/validate_pgn.py",
     t(GAME + "pgn_export.rs", "python_chess_reabre_o_pgn_exportado", "opera_de_morphy_gera_o_texto_esperado", "nenhuma_linha_passa_de_80_colunas")
     + t(STU + "fen_pgn.rs", "export_writes_a_valid_pgn_and_never_overwrites"),
     "PR #77; `python_chess_reabre_o_pgn_exportado` é `#[ignore]` (exige python-chess) e foi **executado e passou** em 2026-11-06 (python-chess instalado em venv local)", "PASS", ""),
    ("DB-04", "Eventos de tentativa ilegal e penalidades não são confundidos com jogadas legais.",
     "chess-game (event, pgn); chess-store",
     t(GAME + "assisted_illegal.rs", "o_evento_de_lance_ilegal_fica_no_log_e_distingue_do_legal")
     + t(GAME + "pgn_export.rs", "infracao_vira_comentario_no_ponto_certo_e_nao_vira_lance")
     + t(STO + "store.rs", "pgn_after_reopen_is_identical_and_has_only_legal_moves")
     + t(STU + "session.rs", "illegal_drop_is_recorded_with_its_reason_outside_the_legal_history"),
     "PRs #73, #77", "PASS", ""),
    ("DB-05", "Interrupção durante a escrita não gera estado parcial; existe recuperação demonstrada.",
     "chess-store (store: transação IMMEDIATE, WAL, synchronous=FULL)",
     t(STO + "crash.rs", "killing_the_writer_mid_batch_leaves_whole_batches_or_nothing")
     + t(STO + "store.rs", "failed_batch_leaves_nothing_behind", "reopen_reports_clean_or_abrupt_close_and_returns_paused")
     + t(STU + "persistence.rs", "unclean_exit_is_reported_as_abrupt_close"),
     "PR #73 (SIGKILL em 5 atrasos; `PRAGMA integrity_check = ok`)", "PASS", ""),
    ("DB-06", "Migrações versionadas preservam dados e preferências.",
     "chess-store (migrations v1 a v3)",
     t(STO + "migration.rs", "opening_a_previous_version_database_migrates_and_preserves_data", "a_failing_migration_rolls_back_and_keeps_the_previous_version",
       "database_from_a_newer_app_is_refused_with_a_clear_error")
     + t(STO + "training.rs", "a_pre_training_api_database_migrates_keeping_old_exercise_rows"),
     "PRs #73, #87", "PASS", ""),
    ("UCI-01", "Engine externa configurada responde ao handshake UCI e joga offline.",
     "chess-uci (engine, controller, player, detect); chess-studio (engine, engine_wizard)",
     t(UCI + "stockfish.rs", "real_stockfish_handshake_search_stop_and_self_play")
     + t(UCI + "stockfish_player.rs", "real_stockfish_plays_black_with_clock_and_elo")
     + t(UCI + "adapter.rs", "handshake_reports_identity_and_announced_options")
     + t(UCI + "player.rs", "full_game_of_the_fake_engine_against_a_scripted_human_reaches_a_result_or_keeps_legal"),
     "PRs #64, #79, #85. Os dois testes com Stockfish 19 real são `#[ignore]` e foram **executados e passaram** em 2026-11-06 (2,53 s e 29,37 s)", "PASS",
     "Provado no nível do controlador com Stockfish real. A partida completa na janela do app contra Stockfish é item do roteiro manual."),
    ("UCI-02", "Timeout/crash não bloqueia a UI nem danifica a partida.",
     "chess-uci (controller com worker próprio, vigia no poll)",
     t(UCI + "adapter.rs", "hung_engine_is_reported_as_timeout_without_blocking_and_is_killed", "crash_during_search_is_detected_and_reported")
     + t(UCI + "player.rs", "crash_keeps_the_game_and_offers_retry_and_replace", "hung_engine_times_out_without_blocking_and_keeps_the_game",
         "engine_that_cannot_start_fails_with_a_clear_message"),
     "PRs #64, #79", "PASS", "`drive` é asserido abaixo de 100 ms nos testes; a responsividade visual da janela entra no roteiro manual."),
    ("UCI-03", "Engine ilegal é rejeitada e causa erro recuperável.",
     "chess-uci (player, engine)",
     t(UCI + "adapter.rs", "illegal_move_is_a_recoverable_error_and_leaves_the_position_unchanged")
     + t(UCI + "player.rs", "illegal_move_is_recoverable_and_never_changes_the_position")
     + t(GAME + "clock.rs", "lance_ilegal_da_engine_e_logado_e_nao_muda_a_partida"),
     "PRs #64, #79", "PASS", ""),
    ("UCI-04", "Sem Engine externa, o aplicativo informa o limite e permite estudo/dois jogadores locais.",
     "chess-studio (engine_wizard, engine, new_game); chess-uci (detect, install)",
     t(STU + "engine.rs", "nothing_found_is_none")
     + t(STU + "engine_wizard.rs", "detection_runs_off_thread_and_lists_results", "every_wizard_message_used_in_the_source_exists", "cancelling_returns_to_idle_without_error")
     + t(STU + "new_game.rs", "local_play_is_the_default_opponent")
     + t(UCI + "detect.rs", "only_candidates_that_speak_uci_are_listed"),
     "PR #85; download com hash: `chess-uci/tests/install.rs` (9 testes + 1 real `#[ignore]`)", "PASS",
     "Lógica e mensagens testadas; a tela do assistente não tem captura (conferir no roteiro manual)."),
    ("TRN-01", "Concluir treino de afogamento explica por que não é xeque-mate.",
     "chess-training (review, run); chess-studio (training_ui)",
     t(TRN + "runs.rs", "stalemate_exercise_ends_in_a_draw_and_review_explains_it_is_not_checkmate"),
     "`docs/evidencias/b13/revisao-afogamento.png`; PRs #87, #89", "PASS", ""),
    ("TRN-02", "Repetição do exercício após erro usa a posição original correta.",
     "chess-training (run); chess-store (exercise_runs)",
     t(TRN + "runs.rs", "repeating_after_an_error_starts_from_the_original_position_and_keeps_the_first_run")
     + t(STO + "training.rs", "a_repeated_run_is_a_new_row_and_the_original_is_not_rewritten"),
     "`docs/evidencias/b13/exercicio-erro-desfeito.png`; PR #87", "PASS", ""),
    ("TRN-03", "Exercício sem dicas mantém o registro do auxílio usado/desligado.",
     "chess-training (AidsRecord); chess-store",
     t(TRN + "runs.rs", "aids_used_and_disabled_are_recorded")
     + t(STO + "training.rs", "aids_survive_reopening_the_database"),
     "PR #87", "PASS", ""),
    ("TRN-04", "Ao menos um exercício de memória registra o resultado da reprodução.",
     "chess-training (MemoryRun) — **somente no PR #90, ainda não integrado**",
     [], "PR #90 aberto (B15, issue #39); relata PASS e capturas em docs/evidencias/b15/ na branch dele, mas **nada disso está na `main`**", "UNVERIFIED",
     "Fora da `main` homologada. Após o merge do #90, reexecutar `cargo test --workspace` e reclassificar (esperado: PASS via `chess-training/tests/memory.rs`)."),
    ("TRN-05", "Histórico do treino quantifica erros e acertos sem alterar resultados competitivos.",
     "chess-training (Metrics); chess-store (metrics)",
     t(TRN + "runs.rs", "metrics_count_hits_errors_and_no_aid_runs_by_kind_and_session")
     + t(STO + "training.rs", "metrics_by_kind_and_session_never_touch_competitive_games")
     + t(STU + "training_ui.rs", "exercise_flow_keeps_the_ongoing_game_and_never_persists_it_as_a_game"),
     "`docs/evidencias/b13/lista-e-metricas.png`; PR #87", "PASS", ""),
    ("PLAT-01", "Compila e executa com instalável/documentação nos três sistemas.",
     "docs/build/{linux,windows,macos}.md; instaláveis: PR #91 (B17)",
     [], "Linux: `cargo build`/`cargo test` ok; guias de build do PR #72. Instaláveis: PR #91 aberto (#41), fora da `main`", "UNVERIFIED",
     "Windows e macOS não verificados (PENDENTE do usuário, DEC-25); instaláveis ainda não integrados."),
    ("PLAT-02", "O jogo continua sem login, internet ou serviço externo.",
     "arquitetura (ADR-0003, ADR-0002); única chamada de rede: `chess-uci/src/install.rs`",
     [], "Auditoria estática: `grep -rl ureq crates` só aponta `chess-uci` (download do Stockfish, com consentimento); nenhuma conta, login ou telemetria", "UNVERIFIED",
     "Não há teste automatizado de uso offline. Roteiro manual: jogar 2P, Exercícios e Engine local com a rede desligada."),
    ("PERF-01", "Benchmark dedicado 1080p Equilibrado mira 60 FPS na máquina homologada.",
     "chess-studio (benchmark)",
     t(STU + "benchmark/stats.rs", "summarize_constant_60fps", "one_percent_low_averages_the_slowest_one_percent"),
     "`docs/evidencias/a6/baseline-linux-rtx4080s*.json` (contaminada)", "UNVERIFIED", "Aguarda B16 (#40) em sessão desbloqueada e máquina ociosa."),
    ("PERF-02", "Benchmark integrado 720p Econômico mira 30 FPS na máquina homologada.",
     "chess-studio (benchmark)",
     [], "Sem medição em GPU integrada", "UNVERIFIED", "PENDENTE do usuário: máquina com iGPU (B16)."),
    ("PERF-03", "Teste de 5 min registra FPS/frame time/RAM/VRAM/CPU/GPU/SO/drivers.",
     "chess-studio (benchmark: config, stats, report, route)",
     t(STU + "benchmark/config.rs", "defaults_are_five_minutes_1080p_balanced_idle")
     + t(STU + "benchmark/report.rs", "frame_times_csv_has_one_row_per_frame", "samples_csv_leaves_missing_vram_empty")
     + t(STU + "benchmark/mod.rs", "parses_nvidia_smi_csv_line"),
     "`docs/evidencias/a6/baseline-linux-rtx4080s.json`", "UNVERIFIED",
     "O instrumento funciona e gerou JSON, mas a medição está contaminada (vsync a 20 Hz, builds concorrentes, VRAM de outros processos). Repetir no B16."),
    ("PERF-04", "Medir desempenho com e sem Engine ativa, documentando a interferência.",
     "chess-studio (benchmark; `--engine`)",
     [], "Só `--engine ociosa` foi medido (A6)", "UNVERIFIED", "Aguarda B16 com Engine ativa."),
]


MODELO = """# Matriz de rastreabilidade do P0 (B18, issue #42)

Gerada por `scripts/gerar_matriz_p0.py`, que **falha** se algum teste ou evidência citado não existir. Para atualizar: edite os dados no script, rode `python3 scripts/gerar_matriz_p0.py` e confira o diff.

- **Base:** `main` em `6a65770bd9bb` (inclui os PRs #61 a #89; os PRs #90 (B15, memória) e #91 (B17, instaláveis) estão abertos e **fora** desta matriz). **Atualização de 09/11:** os status de CAM, INT, UX-02/04/06/09 incorporam a homologação do usuário (janela real) e os PRs #137 e #138.
- **Ambiente:** Linux físico (Ubuntu 26.04, Ryzen 7 3700X, RTX 4080 SUPER, driver 595.99.02), Rust 1.98.1.
- **Suíte:** `cargo test --workspace` = **524 testes aprovados, 0 falhas, 5 `#[ignore]`** (inclui 3 doctests). Os 5 ignorados: perft profundo, Stockfish real (2: handshake e jogador), download real do Stockfish, PGN com python-chess. Quatro foram executados à parte em 2026-11-06 e passaram (perft profundo, os 2 de Stockfish 19 real, PGN com python-chess); o download real do Stockfish não foi reexecutado (passou no PR #85);
- **Escopo:** os {{TOTAL}} IDs de marco P0 da §14 do prompt mestre (CORE, VIS, CAM, INT, AC, RULE, UX, DB, UCI, TRN, PLAT, PERF). UI-01 a UI-12 e UCI-05/06 são V1 e V2-SUM são V2: fora do P0. RULE-05 é "P0/V1" e entra aqui. `VIS-04` não existe na matriz.
- **Resumo:** {{RESUMO}}

Legenda: **PASS** = teste automatizado verde ou evidência verificável; **UNVERIFIED** = depende de verificação manual, de hardware/VM ou de entrega fora da `main` (UNVERIFIED não é PASS); **FAIL** = defeito ou lacuna conhecida; **N/A** = não se aplica, com justificativa (nenhum neste marco). Caminhos de teste são relativos a `crates/`. Números `#NN` são PRs mergeados, salvo indicação.

{{TABELA}}

## O que falta para o P0 ser GO

Itens que só o usuário (ou hardware dele) resolve, mais as pendências de engenharia que bloqueiam o GO.

**Decisão do usuário**
1. **CORE-07:** resolvido pelo detector de posição morta (#93); resta apenas conhecer o limite (bispos presos e fortalezas não são detectados).

**Homologação visual (revisão humana)**
2. **VIS-01** e **VIS-02:** aprovar o 3D profissional e a Mesa imersiva (capturas em `docs/evidencias/b19/`) ou pedir ajustes; confirmar o 2D técnico (VIS-03, coordenadas legíveis).
3. Itens de decisão do checklist: notação pt/en, formato dos Exercícios de afogamento, visual do relógio 3D (ver `docs/relatorios/p0.md`, DECISOES_PENDENTES).

**Roteiro manual em janela real (Linux, sessão desbloqueada)**
4. **CAM-02, INT-01, UX-02:** reteste das correções da homologação de 09/11 (PR #137: relógio fora do enquadramento, Vistas e reset centralizados; PR #138: picking pelo corpo das peças e limiar de arrasto de 6 px). CAM-01 (órbita), CAM-03/04, UX-06, INT-02/UX-04 (e2→e5) e INT-03/UX-09 (cancelar arrasto) já passaram na homologação do usuário em 09/11. Trackpad só no Mac.
5. **INT-03, UX-09:** concluídos em 09/11 (ver item 4); Alt+Tab e clicar no painel no meio de um arrasto seguem cobertos só pelo intérprete.
6. **UX-10:** suavidade da animação de encaixe e do arrasto.
7. Confirmar a partida completa contra o Stockfish na janela (UCI-01), incluindo a tela do assistente de Engine (UCI-04).
8. **PLAT-02:** jogar 2P, Exercícios e Engine local com a rede desligada.

**Hardware e plataformas (B16 #40, B17 #41, A7 #24)**
9. **PERF-01 a PERF-04:** repetir o benchmark de 5 min em máquina ociosa e sessão desbloqueada (1080p Equilibrado e 720p Econômico, com e sem Engine ativa) e medir na GPU integrada.
10. **PLAT-01:** compilar, testar e abrir o app em Windows e macOS (VMs ou máquinas físicas) seguindo `docs/build/`; revisar e mergear os instaláveis (PR #91, issue #41).

**Entrega de engenharia**
11. **TRN-04:** mergear o PR #90 (B15, memória de sequência) e reclassificar para PASS após `cargo test --workspace`.
12. Regerar esta matriz e o relatório (`docs/relatorios/p0.md`) após os itens 4 a 11 e então fechar a issue #42 com `RESULTADO: GO`.
"""


def valida():
    erros = []
    for (idn, _r, _i, testes, evid, _s, _n) in LINHAS:
        for (arq, nome) in testes:
            p = RAIZ / "crates" / arq
            if not p.is_file():
                erros.append(f"{idn}: arquivo inexistente crates/{arq}")
                continue
            if not re.search(rf"\bfn {re.escape(nome)}\b", p.read_text()):
                erros.append(f"{idn}: teste `{nome}` não existe em crates/{arq}")
        for caminho in re.findall(r"`(docs/evidencias/[^`\s]+)`", evid):
            base = caminho.rstrip("*")
            if "*" in caminho:
                if not list((RAIZ / Path(caminho).parent).glob(Path(caminho).name)):
                    erros.append(f"{idn}: evidência sem correspondência {caminho}")
            elif not (RAIZ / caminho).exists():
                erros.append(f"{idn}: evidência inexistente {caminho}")
    ids = [l[0] for l in LINHAS]
    if len(ids) != len(set(ids)):
        erros.append("IDs duplicados")
    return erros


def fmt_testes(testes):
    if not testes:
        return "—"
    return "<br>".join(f"`{a}::{n}`" for a, n in testes)


def gera():
    cont = {"PASS": 0, "UNVERIFIED": 0, "FAIL": 0, "N/A": 0}
    for l in LINHAS:
        cont[l[5]] += 1
    cab = "| ID | Requisito | Implementação | Teste(s) automatizado(s) | Evidência | Status | Nota |\n|---|---|---|---|---|---|---|\n"
    corpo = ""
    grupo = None
    for (idn, req, impl, testes, evid, st, nota) in LINHAS:
        corpo += f"| **{idn}** | {req} | {impl} | {fmt_testes(testes)} | {evid} | **{st}** | {nota or '—'} |\n"
    resumo = " · ".join(f"{k}: {v}" for k, v in cont.items())
    return cont, resumo, cab + corpo


def main():
    erros = valida()
    if erros:
        print("\n".join(erros), file=sys.stderr)
        sys.exit(1)
    cont, resumo, tabela = gera()
    print(f"{len(LINHAS)} IDs válidos. {resumo}")
    if "--check" in sys.argv:
        return
    modelo = MODELO
    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    SAIDA.write_text(modelo.replace("{{RESUMO}}", resumo).replace("{{TABELA}}", tabela).replace("{{TOTAL}}", str(len(LINHAS))))
    print(f"escrito {SAIDA.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
