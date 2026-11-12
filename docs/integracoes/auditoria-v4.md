# Auditoria final do V4 contra a especificação free-first (#258)

Épico [#211](https://github.com/ChicoFigueiredo/Chess-Studio/issues/211). Auditoria de 11/11/2026 do `main` no commit `c723e7f` contra a [especificação free-first](especificacao-free-first-2026-11-11.md), as [decisões de 11/11](../decisoes/) (CX-01 a CX-21, CF, AZ, PO, FC, AD, RD, FD), a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md) e a [ADR-0007](../adr/0007-partida-online-controlador-segue-o-provedor.md).

## Situação depois das correções

Atualizada em 11/11/2026, no commit `02cd92a` da `main`. **O corpo desta auditoria abaixo é o original, do commit `c723e7f`, e não foi reescrito**: os vereditos FALHOU dele descrevem o que foi achado, não o que existe hoje. Esta tabela diz o que aconteceu com cada uma das 10 issues de falha.

| Issue | Falha | Situação | PR |
|---|---|---|---|
| [#261](https://github.com/ChicoFigueiredo/Chess-Studio/issues/261) | Trava de fair play: Engine adversária, busca por Posição e Engine online não consultavam a Trava (crítica) | Corrigida. Os três caminhos pedem licença à Trava (FP-01 a FP-05); a Engine adversária da própria Partida competitiva local é a exceção deliberada | [#286](https://github.com/ChicoFigueiredo/Chess-Studio/pull/286) |
| [#262](https://github.com/ChicoFigueiredo/Chess-Studio/issues/262) | Engine online entregue, mas catálogo "planejada" e consentimento sem a frase dela | Corrigida, junto com as lacunas do #271 (aviso ao fechar, Ação no Ribbon e na Paleta, força, retomada) | [#284](https://github.com/ChicoFigueiredo/Chess-Studio/pull/284) |
| [#263](https://github.com/ChicoFigueiredo/Chess-Studio/issues/263) | "conta", "chaveiro" e código HTTP na interface | Corrigida; o teste de palavras proibidas cobre todos os textos das integrações | [#288](https://github.com/ChicoFigueiredo/Chess-Studio/pull/288) |
| [#264](https://github.com/ChicoFigueiredo/Chess-Studio/issues/264) | Origem sem licença (Central, rating FIDE, Engine remota) | Corrigida: **Licença dos dados** na Central, licença do rating FIDE (migração 18) e Origem na Engine remota | [#288](https://github.com/ChicoFigueiredo/Chess-Studio/pull/288) |
| [#265](https://github.com/ChicoFigueiredo/Chess-Studio/issues/265) | Explorador sem janela temporal nem aviso de amostra | Corrigida: Período, aviso de amostra e texto de privacidade | [#283](https://github.com/ChicoFigueiredo/Chess-Studio/pull/283) |
| [#266](https://github.com/ChicoFigueiredo/Chess-Studio/issues/266) | Central sem filtros Com cadastro, Manual e por estado | Corrigida | [#288](https://github.com/ChicoFigueiredo/Chess-Studio/pull/288) |
| [#267](https://github.com/ChicoFigueiredo/Chess-Studio/issues/267) | Partida online sem vínculo com a Conexão que a joga | **Aberta, em andamento**: será fechada pelo PR da Partida online ([#272](https://github.com/ChicoFigueiredo/Chess-Studio/issues/272)); hoje a Partida não guarda a Conexão | em andamento ([#272](https://github.com/ChicoFigueiredo/Chess-Studio/issues/272)) |
| [#268](https://github.com/ChicoFigueiredo/Chess-Studio/issues/268) | Documentação atrás do `main`; #247 sem item CX-HOM | Corrigida: estado atual, guia e roteiro refeitos; CX-HOM-15 corrigido, CX-HOM-75 novo para o #247, CX-HOM-54 declarado não usado e itens em ordem | este PR |
| [#269](https://github.com/ChicoFigueiredo/Chess-Studio/issues/269) | Sem medição de cobertura nem matriz de ramos | Corrigida em parte: matriz de ramos de segurança e fair play e testes dos ramos sem teste; a **medição de cobertura de linhas ficou pendente** (sem `cargo llvm-cov` na máquina) | [#294](https://github.com/ChicoFigueiredo/Chess-Studio/pull/294) |
| [#270](https://github.com/ChicoFigueiredo/Chess-Studio/issues/270) | Importação de PGN sem prévia, detecção de fonte e confirmação de direitos | Corrigida | [#289](https://github.com/ChicoFigueiredo/Chess-Studio/pull/289) |

Outros PRs que vieram depois da auditoria e mudam o que ela descreve:

- [#287](https://github.com/ChicoFigueiredo/Chess-Studio/pull/287), [#295](https://github.com/ChicoFigueiredo/Chess-Studio/pull/295) e [#297](https://github.com/ChicoFigueiredo/Chess-Studio/pull/297): revisão de segurança do V4 (SEC-01 a SEC-08 corrigidos). Continua aberta a SEC-09 ([#285](https://github.com/ChicoFigueiredo/Chess-Studio/issues/285), `cargo deny check advisories`).
- [#292](https://github.com/ChicoFigueiredo/Chess-Studio/pull/292): varredura visual do V4 em todos os Temas e tamanhos. [#291](https://github.com/ChicoFigueiredo/Chess-Studio/pull/291): prazos de 30 s nas esperas por evento dos testes.
- [#277](https://github.com/ChicoFigueiredo/Chess-Studio/pull/277): roteiro de usabilidade e dados de demonstração (`--demo-data`).
- Em andamento: Engine online visual ([#293](https://github.com/ChicoFigueiredo/Chess-Studio/issues/293)) e investigação do `stop` da análise ([#296](https://github.com/ChicoFigueiredo/Chess-Studio/issues/296)).

O gate final (§15.1 e §13 da especificação) deixou de ser reprovado pela brecha da Trava (#261). Falta fechar o #267 e percorrer o [roteiro de homologação](../homologacao/integracoes.md) com o serviço real: os itens UNVERIFIED continuam UNVERIFIED.

## Resumo

| Veredito | Itens |
|---|---|
| ACEITO | 185 |
| ACEITO (UNVERIFIED com serviço real) | 125 |
| FALHOU | 32 (em 10 issues) |
| ADIADO | 15 |
| **Total** | **357** |

**O V4 não passa no gate final** (§15.1 e §13 da especificação): a Trava de fair play tem brecha (#261), e qualquer falha crítica de fair play reprova a entrega. O resto do que falhou é de transparência e acabamento: Origem e licença (#264), consentimento da Engine online (#262), textos fora do GLOSSARY (#263), Explorador (#265), filtros da Central (#266), Partida online sem vínculo com a Conexão (#267), documentação (#268), cobertura (#269) e prévia da importação (#270).

Os 125 itens UNVERIFIED só fecham com o [roteiro de homologação](../homologacao/integracoes.md) (CX-HOM-01 a 60): a suíte nunca fala com o Lichess, o Chess.com, o PlayStrategy, o Chess-API.com, o FICS nem com o cofre real do sistema (CX-17).

| Issue | FALHOU | Gravidade |
|---|---|---|
| [#261](https://github.com/ChicoFigueiredo/Chess-Studio/issues/261) | Trava de fair play: Engine adversária local, busca por Posição da Biblioteca e Engine online não consultam a Trava | Crítica (reprova o gate) |
| [#262](https://github.com/ChicoFigueiredo/Chess-Studio/issues/262) | Engine online entregue, mas o catálogo a mantém planejada: consentimento sem a frase dela e Central a lista em "O que vem depois" | Alta (consentimento incompleto) |
| [#264](https://github.com/ChicoFigueiredo/Chess-Studio/issues/264) | Origem sem licença: a Central não mostra licença nem restrições; rating FIDE e Engine remota sem licença | Média |
| [#267](https://github.com/ChicoFigueiredo/Chess-Studio/issues/267) | Partida online não guarda a Conexão que a joga; trocar a Conexão padrão não é recusado durante a Partida | Média |
| [#263](https://github.com/ChicoFigueiredo/Chess-Studio/issues/263) | "conta", "chaveiro" e código HTTP na interface | Baixa |
| [#265](https://github.com/ChicoFigueiredo/Chess-Studio/issues/265) | Explorador sem janela temporal nem aviso de amostra; "nenhum dado seu" impreciso | Baixa |
| [#266](https://github.com/ChicoFigueiredo/Chess-Studio/issues/266) | Central sem filtros "Com cadastro", "Manual" e por estado | Baixa |
| [#268](https://github.com/ChicoFigueiredo/Chess-Studio/issues/268) | Estado atual, guia e roteiro atrás do `main`; #247 sem item CX-HOM | Baixa |
| [#269](https://github.com/ChicoFigueiredo/Chess-Studio/issues/269) | Sem medição de cobertura nem matriz de ramos de segurança e fair play | Baixa |
| [#270](https://github.com/ChicoFigueiredo/Chess-Studio/issues/270) | Importação de PGN sem prévia, detecção de fonte e confirmação de direitos | Baixa |

ADIADO (15): Partida online e Engine online no PlayStrategy (#253), conta registrada no FICS (FC-01, CX-18), chat (#233), Chess.com interativo e os Provedores pagos, não confirmados ou sem API (StockfishAPI, Stockfish.online, Chessigma, ChessBase pagos, PGNBase/ChessTempo, Chessify, ChessUrge, PyChess Variants, ICC).

## Como foi feito

- Leitura do código e dos testes no `main`, não do [estado atual](estado-atual.md) nem dos relatórios de PR (o estado atual está atrás do `main`: ver #268). Cada evidência aponta o arquivo e, quando há, o nome do teste. Os nomes de teste foram conferidos nos arquivos; a suíte, o `clippy` e o `cargo deny` **não** foram rodados (tarefa só de leitura).
- Capturas citadas foram abertas e conferidas.
- O #252 (Engine remota e Engine online na interface, PR #259, `55d414a`) e o #254 (FICS como visitante na interface, PR #260, `c723e7f`) entraram no `main` durante a auditoria e foram auditados como os demais. Nenhum item ficou "em andamento".
- Quando vários itens falham pela mesma causa, eles apontam a mesma issue.

### Legenda

| Veredito | Quer dizer |
|---|---|
| ACEITO | Implementado e com evidência na suíte, no código ou em captura. |
| ACEITO (UNVERIFIED com serviço real) | Implementado e testado só com servidor falso, cofre falso ou relógio falso. A coluna diz o que conferir e o item do [roteiro](../homologacao/integracoes.md) (CX-HOM). Três itens (NFR-01, R-4 e G-7) estão aqui por outro motivo: dependem de rodar a suíte, o `clippy` e o `cargo deny`, que esta auditoria não rodou. |
| FALHOU (#n) | Prometido ou decidido e não cumpre, ou cumpre errado. A issue tem os passos. |
| ADIADO | Fora desta entrega por decisão registrada (citada) ou por falta de API oficial. |

Itens que só valem para servidor ou web (o app não tem servidor) estão como ACEITO com "não se aplica".

## 1. Verificações especiais

| Id | Verificação | Veredito | Evidência ou motivo |
|---|---|---|---|
| (a) | Offline por padrão; nenhum pedido sem consentimento por Provedor, inclusive ao abrir o app | ACEITO (UNVERIFIED com serviço real) | Gate checa o consentimento antes de tudo (`crates/chess-net/src/net.rs`, `admit`); `consent.rs`: `every_provider_is_born_disabled_and_the_app_starts_offline`, `disabled_provider_opens_no_connection_and_says_which_provider_to_enable`; ICS: `ics.rs` `a_disabled_provider_opens_no_ics_connection` e, na tela, `online/fics_tests.rs` `with_the_fics_off_no_connection_is_ever_opened`; teste estático `crates/chess-studio/tests/no_network_without_consent.rs`. Ao abrir: PO-18 (`online/tests.rs` `correspondence_survives_closing_and_reopening_the_app_and_continues_by_the_snapshot`), Engine online parada (`engine_online/tests.rs` `nothing_is_requested_until_the_user_turns_it_on`), Engine remota volta à local (`remote_engine.rs`). Conferir com monitor de rede: CX-HOM-01, CX-HOM-03 e CX-HOM-55. O texto do consentimento não cita a Engine online: #262. |
| (b) | Trava de fair play cobre tudo o que calcula, em todos os serviços, sem brecha | FALHOU (#261) | A Engine adversária local (`EnginePlayer`), a busca por Posição da Biblioteca e a Engine online não consultam `fair_play::FairPlay::check`. "Contra a Engine" fica habilitado durante a Partida online em tempo real (`docs/evidencias/partida-online/escuro/playing.png`). |
| (c) | Segredos só no Cofre, nunca em log, banco ou captura | ACEITO (UNVERIFIED com serviço real) | Banco sem segredo (`crates/chess-store/src/migrations.rs`, V16); varredura do diretório de dados atrás do token (`crates/chess-providers/tests/auth.rs` `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it`; `crates/chess-vault/tests/vault.rs` e `own_vault.rs`); log de operações sem segredo (`crates/chess-net/tests/gate.rs` `no_secret_shows_up_in_debug_errors_or_the_operation_log`); só o Cofre e o cabeçalho leem o segredo (`crates/chess-vault/tests/static_secrets.rs`); capturas com cofre falso (`net_service.rs`, `NetService::in_memory`). Conferir no cofre real do sistema: CX-HOM-07 e CX-HOM-08. |
| (d) | Sem scraping; sem pedir senha de Provedor sem API oficial | ACEITO | Só endereços de API do catálogo (`crates/chess-net/src/catalog.rs`; `gate.rs` `an_address_outside_the_catalog_is_refused_before_the_network`); importação manual sem endereço (`catalog.rs` `manual_import_providers_have_no_network_endpoint`); nenhum serviço manual declara senha (`manual_import_or_approval_never_declares_a_password`); campo de segredo só na chave pessoal do Lichess (`connections/model.rs` `the_secret_field_exists_only_in_the_lichess_key_form`); FICS recusa senha (`crates/chess-providers/tests/fics.rs` `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves`) e a tela não tem campo de senha (`online/fics_tests.rs` `the_fics_screen_draws_every_state_in_plain_language_with_no_password_field`). |
| (e) | Licenças e Origem dos dados exibidas | FALHOU (#264) | Explorador, avaliação em cache, Biblioteca e pacotes mostram a Origem com licença. Mas a Central não mostra a licença dos dados nem as restrições de nenhum Provedor (`data_license` e `restrictions` sem uso na interface), o rating FIDE não guarda nem mostra a licença e a Engine remota só mostra o selo do Provedor. |
| (f) | Termos do GLOSSARY e ausência de jargão | FALHOU (#263) | "conta" em `engineonline.ftl` (7 textos) e `analysis.ftl`; "chaveiro" em `authorization.ftl`; código HTTP em `training-download-error-http`. O teste de palavras proibidas só lê `connections.ftl` e `authorization.ftl`. Captura: `docs/evidencias/engine-remota-online/escuro/engine-online-convert.png` ("com a sua conta de Engine"). |
| (g) | Item CX-HOM para cada funcionalidade entregue | FALHOU (#268) | O #247 (Partida de referência exata, hora local, nomes de abertura em português) não tem item; o CX-HOM-15 descreve o comportamento antigo ("abre a busca na Biblioteca"). Os demais entregues têm item (CX-HOM-01 a 53 e 55 a 60, FICS incluído); a numeração pula o CX-HOM-54. |

## 2. Provedores (especificação §2.1, §3 e §4.2)

| Id | Provedor ou recurso | Veredito | Evidência ou motivo |
|---|---|---|---|
| P-01 | Lichess Board API (jogo humano) | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/board/`; `board_session.rs`, `board_actions.rs`; app em `crates/chess-studio/src/online/` (`online/tests.rs`). Conferir com o Lichess real: CX-HOM-35 a 42. |
| P-02 | Lichess Bot API (Engine online) | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/bot.rs`; `engine_online.rs`; tela em `crates/chess-studio/src/engine_online/` (`engine_online/tests.rs`). Conferir com um usuário BOT de teste: CX-HOM-52 e 53. Catálogo ainda a dá como planejada: #262. |
| P-03 | Lichess Opening Explorer | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/explorer.rs`; `explorer.rs` (testes); Painel em `crates/chess-studio/src/explorer/`. Exige Conexão (CX-21). CX-HOM-15 a 17. |
| P-04 | Lichess Cloud Eval | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/eval.rs`; `lichess_queries.rs` `cloud_eval_404_is_a_valid_no_evaluation_remembered_for_a_short_time`. CX-HOM-19. |
| P-05 | Lichess Tablebase | ACEITO (UNVERIFIED com serviço real) | `tablebase.rs`; `lichess_queries.rs` `tablebase_with_eight_pieces_creates_no_request`. CX-HOM-18. |
| P-06 | Lichess DB, problemas e aberturas CC0 | ACEITO (UNVERIFIED com serviço real) | Problemas por rede (`crates/chess-providers/src/puzzles.rs`; `puzzles.rs` testes; `crates/chess-net/tests/download.rs`); aberturas CC0 embutidas (`crates/chess-openings`, `crates/chess-studio/assets/LICENSES.md`). A base de Partidas não é baixada (especificação §1.3). CX-HOM-27, 28 e 44. |
| P-07 | PlayStrategy: perfis e Partidas públicas | ACEITO (UNVERIFIED com serviço real) | `ProviderProfile::PLAYSTRATEGY` (`profile.rs`); `playstrategy.rs` (testes); importação em `online_import.rs`. Formatos sintéticos (AD, "Formatos conferidos"). CX-HOM-46 a 48. |
| P-08 | PlayStrategy: Board e Bot | ADIADO | Issue #253: "Sem Partida online no PlayStrategy nesta etapa". O adaptador existe e é testado com servidor falso (`playstrategy.rs` `simulated_board_game_plays_one_move_through_the_playstrategy_board_api`). Não há issue de seguimento (ver Observações). |
| P-09 | FICS (visitante) | ACEITO (UNVERIFIED com serviço real) | Transporte e adaptador (`crates/chess-net/src/net/ics.rs`, `crates/chess-providers/src/fics/`; `fics.rs`, `ics.rs`, `ics_reader.rs`); tela e Central (`crates/chess-studio/src/online/fics.rs`, `fics_view.rs`, `fics_tests.rs`; `connections/model.rs` `the_fics_card_plays_as_a_guest_never_asks_for_a_password_and_warns_of_plain_text`); capturas `docs/evidencias/fics-visitante/`. Nenhuma conexão real foi aberta e as mensagens do servidor são sintéticas. CX-HOM-55 a 60. |
| P-10 | Chess.com PubAPI | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/chesscom.rs` (só leitura); `games.rs` `chesscom_import_goes_end_to_end_with_origin_and_does_not_duplicate`, `chesscom_revalidates_with_etag_after_the_cache_expires`. CX-HOM-23 e 24. |
| P-11 | Chess.com jogo/OAuth | ADIADO | NEEDS_APPROVAL (especificação §3.5): cartão "Requer aprovação", sem botão de jogar (`catalog.rs` `chess_com_reads_but_playing_needs_approval`). |
| P-12 | Chess-API.com (Engine remota) | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/remote.rs`; `chess_api.rs`; interface `crates/chess-studio/src/remote_engine.rs` e `remote_engine_tests.rs`. CX-HOM-49 a 51. |
| P-13 | StockfishAPI.com | ADIADO | Documentação diverge sobre cartão e excedentes: a especificação §12 manda manter bloqueado; `NotConfirmed`, sem endereço. |
| P-14 | Stockfish.online | ADIADO | Termos, cotas e esquema a validar (§3.8); `NotConfirmed`, sem endereço. |
| P-15 | Chessigma | ADIADO | P2 e fora da lista da CX-04; host da API não confirmado (`catalog.rs`, sem endereço; cartão "Em breve"). |
| P-16 | Queen Alice | ACEITO | Importação manual e nome público sem senha (`catalog.rs`; `connections/model.rs` `linking_a_public_name_is_offered_exactly_where_the_issue_lists_it`); fonte na importação (`library_ui.rs`). CX-HOM-04 e 25. |
| P-17 | ChessBase (conta e web) | ACEITO | Igual ao Queen Alice: importação manual e apelido, sem senha. CX-HOM-25. |
| P-18 | ChessBase pagos | ADIADO | PAID_OPTIONAL e NEEDS_APPROVAL (§12): serviços sem rede no catálogo. |
| P-19 | TWIC | ACEITO | Importação manual com nota "uso pessoal" (`library.ftl` `library-license-twic-personal`). CX-HOM-25. |
| P-20 | PGN Mentor | ACEITO | Importação manual com nota de licença (`library-license-pgn-mentor-free`). CX-HOM-25. |
| P-21 | FIDE (ID público, rating via Lichess) | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/profile.rs`; `lichess_queries.rs` `fide_returns_standard_rapid_and_blitz_separately_with_origin`; `fide_profile.rs`. CX-HOM-32. Licença do dado: #264. |
| P-22 | PGNBase e ChessTempo | ADIADO | NOT_CONFIRMED: cartões informativos, sem scraping. |
| P-23 | Chessify | ADIADO | NOT_CONFIRMED: plano no site não é API. |
| P-24 | ChessUrge | ADIADO | NOT_CONFIRMED: beta. |
| P-25 | PyChess Variants | ADIADO | NOT_CONFIRMED; variantes que o núcleo não conhece. |
| P-26 | ICC | ADIADO | PAID_OPTIONAL. |

## 3. Central de Conexões (especificação §4.1, §4.5 e §4.6)

### 3.1 Experiência obrigatória (§4.1)

| Id | Requisito | Veredito | Evidência ou motivo |
|---|---|---|---|
| C-01 | Provedor e categoria | ACEITO | `connections/model.rs` `groups`, `category_key`; captura `docs/evidencias/central-conexoes/`. |
| C-02 | O que o cliente faz agora, sem prometer o futuro | FALHOU (#262) | `face`/`later` vêm do catálogo; o Lichess lista "Engine online" em "O que vem depois" embora a tela funcione (serviço ainda `Planned`). |
| C-03 | Método real de identificação | ACEITO | `auth_keys`; captura `docs/evidencias/acabamento-integracoes/central-escuro-detalhes/central.png` ("Como se identifica"). |
| C-04 | Custo e elegibilidade, gratuitos primeiro | ACEITO | `free_providers_come_first_inside_every_tier_of_every_category`; `a_provider_with_services_of_different_costs_shows_each_cost_never_a_contradiction`. |
| C-05 | Estado da Conexão | ACEITO | `CardState` (desligado, ligado, nome vinculado, autorizado, expirado, bloqueado, limitado, com falha, importação manual, requer aprovação, em breve, em avaliação); `card_states_follow_the_catalog_and_the_network_state`. |
| C-06 | Permissões concedidas e modo | ACEITO (UNVERIFIED com serviço real) | Permissões lidas do Provedor e mostradas em português (`connections/auth_tests.rs` `permissions_read_in_plain_portuguese_and_extra_ones_are_flagged`). Conferir a lista real: CX-HOM-09. |
| C-07 | Última verificação, diagnóstico, cota e custo | ACEITO | "Última consulta", "Testado às HH:MM" (`connections/probe.rs`; `model.rs` `age_buckets_remaining_minutes_and_last_query`); cota só quando o Provedor informa (nenhum informa). |
| C-08 | Ações adequadas ao método | ACEITO | `card_actions`; `a_card_has_one_primary_action_and_the_rest_in_the_menu`; `authorizing_is_the_lichess_primary_action_and_other_providers_keep_it_disabled`. |
| C-09 | Várias identidades por Provedor; impedir troca silenciosa da identidade numa Partida | FALHOU (#267) | Várias Conexões e padrão existem (`crates/chess-store/tests/connections.rs`), mas a Partida online não guarda a Conexão e "Tornar padrão" não é recusado durante ela. |
| C-10 | Segredo nunca em texto, diálogo, URL, log ou exportação | ACEITO | `auth_tests.rs` `the_master_password_and_the_key_never_appear_in_debug_or_in_the_vault_dialog_text`; `gate.rs` `authorization_and_cookies_cannot_be_set_by_hand`; `cs_secret_field` mascarado (`ui_theme/widgets.rs`). |

### 3.2 Histórias da Central (§4.5)

| Id | História | Veredito | Evidência ou motivo |
|---|---|---|---|
| US-CRED-01 | Descobrir Provedores gratuitos | FALHOU (#266) | Ordem e cartões certos (C-04), mas só os filtros Todos, Gratuitos e Pagos; faltam "Com cadastro", "Manual" e o filtro por estado. Teclado sem evidência de captura (CX-HOM-30). |
| US-CRED-02 | Autorizar o Lichess com segurança | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/tests/auth.rs` (sucesso, `state` errado, recusa, código reutilizado, prazo, escopo errado, cofre); `auth_tests.rs` (fluxo pela Central). CX-HOM-09 a 13. |
| US-CRED-03 | Identidades públicas sem senha | ACEITO (UNVERIFIED com serviço real) | `manual_and_approval_cards_never_show_a_secret_field_or_an_authorize_action`; Chess.com só em `api.chess.com/pub/`; importação manual sem rede. Conferir Chess.com e FIDE reais: CX-HOM-04 e 05. |
| US-CRED-04 | FICS com isolamento de segredo | ACEITO (UNVERIFIED com serviço real) | Só visitante, sem segredo e sem campo de senha (`fics.rs`; `fics_tests.rs` `the_fics_screen_draws_every_state_in_plain_language_with_no_password_field`); aviso de texto puro na Central e no consentimento (FC-14); prazo, queda, entrada hostil e servidor que pede senha testados no adaptador. Conferir com o FICS real: CX-HOM-55 e 56. |
| US-CRED-05 | Gerenciar e revogar | FALHOU (#267) | Revogação local e remota, dados preservados e expiração ok (`auth_tests.rs` `disconnecting_revokes_at_the_provider_deletes_the_secret_and_removes_the_connection`, `when_the_remote_revocation_fails_the_local_secret_is_still_deleted_and_the_user_is_told`); falta recusar a troca durante Partida ativa e o teste com duas Conexões. |
| US-CRED-06 | Zero cobrança silenciosa | ACEITO | Nenhum Provedor pago habilitado (catálogo); `429` pausa (`crates/chess-net/tests/limits.rs`); nenhuma chave de API. |

### 3.3 Critérios integrados (§4.6)

| Id | Critério | Veredito | Evidência ou motivo |
|---|---|---|---|
| K-01 | Catálogo lista todos, inclusive manuais e sob aprovação | ACEITO | `crates/chess-net/tests/catalog.rs` `every_provider_of_the_matrix_is_in_the_catalog`. |
| K-02 | 100% dos segredos fora de preferências, SQLite, PGN, logs e backup | ACEITO (UNVERIFIED com serviço real) | Ver (c). CX-HOM-07 e 08. |
| K-03 | Inspeção automatizada: zero tokens em claro | ACEITO | Varredura do diretório de dados (`chess-providers/tests/auth.rs`), do Cofre (`chess-vault/tests/vault.rs`, `own_vault.rs`) e do log (`gate.rs`). |
| K-04 | OAuth: sucesso, cancelamento, revogação, escopo, retorno adulterado | ACEITO (UNVERIFIED com serviço real) | `auth.rs`; `auth_tests.rs` `a_forged_return_is_refused_and_nothing_is_stored`. CX-HOM-09, 10 e 12. |
| K-05 | Queen Alice, ChessBase, Chess.com e FIDE sem senha | ACEITO | Ver (d). |
| K-06 | FICS não envia senha sem canal protegido | ACEITO | `fics.rs` `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves`, `a_server_asking_for_a_password_gets_none_and_the_connection_closes` (FC-01); catálogo só com visitante (FC-14). |
| K-07 | Capacidade efetiva, motivo da indisponibilidade e último teste | ACEITO (UNVERIFIED com serviço real) | Cartão com estado, motivo das ações desabilitadas e "Testado às". Aprovação visual: CX-HOM-02 e 05. |
| K-08 | Remover credencial interrompe chamadas; Partida mostra transição e reautorizar | ACEITO (UNVERIFIED com serviço real) | `gate.rs` `a_401_or_403_with_the_token_marks_the_connection_and_is_typed`; `online.ftl` `online-error-rejected`. CX-HOM-12. |
| K-09 | Conta humana nunca promovida a BOT em silêncio | ACEITO (UNVERIFIED com serviço real) | AD-09; `engine_online.rs` `upgrading_the_account_to_bot_is_explicit_and_never_automatic`; `engine_online/tests.rs` `converting_to_bot_needs_the_confirmation_and_is_sent_once`. CX-HOM-53. |
| K-10 | FREE FIRST, sem assinatura | ACEITO | Catálogo; nenhum fluxo de compra. |

## 4. Segredos e Cofre (especificação §4.3)

| Id | Requisito | Veredito | Evidência ou motivo |
|---|---|---|---|
| S-01 | Referência opaca; banco sem segredo | ACEITO | `connections` só com `id` (migração 16); `index.json` sem segredo (`vault.rs` `with_a_system_vault_the_secret_goes_there_and_never_to_disk`). |
| S-02 | Cofre do sistema; sem ele, cofre próprio com desbloqueio; nunca texto puro | ACEITO (UNVERIFIED com serviço real) | `crates/chess-vault/src/keyring.rs`, `own.rs`; `own_vault.rs` `without_a_system_vault_nothing_is_stored_until_the_user_creates_the_own_vault`, `a_system_vault_that_appears_later_does_not_take_over_silently`. CX-HOM-07 e 08. |
| S-03 | Servidor ou web | ACEITO | Não se aplica: o app não tem servidor. |
| S-04 | Versão de chave, rotação, backup sem segredo | ACEITO | Formato versionado do `own.vault` (CF-03); `own_vault.rs` `changing_the_master_password`; o app não exporta configuração. |
| S-05 | OAuth pelo navegador, PKCE S256, `state`, retorno verificado | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/auth.rs`; `crates/chess-net/src/loopback.rs` (`loopback.rs` testes); `connections/browser.rs` `only_plain_https_addresses_are_opened`. CX-HOM-09 e 10. |
| S-06 | Chave pessoal mascarada, conferida, troca atômica | ACEITO | `cs_secret_field`; `vault.rs` `a_staged_secret_is_used_only_while_it_lives_and_replaces_only_on_commit`; `auth_tests.rs` `a_valid_personal_key_is_checked_then_stored_and_never_shown_again`. |
| S-07 | Sem reuso entre Provedores ou papéis; sem log do cabeçalho; sem área de transferência | ACEITO | `gate.rs` `a_token_of_one_provider_never_reaches_another_provider_even_from_a_faulty_vault`; `auth_tests.rs` `player_and_engine_authorizations_never_share_the_secret`; widget nunca copia. |
| S-08 | `401`/`403`: para, explica, reautoriza, preserva; revogação local e remota | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `a_connection_the_provider_rejects_becomes_expired_with_a_way_to_reauthorize_keeping_its_data`; `auth.rs` `revoking_deletes_locally_even_when_the_remote_call_fails_and_says_so`. CX-HOM-12 e 13. |
| S-09 | FICS: senha só com transporte cifrado comprovado | ACEITO | FC-01, FC-14 e a pesquisa em `docs/decisoes/2026-11-11-fics.md`; `fics.rs`; o catálogo não declara mais método com senha. |
| S-10 | Desbloqueio ao abrir; bloqueio | ACEITO | `own_vault.rs` `reopening_starts_locked_and_only_the_right_password_unlocks`, `locking_forgets_the_key_and_the_secrets`. |
| S-11 | Auditoria sanitizada | ACEITO | Diário de operações sem segredo (`crates/chess-net/src/status.rs`; `gate.rs` `no_secret_shows_up_in_debug_errors_or_the_operation_log`). |

## 5. Arquitetura, Trava de fair play e rede (especificação §5)

| Id | Requisito | Veredito | Evidência ou motivo |
|---|---|---|---|
| A-01 | Interface nunca guarda segredo | ACEITO | `static_secrets.rs` `only_the_vault_and_the_network_header_read_a_secret_in_clear`. |
| A-02 | Transporte separado do domínio | ACEITO | `chess-net` sem Bevy nem egui; `chess-game` sem rede; a sessão do Provedor é a fonte de comandos (ADR-0007). |
| A-03 | Engines não recebem eventos de Partida humana ativa | ACEITO | Nenhum fluxo de eventos chega a Engine; a brecha de repetir os Lances à mão está em #261. |
| A-04 | Resposta do Provedor não sobrescreve o estado sem verificação | ACEITO | PO-05; `online_game.rs` `retrato_divergente_e_erro_tipado_e_o_log_fica_intacto`; `online_property.rs`. |
| A-05 | Caches separados por dado, permissão e licença | ACEITO | Cache por Perfil local e pedido completo, licença do endereço (`cache.rs` `the_cache_belongs_to_the_local_profile`, `provenance_license_comes_from_the_catalog_endpoint`). |
| A-06 | Consultas de dados sem Conexão | ACEITO | Avaliação em cache, finais, perfis e Partidas públicas sem Conexão; Explorador exige por decisão (CX-21). |
| A-07 | Lance humano: valida, envia uma vez, espera confirmação | ACEITO (UNVERIFIED com serviço real) | PO-04 e PO-09; `board_session.rs` `queda_no_meio_do_envio_nao_repete_o_post_e_confere_o_estado_ao_reconectar`. CX-HOM-38. |
| A-08 | Reconexão: backoff, retrato, sem duplicata, fim enquanto desconectado | ACEITO (UNVERIFIED com serviço real) | `board_session.rs` `reconexao_com_backoff_registra_o_intervalo_e_detecta_fim_enquanto_desconectado`, `eventos_duplicados_e_fora_de_ordem_nao_alteram_nem_regridem`. CX-HOM-38 e 41. |
| A-09 | FICS: parser próprio, `style 12`, entradas hostis | ACEITO (UNVERIFIED com serviço real) | `crates/chess-providers/src/fics/parse.rs`, `tracker.rs`; `fics.rs` `hostile_input_neither_hangs_nor_breaks_the_session`. Transcrições sintéticas. CX-HOM-57 e 58. |
| A-10 | Normalização e Origem (§5.3) | ACEITO | `(Provedor, id remoto)` (`crates/chess-store/src/origin.rs`; `import_origin.rs` `the_same_game_from_two_providers_is_one_game_with_two_origins`); rating FIDE separado do online (FD-02); análise no ponto de vista das Brancas (`lichess_queries.rs`). O código de licença guarda o escopo de direitos; o app não compartilha dados. |
| F-01 | Trava bloqueia cálculo, sugestão, finais, avaliação em cache e automação de Lance | FALHOU (#261) | Coberto pela análise, dica, Explorador, finais, avaliação em cache, Engine remota e estatística da Biblioteca (`crates/chess-studio/src/fair_play.rs`; `online/tests.rs`). Fora da Trava: Engine adversária local, busca por Posição da Biblioteca e Engine online. |
| F-02 | Fila de análise isolada; nada de análise presa à Partida humana | ACEITO | `analysis.rs` (`lock_now`); `remote_engine_tests.rs` `the_lock_turning_on_after_an_answer_stops_further_requests`; `online/tests.rs` `the_engine_analysis_refuses_during_the_online_game_even_in_review`. |
| F-03 | Sem chave de depuração; validação na camada de serviço | ACEITO | Sem chave; `fair_play.rs` `computing_services_are_only_reachable_through_the_lock` (teste estático). A cobertura incompleta dos caminhos é F-01. |
| F-04 | Livros e aberturas em correspondência só onde as regras permitem | ACEITO | CX-08 é mais estrita: sem exceção para correspondência (`fair_play.rs` `correspondence_locks_its_current_position_and_what_is_near_it_anywhere`). |
| F-05 | Engine joga só por identidade de Engine autorizada, com etiqueta | ACEITO (UNVERIFIED com serviço real) | AD-06; `engine_online.rs` `player_connection_does_not_play_as_engine_and_engine_connection_does_not_play_as_person`. CX-HOM-52. |
| F-06 | Análise só depois do fim confirmado | ACEITO (UNVERIFIED com serviço real) | `board_session.rs` `abandonar_e_pedido_a_trava_so_cai_com_a_confirmacao`; `fair_play.rs` `the_lock_only_falls_with_the_confirmed_end`. CX-HOM-37 e 42. |
| N-01 | Cliente por Provedor: prazo, cancelamento, pausa, backoff, `Retry-After`, ETag, 304 | ACEITO | `limits.rs`, `timeout_cancel.rs`, `cache.rs` `an_etag_answered_with_304_renews_the_validity`. |
| N-02 | Fluxo NDJSON incremental, linha máxima, batimento, fim controlado | ACEITO | `ndjson.rs` (testes). |
| N-03 | Cache por Posição e filtros, com validade e Origem; dado privado não vaza | ACEITO | `cache.rs`; `explorer.rs` `masters_filters_go_in_the_request_and_the_second_call_hits_the_cache`. |
| N-04 | PGN em fluxo, `.gz`, encodings, arquivo suspeito | ACEITO | `import_origin.rs` `a_hundred_thousand_games_import_in_bounded_memory`, `gzip_pgn_imports`, `latin1_file_imports_with_accents_and_a_warning`, `a_gzip_bomb_stops_at_the_decompressed_limit`. |
| N-05 | Apagar vínculos, segredos e cache sem tocar os PGNs | ACEITO | FD-08 a FD-11 (`stored_data.rs`; `fide_and_bulk.rs`). |
| N-06 | Modo offline | ACEITO | `cache.rs` `offline_the_cache_comes_back_with_its_provenance_and_age`; o app abre sem Provedor (CX-01). |

## 6. Histórias US-001 a US-032 (especificação §7)

| Id | História | Veredito | Evidência ou motivo |
|---|---|---|---|
| US-001 | Inventariar o que existe | FALHOU (#268) | `estado-atual.md` vale para `1626ebb` e dá como AUSENTE Engine remota, download pela rede, seletor de arquivo, Engine online e PlayStrategy, que estão no `main`. |
| US-002 | Registro e roteamento por capacidade | ACEITO | `catalog.rs` (dados e testes); gate deriva a capacidade do endereço (`gate.rs` `each_request_is_classified_by_the_catalog_as_computing_or_not`); `consent.rs` `turning_a_provider_off_again_blocks_the_next_request`. |
| US-003 | Catálogo gratuito primeiro | FALHOU (#266) | CA-003.1 e 003.2 ok (C-04; `cost_key` com "custo não confirmado"); CA-003.3 sem filtro por estado nem "Com cadastro"/"Manual". |
| US-004 | Cofre por usuário e Provedor | ACEITO (UNVERIFIED com serviço real) | `vault.rs` `the_authorization_is_bound_to_the_provider_and_the_local_profile`, `a_tampered_index_cannot_rebind_a_secret_to_another_profile_or_provider`. CX-HOM-07 e 08. |
| US-005 | OAuth seguro no Lichess | ACEITO (UNVERIFIED com serviço real) | Ver US-CRED-02; desconectar registra local e remoto separados (`auth.rs`). CX-HOM-09 a 13. |
| US-006 | Identidades Chess.com, Queen Alice, ChessBase, FIDE | FALHOU (#264) | Sem senha e sem jogo (ok), mas CA-006.4 pede indicar o uso autorizado da FIDE: nem o cartão nem o rating mostram a licença. |
| US-007a | FICS como visitante | ACEITO (UNVERIFIED com serviço real) | Visitante sem Cofre (`fics.rs`; `fics_tests.rs` `a_guest_game_goes_from_the_seek_through_the_board_and_the_panel_into_the_library`); fragmentação e entrada hostil (`ics_reader.rs`, `fics.rs`); a tela explica por que não há senha. CX-HOM-55 e 56. |
| US-007b | FICS com conta registrada | ADIADO | FC-01 e CX-18: só visitante até haver canal cifrado comprovado (não há, ver `docs/decisoes/2026-11-11-fics.md`). |
| US-008 | Multicontas, permissões, estados, revogação | FALHOU (#267) | CA-008.2 e 008.3 ok (`authorized_connections.rs`; AZ-09); CA-008.1 sem associação da Partida à Conexão e sem teste com duas Conexões. |
| US-009 | Fluxo de eventos e desafios | ACEITO (UNVERIFIED com serviço real) | `board_actions.rs` `fluxo_de_eventos_traduz_desafios_e_partidas_e_reconecta_com_backoff`; `online/tests.rs` `received_challenges_are_declined_with_a_reason_or_accepted_and_the_game_starts`. CX-HOM-40. |
| US-010 | Partida humana de ponta a ponta | ACEITO (UNVERIFIED com serviço real) | `online/tests.rs` `a_full_game_from_seek_to_checkmate_through_the_board_and_into_the_library`, `only_the_authorized_player_connection_plays_never_the_engine_one`. CX-HOM-36 a 38. |
| US-011 | Reconectar e recuperar | ACEITO (UNVERIFIED com serviço real) | `online/tests.rs` `a_drop_reconnects_without_resending_and_the_result_that_came_meanwhile_shows`; a queda testada é de 30 s (`board_session.rs`); 5 s e 2 min ficam para o roteiro. CX-HOM-38 e 41. |
| US-012a | Empate, abandono, abortar, reclamar vitória | ACEITO (UNVERIFIED com serviço real) | `board_session.rs` `reclamar_vitoria_so_depois_do_prazo_do_adversario_ausente`; confirmação de abandono (`online/view.rs`). CX-HOM-39. |
| US-012b | Chat | ADIADO | "Fora deste ticket (#233): Chat" em `docs/decisoes/2026-11-11-partida-online.md`. |
| US-013 | Jogo humano no PlayStrategy | ADIADO | Issue #253 ("Sem Partida online no PlayStrategy nesta etapa"). |
| US-014 | Cliente FICS | ACEITO (UNVERIFIED com serviço real) | Busca, desafios, Lances, resultado, empate, abandono, abortar e observação (`fics_tests.rs` `draw_resign_and_abort_reach_the_fics_once_and_nothing_changes_until_it_confirms`, `observing_stays_in_memory_never_reaches_the_library_and_never_locks`); "Sair do FICS" fecha sem reiniciar o app (FC-17). Formato do Lance enviado não conferido no servidor real (riscos em `2026-11-11-fics.md`). CX-HOM-56 a 60. |
| US-015 | Conta BOT e Partida de Engine | ACEITO (UNVERIFIED com serviço real) | `engine_online.rs` `full_game_of_the_online_engine_is_driven_validated_and_sent_once_per_move`, `engine_failure_resigns_by_default_instead_of_letting_the_clock_run`; `engine_online/tests.rs`. CX-HOM-52 e 53. |
| US-016 | Interface uniforme para Engines UCI | ACEITO | `crates/chess-uci` (`analysis.rs`, `player.rs`, `adapter.rs` testes). |
| US-017 | Engine remota gratuita | ACEITO (UNVERIFIED com serviço real) | `chess_api.rs` `a_newer_request_replaces_the_older_one`, `rate_limited_waits_and_sends_nothing_before_the_deadline`; `remote_engine_tests.rs`. CX-HOM-49 e 50. |
| US-018 | PGN robusto | ACEITO | `crates/chess-pgn/tests/import.rs` (`modo_tolerante_pula_a_partida_ruim_e_importa_as_outras`, `nenhum_prefixo_truncado_do_corpus_causa_panic`), `roundtrip.rs`. |
| US-019 | Origem, direitos, duplicatas | ACEITO | `import_origin.rs` `reimporting_the_same_pgn_adds_nothing`, `provenance_carries_provider_mode_instant_license`, `same_names_but_different_games_are_not_merged`. |
| US-020 | Explorar aberturas por Posição | FALHOU (#265) | Fonte, filtros, total e "Sem dados" ok; faltam a janela temporal (CA-020.1, §3.2) e o aviso de que a amostra não é vantagem objetiva (CA-020.3). |
| US-021 | Nomes de abertura e ECO CC0 | ACEITO | `crates/chess-openings/tests/book.rs`, `names_pt_br.rs`; licença em `assets/LICENSES.md`. |
| US-022 | Pesquisa local | ACEITO | `crates/chess-store/tests/library.rs`; `import_origin.rs` `filters_by_provider_eco_and_pace_in_the_list_and_in_the_library`. |
| US-023 | Importar PGN de Chess.com, Queen Alice, ChessBase | FALHOU (#270) | Importação por fonte e pela API pública ok; CA-023.3 sem prévia e sem contagem "com licença restrita". |
| US-024 | Avaliação em cache | ACEITO (UNVERIFIED com serviço real) | `lichess_queries.rs` (200/404/falhas/Trava); linha "Em cache (Lichess)" com Origem (`explorer/view.rs`). CX-HOM-19. |
| US-025 | Finais exatos | ACEITO (UNVERIFIED com serviço real) | `lichess_queries.rs` `tablebase_returns_category_dtz_dtm_and_ordered_moves`; aba Finais separada da Engine. CX-HOM-18. |
| US-026 | Rating FIDE | FALHOU (#264) | Modalidades e data ok (FD-02, FD-03); CA-026.2 pede os direitos registrados: `fide_ratings` não guarda licença. |
| US-027 | Cota, gratuidade e fallback | ACEITO | `limits.rs` `retry_after_marks_the_provider_limited_and_nothing_is_sent_before_the_deadline`, `without_retry_after_the_pause_is_one_minute`; sem cadeia automática (CX-11). |
| US-028 | Fair play inviolável | FALHOU (#261) | Ver F-01. |
| US-029 | Offline, falhas e acessibilidade | ACEITO (UNVERIFIED com serviço real) | Offline e erros tipados (`status.rs`, `cache.rs`); teclado e leitor de tela sem evidência (o README de `docs/evidencias/acabamento-integracoes/` diz que o foco não foi capturado). Conferir: CX-HOM-01 e 30. |
| US-030 | Observabilidade sem vazamento | ACEITO | `status.rs` (diário); `gate.rs` `no_secret_shows_up_in_debug_errors_or_the_operation_log`. |
| US-031 | Testes de contrato | ACEITO | Respostas gravadas e sintéticas (`crates/chess-net/tests/recorded.rs`, `crates/chess-providers/tests/fixtures.rs` `every_fixture_parses_is_labelled_and_carries_no_cookie`). A CI está desligada; a suíte roda local. |
| US-032 | Parcerias futuras | ACEITO | Cartões "Requer aprovação"; nenhum endereço privado no catálogo. |

## 7. Fluxos (especificação §6)

| Id | Fluxo | Veredito | Evidência ou motivo |
|---|---|---|---|
| FL-6.1 | Jogar online no Lichess e no FICS | ACEITO (UNVERIFIED com serviço real) | Ver US-009 a US-012 e US-014. O PlayStrategy está em US-013 (ADIADO). CX-HOM-35 a 42 e 56 a 59. |
| FL-6.2 | Engine online | ACEITO (UNVERIFIED com serviço real) | Ver US-015. CX-HOM-52 e 53. |
| FL-6.3 | Biblioteca e aberturas | ACEITO (UNVERIFIED com serviço real) | Barras, total, Origem com data, Partida de referência (`explorer/reference.rs`), ECO CC0. CX-HOM-14 e 15. |
| FL-6.4 | Análise local, remota e finais | ACEITO (UNVERIFIED com serviço real) | Engine local, Engine remota explícita, avaliação em cache separada, finais. CX-HOM-18, 19, 49 e 50. |
| FL-6.5 | Importação e exportação de PGN | FALHOU (#270) | Sem prévia, sem detecção de fonte, sem confirmação de direitos. Exportação ok (RD-6). |
| FL-6.6 | Queen Alice como referência | ACEITO | Nome sem senha e importação de arquivo; nenhum tabuleiro sincronizado. |
| FL-6.7 | Pesquisa estilo ChessBase | ACEITO | Busca por jogador, evento, Posição, ECO e ritmo (`library.rs`). |
| FL-6.8 | Perfil e FIDE | ACEITO (UNVERIFIED com serviço real) | `fide_profile.rs`; ratings separados dos online. Licença do dado em #264. CX-HOM-32. |

## 8. Cenários BDD (especificação §8)

| Id | Cenário | Veredito | Evidência ou motivo |
|---|---|---|---|
| B-01 | OAuth com permissão mínima | ACEITO (UNVERIFIED com serviço real) | `auth.rs` `the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it`. CX-HOM-09. |
| B-02 | Token revogado | ACEITO (UNVERIFIED com serviço real) | `gate.rs` `a_401_or_403_with_the_token_marks_the_connection_and_is_typed`. CX-HOM-12. |
| B-03 | Vincular usuário Chess.com | ACEITO (UNVERIFIED com serviço real) | Só `api.chess.com/pub/`; sem jogar. CX-HOM-04, 05 e 23. |
| B-04 | Vincular Queen Alice | ACEITO | Nome e importação, sem senha, sem rede. |
| B-05 | Vincular jogador FIDE | ACEITO (UNVERIFIED com serviço real) | ID público, fonte e data. CX-HOM-32. |
| B-06 | Análise proibida na Partida humana | ACEITO (UNVERIFIED com serviço real) | Pedidos de avaliação recusados sem chamar Engine (`online/tests.rs` `cloud_eval_endgames_explorer_and_remote_engine_send_nothing`). Pedidos de Lance (Engine adversária) em #261. CX-HOM-42. |
| B-07 | Engine só na Conexão de Engine | ACEITO (UNVERIFIED com serviço real) | `engine_online.rs` `player_connection_does_not_play_as_engine_and_engine_connection_does_not_play_as_person`. CX-HOM-52. |
| B-08 | Timeout depois do Lance | ACEITO (UNVERIFIED com serviço real) | `board_session.rs` `queda_no_meio_do_envio_nao_repete_o_post_e_confere_o_estado_ao_reconectar`. CX-HOM-38. |
| B-09 | `429` | ACEITO | `limits.rs` `retry_after_marks_the_provider_limited_and_nothing_is_sent_before_the_deadline`. |
| B-10 | PGN com Partida inválida no meio | ACEITO | `import_origin.rs` `hostile_pgn_is_counted_as_errors_and_the_good_games_survive`, `reimporting_the_same_pgn_adds_nothing`. |

## 9. Requisitos não funcionais e segurança (especificação §9)

| Id | Requisito | Veredito | Evidência ou motivo |
|---|---|---|---|
| NFR-01 | Não regressão | ACEITO (UNVERIFIED com serviço real) | Suíte não rodada nesta auditoria. Conferir: `cargo test --workspace` pela fila. |
| NFR-02 | Fair play: todos os caminhos de Engine testados em modo humano | FALHOU (#261) | Caminhos sem teste e sem Trava (F-01). |
| NFR-03 | Zero segredo em claro | ACEITO (UNVERIFIED com serviço real) | Ver (c). CX-HOM-07 e 08. |
| NFR-04 | Sem Lance duplicado nem regressão sob falhas | ACEITO (UNVERIFIED com serviço real) | `online_property.rs` `qualquer_ordem_de_retratos_com_repeticao_converge_sem_regredir`; queda de 30 s. CX-HOM-38. |
| NFR-05 | Consumo de API: `429`, backoff, ETag | ACEITO | `limits.rs`, `cache.rs`. |
| NFR-06 | Tabuleiro sem bloqueio de rede, p95 < 100 ms | ACEITO (UNVERIFIED com serviço real) | Rede em threads, consulta por quadro (`timeout_cancel.rs` `polling_never_blocks`); tempos de quadro nas capturas (`desempenho.txt`, ~5 ms). p95 com rede real não medido. |
| NFR-07 | Reação ao fluxo, p95 < 500 ms | ACEITO (UNVERIFIED com serviço real) | Sessão lida a cada quadro (`online/mod.rs`, `drive_online`); sem trace com instantes. CX-HOM-37. |
| NFR-08 | PGN sem corrupção silenciosa | ACEITO | `chess-pgn` (import, roundtrip); `replay_property.rs`. |
| NFR-09 | Importação em fluxo, memória limitada | ACEITO | `import_origin.rs` `a_hundred_thousand_games_import_in_bounded_memory`. |
| NFR-10 | Cobertura ≥ 85% e 100% dos ramos de segurança e fair play mapeados | FALHOU (#269) | Sem medição de cobertura nem matriz de ramos; há ramos de fair play sem teste (#261). |
| NFR-11 | Acessibilidade | ACEITO (UNVERIFIED com serviço real) | Selos com ícone e palavra; foco de teclado sem captura. CX-HOM-30. |
| NFR-12 | Cofre e autorização testados em cada sistema | ACEITO (UNVERIFIED com serviço real) | Suíte com cofre falso; Windows e macOS sem execução (CI desligada). CX-HOM-07 a 09 em cada sistema. |
| NFR-13 | Desligar Provedor não impede estudo | ACEITO | `consent.rs`; `cache.rs` `the_cache_reads_offline_even_with_the_provider_turned_off`. |
| NFR-14 | Dados por usuário; apagar vínculo sem apagar PGN | ACEITO | `connections.rs` `connections_belong_to_the_local_profile`; FD-08 a FD-11. |
| NFR-15 | Custo R$ 0 por padrão | ACEITO | Catálogo: nenhum Provedor pago habilitado. |
| NFR-16 | Quebra de contrato vira erro controlado | ACEITO | `chess_api.rs` `malformed_answer_becomes_a_typed_problem_with_a_user_message`; `explorer.rs` `explorer_rate_limit_malformed_body_and_bad_fen`. |
| SEG-1 | Token nunca em URL, análise ou captura | ACEITO | Só cabeçalho (`gate.rs` `the_authorization_goes_only_to_the_authenticated_endpoints_of_its_provider`); capturas com cofre falso. |
| SEG-2 | HTTPS com certificado; sem `http` para segredo | ACEITO | `request.rs` `parse_https`; `gate.rs` `a_route_must_be_https_or_loopback`. O ICS do FICS é texto puro e só leva visitante, sem segredo (FC-01). |
| SEG-3 | Lista de hosts, redirecionamento, limites de tamanho | ACEITO | `gate.rs` `a_redirect_to_another_host_is_not_followed`; `download.rs` (RD-1); `max_body`. |
| SEG-4 | Retorno do OAuth protegido | ACEITO (UNVERIFIED com serviço real) | `loopback.rs` `other_paths_methods_and_hosts_do_not_count_as_the_callback`; `auth.rs` `a_wrong_state_fails_typed_and_nothing_is_exchanged_or_stored`. CX-HOM-10. |
| SEG-5 | PGN sem HTML ativo nem comando | ACEITO | Texto escapado no egui; `import_origin.rs` `hostile_pgn_is_counted_as_errors_and_the_good_games_survive`. |
| SEG-6 | Logs por usuário, segredos mascarados | ACEITO | Ver S-11. |
| SEG-7 | Revogar sem perder dados | ACEITO (UNVERIFIED com serviço real) | Ver S-08. CX-HOM-12 e 13. |
| SEG-8 | Modelo de ameaças | ACEITO | "Modelo de ameaça (resumo)" em `docs/decisoes/2026-11-11-cofre-e-autorizacao.md`. |
| SEG-9 | Dependências, CVEs, licenças, segredos | ACEITO (UNVERIFIED com serviço real) | `deny.toml` com licenças e avisos de segurança; não rodado nesta auditoria (CI desligada). Conferir: `cargo deny check`. |
| SEG-10 | LGPD em servidor | ACEITO | Não se aplica: sem servidor; nada do Perfil local sai (`consent-sends-tail-local-profile`). |
| L-1 | Token cortado, longo, revogado; cofre bloqueado; Perfil removido | ACEITO | `auth.rs` `a_long_token_of_512_and_more_characters_is_accepted`, `a_malformed_personal_token_is_refused_before_the_network`; `vault.rs` `with_the_own_vault_locked_nothing_leaves_and_the_error_is_typed`; AZ-06. |
| L-2 | Códigos HTTP, prazo, host inexistente, sem rede | ACEITO | `explorer.rs` `explorer_http_failures_are_typed`; `timeout_cancel.rs`; `limits.rs`. |
| L-3 | NDJSON cortado, junto, batimento, duplicado, fora de ordem, gigante | ACEITO | `ndjson.rs`; `board_session.rs` `eventos_duplicados_e_fora_de_ordem_nao_alteram_nem_regridem`. |
| L-4 | FEN inválido, Chess960, promoção, roques, en passant, 50/75, repetição | ACEITO | `crates/chess-core/tests/`; Chess960 recusado antes de criar Partida (`online_game.rs` `variante_nao_padrao_e_recusada_antes_de_criar_a_partida`). |
| L-5 | PGN hostil, unicode, compactado suspeito | ACEITO | `import_origin.rs` `a_gzip_bomb_stops_at_the_decompressed_limit`; `chess-pgn/tests/import.rs`. |
| L-6 | Mesmo `gameId` em dois Provedores; mesmo nome; várias Conexões | FALHOU (#267) | Duplicata por `(Provedor, id)` ok (`import_origin.rs`); várias Conexões numa Partida online sem teste e sem vínculo. |
| L-7 | TWIC e ChessBase restritos: só local | ACEITO | Nada sincroniza; nota de licença na importação. |
| L-8 | Análise anterior não vira dica na Partida humana | ACEITO | Análise e linha de cache travadas (`online/tests.rs` `the_hint_and_every_calculating_aid_are_refused_while_rule_aids_stay`). |

## 10. Matriz T-001 a T-035 (especificação §10.2)

| Id | Caso | Veredito | Evidência ou motivo |
|---|---|---|---|
| T-001 | Sem internet e sem Conexões | ACEITO | `consent.rs` `every_provider_is_born_disabled_and_the_app_starts_offline`. |
| T-002 | OAuth correto | ACEITO (UNVERIFIED com serviço real) | `auth.rs`; `auth_tests.rs` `authorizing_in_the_browser_end_to_end_stores_the_secret_only_in_the_vault`. CX-HOM-09. |
| T-003 | OAuth cancelado; `state` adulterado | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `cancelling_closes_the_local_door_and_stores_nothing`, `a_forged_return_is_refused_and_nothing_is_stored`. CX-HOM-10. |
| T-004 | Sem `board:play` | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `a_key_of_the_wrong_role_is_refused_with_an_explanation_in_both_directions`. CX-HOM-11. |
| T-005 | Revogar | ACEITO | `auth.rs` `revoking_succeeds_remotely_and_the_explorer_needs_a_connection_again`. |
| T-006 | Chess.com público | ACEITO (UNVERIFIED com serviço real) | `games.rs` `chesscom_profile_and_archives_parse_the_real_answers`. CX-HOM-05. |
| T-007 | Lance pelo Chess.com | ACEITO | Adaptador sem envio de Lance (`chesscom.rs`); `Play` sob aprovação, nada sai (`catalog.rs` `chess_com_reads_but_playing_needs_approval`). |
| T-008 | Queen Alice | ACEITO | Sem endereço de rede; nome sem senha. |
| T-009 | ChessBase | ACEITO | Só importação. |
| T-010 | FIDE por modalidade | ACEITO (UNVERIFIED com serviço real) | `lichess_queries.rs` `fide_returns_standard_rapid_and_blitz_separately_with_origin`. CX-HOM-32. |
| T-011 | FICS sem canal seguro | ACEITO | `fics.rs` `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves`. |
| T-012 | NDJSON quebrado e repetido | ACEITO | `ndjson.rs`; `board_session.rs`. |
| T-013 | Lance confirmado, resposta perdida | ACEITO | `board_session.rs` `queda_no_meio_do_envio_nao_repete_o_post_e_confere_o_estado_ao_reconectar`; `fake_server_live.rs` `queda_no_meio_do_post_volta_como_erro_e_o_post_nao_se_repete`. |
| T-014 | Fim enquanto offline | ACEITO | `board_session.rs` `reconexao_com_backoff_registra_o_intervalo_e_detecta_fim_enquanto_desconectado`. |
| T-015 | Partida humana com Engine ligada | FALHOU (#261) | A Engine de análise é recusada (`online/tests.rs` `the_engine_analysis_refuses_during_the_online_game_even_in_review`), a Engine adversária local não. |
| T-016 | Mesmo pedido por outra via interna | ACEITO | Sem plugin nem RPC; teste estático `computing_services_are_only_reachable_through_the_lock`. |
| T-017 | Engine com UCI só no endereço de Engine | ACEITO (UNVERIFIED com serviço real) | `engine_online.rs` `full_game_of_the_online_engine_is_driven_validated_and_sent_once_per_move`. CX-HOM-52. |
| T-018 | Engine UCI trava | ACEITO | `chess-uci` (prazos e `FailureKind`); `analysis.rs` `spawn_failure_becomes_a_failed_status_with_retry`. |
| T-019 | PGN com roque, en passant, promoção, variantes | ACEITO | `chess-pgn/tests/import.rs`, `roundtrip.rs`. |
| T-020 | PGN inválido no meio | ACEITO | `import.rs` `modo_tolerante_pula_a_partida_ruim_e_importa_as_outras`. |
| T-021 | Reimportar | ACEITO | `import_origin.rs` `reimporting_the_same_pgn_adds_nothing`. |
| T-022 | Direitos restritos | ACEITO | Nenhum compartilhamento público existe; dado fica local. |
| T-023 | Explorador com filtro e sem dados | ACEITO (UNVERIFIED com serviço real) | `explorer.rs` `lichess_explorer_sends_pace_rating_and_period_filters`; "Sem dados para esta Posição". CX-HOM-15 e 17. |
| T-024 | Transposição | ACEITO | `chess-openings/tests/book.rs`. |
| T-025 | Cloud Eval sem dado | ACEITO (UNVERIFIED com serviço real) | `cloud_eval_404_is_a_valid_no_evaluation_remembered_for_a_short_time`. CX-HOM-19. |
| T-026 | Final não coberto | ACEITO | `tablebase_with_eight_pieces_creates_no_request`; aviso de 7 peças. |
| T-027 | `429` com `Retry-After` | ACEITO | `limits.rs`. |
| T-028 | Chave gratuita expirou | ACEITO | Nenhum Provedor com chave; a Engine remota sugere a local nos problemas (`analysis.ftl` `remote-problem-*`). |
| T-029 | Duas Conexões do Lichess | FALHOU (#267) | Sem teste; Partida não ligada à Conexão. |
| T-030 | Varredura de logs e backups | ACEITO | Ver K-03. |
| T-031 | Desligar Chess-API e Lichess | ACEITO | `remote_engine_tests.rs` `turning_the_provider_off_afterwards_sends_nothing_and_offers_to_turn_it_on`; dados locais seguem. |
| T-032 | PlayStrategy com variante | ADIADO | Jogo no PlayStrategy adiado (#253); a recusa já existe no adaptador (`playstrategy.rs` `non_standard_playstrategy_variant_is_refused_and_nothing_is_created`). |
| T-033 | ICS hostil | ACEITO | `ics_reader.rs` `hostile_bytes_never_panic_and_never_leak_control_characters`; `fics.rs`. |
| T-034 | Dado da FIDE mudou de formato | ACEITO | `lichess_queries.rs` `fide_search_lists_players_and_failures_are_typed`; último valor guardado (FD-04). |
| T-035 | Reinstalação sem segredo em backup | ACEITO | Não há exportação de configuração; segredo só no cofre do sistema ou no `own.vault` cifrado. |

## 11. Gate final (especificação §15.1)

| Id | Item | Veredito | Evidência ou motivo |
|---|---|---|---|
| G-1 | Catálogo e Central sem pedir senha a Provedor sem API | ACEITO | Ver (d) e K-05. |
| G-2 | OAuth, Cofre e revogação no sistema alvo | ACEITO (UNVERIFIED com serviço real) | CX-HOM-07 a 13. |
| G-3 | Board API com Partidas reais e reconexão | ACEITO (UNVERIFIED com serviço real) | CX-HOM-36 a 41. |
| G-4 | Trava bloqueia Engine em Partidas humanas e correspondência | FALHOU (#261) | Ver F-01. |
| G-5 | PGN, ECO, Explorador, avaliação em cache e finais demonstráveis | ACEITO (UNVERIFIED com serviço real) | CX-HOM-14 a 20, 25 e 26. |
| G-6 | Chess.com só leitura; Queen Alice e ChessBase por arquivo; FIDE pública | ACEITO | P-10, P-16, P-17, P-21. |
| G-7 | Offline preservado e regressão verde | ACEITO (UNVERIFIED com serviço real) | Offline ok (CX-01); suíte não rodada nesta auditoria. |
| G-8 | Sem cobrança, endereço privado, scraping ou licença violada | ACEITO | Ver (d), K-10 e NFR-15. |
| G-9 | T-001 a T-035 aplicáveis aprovados | FALHOU (#261) | T-015 (#261) e T-029 (#267) falham. |
| G-10 | Documentação e evidências | FALHOU (#268) | Estado atual, guia e roteiro atrás do `main`. |
| G-11 | Nenhuma falha crítica de segurança ou fair play aberta | FALHOU (#261) | A brecha da Trava é falha crítica de fair play (especificação §13: reprova a entrega). |

## 12. Decisões e ADRs

### 12.1 Provedores, Conexões e jogo online (CX-01 a CX-21)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| CX-01 | Offline por padrão | ACEITO (UNVERIFIED com serviço real) | Ver (a). CX-HOM-01. |
| CX-02 | Dados públicos primeiro; Central desde o início | ACEITO | Ordem dos PRs (#213, #214 antes de #224 e #230). |
| CX-03 | Jogo online depois da Biblioteca, com a Trava pronta antes | ACEITO | #226 antes de #230 e #233; a Trava da rede veio no #213. |
| CX-04 | Primeira leva | ACEITO (UNVERIFIED com serviço real) | Chess.com, PGN manual, Engine remota, Engine online, PlayStrategy (dados), FICS (visitante) e FIDE no `main`; a Partida online no PlayStrategy está adiada pelo #253 (P-08). Cada um com o seu CX-HOM (seções 2 e 6). |
| CX-05 | Termos Provedor, Conexão, Central; evitar conta, login, credencial | FALHOU (#263) | "conta" na Engine online e na Engine remota. |
| CX-06 | Arquivo > Conexões; estado da rede na barra | ACEITO | `ribbon/backstage.rs`; `status_bar.rs`; capturas `central-conexoes/barra-offline`. |
| CX-07 | Controlador segue o Provedor | ACEITO (UNVERIFIED com serviço real) | PO-01 a PO-07. CX-HOM-38. |
| CX-08 | Trava bloqueia tudo o que calcula, sem exceção | FALHOU (#261) | Ver F-01. |
| CX-09 | Consentimento por Provedor, com aviso do que vai | ACEITO | `provider_consent.rs`; `consent.rs` `the_consent_notice_says_what_is_sent_and_where`. A frase da Engine online falta: #262. |
| CX-10 | Cofre do sistema; senão, cofre próprio com senha-mestra | ACEITO (UNVERIFIED com serviço real) | Ver S-02. CX-HOM-07 e 08. |
| CX-11 | Engine remota por escolha explícita, com selo; sem cadeia | ACEITO (UNVERIFIED com serviço real) | `remote_engine_tests.rs` `the_remote_engine_is_never_the_default_and_a_local_choice_drops_it`; avaliação em cache separada com Origem. CX-HOM-19 e 49. |
| CX-12 | "Engine online" e Conexão de Engine; "BOT" só onde o Provedor exige | ACEITO | `engine_online/tests.rs` `every_engine_online_text_exists_in_pt_br_and_uses_the_glossary_term`. O "conta" que acompanha "BOT" está em #263. |
| CX-13 | Painel Explorador e avaliação em cache na Análise | ACEITO (UNVERIFIED com serviço real) | `explorer/`. CX-HOM-14 a 19. |
| CX-14 | Importação sob demanda, filtros, Atualizar, duplicatas, Origem; nada ao abrir | ACEITO (UNVERIFIED com serviço real) | `online_import.rs`; `games.rs`. CX-HOM-22 a 24, 47 e 48. |
| CX-15 | Buscar, desafiar, ser desafiado, Engine do Lichess, correspondência | ACEITO (UNVERIFIED com serviço real) | `online/tests.rs`. CX-HOM-36 a 41. |
| CX-16 | Pacote opcional de problemas CC0 | ACEITO (UNVERIFIED com serviço real) | `puzzles.rs`; `training_ui/download.rs`. CX-HOM-27, 28 e 44. |
| CX-17 | Suíte sem serviço real | ACEITO | Servidores falsos (`chess_net::testing`); roteiro manual. |
| CX-18 | FICS só como visitante | ACEITO | FC-01 no adaptador; FC-14 no catálogo e na Central (sem método com senha). |
| CX-19 | Partida online sem arbitragem local | ACEITO | `online_game.rs` `so_o_provedor_decide_o_fim_nem_mate_nem_bandeira_locais`. |
| CX-20 | Rede síncrona com `ureq` em threads | ACEITO | `transport.rs` (único `ureq`); teste estático. |
| CX-21 | Explorador exige autorização | ACEITO (UNVERIFIED com serviço real) | `explorer.rs` `without_a_lichess_connection_the_explorer_says_so_before_the_network`. CX-HOM-15 e 16. |

### 12.2 Cofre e autorização (CF-01 a CF-12) e Central (AZ-01 a AZ-12)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| CF-01 | Crate `chess-vault` sem Bevy nem egui | ACEITO | `crates/chess-vault/Cargo.toml`. |
| CF-02 | `keyring` 4.2 com stores padrão | ACEITO (UNVERIFIED com serviço real) | `Cargo.toml`; suíte com `MemoryKeyring`. CX-HOM-07. |
| CF-03 | Cofre próprio Argon2id + XChaCha20-Poly1305 | ACEITO | `own.rs`; `own_vault.rs` `a_tampered_truncated_or_unknown_file_is_refused_and_nothing_is_read`. |
| CF-04 | Escolha do cofre sem troca silenciosa | ACEITO | `own_vault.rs` `a_system_vault_that_appears_later_does_not_take_over_silently`. |
| CF-05 | Índice sem segredo e sem religar segredo | ACEITO | `vault.rs` `a_tampered_index_cannot_rebind_a_secret_to_another_profile_or_provider`. |
| CF-05b | Leitura prévia em fundo | ACEITO | `vault.rs` `preloading_reads_the_system_vault_once_so_requests_never_wait_for_it`. |
| CF-06 | Token em conferência | ACEITO | `vault.rs` `a_staged_secret_is_used_only_while_it_lives_and_replaces_only_on_commit`. |
| CF-07 | Onde o token vai | ACEITO | `catalog.rs` `the_token_goes_only_where_the_provider_documents_authentication`; `gate.rs`. |
| CF-08 | OAuth do Lichess | ACEITO (UNVERIFIED com serviço real) | `auth.rs` (testes). CX-HOM-09. |
| CF-09 | Loopback | ACEITO | `loopback.rs` `the_server_is_not_reachable_from_another_interface`. |
| CF-10 | Recusa do Provedor | ACEITO | `vault.rs` `a_rejection_by_the_provider_marks_the_reference_and_keeps_the_secret`. |
| CF-11 | Revogação | ACEITO (UNVERIFIED com serviço real) | `auth.rs` `revoking_with_the_own_vault_locked_cannot_reach_the_provider_but_still_forgets_locally`. CX-HOM-13. |
| CF-12 | Explorador com Conexão | ACEITO (UNVERIFIED com serviço real) | `explorer.rs` `the_explorer_carries_the_token_of_its_connection_and_nothing_else_does`. CX-HOM-16. |
| AZ-01 | Cofre aberto pelo `NetService`, leitura em fundo | ACEITO | `net_service.rs` (`start_preload`). |
| AZ-02 | Cofre falso nas capturas e testes | ACEITO | `NetService::in_memory` (`secrets::volatile_vault`). |
| AZ-03 | Senha-mestra só quando preciso | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `without_a_system_vault_nothing_is_stored_until_a_master_password_is_created`. CX-HOM-08. |
| AZ-04 | Reserva da Conexão | ACEITO | `auth_tests.rs` `a_reservation_left_behind_by_a_closed_app_is_discarded_with_its_secret`. |
| AZ-05 | Nome já vinculado é absorvido | ACEITO | `authorized_connections.rs` `authorizing_a_name_already_linked_absorbs_the_plain_one_keeping_its_nickname`. |
| AZ-06 | Segredo apagado antes da Conexão; `id` nunca volta | ACEITO | `auth_tests.rs` `removing_a_connection_keeps_it_when_the_vault_refuses_to_delete_the_secret`; `authorized_connections.rs` `a_removed_connection_id_is_never_reused`. |
| AZ-07 | Desconectar | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `disconnecting_revokes_at_the_provider_deletes_the_secret_and_removes_the_connection`. CX-HOM-13. |
| AZ-08 | Autorização recusada | ACEITO (UNVERIFIED com serviço real) | `auth_tests.rs` `a_connection_the_provider_rejects_becomes_expired_with_a_way_to_reauthorize_keeping_its_data`. CX-HOM-12. |
| AZ-09 | Jogador e Engine online separados | ACEITO | `auth_tests.rs` `player_and_engine_authorizations_never_share_the_secret`. |
| AZ-10 | Regra para abrir o navegador | ACEITO | `connections/browser.rs` (testes). |
| AZ-11 | Filtros do Explorador lembrados por Perfil local | ACEITO | `explorer/prefs.rs` (testes); decisão `2026-11-11-preferencias-do-explorador.md`. |
| AZ-12 | Serviços com adaptador em "pronto" | ACEITO | `catalog.rs`; a exceção da Engine online está em RD-3. |

### 12.3 Partida online (PO-01 a PO-19)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| PO-01 | Política Online e `OnlineGame` no setup | ACEITO | `online_game.rs` `setup_online_faz_ida_e_volta_e_setup_adulterado_e_recusado`. |
| PO-02 | Versão do payload continua 1 | ACEITO | `online_game.rs` `eventos_novos_tem_tipo_estavel_na_versao_atual_do_payload`. |
| PO-03 | Um Comando: o retrato | ACEITO | `online_game.rs` `retrato_que_pula_lances_aplica_so_o_que_falta`, `retrato_repetido_nao_gera_eventos`. |
| PO-04 | Intenção enviada fora do log | ACEITO | `online_game.rs` `intencao_enviada_fica_fora_do_log_ate_a_confirmacao`. |
| PO-05 | Reconciliação tudo ou nada | ACEITO | `online_game.rs` `lance_ilegal_do_provedor_dessincroniza_sem_corromper_o_log`, `roque_na_notacao_rei_para_torre_e_aceito`. |
| PO-06 | Só o Provedor encerra; abortada sem resultado | ACEITO | `online_game.rs` `abortada_termina_sem_resultado`; `online_games.rs` `partida_online_abortada_conta_como_terminada_sem_resultado`. |
| PO-07 | Relógio do Provedor | ACEITO | `online_game.rs` `lance_confirmado_pelo_provedor_entra_no_log_com_os_relogios`; `board_session.rs` `fim_por_tempo_vem_do_provedor_nunca_do_relogio_local`. |
| PO-08 | `FairPlayHold` por Partida | ACEITO | `online_game.rs` `trava_fica_ligada_do_inicio_ate_a_confirmacao_do_fim`; `guard.rs` (teste). |
| PO-09 | Envio único do Lance | ACEITO | Ver T-013. |
| PO-10 | Volta de Lance recusada | ACEITO | `board_session.rs` `pedido_de_volta_de_lance_do_adversario_e_recusado_uma_vez`. |
| PO-11 | Busca só rápido ou mais lento | ACEITO (UNVERIFIED com serviço real) | `board_actions.rs` `busca_de_adversario_respeita_os_limites_da_board_api`. CX-HOM-37. |
| PO-12 | Só a Conexão de jogador joga | ACEITO | `board_session.rs` `conexao_de_engine_nunca_joga_como_pessoa`. |
| PO-13 | Trava da correspondência por estrutura | ACEITO (UNVERIFIED com serviço real) | `fair_play.rs` (testes de correspondência); `online/tests.rs` `a_correspondence_game_locks_only_itself_and_its_positions`. A busca por Posição da Biblioteca fica fora: #261. CX-HOM-42. |
| PO-14 | Ponto único no app | ACEITO | `fair_play.rs`; teste estático. Os serviços que a decisão lista passam pela Trava; os que ela não lista estão em #261. |
| PO-15 | Dois gatilhos, uma trava | ACEITO | `net_service.rs` `the_local_competitive_game_and_an_online_game_never_release_each_others_lock`. |
| PO-16 | Só o serviço online grava a Partida | ACEITO | `online/tests.rs` `a_full_game_from_seek_to_checkmate_through_the_board_and_into_the_library`. |
| PO-17 | Sessão da correspondência abre só no tabuleiro | ACEITO (UNVERIFIED com serviço real) | `online/tests.rs` `correspondence_survives_closing_and_reopening_the_app_and_continues_by_the_snapshot`. CX-HOM-41. |
| PO-18 | Nenhuma rede ao abrir o app | ACEITO (UNVERIFIED com serviço real) | Mesmo teste; aviso "Atualizar". CX-HOM-41. |
| PO-19 | Intenção enviada no tabuleiro | ACEITO | `online/tests.rs` `the_board_never_moves_out_of_turn_nor_while_a_move_is_being_sent`, `a_refused_move_goes_back_and_never_reaches_the_log`. |

### 12.4 FICS (FC-01 a FC-21)

Nenhuma conexão ao FICS real foi aberta; as mensagens do servidor são transcrições sintéticas. O que depende do texto real do servidor fica UNVERIFIED, com o item do roteiro.

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| FC-01 | Só visitante; senha recusada antes da rede | ACEITO | `fics.rs` `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves`. |
| FC-02 | Endereço ICS separado | ACEITO | `ics.rs` `the_catalog_lists_only_the_plaintext_fics_port`. A parte "o FICS continua planejado" foi substituída pela FC-14. |
| FC-03 | Transporte ICS pelo gate; desligar fecha | ACEITO | `ics.rs` `turning_the_provider_off_closes_the_open_connection`, `a_command_never_hides_another_line_or_a_control_byte`; teste estático. |
| FC-04 | Enquadramento puro | ACEITO | `ics_reader.rs` (testes de propriedade). |
| FC-05 | Parser separado | ACEITO (UNVERIFIED com serviço real) | `fics/parse.rs`; textos de servidor sintéticos. CX-HOM-57 e 58. |
| FC-06 | Do `style 12` ao retrato | ACEITO (UNVERIFIED com serviço real) | `fics.rs` `boards_repeated_late_or_cut_never_regress_or_diverge`. CX-HOM-57. |
| FC-07 | Envio do Lance sem reenvio | ACEITO (UNVERIFIED com serviço real) | `fics.rs` `a_full_guest_game_with_repeated_late_and_cut_boards`; formato do Lance não conferido no servidor real. CX-HOM-57. |
| FC-08 | Sem reconexão automática | ACEITO | `fics.rs` `losing_the_connection_mid_game_loses_the_unconfirmed_move_and_keeps_the_guard_until_dropped`. |
| FC-09 | Só tipos de xadrez padrão | ACEITO | `fics.rs` `variants_are_refused_before_the_network_and_never_become_a_game`. |
| FC-10 | Observar sem gravar | ACEITO | `fics.rs` `observing_a_game_follows_it_without_storing_or_locking_anything`. |
| FC-11 | Trava como no Lichess | ACEITO | Hold na sessão (FC-08, mesmo teste). |
| FC-12 | Ajustes depois do login; prazo | ACEITO (UNVERIFIED com serviço real) | `fics.rs` `the_login_has_a_deadline_on_the_injected_clock`. CX-HOM-56. |
| FC-13 | Uma sessão abstrata para Lichess e FICS | ACEITO | `online/service.rs` (`GameSession`); `fics_tests.rs` `a_guest_game_goes_from_the_seek_through_the_board_and_the_panel_into_the_library`; os testes do Lichess (`online/tests.rs`) seguem. |
| FC-14 | Jogar do FICS pronto, só visitante; frase de consentimento | ACEITO | `catalog.rs` (FICS `Ready`, `AuthMethod::None`, `CATALOG_VERSION` 7); `connections/model.rs` `the_fics_card_plays_as_a_guest_never_asks_for_a_password_and_warns_of_plain_text`; captura `docs/evidencias/fics-visitante/escuro/central-fics.png`. CX-HOM-55. |
| FC-15 | Conexão só por "Entrar como visitante" com o FICS ligado | ACEITO (UNVERIFIED com serviço real) | `fics_tests.rs` `with_the_fics_off_no_connection_is_ever_opened`. CX-HOM-56. |
| FC-16 | Queda: aviso, Trava até descartar, Partida sem resultado | ACEITO (UNVERIFIED com serviço real) | `fics_tests.rs` `the_lock_refuses_analysis_and_a_drop_keeps_it_until_the_session_is_discarded`. CX-HOM-59. |
| FC-17 | Sair do FICS com confirmação | ACEITO (UNVERIFIED com serviço real) | `online/fics_view.rs`; textos `fics-leave`, `fics-leave-warning`, `fics-leave-confirm` (`fics.ftl`). CX-HOM-56. |
| FC-18 | Observar só em memória | ACEITO (UNVERIFIED com serviço real) | `fics_tests.rs` `observing_stays_in_memory_never_reaches_the_library_and_never_locks`. CX-HOM-60. |
| FC-19 | Dessincronização sem Lance inventado | ACEITO | `fics_tests.rs` `a_desync_is_shown_without_inventing_a_move_until_a_coherent_board`. |
| FC-20 | Origem do FICS | ACEITO | Metadata `online` com Provedor, número, servidor, modo e licença "Termos do FICS" (`online/service.rs`, `metadata`); teste de ponta a ponta de FC-13. |
| FC-21 | A tela de busca fecha quando a Partida vai ao tabuleiro | ACEITO (UNVERIFIED com serviço real) | `online/mod.rs` (`after_frame`); teste de FC-13. CX-HOM-57. |

### 12.5 Adaptadores da etapa 5 (AD-01 a AD-10)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| AD-01 | Perfil de Provedor | ACEITO (UNVERIFIED com serviço real) | `playstrategy.rs` `the_lichess_defaults_did_not_change`; formatos do PlayStrategy sintéticos. CX-HOM-46 e 47. |
| AD-02 | O que o PlayStrategy não tem: recusa antes do pedido | ACEITO | `playstrategy.rs` `what_playstrategy_does_not_offer_is_refused_before_any_request`. |
| AD-03 | Mensagens por Provedor | ACEITO | `playstrategy.rs` `personal_token_is_checked_at_playstrategy_and_errors_name_the_provider`. |
| AD-04 | Engine online na mesma sessão | ACEITO (UNVERIFIED com serviço real) | `engine_online.rs` `a_dropped_send_is_never_repeated_blindly_and_the_provider_state_decides`. CX-HOM-52. |
| AD-05 | Partida de Engine não é de pessoa | ACEITO | `online_game.rs` `partida_da_engine_online_nunca_conta_como_partida_de_pessoa_nem_liga_a_trava`. |
| AD-06 | Conexões separadas nos dois sentidos | ACEITO | `engine_online.rs` `player_connection_does_not_play_as_engine_and_engine_connection_does_not_play_as_person`. |
| AD-07 | Política de desafios | ACEITO (UNVERIFIED com serviço real) | `engine_online.rs` `challenge_policy_decides_by_variant_pace_rating_opponent_and_concurrency`; `engine_online/tests.rs` `challenges_outside_the_users_limits_are_declined_with_the_reason`. CX-HOM-53. |
| AD-08 | Falha da Engine: abandono | ACEITO (UNVERIFIED com serviço real) | `engine_online/tests.rs` `an_engine_that_does_not_start_resigns_instead_of_letting_the_clock_run`. CX-HOM-53. |
| AD-09 | Conversão em BOT explícita | ACEITO (UNVERIFIED com serviço real) | Ver K-09. CX-HOM-53. |
| AD-10 | Engine remota pelo contrato de análise | ACEITO (UNVERIFIED com serviço real) | `chess_api.rs`; `remote_engine_tests.rs` `the_remote_answer_shows_like_any_engine_and_only_the_fen_leaves`. CX-HOM-49 e 50. |

### 12.6 Acabamento da etapa 5 (RD-1 a RD-6)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| RD-1 | Redirecionamento só para o catálogo | ACEITO (UNVERIFIED com serviço real) | `download.rs` `recusa_redirecionamento_para_fora_do_catalogo_e_nao_pede_la`, `laco_de_redirecionamentos_para_depois_do_limite_de_saltos`. CX-HOM-44. |
| RD-2 | Por que não só recusar | ACEITO | Justificativa de RD-1; `download.rs` `redirecionamento_nao_vale_para_consulta_comum`. |
| RD-3 | Consentimento = uso real; quem entrega muda para pronto | FALHOU (#262) | O #252 entregou a Engine online sem mudar o serviço para pronto nem acrescentar a frase. |
| RD-4 | Baixar e Jogar prontos | ACEITO | `catalog.rs`. "Engine online segue planejada" foi superado pelo #252 (ver RD-3). |
| RD-5 | Linguagem do catálogo e da Central | ACEITO | `catalog.rs` `provider_texts_use_plain_language_without_developer_jargon`; `model.rs` `no_interface_text_uses_stage_jargon_or_the_banned_words`. Os outros `.ftl` estão em #263. |
| RD-6 | Exportar PGN | ACEITO (UNVERIFIED com serviço real) | `file_picker.rs`; `fen_pgn.rs` (seletor falso). Seletor real do sistema: CX-HOM-43. |

### 12.7 Rating FIDE, dados guardados e Origens (FD-01 a FD-11)

| Id | Decisão | Veredito | Evidência ou motivo |
|---|---|---|---|
| FD-01 | Rating via Lichess | ACEITO (UNVERIFIED com serviço real) | `lichess_queries.rs` `fide_returns_standard_rapid_and_blitz_separately_with_origin`. CX-HOM-32. |
| FD-02 | Três ritmos separados | ACEITO | `fide_and_bulk.rs` `the_saved_rating_is_read_back_offline_with_standard_rapid_and_blitz_apart`. |
| FD-03 | Data da consulta | ACEITO | `lichess_queries.rs` `refreshing_a_fide_rating_asks_the_provider_again_and_moves_the_query_date`; a hora local do #247 substituiu o "UTC". |
| FD-04 | Último valor por Perfil local e ID | ACEITO | `fide_and_bulk.rs` `a_new_query_replaces_the_value_and_each_local_profile_keeps_its_own`. |
| FD-05 | Consulta só com o Lichess ligado; sem Trava | ACEITO (UNVERIFIED com serviço real) | `fide_profile.rs`; capacidade `Query` não calcula. CX-HOM-32. |
| FD-06 | Atualizar vai ao Lichess | ACEITO (UNVERIFIED com serviço real) | Teste de FD-03. CX-HOM-32. |
| FD-07 | Busca por nome | ACEITO (UNVERIFIED com serviço real) | `lichess_queries.rs` `fide_search_lists_players_and_failures_are_typed`. CX-HOM-32. |
| FD-08 | Limpar dados guardados | ACEITO | `connections/stored_data.rs` (testes). CX-HOM-33. |
| FD-09 | Importações por Origem | ACEITO | `fide_and_bulk.rs` `deleting_a_provider_takes_all_its_imports_and_nothing_of_the_others`. CX-HOM-34. |
| FD-10 | Partida com duas Origens | ACEITO | `fide_and_bulk.rs` `a_game_that_also_came_from_another_origin_stays_and_is_counted_apart`. |
| FD-11 | Nunca as Partidas próprias | ACEITO | `fide_and_bulk.rs` `deleting_one_import_leaves_the_other_imports_and_every_played_game`. |

### 12.8 ADRs e regras do projeto

| Id | Regra | Veredito | Evidência ou motivo |
|---|---|---|---|
| ADR6-1 | Offline por padrão; Provedor nasce desligado | ACEITO (UNVERIFIED com serviço real) | Ver (a). CX-HOM-01. |
| ADR6-2 | Conexão como vínculo do Perfil local | ACEITO | `crates/chess-store/src/connections.rs`; `connections.rs` (testes). |
| ADR6-3 | Todo dado de fora guarda a Origem com licença | FALHOU (#264) | Ver (e). |
| ADR6-4 | Só API ou exportação documentada; nunca senha nem automação | ACEITO | Ver (d). |
| ADR6-5 | Segredos no cofre do sistema ou no cofre próprio | ACEITO (UNVERIFIED com serviço real) | Ver S-02. CX-HOM-07 e 08. |
| ADR6-6 | Engine remota explícita, consentimento por Provedor | ACEITO (UNVERIFIED com serviço real) | Ver CX-11. CX-HOM-49. |
| ADR6-7 | Custo zero por padrão | ACEITO | Ver NFR-15. |
| ADR6-8 | Camada de rede única por Provedor | ACEITO | Teste estático `no_network_without_consent.rs`. |
| ADR6-9 | Falha de rede nunca corrompe Partida nem trava o uso offline | ACEITO | `board_session.rs` `retrato_divergente_vira_aviso_tipado_sem_mexer_no_log`; `cache.rs`. |
| ADR7-1 | Controlador registra só o que o Provedor confirma | ACEITO (UNVERIFIED com serviço real) | PO-04, PO-05. CX-HOM-38. |
| ADR7-2 | Sem reenvio às cegas; sem regressão | ACEITO | T-013; `online_property.rs`. |
| ADR7-3 | Políticas de arbitragem locais não se aplicam | ACEITO | CX-19. |
| ADR7-4 | Trava bloqueia tudo o que calcula, na camada de serviço | FALHOU (#261) | Ver F-01. |
| ADR7-5 | Auxílios que só mostram regras continuam | ACEITO | `online/tests.rs` `the_hint_and_every_calculating_aid_are_refused_while_rule_aids_stay`. |
| ADR7-6 | Análise só depois do fim confirmado | ACEITO (UNVERIFIED com serviço real) | Ver F-06. CX-HOM-42. |
| ADR7-7 | Engine só joga por Conexão de Engine própria | ACEITO | AD-06. |
| R-1 | ADR-0001: só o controlador escreve a Partida | ACEITO | PO-16; `session.rs` (`apply` ignora a Partida online). |
| R-2 | ADR-0002: log de eventos, tempo injetado | ACEITO | `online_games.rs` `partida_online_em_andamento_reabre_igual_do_banco`; `FakeClock` na suíte. |
| R-3 | ADR-0003: Lance de Engine validado pelo núcleo | ACEITO | `engine_online.rs` `illegal_engine_move_is_refused_by_the_rules_core_and_nothing_is_sent`; `chess_api.rs` `an_illegal_best_move_is_refused_and_nothing_is_shown`. |
| R-4 | ADR-0004: sem GPL/AGPL/LGPL | ACEITO (UNVERIFIED com serviço real) | `deny.toml`; dependências novas declaram MIT/Apache (`chess-vault/Cargo.toml`). `cargo deny check licenses` não rodado nesta auditoria. |

## Issues abertas

| Issue | O que falha | Itens |
|---|---|---|
| [#261](https://github.com/ChicoFigueiredo/Chess-Studio/issues/261) | Trava de fair play: Engine adversária, busca por Posição e Engine online não consultam a Trava | (b), F-01, US-028, NFR-02, T-015, G-4, G-9, G-11, CX-08, ADR7-4 |
| [#262](https://github.com/ChicoFigueiredo/Chess-Studio/issues/262) | Engine online entregue, mas o catálogo a mantém planejada: consentimento sem a frase e Central em "O que vem depois" | C-02, RD-3 |
| [#263](https://github.com/ChicoFigueiredo/Chess-Studio/issues/263) | Textos fora do GLOSSARY: "conta", "chaveiro" e código HTTP | (f), CX-05 |
| [#264](https://github.com/ChicoFigueiredo/Chess-Studio/issues/264) | Origem sem licença: Central, rating FIDE e Engine remota | (e), US-006, US-026, ADR6-3 |
| [#265](https://github.com/ChicoFigueiredo/Chess-Studio/issues/265) | Explorador: janela temporal, aviso de amostra e texto de privacidade | US-020 |
| [#266](https://github.com/ChicoFigueiredo/Chess-Studio/issues/266) | Central: filtros Com cadastro, Manual e por estado | US-CRED-01, US-003 |
| [#267](https://github.com/ChicoFigueiredo/Chess-Studio/issues/267) | Partida online não ligada à Conexão; troca da Conexão padrão durante a Partida | C-09, US-CRED-05, US-008, L-6, T-029, G-9 |
| [#268](https://github.com/ChicoFigueiredo/Chess-Studio/issues/268) | Documentação atrás do `main`: estado atual, guia e roteiro CX-HOM | (g), US-001, G-10 |
| [#269](https://github.com/ChicoFigueiredo/Chess-Studio/issues/269) | Cobertura e matriz de ramos de segurança e fair play | NFR-10 |
| [#270](https://github.com/ChicoFigueiredo/Chess-Studio/issues/270) | Importação de PGN: prévia, detecção da fonte e confirmação de direitos | US-023, FL-6.5 |

## Observações (não mudam veredito)

- **PlayStrategy online sem seguimento.** A CX-04 põe o PlayStrategy na primeira leva e a etapa 5 do épico o cita; o #253 deixou a Partida online "para outra etapa", mas não há issue aberta para ela. O adaptador (Board e Bot) já existe e está testado com servidor falso.
- **CI desligada.** A suíte, o `clippy` e o `cargo deny` só rodam localmente; Windows e macOS não são exercitados (NFR-12, SEG-9).
- **Licença das Partidas do Lichess.** A Biblioteca rotula as Partidas importadas do Lichess como CC0 (`library-license-lichess-cc0`) e a Partida online grava `CC0-1.0`, enquanto o catálogo dá "Termos do Lichess" para os endereços de API. As duas leituras são defensáveis (a base do Lichess é CC0); vale alinhar o texto quando o #264 mexer nas licenças.
- **Tempo de queda nos testes.** A reconexão tem teste determinístico com 30 s; os 5 s e 2 min da CA-011.1 ficam para o roteiro (CX-HOM-38).
- **Erro de download do Stockfish** (`wizard.ftl`, `wizard-error-http`) mostra "HTTP { $code }". É anterior ao V4; entra no #263 se quiser alinhar.
