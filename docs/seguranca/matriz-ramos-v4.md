# Matriz dos ramos de segurança e fair play do V4 (#269, NFR-10)

NFR-10 da [especificação](../integracoes/especificacao-free-first-2026-11-11.md) pede cobertura de linhas nos adaptadores, parsers e Central e **100% dos ramos de segurança e fair play mapeados por teste**, com evidência "cobertura + lista de ramos". Este documento é a lista de ramos: uma linha por ramo de decisão que protege segredo, consentimento, catálogo, Trava de fair play ou entrada hostil, com o teste que cobre o ramo **positivo** (o que passa) e o **negativo** (o que é recusado). Nasceu da [auditoria do V4](../integracoes/auditoria-v4.md) e complementa a [revisão de segurança](revisao-v4.md).

Os caminhos são relativos à raiz do repositório. Teste sem arquivo informado mora no mesmo arquivo da linha anterior da tabela. Todos rodam contra `FakeServer`, `FakeIcsServer`, `FakeClock` e cofres em memória: nenhum serviço real é chamado.

Legenda: **SEM TESTE** marca um ramo sem teste; **N/A** marca um ramo que a suíte não alcança sem falha do sistema operacional (justificado na linha). Os ramos que estavam **SEM TESTE** quando esta matriz foi levantada ganharam teste neste PR e aparecem marcados com **(novo)**.

## Resultado

Nenhum ramo ficou **SEM TESTE**. Os ramos sem teste negativo (ou positivo) achados no levantamento ganharam teste neste PR e estão marcados com **(novo)**:

- `crates/chess-net/tests/gate.rs`: `an_explicit_port_or_a_malformed_host_is_refused_before_the_network`
- `crates/chess-net/tests/consent.rs`: `a_consent_database_the_app_cannot_read_is_a_typed_storage_error`
- `crates/chess-net/tests/loopback.rs`: `an_oversized_or_non_text_head_is_dropped_and_does_not_spend_the_callback`
- `crates/chess-providers/tests/auth.rs`: `a_callback_without_a_usable_code_or_with_another_error_is_malformed_and_exchanges_nothing`, `a_token_answer_that_is_not_a_bearer_token_is_malformed_and_stores_nothing`
- `crates/chess-vault/tests/vault.rs`: `a_disk_failure_opening_the_vault_is_a_typed_io_error`
- `crates/chess-uci/tests/adapter.rs`: `set_option_refuses_names_and_values_that_would_inject_another_command`
- `crates/chess-studio/src/fair_play_matrix_tests.rs`: a grade `Computation` x estado e os dois testes que amarram a matriz ao código

Dois ramos são **N/A** (falha do sistema operacional que a suíte não provoca), ambos em Loopback e OAuth. Cobertura de linhas: ver [Cobertura](#cobertura).

## Gate do `chess-net`

`Shared::admit` (`crates/chess-net/src/net.rs`) decide, nesta ordem: consentimento, endereço no catálogo, serviço oferecido, Trava, autorização do Cofre. É chamado ao enviar e de novo logo antes de cada ida à rede.

| Ramo | Positivo | Negativo |
|---|---|---|
| Consentimento: Provedor ligado deixa passar; nasce desligado | `crates/chess-net/tests/consent.rs`: `consent_is_per_local_profile_and_survives_reopening` | `every_provider_is_born_disabled_and_the_app_starts_offline`, `disabled_provider_opens_no_connection_and_says_which_provider_to_enable` |
| Consentimento revogado vale já no próximo pedido | `consent.rs`: `turning_a_provider_off_again_blocks_the_next_request` (o pedido anterior passou) | o mesmo teste (o seguinte recusa) |
| Provedor sem serviço de rede não liga | `consent.rs`: `every_provider_is_born_disabled_and_the_app_starts_offline` (os de rede ligam em `consent_is_per_local_profile_and_survives_reopening`) | `a_provider_without_network_cannot_be_turned_on` |
| Aviso do que é enviado cita os hosts | `consent.rs`: `the_consent_notice_says_what_is_sent_and_where` | idem (falha se faltar host) |
| Consentimento é por Perfil local e some com o Perfil | `consent.rs`: `consent_is_per_local_profile_and_survives_reopening` | idem (outro Perfil volta a offline; `forget_profile`) |
| Banco do consentimento ilegível ou de versão nova: erro `Storage`, nunca "sem consentimentos" em silêncio | `consent.rs`: `consent_is_per_local_profile_and_survives_reopening` | `consent.rs`: `a_consent_database_the_app_cannot_read_is_a_typed_storage_error` **(novo)** |
| Catálogo: só endereço do Provedor, HTTPS, host e caminho do catálogo | `crates/chess-net/tests/gate.rs`: `each_request_is_classified_by_the_catalog_as_computing_or_not` | `an_address_outside_the_catalog_is_refused_before_the_network` (outro Provedor, caminho fora, sem HTTPS, userinfo, site) |
| Catálogo: porta explícita e host fora de `[a-z0-9.-]` nunca saem | `gate.rs`: `each_request_is_classified_by_the_catalog_as_computing_or_not` | `an_explicit_port_or_a_malformed_host_is_refused_before_the_network` **(novo)** |
| Rota precisa ser HTTPS ou loopback | `gate.rs`: `a_route_must_be_https_or_loopback` (loopback aceito) | idem (resto recusado) |
| Catálogo: cada Provedor, endereço e capacidade declarados | `crates/chess-net/tests/catalog.rs`: `every_provider_of_the_matrix_is_in_the_catalog`, `every_endpoint_belongs_to_one_provider_and_to_a_declared_service` | `manual_import_providers_have_no_network_endpoint`, `only_standard_chess_is_accepted_and_offered_variants_are_refused_by_name` |
| Serviço não oferecido (`NotOffered`) | `crates/chess-net/tests/catalog.rs`: `playstrategy_public_games_are_ready_but_online_play_is_not_yet` (o que está pronto passa) | `crates/chess-providers/tests/playstrategy.rs`: `what_playstrategy_does_not_offer_is_refused_before_any_request` |
| Trava de fair play: o que calcula é recusado enquanto ela está ligada | `gate.rs`: `the_guard_lets_through_what_does_not_compute_and_releases_at_the_end` | `with_the_fair_play_guard_engaged_nothing_that_computes_leaves`, `the_guard_is_consulted_again_right_before_the_network` |
| Capacidades que calculam são as que a Trava bloqueia | `catalog.rs`: `chess_api_is_a_computing_engine_without_token_and_documents_that_no_key_is_needed` | `computing_capabilities_are_the_ones_the_fair_play_guard_blocks` |
| Trava própria e trava de outra Partida (`is_engaged_besides`) | `crates/chess-net/src/guard.rs`: `quem_segura_a_propria_trava_sabe_se_ha_outra` | `a_trava_so_cai_quando_a_ultima_partida_solta` |
| Token só vai aos endereços autenticados do próprio Provedor | `gate.rs`: `the_authorization_goes_only_to_the_authenticated_endpoints_of_its_provider` | `a_token_of_one_provider_never_reaches_another_provider_even_from_a_faulty_vault`, `catalog.rs`: `the_token_goes_only_where_the_provider_documents_authentication` |
| Conexão nomeada sem segredo nunca sai sem cabeçalho | `gate.rs`: `the_authorization_goes_only_to_the_authenticated_endpoints_of_its_provider` | `naming_a_connection_without_a_secret_never_goes_out_unauthenticated`, `a_capability_that_needs_a_connection_is_refused_without_one` |
| 401/403 marca a Conexão; escopos só em pedido autenticado; revogação com o token | `gate.rs`: `revocation_goes_out_as_delete_with_the_token`, `the_scopes_of_the_token_come_back_only_on_authenticated_requests` | `a_401_or_403_with_the_token_marks_the_connection_and_is_typed` |
| Cabeçalhos: `Authorization` e `Cookie` não se definem à mão; User-Agent genérico | `gate.rs`: `the_user_agent_is_the_generic_one_of_the_app` | `authorization_and_cookies_cannot_be_set_by_hand` |
| Segredo fora de `Debug`, erro, log da operação e log em `TRACE` | `gate.rs`: `the_debug_of_a_request_hides_the_body` | `no_secret_shows_up_in_debug_errors_or_the_operation_log`, `crates/chess-net/tests/log_secrets.rs`: `no_secret_reaches_the_log_even_at_trace_level` |

## Redirecionamento e download

| Ramo | Positivo | Negativo |
|---|---|---|
| Redirecionamento de consulta nunca é seguido para outro host | `crates/chess-net/tests/gate.rs`: `a_redirect_to_another_host_is_not_followed` (host igual no catálogo é tratado pelo download abaixo) | idem |
| Download segue só redirecionamento para endereço de download do catálogo | `crates/chess-net/tests/download.rs`: `segue_redirecionamento_para_outro_endereco_de_download_do_catalogo`, `segue_redirecionamento_relativo_no_mesmo_host` | `recusa_redirecionamento_para_fora_do_catalogo_e_nao_pede_la` |
| Redirecionamento para outra capacidade, sem HTTPS, com porta ou consulta | `download.rs`: `segue_redirecionamento_para_outro_endereco_de_download_do_catalogo` | `recusa_redirecionamento_para_outra_capacidade_sem_https_porta_ou_consulta` |
| Laço de redirecionamentos para no limite de saltos | `download.rs`: `segue_redirecionamento_relativo_no_mesmo_host` | `laco_de_redirecionamentos_para_depois_do_limite_de_saltos`, `redirecionamento_nao_vale_para_consulta_comum` |
| Download só do que o catálogo marca como download; Provedor desligado não abre conexão | `download.rs`: `o_corpo_chega_inteiro_e_em_ordem_com_o_tamanho_anunciado_e_sem_ir_para_o_cache` | `so_baixa_o_que_o_catalogo_marca_como_download`, `provedor_desligado_nao_abre_conexao_nenhuma` |

## Limites e cache

| Ramo | Positivo | Negativo |
|---|---|---|
| `Retry-After` / limite de taxa: nada sai antes do prazo | `crates/chess-net/tests/limits.rs`: `a_patient_request_waits_the_announced_time_and_retries_once` | `retry_after_marks_the_provider_limited_and_nothing_is_sent_before_the_deadline`, `without_retry_after_the_pause_is_one_minute` |
| Tentativas são limitadas; erro de cliente não repete | `limits.rs`: `server_errors_are_retried_with_exponential_backoff_and_jitter` | `retries_are_bounded_and_the_provider_shows_as_failing`, `client_errors_are_not_retried` |
| Corpo de resposta tem teto e não entra no cache | `limits.rs`: `the_token_bucket_spaces_a_burst_to_the_configured_rate` (respostas normais passam) | `a_giant_response_is_cut_at_the_limit_and_never_cached`, `an_endless_body_is_cut_at_the_limit` |
| Prazo e cancelamento | `crates/chess-net/tests/timeout_cancel.rs`: `polling_never_blocks` | `a_stalled_server_ends_in_a_typed_timeout`, `cancelling_answers_at_once_and_the_worker_is_released`, `a_request_cancelled_while_queued_never_reaches_the_network` |
| Cache é do Perfil local e do Provedor; FEN inválida não sai | `crates/chess-net/tests/cache.rs`: `a_second_equal_request_within_the_ttl_does_not_touch_the_network` | `the_cache_belongs_to_the_local_profile`, `clearing_the_cache_is_per_provider`, `an_invalid_fen_is_refused_before_the_network` |

## `chess-vault`

| Ramo | Positivo | Negativo |
|---|---|---|
| Cofre do sistema: segredo vai para lá e nunca para o disco | `crates/chess-vault/tests/vault.rs`: `with_a_system_vault_the_secret_goes_there_and_never_to_disk`, `preloading_reads_the_system_vault_once_so_requests_never_wait_for_it` | `a_failing_system_vault_is_reported_and_the_reference_stays` |
| Segredo vazio recusado | `vault.rs`: `replacing_and_deleting` | `with_a_system_vault_the_secret_goes_there_and_never_to_disk` (`EmptySecret`) |
| Autorização presa ao Provedor e ao Perfil local | `vault.rs`: `the_authorization_is_bound_to_the_provider_and_the_local_profile` | `a_tampered_index_cannot_rebind_a_secret_to_another_profile_or_provider` |
| Conferência antes de guardar (segredo em espera só vale se confirmado) | `vault.rs`: `a_staged_secret_is_used_only_while_it_lives_and_replaces_only_on_commit` | idem (descartado sem commit) |
| Rejeição do Provedor marca a referência e mantém o segredo | `vault.rs`: `a_rejection_by_the_provider_marks_the_reference_and_keeps_the_secret` | idem |
| Nada de segredo em `Debug`, erro ou tipo `Secret` | `vault.rs`: `no_secret_shows_up_in_debug_or_errors` | `crates/chess-vault/tests/static_secrets.rs`: `the_secret_type_keeps_debug_hidden_and_has_no_display`, `only_the_vault_and_the_network_header_read_a_secret_in_clear` |
| A rede recebe o cabeçalho só para endereços autenticados | `vault.rs`: `the_network_gets_the_header_from_the_vault_only_for_authenticated_endpoints` | `with_the_own_vault_locked_nothing_leaves_and_the_error_is_typed` |
| Sem cofre do sistema, nada é guardado até criar o cofre próprio | `crates/chess-vault/tests/own_vault.rs`: `reopening_starts_locked_and_only_the_right_password_unlocks` | `without_a_system_vault_nothing_is_stored_until_the_user_creates_the_own_vault`, `operations_of_the_own_vault_need_the_own_vault` |
| Senha-mestra: certa destranca, errada e fraca recusadas; trocar a senha | `own_vault.rs`: `changing_the_master_password` | `reopening_starts_locked_and_only_the_right_password_unlocks` (senha errada) |
| Bloquear esquece a chave e os segredos; excluir bloqueado vale no próximo desbloqueio | `own_vault.rs`: `locking_forgets_the_key_and_the_secrets`, `deleting_while_locked_drops_the_reference_now_and_the_secret_at_the_next_unlock` | idem |
| Arquivo adulterado, truncado ou de formato desconhecido é recusado | `own_vault.rs`: `reopening_starts_locked_and_only_the_right_password_unlocks` | `a_tampered_truncated_or_unknown_file_is_refused_and_nothing_is_read`, `a_file_cut_at_any_length_is_refused_without_panic` |
| Pasta e arquivos privados ao usuário; cofre do sistema que aparece depois não assume | `own_vault.rs`: `the_vault_directory_and_files_are_private_to_the_user` | `a_system_vault_that_appears_later_does_not_take_over_silently` |
| Falha de disco ao abrir o Cofre: erro `Io`, sem segredo na mensagem | `vault.rs`: `with_a_system_vault_the_secret_goes_there_and_never_to_disk` | `vault.rs`: `a_disk_failure_opening_the_vault_is_a_typed_io_error` **(novo)** |

## Loopback e OAuth

Servidor do retorno: `crates/chess-net/src/loopback.rs`. Fluxo PKCE do Lichess: `crates/chess-providers/src/auth.rs`. Uso no app: `crates/chess-studio/src/connections/auth.rs`.

| Ramo | Positivo | Negativo |
|---|---|---|
| Retorno único recebido, respondido, porta fecha | `crates/chess-net/tests/loopback.rs`: `the_single_callback_is_received_answered_and_the_port_closes` | idem (segunda conexão recusada) |
| Caminho, método e `Host` conferidos (rebinding de DNS) | `loopback.rs`: `the_single_callback_is_received_answered_and_the_port_closes` | `other_paths_methods_and_hosts_do_not_count_as_the_callback` |
| Só pares de loopback | `crates/chess-net/src/loopback.rs`: `only_loopback_peers_are_accepted` | `loopback.rs`: `the_server_is_not_reachable_from_another_interface` |
| Caminho de retorno inválido | `loopback.rs` (src): `query_values_are_decoded` | `a_bad_path_is_refused` |
| Cabeçalho acima do limite ou que não é UTF-8 não gasta o retorno | `loopback.rs`: `the_single_callback_is_received_answered_and_the_port_closes` | `an_oversized_or_non_text_head_is_dropped_and_does_not_spend_the_callback` **(novo)** |
| Prazo pelo relógio injetado e cancelamento | `loopback.rs`: `the_single_callback_is_received_answered_and_the_port_closes` | `the_deadline_comes_from_the_injected_clock`, `cancelling_stops_waiting_and_closes_the_port` |
| `state` confere (PKCE completo guarda o token só no Cofre) | `crates/chess-providers/tests/auth.rs`: `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it` | `a_wrong_state_fails_typed_and_nothing_is_exchanged_or_stored`, `state_repeated_or_missing_is_also_a_mismatch` |
| Recusa do usuário só vale com o `state` certo | `auth.rs`: `the_user_refusing_in_the_browser_is_refused_only_with_the_right_state` | idem (`state` errado vira `StateMismatch`) |
| Retorno sem código utilizável, código repetido, fora do ASCII gráfico ou outro erro | `auth.rs`: `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it` | `a_callback_without_a_usable_code_or_with_another_error_is_malformed_and_exchanges_nothing` **(novo)** |
| Código já usado recusado pelo Provedor | `auth.rs`: `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it` | `a_reused_code_is_refused_by_the_provider_and_typed` |
| Resposta de troca que não é um token Bearer válido | `auth.rs`: `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it` | `a_token_answer_that_is_not_a_bearer_token_is_malformed_and_stores_nothing` **(novo)** |
| Escopos do token decidem o papel da Conexão (jogador x Engine) | `auth.rs`: `an_engine_connection_asks_for_bot_play_only`, `a_personal_token_is_checked_and_its_real_scopes_decide` | `an_engine_scope_on_a_player_connection_is_refused_and_not_stored`; no app, `crates/chess-studio/src/connections/auth_tests.rs`: `a_key_of_the_wrong_role_is_refused_with_an_explanation_in_both_directions`, `a_key_with_both_permissions_is_also_refused` |
| Token pessoal malformado não sai do computador; retorno forjado no app | `auth.rs`: `a_long_token_of_512_and_more_characters_is_accepted` | `a_malformed_personal_token_is_refused_before_the_network`; `auth_tests.rs`: `malformed_or_empty_keys_never_leave_the_computer`, `a_forged_return_is_refused_and_nothing_is_stored` |
| Sem consentimento ou sem Cofre que guarde, nada começa; Provedor desligado pede antes | `auth.rs`: `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it` | `nothing_starts_without_consent_or_a_vault_that_can_store`; `auth_tests.rs`: `a_provider_that_is_off_asks_first_and_resumes_when_turned_on` |
| Revogação apaga o segredo local mesmo com falha remota | `auth.rs`: `revoking_succeeds_remotely_and_the_explorer_needs_a_connection_again` | `revoking_deletes_locally_even_when_the_remote_call_fails_and_says_so`, `revoking_with_the_own_vault_locked_cannot_reach_the_provider_but_still_forgets_locally` |
| Erro de `accept` do sistema operacional no laço do loopback | N/A | N/A: o ramo só dorme e tenta de novo; a suíte não consegue fazer o SO falhar um `accept` |
| Falha ao ligar a porta de loopback (porta indisponível) | N/A | N/A: depende do SO recusar `bind` em porta efêmera; o erro `Unavailable` aparece no tipo e na interface (`auth_tests.rs`: `a_failed_browser_open_is_reported_and_can_be_retried` cobre a falha do navegador, não a do `bind`) |

## Trava de fair play (`crates/chess-studio/src/fair_play.rs`)

A grade `Computation` x estado está em `crates/chess-studio/src/fair_play_matrix_tests.rs` (**novo**): cada teste percorre `Computation::ALL` e confere todos os pontos de entrada (`check`, `check_fen`, `check_game`, `check_anywhere`, `check_start`). Dois testes dessa mesma suíte garantem que a tabela abaixo não fica para trás: `every_computation_variant_is_tracked_in_all` (toda variante está em `Computation::ALL`) e `every_computation_has_a_row_in_the_branch_matrix` (toda variante tem uma linha com `` `Computation::X` `` neste arquivo). Um novo valor de `Computation` quebra os dois até ganhar linha aqui.

### Por serviço (`Computation`)

Positivo: o serviço calcula quando nenhuma trava vale (`matrix_free_state_allows_every_computation_on_every_entry_point`). Negativo: o teste do consumidor real do serviço.

| Computation | Quem consulta | Teste negativo do consumidor (recusa sob a Trava) | Teste positivo do consumidor |
|---|---|---|---|
| `Computation::EngineAnalysis` | `analysis.rs` (`Analysis::drive`), barra de avaliação, seta da Engine | `crates/chess-studio/src/online/tests.rs`: `the_engine_analysis_refuses_during_the_online_game_even_in_review`; `online/fics_tests.rs`: `the_lock_refuses_analysis_and_a_drop_keeps_it_until_the_session_is_discarded` | `fair_play.rs`: `without_online_games_everything_is_allowed`; `online/tests.rs`: `against_the_lichess_engine_the_lock_never_engages` |
| `Computation::Hint` | `hud_policy.rs` (`effective`): dica, setas, casas atacadas | `online/tests.rs`: `the_hint_and_every_calculating_aid_are_refused_while_rule_aids_stay` | o mesmo teste (a ajuda de regras segue) |
| `Computation::RemoteEngine` | `GuardedAnalysis::start_remote` e `analyze` | `crates/chess-studio/src/remote_engine_tests.rs`: `the_fair_play_lock_refuses_before_any_request`, `the_lock_turning_on_after_an_answer_stops_further_requests`, `the_guarded_remote_engine_checks_the_lock_on_every_request`; `crates/chess-providers/tests/chess_api.rs`: `fair_play_lock_refuses_the_remote_engine_because_it_computes` | `chess_api.rs`: `happy_answer_arrives_as_an_ordinary_engine_update_with_only_the_fen_sent` |
| `Computation::CloudEval` | `explorer/driver.rs`: avaliação em cache de um Provedor | `online/tests.rs`: `cloud_eval_endgames_explorer_and_remote_engine_send_nothing`; `a_correspondence_game_locks_only_itself_and_its_positions` | `fair_play.rs`: `the_lock_only_falls_with_the_confirmed_end` |
| `Computation::Endgames` | `explorer/driver.rs`: finais exatos | `online/tests.rs`: `cloud_eval_endgames_explorer_and_remote_engine_send_nothing` | `fair_play.rs`: `correspondence_leaves_other_games_and_its_past_opening_free` |
| `Computation::Explorer` | `explorer/mod.rs`, `explorer/driver.rs`: Mestres, Lichess, Um jogador | `online/tests.rs`: `cloud_eval_endgames_explorer_and_remote_engine_send_nothing`; `crates/chess-providers/tests/explorer.rs`: `fair_play_lock_refuses_every_explorer_endpoint` | `explorer.rs`: `masters_happy_path_returns_moves_games_opening_and_provenance` |
| `Computation::LibraryStats` | `fair_play::opening_stats` (Explorador e Painel Aberturas) | `online/tests.rs`: `the_local_library_statistics_refuse_for_the_online_game` | `fair_play.rs`: `correspondence_leaves_other_games_and_its_past_opening_free` |
| `Computation::LibrarySearch` | `fair_play::search_library` e `check_library_search` (FP-03) | `fair_play_tests.rs`: `the_library_position_search_refuses_a_correspondence_position`; `library_ui.rs`: `the_position_search_is_refused_under_the_fair_play_lock_and_the_rest_stays` | o segundo teste (a busca sem Posição segue) |
| `Computation::OpponentEngine` | `GuardedOpponent` (FP-01, FP-02) | `fair_play_tests.rs`: `the_opponent_engine_never_starts_while_a_real_time_game_is_off_the_board`, `a_running_opponent_engine_stops_when_the_lock_engages`, `a_reopened_engine_game_does_not_resume_under_the_lock` | `a_local_competitive_game_against_the_engine_keeps_its_opponent`, `correspondence_refuses_the_opponent_engine_only_at_its_positions`; `fair_play.rs`: `the_opponent_engine_of_the_local_competitive_game_plays_under_its_own_lock` |
| `Computation::OnlineEngine` | `GuardedMoveEngine` e o serviço da Engine online (FP-04) | `fair_play_tests.rs`: `it_does_not_turn_on_while_a_real_time_human_game_is_in_progress`, `a_challenge_that_arrives_under_the_lock_is_declined_and_never_accepted`, `a_game_from_the_correspondence_position_never_reaches_the_engine`, `a_game_in_progress_stops_calculating_when_the_lock_engages`; `fair_play.rs`: `the_online_engine_never_receives_a_locked_position_and_leaves_for_good` | `fair_play_tests.rs`: `under_a_correspondence_lock_only_challenges_from_a_free_start_are_accepted` |

Acesso só pela Trava (nenhum serviço que calcula é criado fora dela): `fair_play.rs`: `computing_services_are_only_reachable_through_the_lock` (teste estático sobre o código-fonte).

### Por estado da Trava (cada `Computation` x cada `Subject`/estado)

Todos os testes de `fair_play_matrix_tests.rs` percorrem as dez `Computation`.

| Estado / ramo de `FairPlay` | Positivo | Negativo |
|---|---|---|
| Livre: nenhuma trava | `matrix_free_state_allows_every_computation_on_every_entry_point` | `matrix_provider_engine_and_local_games_are_not_followed` (nada entra na trava) |
| Trava da rede ligada (`LockScope::Everything`) | `matrix_network_guard_refuses_every_computation_on_every_entry_point` (depois de soltar, volta a passar) | idem; `fair_play.rs`: `the_network_guard_locks_everything_for_every_service` |
| Trava da Partida competitiva local: só a Engine adversária escapa (FP-02) | `matrix_competitive_hold_spares_only_the_opponent_engine` | idem (Partida online por cima trava a adversária também) |
| Partida online em tempo real, sem a sessão aberta | `matrix_real_time_game_refuses_every_computation_everywhere` (fim confirmado solta) | idem; `fair_play.rs`: `a_real_time_game_in_progress_locks_everything_even_without_its_session` |
| Correspondência: a própria Partida em qualquer meio-lance (`LockScope::ThisGame`) | `matrix_correspondence_locks_game_and_structure_but_not_the_rest` (Posição livre passa) | idem; `fair_play.rs`: `the_correspondence_game_itself_is_locked_at_every_ply` |
| Correspondência: Posição de mesma estrutura, até dois meios-lances à frente (`LockScope::ThisPosition`) | `fair_play.rs`: `correspondence_leaves_other_games_and_its_past_opening_free` | `matrix_correspondence_locks_game_and_structure_but_not_the_rest`; `fair_play.rs`: `correspondence_locks_its_current_position_and_what_is_near_it_anywhere` |
| Descendência: variante e estudo que partem de Posição travada | `matrix_check_game_follows_the_lineage_of_the_shown_game` (Partida livre passa) | o mesmo teste; `fair_play.rs`: `a_variation_shown_from_a_locked_position_stays_locked` |
| A trava acompanha o Lance e cai só com o fim confirmado | `fair_play.rs`: `the_lock_only_falls_with_the_confirmed_end` | `a_move_in_the_correspondence_moves_the_lock_along` |
| Contra a Engine do Provedor não há trava | `fair_play.rs`: `against_the_providers_engine_nothing_is_locked`; `online/tests.rs`: `against_the_lichess_engine_the_lock_never_engages` | `matrix_provider_engine_and_local_games_are_not_followed` |
| Ligar antes de saber a Posição (`check_anywhere`, `check_start`, desafio sem Posição) | `matrix_correspondence_locks_game_and_structure_but_not_the_rest` (`check_anywhere` passa só com correspondência) | o mesmo teste (`check_start(None)` recusa); `fair_play.rs`: `a_game_that_starts_in_an_unknown_position_waits_for_every_lock` |
| FEN ilegível (`check_fen`) | `fair_play.rs`: `an_unreadable_fen_is_refused_while_any_lock_is_on` (sem trava passa) | o mesmo teste e `matrix_correspondence_locks_game_and_structure_but_not_the_rest` |
| Lock de estado envenenado: na dúvida, trava | todos os outros (lock sadio) | `matrix_a_poisoned_state_lock_refuses_everything` |

## ICS (FICS)

| Ramo | Positivo | Negativo |
|---|---|---|
| Provedor ligado abre conexão; desligado não | `crates/chess-net/tests/ics.rs`: `lines_flow_both_ways_and_the_link_counts_as_an_active_connection` | `a_disabled_provider_opens_no_ics_connection`, `turning_the_provider_off_closes_the_open_connection` |
| Só os endereços ICS do catálogo, só a porta de texto puro do FICS | `ics.rs`: `the_catalog_lists_only_the_plaintext_fics_port` | `only_the_ics_addresses_of_the_catalog_are_reachable`, `a_route_that_is_not_loopback_is_refused_for_ics` |
| Comando enviado nunca esconde outra linha nem byte de controle | `ics.rs`: `lines_flow_both_ways_and_the_link_counts_as_an_active_connection` | `a_command_never_hides_another_line_or_a_control_byte` |
| Entrada hostil não trava nem derruba a conexão | `ics.rs`: `the_server_closing_ends_the_link` | `hostile_input_neither_hangs_nor_kills_the_connection`; `crates/chess-net/tests/ics_reader.rs`: `hostile_bytes_never_panic_and_never_leak_control_characters`, `a_giant_line_is_dropped_once_and_the_next_line_survives` |
| Despejo de linhas não enche a memória nem perde linha | `ics.rs`: `a_slow_reader_still_gets_every_line_in_order` | `a_flooding_server_cannot_fill_the_memory_while_nobody_reads`; `crates/chess-providers/tests/fics.rs`: `a_flood_of_lines_is_handled_a_slice_per_poll_without_loss` |
| Recusa de conexão marca o Provedor; soltar o `IcsLink` fecha a conexão | `ics.rs`: `dropping_the_link_closes_the_connection` | `a_refused_connection_marks_the_provider_failing` |
| Senha nunca sai por canal sem cifra; servidor que pede senha não recebe | `crates/chess-providers/tests/fics.rs`: `a_full_guest_game_with_repeated_late_and_cut_boards` (convidado) | `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves`, `a_server_asking_for_a_password_gets_none_and_the_connection_closes` |
| Variantes recusadas antes da rede; login com prazo; observar não trava | `fics.rs`: `observing_a_game_follows_it_without_storing_or_locking_anything` | `variants_are_refused_before_the_network_and_never_become_a_game`, `the_login_has_a_deadline_on_the_injected_clock`, `losing_the_connection_mid_game_loses_the_unconfirmed_move_and_keeps_the_guard_until_dropped` |

## Engine online

A Engine online (Lichess Board/Bot API) passa pela Trava (linhas `Computation::OnlineEngine` acima) e por estes ramos do adaptador (`crates/chess-providers/tests/engine_online.rs`) e do serviço (`crates/chess-studio/src/engine_online/tests.rs`).

| Ramo | Positivo | Negativo |
|---|---|---|
| Lance da Engine validado pelo núcleo de regras antes de enviar (ADR-0003) | `engine_online.rs`: `full_game_of_the_online_engine_is_driven_validated_and_sent_once_per_move` | `illegal_engine_move_is_refused_by_the_rules_core_and_nothing_is_sent`, `one_illegal_move_then_a_legal_one_keeps_the_game_going`; `engine_online/tests.rs`: `an_illegal_engine_move_is_refused_by_the_rules_core_and_never_sent` |
| Falha da Engine abandona em vez de deixar o relógio correr | `engine_online.rs`: `full_game_of_the_online_engine_is_driven_validated_and_sent_once_per_move` | `engine_failure_resigns_by_default_instead_of_letting_the_clock_run`; `engine_online/tests.rs`: `an_engine_that_does_not_start_resigns_instead_of_letting_the_clock_run` |
| Só a Conexão de Engine joga como Engine; só a de jogador joga como pessoa | `engine_online.rs`: `player_connection_does_not_play_as_engine_and_engine_connection_does_not_play_as_person` | `a_person_cannot_play_through_an_engine_session`, `a_game_of_a_person_never_resumes_as_engine_game_and_vice_versa`; `crates/chess-studio/src/online/tests.rs`: `only_the_authorized_player_connection_plays_never_the_engine_one` |
| Desafios decididos pela política (variante, ritmo, nível, adversário, limite) | `engine_online.rs`: `challenge_policy_decides_by_variant_pace_rating_opponent_and_concurrency` | `declined_challenge_tells_the_provider_why`; `engine_online/tests.rs`: `challenges_outside_the_users_limits_are_declined_with_the_reason`, `the_games_limit_stops_accepting_and_the_user_chose_it` |
| Nada sai sem ligar, sem Conexão e sem Provedor; envio que cai não se repete; conversão para bot é explícita | `engine_online/tests.rs`: `it_accepts_a_challenge_plays_validated_moves_and_stores_the_game` | `nothing_is_requested_until_the_user_turns_it_on`, `it_needs_the_engine_connection_and_the_provider_before_any_request`, `a_provider_turned_off_while_running_sends_nothing_more`; `engine_online.rs`: `a_dropped_send_is_never_repeated_blindly_and_the_provider_state_decides`, `upgrading_the_account_to_bot_is_explicit_and_never_automatic`; `engine_online/tests.rs`: `converting_to_bot_needs_the_confirmation_and_is_sent_once` |

## Filtros de entrada

| Ramo | Positivo | Negativo |
|---|---|---|
| Linha ICS: controle e ANSI removidos, negociação telnet some | `crates/chess-net/tests/ics_reader.rs`: `clean_lines_come_back_exactly_with_the_fics_line_ending`, `how_the_bytes_are_split_does_not_change_the_lines` | `hostile_bytes_never_panic_and_never_leak_control_characters`, `telnet_negotiation_anywhere_is_removed_without_touching_the_text` |
| NDJSON: linha acima do limite, UTF-8 inválido, fim truncado | `crates/chess-net/tests/ndjson.rs`: `lines_split_across_reads_are_joined`, `empty_lines_are_heartbeats_and_crlf_is_accepted` | `a_line_above_the_limit_is_reported_and_the_reader_resyncs`, `invalid_utf8_is_an_error_not_a_panic`, `a_clean_end_of_stream_and_a_truncated_one_are_told_apart` |
| Fluxo NDJSON obedece ao mesmo gate; fechar para a thread | `ndjson.rs`: `a_stream_delivers_lines_and_then_ends` | `a_stream_obeys_the_same_gate_as_any_request`, `closing_a_stream_stops_its_thread` |
| Opção UCI com controle ou quebra de linha não vira outro comando | `crates/chess-uci/tests/manager.rs`: `configured_options_reach_the_engine_and_hostile_ones_never_do` | `values_cannot_inject_uci_commands`; `crates/chess-uci/tests/adapter.rs`: `set_option_refuses_names_and_values_that_would_inject_another_command` **(novo)** |
| Bomba de descompressão nos problemas do Lichess | `crates/chess-training/tests/lichess.rs`: `a_long_line_below_the_limit_is_just_an_invalid_row` | `a_giant_line_from_a_decompression_bomb_stops_the_import_without_buffering_it`; `crates/chess-providers/tests/puzzles.rs`: `bomba_de_descompressao_falha_sem_encher_a_memoria_nem_instalar` |
| Identificador e apelido de Conexão sem espaço nem controle | `crates/chess-store/tests/connections.rs` (nomes válidos guardados) | `InvalidConnection` em `connections.rs` (linhas 137 e 144) e `authorized_connections.rs` (232, 243, 252) |
| Endereço aberto no navegador: só HTTPS puro; página de autorização do Lichess exata | `crates/chess-studio/src/connections/browser.rs`: `the_real_lichess_authorization_page_is_accepted_by_an_explicit_rule`, `every_catalog_page_can_be_opened` | `only_plain_https_addresses_are_opened`, `any_other_authorization_address_is_refused` |
| Rede só nas camadas de rede; soquete cru só no servidor falso, no loopback e no ICS | `crates/chess-studio/tests/no_network_without_consent.rs`: `only_the_engine_installer_and_the_provider_layer_speak_http` | `raw_sockets_exist_only_in_the_fake_server_the_oauth_loopback_and_the_ics_transport`, `network_cargo_dependencies_exist_only_in_the_two_network_crates`, `inside_the_provider_layer_only_the_gate_reaches_the_transports` |

## Achados fora desta matriz

Abertos como issues próprias, ainda sem teste por serem correção de produção (não de cobertura): [#278](https://github.com/ChicoFigueiredo/Chess-Studio/issues/278) a [#282](https://github.com/ChicoFigueiredo/Chess-Studio/issues/282) e [#285](https://github.com/ChicoFigueiredo/Chess-Studio/issues/285) (revisão de segurança). Ao corrigir uma delas, a linha correspondente entra aqui.

## Cobertura

`cargo llvm-cov --version` não existe na máquina (`error: no such command: llvm-cov`). A instrução do ticket é não instalar nada, então a **medição de linhas ficou pendente**: `fair_play.rs`, `crates/chess-net/src/net*.rs` e `chess-vault` não têm percentual registrado. Para medir: `cargo install cargo-llvm-cov --locked` (MIT OR Apache-2.0) e rodar `cargo llvm-cov -p chess-net -p chess-vault -p chess-providers` pela `cargo-fila`, sem CI. Até lá, a evidência de NFR-10 sobre ramos é esta matriz: cada ramo de segurança e fair play acima tem um teste positivo e um negativo nomeados, e a ausência de linha para uma `Computation` nova derruba a suíte.
