# Estado atual para as integrações (CX-1, 11/11/2026)

Inventário do código em `main` (commit `5c27ec8`) para o programa de Provedores e Conexões. Cumpre a "fase zero" da [especificação](especificacao-free-first-2026-11-11.md) (seção 5.1) sob as [decisões](../decisoes/2026-11-11-conexoes.md), a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md) e a [ADR-0007](../adr/0007-partida-online-controlador-segue-o-provedor.md). A especificação previa o arquivo em `docs/chess-integration/current-state.md`; vale este caminho (issue #212).

Só documentação. Nada aqui propõe reescrita: cada lacuna aponta a **costura** onde a integração entra.

## Como ler

- **EXISTE**: pronto e testado; reaproveitar.
- **PARCIAL**: existe, mas falta algo que as integrações pedem; o texto diz o quê.
- **AUSENTE**: sem código. Procurei por palavras-chave nas fontes de `crates/*/src` (a busca vem dita em cada caso).
- Nenhuma capacidade ficou "DESCONHECIDA".
- Links são relativos a este arquivo. `:n` é a linha no commit acima.

## Resumo

| Capacidade | Situação |
|---|---|
| PGN: leitura, escrita, tags, variações | EXISTE |
| PGN: arquivos grandes, encodings, `[%clk]` | PARCIAL |
| FEN | EXISTE |
| UCI e gerenciador de Engines | EXISTE |
| Engine remota (não é processo local) | AUSENTE |
| Aberturas e ECO (offline) | EXISTE |
| Biblioteca: esquema, busca, importação | EXISTE |
| Biblioteca: duplicatas e Origem | AUSENTE / PARCIAL |
| Explorador local (estatística da Biblioteca) | EXISTE |
| Explorador de Provedores, avaliação em cache, finais | AUSENTE |
| Treino e Exercícios: formato e catálogo de fábrica | EXISTE |
| Treino: importar Exercícios de fora | PARCIAL |
| Controlador da Partida e fontes de comando | EXISTE (só locais) |
| Fonte remota de comandos / Partida online | AUSENTE |
| Relógio | EXISTE (local) |
| Políticas e Auxílios; trava de fair play | PARCIAL |
| Rede: cliente existente | PARCIAL |
| Camada de rede por Provedor | AUSENTE |
| Consentimento de rede por Provedor | AUSENTE |
| Cofre de segredos | AUSENTE |
| Perfil local e preferências | EXISTE |
| Conexão (vínculo Perfil-Provedor) | AUSENTE |
| Bastidores, Ribbon, barra de status, Painéis | EXISTE |
| Central de Conexões, Painel Explorador | AUSENTE |
| Lista de Ações | EXISTE |
| i18n Fluent | EXISTE |

## 1. Regras e núcleo

### 1.1 FEN: EXISTE

- Leitura e escrita: [`CanonicalPosition::from_fen`](../../crates/chess-core/src/position.rs) (`:37`) e `to_fen` (`:69`). FEN inválido é erro tipado, nunca pânico.
- Testes: [`crates/chess-core/tests/fen.rs`](../../crates/chess-core/tests/fen.rs). Perft em [`tests/perft.rs`](../../crates/chess-core/tests/perft.rs).
- A tag `FEN` do PGN passa pelo mesmo caminho ([`import.rs`, teste do FEN inválido](../../crates/chess-pgn/tests/import.rs)).
- **Só xadrez padrão.** Procurei `960`, `variant` e `Chess960` em `crates/chess-core/src` e `crates/chess-pgn/src`: nenhuma ocorrência. Variantes do Lichess, PlayStrategy e FICS (spec seção 3.3) não têm base. Fora das cinco etapas; ao chegarem, o Provedor deve recusar a variante na camada de Provedor, antes de tocar a Partida.

### 1.2 Chave de Posição para estatística e transposição: EXISTE

- [`library::position_key`](../../crates/chess-store/src/library.rs) (`:120`) e [`fen_key`](../../crates/chess-store/src/library.rs) (`:145`): FEN sem contadores, en passant só com captura legal.
- Mesma ideia, outra chave, no livro: [`RepetitionKey`](../../crates/chess-openings/src/lib.rs).
- **Costura:** consultas ao Explorador de Provedores (`fen` na URL) devem usar `fen_key`/`to_fen` e cachear por essa chave, para casar com o índice local.

## 2. PGN

### 2.1 Leitura e escrita: EXISTE

- Parser próprio, sem GPL ([`parser.rs`](../../crates/chess-pgn/src/parser.rs), [`lexer.rs`](../../crates/chess-pgn/src/lexer.rs)). Árvore com variações aninhadas, comentários, NAGs: [`model.rs`](../../crates/chess-pgn/src/model.rs).
- Duas entradas: `parse_pgn` (`parser.rs:19`, falha no primeiro erro) e `parse_pgn_lenient` (`parser.rs:30`, pula a partida ruim e relata o erro com linha e coluna).
- Cada Lance é validado pelo `chess-core`; Lance ilegal vira erro tipado ([`PgnErrorKind`](../../crates/chess-pgn/src/error.rs)).
- Escrita: [`PgnGame::to_pgn`](../../crates/chess-pgn/src/export.rs) (`:21`) para árvores importadas; [`Game::to_pgn`](../../crates/chess-game/src/pgn.rs) para a Partida do app, com Infrações como comentários e a tag `EngineElo`.
- Árvore importada vira Partida Assistida: [`to_assisted_game`](../../crates/chess-pgn/src/convert.rs) (`:45`).
- Testes: [`import.rs`](../../crates/chess-pgn/tests/import.rs) (tags com escape, NAGs, `;`, `%`, FEN inicial, tolerância), [`roundtrip.rs`](../../crates/chess-pgn/tests/roundtrip.rs) (exporta e reimporta sem perda, JSON da árvore), [`pgn_export.rs`](../../crates/chess-game/tests/pgn_export.rs).

### 2.2 Tags: PARCIAL

- O modelo guarda **todas** as tags, na ordem e com repetidas (`PgnGame.tags`, [`model.rs`](../../crates/chess-pgn/src/model.rs)), e a árvore inteira vai em JSON para `imported_games.tree_json`.
- Só `Event`, `White`, `Black`, `Date` e `Result` viram colunas ([`imported.rs`](../../crates/chess-store/src/imported.rs), `import_pgn_for`, `:64`). `Site`, `WhiteElo`, `TimeControl`, `ECO`, `Termination`, `UTCDate` ficam só no JSON: não são pesquisáveis nem filtráveis.
- A exportação do app fixa `Event "Partida casual"`, `White "Brancas"`, `Black "Pretas"`, `Site "Chess Studio"` em [`Session::export_pgn`](../../crates/chess-studio/src/session.rs) (`:252`).
- **Costura:** quem precisa de tags novas (Provedor, URL da Partida, ritmo, Elo) estende as colunas de `imported_games` por migração nova em [`migrations.rs`](../../crates/chess-store/src/migrations.rs) (hoje 12 versões, última `V12_OPENING_EXPLORER`) e as opções de [`PgnOptions`](../../crates/chess-game/src/pgn.rs) para a escrita.

### 2.3 Arquivos grandes: PARCIAL

- O parser recebe `&str` inteiro; `import_file` lê o arquivo todo em memória ([`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs) `:243`) e importa numa única transação.
- Desempenho medido: 1000 partidas em menos de 2 s ([`perf.rs`](../../crates/chess-pgn/tests/perf.rs) `:35`); nenhum prefixo truncado causa pânico ([`import.rs`](../../crates/chess-pgn/tests/import.rs) `:287`).
- Não há leitura em fluxo, progresso, cancelamento nem divisão em lotes. Um arquivo de milhões de partidas (TWIC, PGN Mentor) pressiona memória e trava a thread que importa.
- **Costura:** um iterador de partidas sobre `Read` ao lado de `parse_pgn_lenient` (mesmo lexer), chamado por `Store::import_pgn_for` em lotes. `import_file` é síncrono; a Central deve chamá-lo numa thread, como o gerenciador de Engines faz.

### 2.4 Encodings: PARCIAL

- UTF-8 com BOM e CRLF funcionam ([`import.rs`](../../crates/chess-pgn/tests/import.rs) `:145`).
- Outros encodings (Latin-1 de PGNs antigos de ChessBase) não: `import_file` usa `String::from_utf8_lossy` e os bytes inválidos viram `�` ([`library_ui.rs:248-249`](../../crates/chess-studio/src/library_ui.rs)). Nenhum aviso ao usuário.
- **Costura:** decodificação em `library_ui::import_file`, antes do parser; o parser continua só com `&str`.

### 2.5 Duplicatas: AUSENTE

- Procurei `dedup`, `duplicate` e `duplicat` em `crates/chess-store/src`: nenhuma ocorrência. Importar o mesmo arquivo duas vezes grava tudo de novo.
- **Costura:** coluna de impressão digital (hash de tags de identidade + Lances da linha principal) em `imported_games`, conferida em `import_pgn_for`, que já devolve [`ImportReport`](../../crates/chess-store/src/imported.rs); acrescentar `skipped_duplicates` ali. O índice incremental da Biblioteca já é por assinatura ([`library.rs`](../../crates/chess-store/src/library.rs), cabeçalho).

### 2.6 Comentários `[%clk]` e `[%eval]`: AUSENTE

- Procurei `clk` e `[%` em `crates/chess-pgn/src`: nenhuma ocorrência. Os comentários passam como texto; o relógio de PGNs do Lichess e do Chess.com não é interpretado.
- **Costura:** `MoveNode.comments` ([`model.rs`](../../crates/chess-pgn/src/model.rs)); extrair em camada acima, sem mudar o parser.

## 3. Engines e UCI

### 3.1 Protocolo e processo: EXISTE

- Processo filho com handshake, timeouts e log limitado: [`engine.rs`](../../crates/chess-uci/src/engine.rs), [`protocol.rs`](../../crates/chess-uci/src/protocol.rs), [`controller.rs`](../../crates/chess-uci/src/controller.rs) (`EngineController`, `:147`).
- Todo Lance da Engine é só proposta e passa pelo núcleo: `Engine::search_legal_move` ([`engine.rs:302`](../../crates/chess-uci/src/engine.rs)), em linha com a ADR-0003.
- Análise com várias linhas: [`AnalysisController`](../../crates/chess-uci/src/analysis.rs) (`:181`), `Score`, `AnalysisLine`.
- Jogador de Engine: [`EnginePlayer::drive(&mut Game, Timestamp)`](../../crates/chess-uci/src/player.rs) (`:139`) injeta o Lance na Partida como `Command::EngineMove`.
- Servidor falso para testes: [`bin/fake_uci.rs`](../../crates/chess-uci/src/bin/fake_uci.rs); testes em [`player.rs`](../../crates/chess-uci/tests/player.rs), [`analysis.rs`](../../crates/chess-uci/tests/analysis.rs), [`adapter.rs`](../../crates/chess-uci/tests/adapter.rs). Os testes com Stockfish e Lc0 reais são `--ignored`.

### 3.2 Gerenciador de Engines: EXISTE

- Catálogo fixado com SHA-256 ([`catalog.rs`](../../crates/chess-uci/src/catalog.rs)), detecção ([`detect.rs`](../../crates/chess-uci/src/detect.rs)), instalação, atualização e remoção segura ([`manager.rs`](../../crates/chess-uci/src/manager.rs)), backends e hardware ([`backend.rs`](../../crates/chess-uci/src/backend.rs)), carga ([`load.rs`](../../crates/chess-uci/src/load.rs)).
- Registro no banco: [`EngineRecord`](../../crates/chess-store/src/engines.rs) com `EngineOrigin::{Catalog, Detected, Manual}`, opções UCI e status; teste em [`engines.rs`](../../crates/chess-store/tests/engines.rs).
- Interface: [`engine_manager.rs`](../../crates/chess-studio/src/engine_manager.rs), [`engine_wizard.rs`](../../crates/chess-studio/src/engine_wizard.rs), [`engine.rs`](../../crates/chess-studio/src/engine.rs). Segurança: [`seguranca-engines.md`](../decisoes/seguranca-engines.md).
- Consentimento de execução: [`engine_consent.rs`](../../crates/chess-studio/src/engine_consent.rs). Nenhum processo sobe sem ele.

### 3.3 Engine remota: AUSENTE

- Procurei `remote`, `websocket`, `lichess`, `chess.com` e `cloud` nas fontes de `crates/*/src`: nada além de comentários. A `EngineConfig` ([`config.rs:39`](../../crates/chess-uci/src/config.rs)) descreve um **executável local**; `AnalysisController::start(config)` e `EnginePlayer::new(seat, config)` só sabem subir processo.
- **Costura (CX-11, sem cadeia automática):** a interface comum já existe como o par de eventos de `AnalysisController::poll` ([`AnalysisEvent`/`AnalysisUpdate`](../../crates/chess-uci/src/analysis.rs)) e `EnginePlayer::drive`. Uma Engine remota implementa o mesmo contrato de eventos num módulo novo de Provedor, escolhida pelo mesmo seletor de Engine, com o selo do Provedor no `EngineChip` da barra de status ([`status_bar.rs:64`](../../crates/chess-studio/src/status_bar.rs)). `EngineRecord.protocol` ([`engines.rs`](../../crates/chess-store/src/engines.rs)) já é texto livre e comporta um valor novo; `path` é obrigatório e único, então um registro remoto pede campo opcional ou tabela irmã (migração nova).
- Lance remoto continua só proposta: passar por `Game::handle(Command::EngineMove)`, que valida.

## 4. Aberturas e ECO

### 4.1 Livro offline: EXISTE

- [`chess-openings`](../../crates/chess-openings/src/lib.rs): `OpeningBook::embedded()`, `lookup`, `along`. Dados CC0 do `lichess-org/chess-openings`, embutidos por `include_str!`; licença registrada em [`assets/LICENSES.md`](../../crates/chess-studio/assets/LICENSES.md). Testes: [`book.rs`](../../crates/chess-openings/tests/book.rs) (transposição, linha mais longa, linhas ilegais ignoradas).
- Sem dependência de Bevy nem de SQLite.
- Nomes só em inglês, como na origem. Mostrado no [Painel Aberturas](../../crates/chess-studio/src/openings.rs) (`opening_at`).

### 4.2 Explorador local: EXISTE

- [`Store::opening_stats(profile, fen)`](../../crates/chess-store/src/openings.rs) (`:82`) devolve [`OpeningMoveStats`](../../crates/chess-store/src/openings.rs) por Lance (partidas, vitórias, empates, derrotas, último uso) sobre a Biblioteca do Perfil local; índice derivado e incremental (`opening_moves`, 40 meios-lances).
- Testes: [`openings.rs`](../../crates/chess-store/tests/openings.rs) (transposição, isolamento por Perfil, reconstrução, consulta abaixo de 50 ms com 10 mil partidas).
- Painel: [`openings.rs`](../../crates/chess-studio/src/openings.rs). Fica indisponível numa Partida competitiva em andamento (`Availability::Competitive`).

### 4.3 Explorador de Provedores e finais: AUSENTE

- Nada consulta mestres, Lichess ou jogador; nada consulta finais de até sete peças.
- **Costura:** o Painel Explorador (CX-13) nasce como [`PanelDef`](../../crates/chess-studio/src/dock.rs) (`:80`) registrado por `register_panel` (`:125`), com abas Aberturas e Finais. A aba Aberturas mostra o `OpeningMoveStats` local **e** o resultado do Provedor lado a lado, mesmo formato de barras (`openings::percentages`). O resultado externo carrega a Origem (ver 7.1) e é tipo separado de `OpeningMoveStats` (que não tem Provedor).

## 5. Biblioteca (`chess-store`)

### 5.1 Esquema e Perfil: EXISTE

- SQLite via `rusqlite` com migrações versionadas em `PRAGMA user_version` ([`migrations.rs`](../../crates/chess-store/src/migrations.rs)), 12 versões; teste de migração de banco antigo em [`migration.rs`](../../crates/chess-store/tests/migration.rs) e [`library.rs`](../../crates/chess-store/tests/library.rs) (`:504`).
- Tabelas: `profiles`, `ui_preferences`, `games`, `game_events`, `imported_games`, `library_entries` (+ FTS5 `library_fts` e `library_positions`), `favorite_positions`, `exercises`, `exercise_runs`, `engine_installations`, `opening_moves`, Súmula.
- Log de eventos como fonte de verdade (ADR-0002): [`Store::append_events`](../../crates/chess-store/src/store.rs) e `load_game`; teste de queda em [`crash.rs`](../../crates/chess-store/tests/crash.rs).

### 5.2 Busca: EXISTE

- [`Store::search_library`](../../crates/chess-store/src/library.rs) (`:554`) com [`LibraryQuery`](../../crates/chess-store/src/library.rs): texto livre (FTS5, sem acento, prefixo), jogador, resultado, Política, intervalo de datas (parciais `2023.??.??`), **Posição** (FEN), origem (`Played`/`Imported`), Perfil, página.
- Testes: [`library.rs`](../../crates/chess-store/tests/library.rs) (busca por Posição com e sem contadores, variações, paginação, acentos).
- Cobre US-022 quase por inteiro. Não filtra por Elo, ritmo ou ECO (colunas ausentes; ver 2.2).

### 5.3 Importação: EXISTE (local)

- [`Store::import_pgn_for(profile, pgn, source)`](../../crates/chess-store/src/imported.rs) (`:64`): leniente, uma transação, relatório de partidas puladas. Testes: [`pgn_import.rs`](../../crates/chess-store/tests/pgn_import.rs).
- Entrada na interface: [`library_ui::import_file`](../../crates/chess-studio/src/library_ui.rs) (`:243`) e leitura de pasta.
- **Costura para importação de Provedor (CX-14):** a rede devolve texto PGN; ele entra por `import_pgn_for` com `source` identificando o Provedor. Filtros (período, ritmo, cor) e "Atualizar" ficam no módulo do Provedor, não no `chess-store`.

### 5.4 Origem dos dados: PARCIAL

- `imported_games.source` é um texto livre (hoje, o nome do arquivo; [`ImportedSummary.source`](../../crates/chess-store/src/imported.rs)). Falta o resto da Origem do glossário: Provedor, instante da consulta, licença, modo de importação.
- **Costura:** colunas novas em `imported_games` (migração 13) e um tipo `Provenance` no `chess-store`, preenchido por `import_pgn_for` (parâmetro `source: Option<&str>` vira estrutura). A Partida jogada no app não tem Origem de fora e não precisa.

## 6. Treino e Exercícios

### 6.1 Formato e catálogo: EXISTE

- [`Exercise`](../../crates/chess-training/src/exercise.rs): `id`, `ExerciseKind` (8 tipos), FEN, `Solution` (sequência UCI, mate em um, afogamento…), explicação.
- Catálogo de fábrica: JSON embutido por `include_str!` em [`catalog.rs`](../../crates/chess-training/src/catalog.rs) (`Catalog::bundled`, `:117`) a partir de [`exercises/*.json`](../../crates/chess-training/exercises/); ids duplicados são recusados.
- Persistência: [`Store::upsert_exercises`](../../crates/chess-store/src/training.rs) (`:45`), execuções e métricas em [`training.rs`](../../crates/chess-store/src/training.rs); testes em [`tests/training.rs`](../../crates/chess-store/tests/training.rs), [`content.rs`](../../crates/chess-training/tests/content.rs), [`catalog.rs`](../../crates/chess-training/tests/catalog.rs).

### 6.2 Importar Exercícios novos: PARCIAL

- O leitor existe: `Catalog::from_json` (`catalog.rs:88`) e `Catalog::from_dir` (`:126`). **Mas** o app só chama `Catalog::bundled()` ([`training_ui.rs:142`](../../crates/chess-studio/src/training_ui.rs), [`library_ui.rs:510`](../../crates/chess-studio/src/library_ui.rs)); procurei `from_dir` em `crates/chess-studio/src`: nenhum uso. Não há tela nem pasta de usuário.
- O `ExerciseKind` não tem "tática por tema/dificuldade" (o pacote de problemas do Lichess, CX-16, traz tema e classificação). Os 8 tipos são de regras e memória; um problema do Lichess (FEN + linha de Lances) cabe em `Solution::Moves`, mas faltam campos de tema, dificuldade e Origem.
- **Costura:** pacote baixado vira JSON no mesmo formato, lido por `Catalog::from_json`, gravado por `upsert_exercises`; Origem e "separados dos de fábrica" pedem uma coluna `origin` em `exercises` (migração) e campos opcionais `theme`/`rating` em `Exercise` com `#[serde(default)]`, que não quebram os JSON de fábrica.

## 7. Partida, relógio e arbitragem

### 7.1 Controlador e fontes de comando: EXISTE (só locais)

- `Game::handle(Command, Timestamp) -> Result<Vec<Event>, Rejection>` ([`game.rs:34`](../../crates/chess-game/src/game.rs)) é o único escritor; `Game::replay` reconstrói do log. Interface principal de testes: [`full_games.rs`](../../crates/chess-game/tests/full_games.rs), [`replay_property.rs`](../../crates/chess-game/tests/replay_property.rs), [`event_serde.rs`](../../crates/chess-game/tests/event_serde.rs).
- [`Command`](../../crates/chess-game/src/command.rs) (`:8`) tem `Touch`, `Release`, `ChoosePromotion`, `PressClock`, `OfferDraw`, `AcceptDraw`, `DeclineDraw`, `Resign`, `Pause`, `Undo`, `Redo` e `EngineMove { uci }`. As fontes hoje são: gestos do usuário (via [`Session::gesture`](../../crates/chess-studio/src/session.rs), `:371`) e a Engine local (`EnginePlayer::drive`).
- Nenhum `Command` carrega o instante do Provedor, a confirmação ou o evento do adversário. `Timestamp` tem `mono_ms` e `wall_ms` opcional ([`time.rs:11`](../../crates/chess-game/src/time.rs)); não há campo de "relógio autoritativo remoto".
- **Costura (ADR-0007):** a fonte remota entra como novo módulo ao lado de `EnginePlayer`, chamando `Game::handle` com os mesmos tipos. Faltam (a) variantes de `Command` para "Lance do adversário confirmado" e "relógio/resultado do Provedor", (b) variantes de `EventKind` correspondentes (hoje `EventKind` cobre toque, Lance, Infração, pausa, bandeira, empate, resultado; `PAYLOAD_VERSION = 1`, [`event.rs:18`](../../crates/chess-game/src/event.rs)), (c) `EndReason` do Provedor (abandono, tempo do Provedor), em [`outcome.rs:28`](../../crates/chess-game/src/outcome.rs). Ambos os enums são `#[non_exhaustive]`, então ampliar não quebra quem usa. Como o log é imutável (ADR-0002), eventos novos exigem decidir a versão do payload.
- **Intenção pendente** (Lance local enviado, sem confirmação): não existe estado para isso. `Session` ([`session.rs:70`](../../crates/chess-studio/src/session.rs)) já separa a Posição visual provisória do log (ADR-0001); é o lugar natural para a "intenção enviada".

### 7.2 Relógio: EXISTE (local)

- O relógio é derivado dos instantes dos Eventos: [`clock.rs`](../../crates/chess-game/src/clock.rs) (`ClockView::at`, `Game::clock_at`), testes em [`clock.rs`](../../crates/chess-game/tests/clock.rs). Tempo injetado, sem `Instant::now()` no domínio (ADR-0002).
- Quem lê o relógio do sistema é `Session` ([`session.rs:363`](../../crates/chess-studio/src/session.rs), `SystemTime::now()` para `wall_ms`), camada de interface, como deve ser.
- Para a Partida online, o tempo vem do Provedor (ADR-0007): o `Timestamp` do evento remoto deve ser montado pelo módulo do Provedor, não por `Session::now`.

### 7.3 Políticas e Auxílios: PARCIAL

- [`PolicyKind`](../../crates/chess-game/src/setup.rs) tem `Assisted`, `FideSimulation`, `SuddenElimination` e é `#[non_exhaustive]`. Não existe Política "online" (CX-19: a Partida online não usa as Políticas locais).
- Auxílios: [`aids.rs`](../../crates/chess-studio/src/aids.rs) e a permissão [`Allowance`](../../crates/chess-studio/src/hud_policy.rs) (`:40`): `destinations`, `ghosts`, `suggestion_arrows`, `engine_line`, `hints`, `check`, `selection`, `last_move`. Só leitura (ADR-0001).
- **Costura da Política online:** `GameSetup` ([`setup.rs`](../../crates/chess-game/src/setup.rs)) é imutável e serializa em `SetupWire`; um valor novo de `PolicyKind` (ou campo opcional de Provedor no setup) é a marca "Partida online" no log. `GameSetup.engine: Option<EngineSeat>` é o modelo a seguir para um campo de adversário remoto.

### 7.4 Trava de fair play: PARCIAL (só na interface)

O que existe é um bloqueio **da interface**, para a Partida competitiva local:

- [`Session::competitive_in_progress`](../../crates/chess-studio/src/session.rs) (`:458`) alimenta `PanelAccess::OutsideCompetitive` ([`dock.rs:66`](../../crates/chess-studio/src/dock.rs), `PanelDef::accessible`, `:91`), `analysis::lock_reason` ([`analysis.rs:64`](../../crates/chess-studio/src/analysis.rs)), `openings::availability` e `Allowance`.
- A ADR-0007 exige o contrário: bloqueio na **camada de serviço**, sem chave de depuração. Hoje, `AnalysisController::start` ([`analysis.rs:193`](../../crates/chess-uci/src/analysis.rs)) e `Store::opening_stats` ([`openings.rs:82`](../../crates/chess-store/src/openings.rs)) não consultam nada: quem os chama é que checa. Quem ignorar o Painel os alcança.
- Não existe tipo `FairPlayGuard`, avaliação em cache, finais exatos nem Provedor para travar.
- **Costura:** um tipo único consultado antes de todo serviço que calcula. O ponto natural é um parâmetro obrigatório nas entradas `AnalysisController::start`/`analyze`, `EnginePlayer` (quando for a Engine de apoio), nas consultas do Explorador e nas futuras de cache e finais. A decisão (guarda ativa por Partida) sai do `GameSetup`/`Snapshot` da Partida online, que o `chess-game` conhece sem Bevy. Os testes negativos exigidos (ADR-0007) ficam na suíte do crate dono do serviço, como [`analysis.rs`](../../crates/chess-uci/tests/analysis.rs).

## 8. Rede, consentimento e segredos

### 8.1 Rede existente: PARCIAL

- Um único cliente: `ureq` 3 (`rustls`, sem recursos padrão) só em `chess-uci` ([`Cargo.toml`](../../crates/chess-uci/Cargo.toml)), usado no download da Engine ([`install.rs`](../../crates/chess-uci/src/install.rs), agente em `:279`): `timeout_connect` de 20 s, `https_only`, no máximo 5 redirecionamentos, SHA-256 conferido, corpo limitado, cancelável por `AtomicBool`, progresso por callback. Hosts permitidos: [`OFFICIAL_HOSTS`](../../crates/chess-uci/src/catalog.rs) e `is_official_url` (`:34`).
- O teste estático [`no_network_without_consent.rs`](../../crates/chess-studio/tests/no_network_without_consent.rs) (`:60`) varre `crates/*/src` e **falha** se aparecer `ureq::`, `TcpStream`, `reqwest` ou `std::net::` fora do instalador, e se qualquer outro crate declarar `ureq`, `reqwest`, `hyper`, `isahc`, `surf`, `attohttpc` (exceção apenas `chess-uci` + `ureq`).
- Testes do cliente com servidor HTTP falso em `TcpListener`: [`install.rs`](../../crates/chess-uci/tests/install.rs) (`serve`, `:67`) e [`manager.rs`](../../crates/chess-uci/tests/manager.rs). Esse é o padrão para a CX-17.
- Não existe: `User-Agent`, `Retry-After`, limite de taxa, cache com Origem, retentativa. Procurei `retry-after`, `rate.?limit` e `user.agent` em `crates/*/src`: nenhuma ocorrência.
- **Atenção (risco):** o teste estático é uma trava do projeto e será quebrado de propósito pela primeira integração. Ele precisa ser reescrito junto com a ADR-0006, e não contornado: o conteúdo é "só o módulo de rede (e o instalador) falam com a rede", não "só o instalador". O cabeçalho do teste ainda cita a ADR-0003 ("o único cliente de rede é o download de Engine"); a ADR-0006 já declarou essa frase superada.
- **Costura (CX-20, rede síncrona em threads):** crate novo de Provedores (por exemplo `chess-providers`), que depende de `ureq` como o `chess-uci` hoje e **não** de Bevy; `chess-uci` continua dono do download fixado. O modelo de thread + canal já está pronto em [`engine_manager.rs`](../../crates/chess-studio/src/engine_manager.rs) (`mpsc`, `poll` por quadro).

### 8.2 Consentimento: AUSENTE (por Provedor)

- Existe consentimento de **execução de Engine** ([`engine_consent.rs`](../../crates/chess-studio/src/engine_consent.rs)) e confirmação do download (painel com licença, fonte e tamanho no [gerenciador](../../crates/chess-studio/src/engine_manager.rs)). A barra de status mostra "Engine + consentimento" ([`EngineChip`](../../crates/chess-studio/src/status_bar.rs), `:64`).
- Não existe estado "Provedor ligado". Procurei `provider` e `connection` nas fontes de `crates/*/src`: nada.
- **Costura:** o estado nasce desligado por Provedor e por Perfil local, em `ui_preferences` via [`Store::get_pref`/`set_pref`](../../crates/chess-store/src/store.rs) (`:581`, `:594`), a chave JSON por Perfil. O aviso do que será enviado reaproveita o padrão de `Consent::launch`/`confirm` do `engine_consent`, que é pura (sem Bevy).

### 8.3 Cofre de segredos: AUSENTE

- Nenhuma dependência de cofre do SO ou de cifra no workspace: procurei `keyring`, `aes`, `chacha` e `argon` em todos os `Cargo.toml`. Só `sha2` ([`chess-uci/Cargo.toml`](../../crates/chess-uci/Cargo.toml)).
- Preferências ficam em JSON puro no SQLite (`ui_preferences`): **não servem para segredo**.
- **Costura:** trait `SecretVault` num crate novo, sem Bevy e sem SQLite, com duas implementações (SO; arquivo cifrado com senha-mestra) e uma falsa para teste. Dependências novas passam por `cargo deny check licenses` (ADR-0004). O `chess-store` guarda no máximo uma referência (id do segredo), nunca o valor.
- Observabilidade sem vazamento (US-030): o log UCI é limitado ([`LogEntry`](../../crates/chess-uci/src/protocol.rs)), mas não há redação de segredo, porque não há segredo. Quando houver, o log de rede nasce com redação.

## 9. Perfil local, preferências e Conexão

### 9.1 Perfil local: EXISTE

- [`Store::create_profile`, `rename_profile`, `delete_profile`, `set_active_profile`](../../crates/chess-store/src/store.rs); [`ProfileInfo`](../../crates/chess-store/src/store.rs) (`:25`). Dados isolados por Perfil (Partidas, preferências, Biblioteca, estatística): [`local_profiles.rs`](../../crates/chess-store/tests/local_profiles.rs).
- Interface: [`local_profiles.rs`](../../crates/chess-studio/src/local_profiles.rs) e página "Perfil local" dos bastidores. Não confundir com Perfil de interface ([`profiles.rs`](../../crates/chess-studio/src/profiles.rs)) nem Perfil de treino.

### 9.2 Preferências: EXISTE

- Chaves JSON por Perfil em `ui_preferences` ([`persistence.rs:38-77`](../../crates/chess-studio/src/persistence.rs): `aids`, `hud`, `presentation`, `quality`, `dock_layout`, `ribbon_state`, `ui_theme`, `ui_scale`…). `Persistence::load_prefs`/`sync_prefs` gravam na troca e ao fechar.
- **Costura:** novas chaves (`connections`, `network_status`) seguem o mesmo padrão.

### 9.3 Conexão: AUSENTE

- Não há entidade que ligue um Perfil local a um Provedor (modo de identificação, estado, escopos, tempo de autorização). Procurei `Connection`, `Provider` e `oauth` em `crates/*/src`: sem ocorrências de domínio.
- **Costura:** tabela `connections` (`profile_id`, `provider`, `kind`, `identifier` público, `state`, `secret_ref`) por migração no `chess-store`, com a regra do glossário: pode haver várias por Provedor; a do jogador e a de Engine não se misturam.

## 10. Interface

### 10.1 Lista de Ações: EXISTE

- Lista única em [`chess-input/src/actions.rs`](../../crates/chess-input/src/actions.rs): [`meta(Action)`](../../crates/chess-input/src/actions.rs) (`:111`) dá rótulo, descrição, ícone, KeyTip, [`RibbonTab`](../../crates/chess-input/src/actions.rs) e grupo; `state(action, &ActionContext)` dá habilitada, marcada ou o motivo. Ribbon, Barra de Acesso Rápido, KeyTips e Paleta consomem a mesma lista. [`Action`](../../crates/chess-input/src/controls.rs) (`:22`) é um enum; testes em [`tests/actions.rs`](../../crates/chess-input/tests/actions.rs).
- Já existem `OpenLibrary`, `OpenOpenings`, `ExportPgn`, `PanelEngines`, `PanelOpenings`, `PanelLibrary`. Não há ação de Provedor, Central ou Explorador.
- **Costura:** novas variantes de `Action` (`OpenConnections`, `PanelExplorer`, `ImportFromProvider`…) com `meta()` e `state()`. O `ActionContext` (`:398`) ganha os fatos de que `state()` precisa (rede ligada? Partida online em curso, para desabilitar com motivo `reason-*`).

### 10.2 Bastidores, Ribbon, barra de status, Painéis: EXISTE

- Bastidores: [`ribbon/backstage.rs`](../../crates/chess-studio/src/ribbon/backstage.rs). `BackstagePage` (`:20`) tem hoje `Profiles` e `LocalProfile`; `ENTRIES` (`:35`) lista 7 entradas (Nova, Abrir PGN, Biblioteca, Perfis, Perfil local, Controles, Opções). Especificação visual: [`Backstage.md`](../design/componentes/Backstage.md).
- **Costura da Central de Conexões (CX-06):** `BackstagePage::Connections` e uma entrada em `ENTRIES` (o array tem tamanho fixo `[_; 7]` e vira 8), com título em `page_title_id` e `label_id`. A aba Arquivo do Ribbon abre os bastidores ([`ribbon/mod.rs`](../../crates/chess-studio/src/ribbon/mod.rs), `KeyTipOutcome::OpenBackstage`).
- Painéis: registro em [`dock.rs`](../../crates/chess-studio/src/dock.rs) (`PanelRegistry`, `register_panel`, `ids`, `:48`). Painel novo entra no layout salvo como aba extra sem reiniciar o layout do usuário (comentário em `:122-124`).
- Barra de status: [`status_bar.rs`](../../crates/chess-studio/src/status_bar.rs). `StatusSnapshot` (`:43`) é montado sem Bevy e `status_text` é pura. **Costura do estado da rede:** campo novo no `StatusSnapshot` e um chip ao lado de `EngineChip`; sem rede ligada o chip diz "Offline".
- Detalhe sobre Painéis destacados: [`detached.rs`](../../crates/chess-studio/src/detached.rs). Painéis são só leitura da Partida; só `Session` recebe intenções.

### 10.3 i18n Fluent: EXISTE

- Arquivos `.ftl` em [`assets/i18n/pt-BR/`](../../crates/chess-studio/assets/i18n/pt-BR/) (24 arquivos, um por área: `ribbon.ftl`, `library.ftl`, `openings.ftl`, `manager.ftl`…), embutidos por `include_str!` em [`i18n.rs`](../../crates/chess-studio/src/i18n.rs) (`PT_BR`, `:10`). `I18n::t` e `t_with(key, args)`; chave ausente devolve a própria chave.
- **Costura:** `connections.ftl` e `explorer.ftl` entram na lista `PT_BR`. Mensagens de erro de rede e de Provedor passam por ids Fluent; os erros tipados de rede não carregam texto de interface (padrão de `InstallError` e `PgnError`).

## 11. Tabela US-001..US-032

Etapas (das [decisões](../decisoes/2026-11-11-conexoes.md)): **E1** Fundação, **E2** Dados públicos, **E3** Conexões com segredo, **E4** Partida online no Lichess, **E5** Mais redes. "Fora" = fora das cinco etapas. A situação diz o que o código tem **hoje** para a história.

| US | Título (spec) | Etapa | Situação | O que existe / falta |
|---|---|---|---|---|
| US-001 | Inventariar arquitetura | E1 | EXISTE | Este documento. |
| US-002 | Registro e roteamento de Provedores | E1 | AUSENTE | Sem tipo de Provedor; crate novo (8.1). |
| US-003 | Catálogo gratuito primeiro | E1 | AUSENTE | Catálogo só de Engines ([`catalog.rs`](../../crates/chess-uci/src/catalog.rs)); molde reaproveitável. |
| US-004 | Cofre de segredos | E3 | AUSENTE | 8.3. |
| US-005 | OAuth seguro no Lichess | E3 | AUSENTE | Sem navegador, redirecionamento local nem PKCE; `sha2` já está no workspace. |
| US-006 | Identidades Chess.com, Queen Alice, ChessBase, FIDE | E2 | AUSENTE | Só nome público e importação manual; importar PGN existe (5.3). |
| US-007 | FICS com transporte protegido | E5 | AUSENTE | Sem TCP/Telnet; CX-18. |
| US-008 | Multicontas, permissões, estados, revogação | E3 | AUSENTE | Perfil local existe (9.1); Conexão não (9.3). |
| US-009 | Stream de eventos e desafios Lichess | E4 | AUSENTE | Sem cliente de fluxo; thread + canal pronto (8.1). |
| US-010 | Jogar humano de ponta a ponta | E4 | AUSENTE | Falta fonte remota no controlador (7.1). |
| US-011 | Reconectar e recuperar estado | E4 | AUSENTE | `Game::replay` ajuda; falta reconciliar relógio e descartar eventos repetidos (7.1). |
| US-012 | Ações de partida (empate, desistência, abort, chat) | E4 | PARCIAL | `OfferDraw`, `AcceptDraw`, `DeclineDraw`, `Resign` existem localmente ([`command.rs`](../../crates/chess-game/src/command.rs)); abort e chat não. |
| US-013 | PlayStrategy humano | E5 | AUSENTE | Depende de E4. |
| US-014 | Cliente FICS | E5 | AUSENTE | 1.1 (variantes) e 8.1. |
| US-015 | Conta BOT / Engine online | E5 | AUSENTE | Partida contra Engine local existe ([`EnginePlayer`](../../crates/chess-uci/src/player.rs)); CX-12 diz "Engine online". |
| US-016 | Interface uniforme para várias Engines UCI | E1 | EXISTE | 3.1 e 3.2; `uci-probe`, Stockfish e Lc0 no catálogo. |
| US-017 | Engine remota gratuita para análise | E3 | AUSENTE | 3.3. |
| US-018 | Importar e exportar PGN robustamente | E2 | PARCIAL | 2.1 completo; faltam arquivo grande, encoding, tags (2.2 a 2.4). |
| US-019 | Origem, direitos e deduplicação | E2 | PARCIAL | `source` livre (5.4); deduplicação ausente (2.5). |
| US-020 | Explorar aberturas por posição | E2 | PARCIAL | Explorador local pronto (4.2); Provedores ausentes (4.3). |
| US-021 | Nomes de aberturas e ECO | E1 | EXISTE | 4.1. |
| US-022 | Pesquisa local por jogador, evento, posição | E1 | EXISTE | 5.2. |
| US-023 | Importar PGN Chess.com, Queen Alice, ChessBase | E2 | PARCIAL | Importa qualquer PGN; faltam guia por origem e API pública do Chess.com. |
| US-024 | Cloud Eval gratuito | E2 | AUSENTE | Sem avaliação em cache; Painel Análise ([`analysis.rs`](../../crates/chess-studio/src/analysis.rs)) tem onde mostrar a dica separada (CX-11). |
| US-025 | Tablebase exata | E2 | AUSENTE | Aba Finais do Painel Explorador (4.3). |
| US-026 | Rating FIDE | E2 | AUSENTE | `ProfileInfo` só tem nome ([`store.rs:25`](../../crates/chess-store/src/store.rs)). |
| US-027 | Cota, gratuidade e fallback | E1 | AUSENTE | Camada de rede (8.1); CX-04 "custo zero". |
| US-028 | Fair play inviolável por vias de serviço | E4 | PARCIAL | Só na interface (7.4); a trava deve estar pronta antes do jogo online (CX-03). |
| US-029 | Offline, falhas e acessibilidade | E1 | PARCIAL | Offline por padrão já é verdade e testada (8.1); falta o estado "Offline" visível, erro de rede tipado e Partida intacta sob falha. |
| US-030 | Observabilidade sem vazar segredo | E1 | PARCIAL | Log UCI limitado (8.3); sem log de rede nem redação. |
| US-031 | Testes contratualizados | E1 | PARCIAL | Servidor HTTP falso em `TcpListener` e `fake_uci` (3.1, 8.1); sem gravação de respostas (CX-17). |
| US-032 | Parcerias futuras | Fora | AUSENTE | Cartões "requer aprovação" no catálogo (CX-04), sem código. |

## 12. Pontos de atenção para quem implementa

1. **Teste estático de rede.** [`no_network_without_consent.rs`](../../crates/chess-studio/tests/no_network_without_consent.rs) fará o primeiro PR de rede falhar. Reescrevê-lo (módulo de rede permitido, o resto proibido, `ureq` só nos crates declarados) é parte da E1, não uma exceção.
2. **Ordem do PGN.** Origem, deduplicação, tags e streaming mexem todos em `imported_games` e `import_pgn_for`. Uma migração única (13) para as colunas evita brigar por versão de esquema.
3. **Log imutável.** Evento e `Command` novos para a Partida online mexem no formato de ADR-0002 (`PAYLOAD_VERSION`). Decidir antes de codificar se a versão sobe.
4. **Trava de fair play** hoje é só de interface (7.4). Vale desenhá-la antes do Explorador de Provedores da E2, para o serviço nascer já consultando a guarda.
5. **Perfil de interface não é Perfil local.** As preferências de Conexão são do Perfil local (`ui_preferences`), não do Perfil de interface.
6. **Licenças.** Todo crate novo (cofre, cifra, OAuth, TLS) passa por `cargo deny check licenses` (ADR-0004). Dados CC0 recebem linha em [`assets/LICENSES.md`](../../crates/chess-studio/assets/LICENSES.md).
