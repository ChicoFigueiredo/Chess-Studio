# Estado atual para as integrações (inventário CX-1, atualizado em 11/11/2026)

Inventário do código em `main` para o programa de Provedores e Conexões. A primeira versão (commit `5c27ec8`) cumpriu a "fase zero" da [especificação](especificacao-free-first-2026-11-11.md) (seção 5.1). Esta versão foi atualizada no commit `1626ebb`, depois das etapas 1 e 2, da autorização do Lichess (etapa 3) e do domínio da Partida online (etapa 4, sem interface). Valem as [decisões](../decisoes/2026-11-11-conexoes.md), a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md) e a [ADR-0007](../adr/0007-partida-online-controlador-segue-o-provedor.md). Quem usa o app lê o [guia](guia.md); quem homologa lê o [roteiro](../homologacao/integracoes.md).

Só documentação. Cada afirmação aponta o arquivo onde o código ou o teste mora. Não há números de linha: eles envelhecem a cada PR.

## Como ler

- **EXISTE**: pronto e testado na suíte (servidor falso, relógio falso, cofre falso).
- **PARCIAL**: existe, mas falta algo; o texto diz o quê.
- **AUSENTE**: sem código.
- **Verificação com serviço real**: a suíte nunca chama o serviço real ([CX-17](../decisoes/2026-11-11-conexoes.md)). Tudo o que fala com um Provedor de verdade está **UNVERIFIED** até o usuário percorrer o [roteiro](../homologacao/integracoes.md).
- Links são relativos a este arquivo.

## Resumo

| Capacidade | Situação | Onde |
|---|---|---|
| PGN: leitura, escrita, tags, variações | EXISTE | [seção 2.1](#21-leitura-e-escrita-existe) |
| PGN: arquivos grandes em fluxo, encodings, tags | EXISTE | [2.2 a 2.4](#22-tags-existe) |
| PGN: duplicatas | EXISTE | [2.5](#25-duplicatas-existe) |
| PGN: comentários `[%clk]` e `[%eval]` | AUSENTE | [2.6](#26-comentários-clk-e-eval-ausente) |
| FEN | EXISTE | [1.1](#11-fen-existe) |
| UCI e gerenciador de Engines | EXISTE | [3.1 e 3.2](#31-protocolo-e-processo-existe) |
| Engine remota | AUSENTE | [3.3](#33-engine-remota-ausente) |
| Aberturas e ECO (offline) e Explorador local | EXISTE | [4.1 e 4.2](#41-livro-offline-existe) |
| Explorador de Provedores, avaliação em cache, finais | EXISTE (UNVERIFIED com serviço real) | [4.3](#43-explorador-de-provedores-e-finais-existe) |
| Biblioteca: esquema, busca, importação, Origem | EXISTE | [5](#5-biblioteca-chess-store) |
| Importação online (Lichess, Chess.com) | EXISTE (UNVERIFIED com serviço real) | [5.3](#53-importação-existe) |
| Treino: pacotes de Exercícios e leitor do Lichess | EXISTE | [6.2](#62-importar-exercícios-novos-existe) |
| Download de pacotes pela rede | AUSENTE | [6.2](#62-importar-exercícios-novos-existe) |
| Partida online: domínio e Board API | EXISTE (sem interface) | [7.1](#71-controlador-e-fontes-de-comando-existe) |
| Partida online: interface | AUSENTE | [7.1](#71-controlador-e-fontes-de-comando-existe) |
| Trava de fair play | PARCIAL | [7.4](#74-trava-de-fair-play-parcial) |
| Camada de rede por Provedor | EXISTE | [8.1](#81-rede-existe) |
| Consentimento por Provedor | EXISTE | [8.2](#82-consentimento-existe) |
| Cofre de segredos | EXISTE (UNVERIFIED com serviço real) | [8.3](#83-cofre-de-segredos-existe) |
| Autorização do Lichess | EXISTE (UNVERIFIED com serviço real) | [8.4](#84-autorização-do-lichess-existe) |
| Conexão (vínculo Perfil local e Provedor) | EXISTE | [9.3](#93-conexão-existe) |
| Central de Conexões, Painel Explorador, barra de status | EXISTE | [10](#10-interface) |
| Engine online, PlayStrategy, FICS | AUSENTE | [11](#11-tabela-us-001us-032) |

## 1. Regras e núcleo

### 1.1 FEN: EXISTE

- Leitura e escrita: [`CanonicalPosition::from_fen` e `to_fen`](../../crates/chess-core/src/position.rs). FEN inválido é erro tipado, nunca pânico. Teste: [`tests/fen.rs`](../../crates/chess-core/tests/fen.rs). Perft: [`tests/perft.rs`](../../crates/chess-core/tests/perft.rs).
- A tag `FEN` do PGN passa pelo mesmo caminho ([`import.rs`](../../crates/chess-pgn/tests/import.rs)).
- **Só xadrez padrão.** A sessão do Lichess recusa variante antes de criar a Partida ([`board/session.rs`](../../crates/chess-providers/src/board/session.rs), [`setup.rs`](../../crates/chess-game/src/setup.rs)); a importação do Chess.com descarta Partidas de variante ([`pgn.rs`](../../crates/chess-providers/src/pgn.rs)). Chess960 e outras variantes (Provedores como PlayStrategy e FICS) não têm base no núcleo ([`chess-core`](../../crates/chess-core/src/position.rs)).

### 1.2 Chave de Posição para estatística e transposição: EXISTE

- [`position_key` e `fen_key`](../../crates/chess-store/src/library.rs): FEN sem contadores, en passant só com captura legal. O Explorador de Provedores e o cache usam o FEN completo no pedido ([`explorer.rs`](../../crates/chess-providers/src/explorer.rs)).
- No livro: [`RepetitionKey`](../../crates/chess-openings/src/lib.rs).

## 2. PGN

### 2.1 Leitura e escrita: EXISTE

- Parser próprio, sem GPL ([`parser.rs`](../../crates/chess-pgn/src/parser.rs), [`lexer.rs`](../../crates/chess-pgn/src/lexer.rs)). Árvore com variações, comentários e NAGs: [`model.rs`](../../crates/chess-pgn/src/model.rs).
- Duas entradas: `parse_pgn` (falha no primeiro erro) e `parse_pgn_lenient` (pula a Partida ruim e relata linha e coluna), ambas em [`parser.rs`](../../crates/chess-pgn/src/parser.rs). Cada Lance passa pelo `chess-core` ([`error.rs`](../../crates/chess-pgn/src/error.rs)).
- Escrita: [`export.rs`](../../crates/chess-pgn/src/export.rs) para árvores importadas e [`pgn.rs`](../../crates/chess-game/src/pgn.rs) para a Partida do app. Árvore importada vira Partida Assistida em [`convert.rs`](../../crates/chess-pgn/src/convert.rs).
- Testes: [`import.rs`](../../crates/chess-pgn/tests/import.rs), [`roundtrip.rs`](../../crates/chess-pgn/tests/roundtrip.rs), [`pgn_export.rs`](../../crates/chess-game/tests/pgn_export.rs).

### 2.2 Tags: EXISTE

- Todas as tags ficam no modelo ([`model.rs`](../../crates/chess-pgn/src/model.rs)) e a árvore vai em JSON para `imported_games`.
- A migração 13 acrescentou colunas pesquisáveis: Provedor, modo de importação, instante, licença, URL, id remoto, impressão digital, `site`, Elo das Brancas e das Pretas, ritmo (`time_control`, `pace`) e ECO ([`migrations.rs`](../../crates/chess-store/src/migrations.rs), `V13_IMPORT_ORIGIN`). Teste: [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs).
- A Biblioteca filtra por origem, ECO e ritmo ([`library.rs`](../../crates/chess-store/src/library.rs), [`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs)).
- A exportação do app ainda fixa `Event`, `White`, `Black` e `Site` ([`session.rs`](../../crates/chess-studio/src/session.rs)); as tags de uma Partida importada são preservadas ao exportar ([`pgn_export.rs`](../../crates/chess-game/tests/pgn_export.rs)).

### 2.3 Arquivos grandes: EXISTE

- Leitura em fluxo, linha a linha, uma Partida em memória, com limite de linha e de Partida: [`stream.rs`](../../crates/chess-pgn/src/stream.rs). Testes: [`stream.rs`](../../crates/chess-pgn/tests/stream.rs), [`stream_boundaries.rs`](../../crates/chess-pgn/tests/stream_boundaries.rs).
- Importação em lotes por transação, com progresso e cancelamento: `import_pgn_stream` e `import_pgn_file` em [`imported.rs`](../../crates/chess-store/src/imported.rs). `.pgn.gz` é detectado pelos bytes. A interface importa numa thread ([`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs)).
- Desempenho de referência: [`perf.rs`](../../crates/chess-pgn/tests/perf.rs).

### 2.4 Encodings: EXISTE

- UTF-8 com BOM e CRLF ([`import.rs`](../../crates/chess-pgn/tests/import.rs)). Windows-1252 e Latin-1 são convertidos por conversão própria, com aviso ao usuário ([`stream.rs`](../../crates/chess-pgn/src/stream.rs), texto `library-import-converted` em [`library.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/library.ftl)).

### 2.5 Duplicatas: EXISTE

- Impressão digital de 128 bits (jogadores normalizados, data, resultado, FEN inicial e Lances da linha principal) e o par (Provedor, id remoto) para Lichess e Chess.com: [`origin.rs`](../../crates/chess-store/src/origin.rs). Índice único por Perfil local em `V13_IMPORT_ORIGIN` ([`migrations.rs`](../../crates/chess-store/src/migrations.rs)).
- Reimportar não duplica. A mesma Partida vinda de dois Provedores é uma só, com as duas Origens (tabela `imported_game_origins`). O resumo diz novas, repetidas e com erro ([`ImportReport`](../../crates/chess-store/src/imported.rs)). Teste: [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs).

### 2.6 Comentários `[%clk]` e `[%eval]`: AUSENTE

- O parser trata `[%clk ...]` apenas para não dividir Partidas em arquivos grandes ([`stream.rs`](../../crates/chess-pgn/src/stream.rs)). Nenhum código interpreta o relógio nem a avaliação dos comentários ([`model.rs`](../../crates/chess-pgn/src/model.rs), `MoveNode.comments`). Os comentários passam como texto.

## 3. Engines e UCI

### 3.1 Protocolo e processo: EXISTE

- Processo filho com handshake, timeouts e log limitado: [`engine.rs`](../../crates/chess-uci/src/engine.rs), [`protocol.rs`](../../crates/chess-uci/src/protocol.rs), [`controller.rs`](../../crates/chess-uci/src/controller.rs).
- Todo Lance da Engine é só proposta e passa pelo núcleo ([`engine.rs`](../../crates/chess-uci/src/engine.rs), ADR-0003). Análise com várias linhas: [`analysis.rs`](../../crates/chess-uci/src/analysis.rs). Jogador de Engine: [`player.rs`](../../crates/chess-uci/src/player.rs).
- Servidor falso: [`fake_uci.rs`](../../crates/chess-uci/src/bin/fake_uci.rs). Testes: [`player.rs`](../../crates/chess-uci/tests/player.rs), [`analysis.rs`](../../crates/chess-uci/tests/analysis.rs), [`adapter.rs`](../../crates/chess-uci/tests/adapter.rs).

### 3.2 Gerenciador de Engines: EXISTE

- Catálogo fixado com SHA-256 ([`catalog.rs`](../../crates/chess-uci/src/catalog.rs)), detecção ([`detect.rs`](../../crates/chess-uci/src/detect.rs)), instalação e remoção ([`manager.rs`](../../crates/chess-uci/src/manager.rs)), backends ([`backend.rs`](../../crates/chess-uci/src/backend.rs)).
- Registro no banco: [`engines.rs`](../../crates/chess-store/src/engines.rs); teste [`engines.rs`](../../crates/chess-store/tests/engines.rs).
- Interface: [`engine_manager.rs`](../../crates/chess-studio/src/engine_manager.rs), [`engine_wizard.rs`](../../crates/chess-studio/src/engine_wizard.rs), [`engine.rs`](../../crates/chess-studio/src/engine.rs). Segurança: [`seguranca-engines.md`](../decisoes/seguranca-engines.md). Consentimento de execução: [`engine_consent.rs`](../../crates/chess-studio/src/engine_consent.rs).

### 3.3 Engine remota: AUSENTE

- A `EngineConfig` ([`config.rs`](../../crates/chess-uci/src/config.rs)) descreve só um executável local. Os serviços de análise remota estão no catálogo como cartões "Em breve" ou "Requer aprovação" ([`catalog.rs`](../../crates/chess-net/src/catalog.rs)), sem adaptador em [`chess-providers`](../../crates/chess-providers/src/lib.rs).
- A "Engine online" tem só a autorização separada (seção 8.4); nada joga com ela.

## 4. Aberturas e ECO

### 4.1 Livro offline: EXISTE

- [`chess-openings`](../../crates/chess-openings/src/lib.rs): `OpeningBook::embedded()`, dados CC0 do `lichess-org/chess-openings`; licença em [`LICENSES.md`](../../crates/chess-studio/assets/LICENSES.md). Teste: [`book.rs`](../../crates/chess-openings/tests/book.rs). Painel: [`openings.rs`](../../crates/chess-studio/src/openings.rs).

### 4.2 Explorador local: EXISTE

- `Store::opening_stats` em [`openings.rs`](../../crates/chess-store/src/openings.rs) dá a estatística por Lance sobre a Biblioteca do Perfil local. Teste: [`openings.rs`](../../crates/chess-store/tests/openings.rs).
- É a fonte **Minha Biblioteca** do Painel Explorador (seção 4.3), sem rede.

### 4.3 Explorador de Provedores e finais: EXISTE

- Adaptadores: Explorador de mestres, de todos os jogadores e de um jogador (fluxo NDJSON com resultados parciais) em [`explorer.rs`](../../crates/chess-providers/src/explorer.rs); finais de até sete peças em [`tablebase.rs`](../../crates/chess-providers/src/tablebase.rs); avaliação em cache em [`eval.rs`](../../crates/chess-providers/src/eval.rs). Testes com servidor falso e respostas gravadas: [`explorer.rs`](../../crates/chess-providers/tests/explorer.rs), [`lichess_queries.rs`](../../crates/chess-providers/tests/lichess_queries.rs), [`fixtures/`](../../crates/chess-providers/tests/fixtures).
- O Explorador do Lichess responde sem autorização com erro; por isso as três fontes exigem Conexão autorizada. O adaptador devolve `SignInRequired` antes da rede ([`error.rs`](../../crates/chess-providers/src/error.rs), [`lichess.rs`](../../crates/chess-providers/src/lichess.rs)). Decisões: [`cofre-e-autorizacao.md`](../decisoes/2026-11-11-cofre-e-autorizacao.md).
- Painel Explorador (abas Aberturas e Finais, fonte, filtros de ritmo e rating, Origem, estados) e a linha "Em cache (Lichess)" do Painel Análise: [`explorer/mod.rs`](../../crates/chess-studio/src/explorer/mod.rs), [`explorer/view.rs`](../../crates/chess-studio/src/explorer/view.rs), [`explorer/driver.rs`](../../crates/chess-studio/src/explorer/driver.rs), [`explorer/prefs.rs`](../../crates/chess-studio/src/explorer/prefs.rs), [`explorer.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/explorer.ftl). Diretriz visual: [`ExplorerPanel.md`](../design/componentes/ExplorerPanel.md). Capturas: [`evidencias/explorador`](../evidencias/explorador/README.md).
- **UNVERIFIED com serviço real:** itens [CX-HOM-15 a CX-HOM-20](../homologacao/integracoes.md).

## 5. Biblioteca (`chess-store`)

### 5.1 Esquema e Perfil: EXISTE

- SQLite com migrações em `PRAGMA user_version` ([`migrations.rs`](../../crates/chess-store/src/migrations.rs)): **16 versões**. As novas: 13 (Origem, duplicatas, metadados), 14 (pacotes de Exercícios), 15 (Conexões) e 16 (Conexões autorizadas, `AUTOINCREMENT`). Testes de migração: [`migration.rs`](../../crates/chess-store/tests/migration.rs), [`undo_reopens.rs`](../../crates/chess-store/tests/undo_reopens.rs).
- Log de eventos como fonte de verdade (ADR-0002): [`store.rs`](../../crates/chess-store/src/store.rs); teste de queda em [`crash.rs`](../../crates/chess-store/tests/crash.rs).

### 5.2 Busca: EXISTE

- `Store::search_library` com texto livre, jogador, resultado, Política, datas, Posição, origem, Provedor, ECO e ritmo: [`library.rs`](../../crates/chess-store/src/library.rs). Testes: [`library.rs`](../../crates/chess-store/tests/library.rs), [`library_profiles.rs`](../../crates/chess-store/tests/library_profiles.rs).

### 5.3 Importação: EXISTE

- Arquivo: [`imported.rs`](../../crates/chess-store/src/imported.rs) e a interface em [`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs) (fonte do arquivo: Queen Alice, ChessBase, TWIC, PGN Mentor, Lichess, Chess.com, outro; nota de licença; andamento; Cancelar). Testes: [`pgn_import.rs`](../../crates/chess-store/tests/pgn_import.rs), [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs).
- Online: Partidas públicas do Lichess e do Chess.com sob demanda, com período, ritmo, cor, máximo e **Atualizar**, numa thread de fundo: [`online_import.rs`](../../crates/chess-studio/src/online_import.rs), [`online_import_ui.rs`](../../crates/chess-studio/src/online_import_ui.rs), adaptadores em [`games.rs`](../../crates/chess-providers/src/games.rs) e [`chesscom.rs`](../../crates/chess-providers/src/chesscom.rs). Testes: [`games.rs`](../../crates/chess-providers/tests/games.rs).
- **UNVERIFIED com serviço real:** [CX-HOM-22 a CX-HOM-24](../homologacao/integracoes.md).

### 5.4 Origem dos dados: EXISTE

- Tipos `Provenance`, `ImportMode`, `ImportSource`, `Origin`, `Pace`: [`origin.rs`](../../crates/chess-store/src/origin.rs). A Origem aparece no detalhe da Partida ([`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs)) e no rodapé do Explorador ([`explorer/view.rs`](../../crates/chess-studio/src/explorer/view.rs)). A resposta de rede carrega a Origem ([`answer.rs`](../../crates/chess-providers/src/answer.rs)).

## 6. Treino e Exercícios

### 6.1 Formato e catálogo: EXISTE

- [`Exercise`](../../crates/chess-training/src/exercise.rs) ganhou campos opcionais com `serde(default)`: temas, rating, Origem e pacote. Existe `ExerciseKind::Tactic`. O catálogo de fábrica carrega sem mudança ([`catalog.rs`](../../crates/chess-training/src/catalog.rs)). Testes: [`catalog.rs`](../../crates/chess-training/tests/catalog.rs), [`content.rs`](../../crates/chess-training/tests/content.rs).

### 6.2 Importar Exercícios novos: EXISTE

- Leitor de problemas do Lichess em fluxo (`.csv` e `.csv.zst`), com seleção por tema, dificuldade e quantidade, e cancelamento: [`lichess.rs`](../../crates/chess-training/src/lichess.rs). Teste: [`lichess.rs`](../../crates/chess-training/tests/lichess.rs).
- Pacotes por Perfil local no disco (instalar, listar, remover): [`packs.rs`](../../crates/chess-training/src/packs.rs). Filtros: [`filter.rs`](../../crates/chess-training/src/filter.rs). Teste: [`packs.rs`](../../crates/chess-training/tests/packs.rs).
- Remover um pacote oculta os Exercícios já resolvidos e mantém as Resoluções: [`training_packs.rs`](../../crates/chess-store/tests/training_packs.rs).
- Interface (Treino > Pacotes, filtros, seção por pacote): [`training_ui/packs.rs`](../../crates/chess-studio/src/training_ui/packs.rs), [`training.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/training.ftl).
- **Falta:** baixar o pacote pela rede (o serviço `Download` do Lichess está como "Em breve" no [`catalog.rs`](../../crates/chess-net/src/catalog.rs)) e um seletor de arquivo do sistema (o caminho é digitado).

## 7. Partida, relógio e arbitragem

### 7.1 Controlador e fontes de comando: EXISTE

- `Game::handle(Command, Timestamp)` segue o único escritor ([`game.rs`](../../crates/chess-game/src/game.rs)).
- **Domínio da Partida online (#230):** `PolicyKind::Online`, `OnlineGame` no setup, `Command::Provider(ProviderSnapshot)`, intenção enviada fora do log, reconciliação "tudo ou nada", relógio e fim do Provedor, `GameAborted`: [`online.rs`](../../crates/chess-game/src/online.rs), [`setup.rs`](../../crates/chess-game/src/setup.rs), [`command.rs`](../../crates/chess-game/src/command.rs), [`event.rs`](../../crates/chess-game/src/event.rs). Testes: [`online_game.rs`](../../crates/chess-game/tests/online_game.rs), [`online_property.rs`](../../crates/chess-game/tests/online_property.rs). Decisões: [`partida-online.md`](../decisoes/2026-11-11-partida-online.md).
- **Board API do Lichess:** [`board/mod.rs`](../../crates/chess-providers/src/board/mod.rs) (só Conexão de jogador), [`session.rs`](../../crates/chess-providers/src/board/session.rs) (`poll()` por quadro, envio único, reconexão), [`link.rs`](../../crates/chess-providers/src/board/link.rs), [`json.rs`](../../crates/chess-providers/src/board/json.rs). Testes: [`board_session.rs`](../../crates/chess-providers/tests/board_session.rs), [`board_actions.rs`](../../crates/chess-providers/tests/board_actions.rs).
- **Falta a interface:** nenhum código de `chess-studio` abre o fluxo de eventos, mostra desafios, cria a Partida online nem envia Lances. A lista do que a interface fará está no fim de [`partida-online.md`](../decisoes/2026-11-11-partida-online.md).

### 7.2 Relógio: EXISTE

- Derivado dos instantes dos Eventos ([`clock.rs`](../../crates/chess-game/src/clock.rs), teste [`clock.rs`](../../crates/chess-game/tests/clock.rs)). Online, `ClockReported` substitui o relógio derivado pelo do Provedor ([`online.rs`](../../crates/chess-game/src/online.rs)).

### 7.3 Políticas e Auxílios: EXISTE

- `PolicyKind::Online` existe ao lado das três locais ([`setup.rs`](../../crates/chess-game/src/setup.rs), [`policy.rs`](../../crates/chess-game/src/policy.rs)). A Partida online não usa as Políticas de arbitragem locais (CX-19). Auxílios: [`aids.rs`](../../crates/chess-studio/src/aids.rs), [`hud_policy.rs`](../../crates/chess-studio/src/hud_policy.rs).

### 7.4 Trava de fair play: PARCIAL

- **Existe na camada de rede:** [`FairPlayGuard`](../../crates/chess-net/src/guard.rs) com `engage`, `release` e `hold` (a trava só cai quando a última Partida solta). O gate recusa as capacidades que calculam (Explorador, finais, avaliação em cache, análise remota) com `FairPlayLocked` ([`catalog.rs`](../../crates/chess-net/src/catalog.rs), `computes`; teste [`gate.rs`](../../crates/chess-net/tests/gate.rs)).
- **Gatilho hoje:** o app liga a trava durante uma **Partida competitiva local** em andamento ([`net_service.rs`](../../crates/chess-studio/src/net_service.rs), `follow_competitive`). A barra de status mostra "Trava de fair play" ([`status_bar.rs`](../../crates/chess-studio/src/status_bar.rs)).
- **Domínio:** `Game::online_human_in_progress()` diz quando há Partida online contra uma pessoa ([`game.rs`](../../crates/chess-game/src/game.rs)); a sessão do Lichess segura um `FairPlayHold` ([`session.rs`](../../crates/chess-providers/src/board/session.rs)). Nada disso é acionado pela interface ainda.
- **Falta:** a Engine local e a análise (`AnalysisController::start`, [`analysis.rs`](../../crates/chess-uci/src/analysis.rs)) ainda não consultam a trava; o bloqueio delas é só da interface ([`analysis.rs`](../../crates/chess-studio/src/analysis.rs), [`dock.rs`](../../crates/chess-studio/src/dock.rs)). A própria decisão registra isso: [`partida-online.md`](../decisoes/2026-11-11-partida-online.md).

## 8. Rede, consentimento e segredos

### 8.1 Rede: EXISTE

- Crate [`chess-net`](../../crates/chess-net/src/lib.rs), a única porta de saída para Provedores, sem Bevy nem egui. Todo pedido passa por um gate ([`net.rs`](../../crates/chess-net/src/net.rs)): consentimento, endereço HTTPS no catálogo, serviço liberado, Trava de fair play, autorização do Cofre. Depois: fila por Provedor, cache com Origem, limite de taxa (GCRA), `Retry-After`, backoff com jitter, recusa de redirecionamento ([`limits.rs`](../../crates/chess-net/src/limits.rs), [`store.rs`](../../crates/chess-net/src/store.rs), [`request.rs`](../../crates/chess-net/src/request.rs), [`response.rs`](../../crates/chess-net/src/response.rs)).
- O único uso de `ureq` está em [`transport.rs`](../../crates/chess-net/src/transport.rs). O download da Engine segue em [`install.rs`](../../crates/chess-uci/src/install.rs).
- Catálogo com 19 Provedores, serviços, endereços permitidos, licença do dado, o que é enviado e o estado de cada serviço (pronto, em breve, importação manual, requer aprovação): [`catalog.rs`](../../crates/chess-net/src/catalog.rs). Teste: [`catalog.rs`](../../crates/chess-net/tests/catalog.rs).
- O teste estático [`no_network_without_consent.rs`](../../crates/chess-studio/tests/no_network_without_consent.rs) foi reescrito: só o módulo de rede, o instalador de Engines e a escuta em `127.0.0.1` (seção 8.4) falam com a rede.
- Testes: servidor falso e respostas gravadas ([`testing/`](../../crates/chess-net/src/testing/mod.rs), [`recorded.rs`](../../crates/chess-net/tests/recorded.rs), [`fixtures/`](../../crates/chess-net/tests/fixtures)), [`cache.rs`](../../crates/chess-net/tests/cache.rs), [`limits.rs`](../../crates/chess-net/tests/limits.rs), [`timeout_cancel.rs`](../../crates/chess-net/tests/timeout_cancel.rs).
- Estado da rede para a interface, com diário sem segredo: [`status.rs`](../../crates/chess-net/src/status.rs). A interface o consome por [`net_service.rs`](../../crates/chess-studio/src/net_service.rs), que abre `rede.db` ao lado do banco das Partidas.

### 8.2 Consentimento: EXISTE

- Por Provedor e por Perfil local, desligado por padrão, gravado em `rede.db` ([`store.rs`](../../crates/chess-net/src/store.rs)). Nenhuma conexão se abre para um Provedor desligado. Teste: [`consent.rs`](../../crates/chess-net/tests/consent.rs).
- Interruptor "Ligado" na Central e diálogo "Ligar <Provedor>?" para os recursos: [`provider_consent.rs`](../../crates/chess-studio/src/provider_consent.rs), [`connections/view.rs`](../../crates/chess-studio/src/connections/view.rs). O consentimento de execução de Engine segue separado ([`engine_consent.rs`](../../crates/chess-studio/src/engine_consent.rs)).

### 8.3 Cofre de segredos: EXISTE

- Crate [`chess-vault`](../../crates/chess-vault/src/lib.rs): cofre do sistema atrás do trait `OsKeyring` ([`keyring.rs`](../../crates/chess-vault/src/keyring.rs)) e cofre próprio cifrado com senha-mestra ([`own.rs`](../../crates/chess-vault/src/own.rs)), com índice de referências sem segredo ([`index.rs`](../../crates/chess-vault/src/index.rs)). Nunca troca de cofre em silêncio. `Secret` zera a memória e não imprime ([`secret.rs`](../../crates/chess-net/src/secret.rs)).
- Testes: [`vault.rs`](../../crates/chess-vault/tests/vault.rs), [`own_vault.rs`](../../crates/chess-vault/tests/own_vault.rs), [`static_secrets.rs`](../../crates/chess-vault/tests/static_secrets.rs) (inclui a ausência de segredo em log). A suíte usa um cofre falso ([`testing.rs`](../../crates/chess-vault/src/testing.rs)).
- Na interface: faixa do Cofre, diálogo da senha-mestra e campo mascarado ([`connections/view_auth.rs`](../../crates/chess-studio/src/connections/view_auth.rs), [`secrets.rs`](../../crates/chess-studio/src/secrets.rs), [`authorization.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/authorization.ftl)). Decisões: [`cofre-e-autorizacao.md`](../decisoes/2026-11-11-cofre-e-autorizacao.md) e [`autorizar-lichess-na-central.md`](../decisoes/2026-11-11-autorizar-lichess-na-central.md).
- **UNVERIFIED com o cofre real do sistema:** [CX-HOM-07 e CX-HOM-08](../homologacao/integracoes.md).

### 8.4 Autorização do Lichess: EXISTE

- Fluxo pelo navegador com verificador e desafio, `state` comparado em tempo constante e permissão mínima por papel (jogador ou Engine, nunca os dois): [`auth.rs`](../../crates/chess-providers/src/auth.rs). Chave pessoal, teste, revogação (apaga o segredo local sempre e informa a falha remota): o mesmo arquivo. Testes: [`auth.rs`](../../crates/chess-providers/tests/auth.rs).
- A única escuta do app, em `127.0.0.1`, porta efêmera, um retorno só e prazo: [`loopback.rs`](../../crates/chess-net/src/loopback.rs). Teste: [`loopback.rs`](../../crates/chess-net/tests/loopback.rs).
- Regra de abertura do navegador (só a página de autorização do Lichess): [`connections/browser.rs`](../../crates/chess-studio/src/connections/browser.rs). Fluxo na Central, com reserva da Conexão antes do navegador, "Autorização expirada" e "Autorizar de novo": [`connections/auth.rs`](../../crates/chess-studio/src/connections/auth.rs), testes em [`auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs).
- Entrada separada para a Engine online (outra Conexão, outra permissão, outro segredo): [`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs). Nada usa essa Conexão ainda.
- Capturas: [`evidencias/autorizar-lichess`](../evidencias/autorizar-lichess/README.md).
- **UNVERIFIED com o Lichess real:** [CX-HOM-09 a CX-HOM-13](../homologacao/integracoes.md).

## 9. Perfil local, preferências e Conexão

### 9.1 Perfil local: EXISTE

- Criar, renomear, excluir e trocar: [`store.rs`](../../crates/chess-store/src/store.rs). Dados isolados por Perfil: [`local_profiles.rs`](../../crates/chess-store/tests/local_profiles.rs). Interface: [`local_profiles.rs`](../../crates/chess-studio/src/local_profiles.rs).
- Apagar um Perfil local ou uma Conexão apaga antes o segredo no Cofre; se o Cofre recusa, nada é removido ([`authorized_connections.rs`](../../crates/chess-store/tests/authorized_connections.rs), [`autorizar-lichess-na-central.md`](../decisoes/2026-11-11-autorizar-lichess-na-central.md), AZ-06).

### 9.2 Preferências: EXISTE

- Chaves JSON por Perfil em `ui_preferences` ([`persistence.rs`](../../crates/chess-studio/src/persistence.rs)). O Explorador guarda aba, fonte, jogador, cor e filtros em `explorer.panel` ([`explorer/prefs.rs`](../../crates/chess-studio/src/explorer/prefs.rs)).

### 9.3 Conexão: EXISTE

- Tabela `connections` (migrações 15 e 16): várias por Provedor, apelido, padrão, papel jogador ou Engine, tipo (nome público, ID público, autorizada pelo navegador, autorizada por chave), permissões e instante: [`connections.rs`](../../crates/chess-store/src/connections.rs), [`migrations.rs`](../../crates/chess-store/src/migrations.rs). Testes: [`connections.rs`](../../crates/chess-store/tests/connections.rs), [`authorized_connections.rs`](../../crates/chess-store/tests/authorized_connections.rs).
- A chave do segredo no Cofre é o `id` da Conexão; `AUTOINCREMENT` impede reaproveitar um `id` apagado.

## 10. Interface

### 10.1 Lista de Ações: EXISTE

- Lista única em [`actions.rs`](../../crates/chess-input/src/actions.rs). Ações novas: `OpenConnections` (aba Arquivo, grupo Conexões), `PanelExplorer` e `OpenExplorer` (aba Análise, grupo Explorador) e `ImportOnline` (aba Biblioteca). `OpenLibrary`, `OpenOpenings` e `ImportOnline` ficam desabilitadas com motivo numa Partida competitiva.

### 10.2 Bastidores, Ribbon, barra de status, Painéis: EXISTE

- Bastidores com a página Conexões: [`ribbon/backstage.rs`](../../crates/chess-studio/src/ribbon/backstage.rs). Especificação: [`Backstage.md`](../design/componentes/Backstage.md).
- Central de Conexões (cartões resumidos, Detalhes, filtros, estados, ações, Conexões vinculadas): [`connections/view.rs`](../../crates/chess-studio/src/connections/view.rs), [`model.rs`](../../crates/chess-studio/src/connections/model.rs), [`mod.rs`](../../crates/chess-studio/src/connections/mod.rs), [`connections.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/connections.ftl). Capturas: [`evidencias/central-conexoes`](../evidencias/central-conexoes/README.md).
- Indicador de rede e "Trava de fair play" na barra de status: [`status_bar.rs`](../../crates/chess-studio/src/status_bar.rs).
- Painel Explorador no dock ([`dock.rs`](../../crates/chess-studio/src/dock.rs)); o Perfil de interface "Estudo e análise" o inclui ([`profiles.rs`](../../crates/chess-studio/src/profiles.rs)).
- Componentes novos: [`StatusBadge.md`](../design/componentes/StatusBadge.md), [`OverflowMenu.md`](../design/componentes/OverflowMenu.md), [`Card.md`](../design/componentes/Card.md), [`Segmented.md`](../design/componentes/Segmented.md), [`ExplorerPanel.md`](../design/componentes/ExplorerPanel.md), [`SecretField.md`](../design/componentes/SecretField.md), [`ThemeCard.md`](../design/componentes/ThemeCard.md).

### 10.3 i18n Fluent: EXISTE

- 27 arquivos em [`assets/i18n/pt-BR/`](../../crates/chess-studio/assets/i18n/pt-BR/); os novos são [`connections.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/connections.ftl), [`authorization.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/authorization.ftl) e [`explorer.ftl`](../../crates/chess-studio/assets/i18n/pt-BR/explorer.ftl). Testes proíbem "etapa", "401", "token", "conta", "login" e "credencial" nos textos da interface ([`connections/view.rs`](../../crates/chess-studio/src/connections/view.rs)). Embutidos por [`i18n.rs`](../../crates/chess-studio/src/i18n.rs).

## 11. Tabela US-001..US-032

Etapas (das [decisões](../decisoes/2026-11-11-conexoes.md)): **E1** Fundação, **E2** Dados públicos, **E3** Conexões com segredo, **E4** Partida online no Lichess, **E5** Mais redes. "Fora" = fora das cinco etapas. A situação diz o que o código tem **hoje**. "UNVERIFIED" = falta o serviço real ([roteiro](../homologacao/integracoes.md)).

| US | Título (spec) | Etapa | Situação | O que existe / falta |
|---|---|---|---|---|
| US-001 | Inventariar arquitetura | E1 | EXISTE | Este documento. |
| US-002 | Registro e roteamento de Provedores | E1 | EXISTE | [`catalog.rs`](../../crates/chess-net/src/catalog.rs) e [`net.rs`](../../crates/chess-net/src/net.rs) (8.1). |
| US-003 | Catálogo gratuito primeiro | E1 | EXISTE | [`catalog.rs`](../../crates/chess-net/src/catalog.rs); a Central ordena gratuitos primeiro ([`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs)). |
| US-004 | Cofre de segredos | E3 | EXISTE (UNVERIFIED) | [`chess-vault`](../../crates/chess-vault/src/lib.rs) (8.3). |
| US-005 | OAuth seguro no Lichess | E3 | EXISTE (UNVERIFIED) | [`auth.rs`](../../crates/chess-providers/src/auth.rs), [`loopback.rs`](../../crates/chess-net/src/loopback.rs) (8.4). |
| US-006 | Identidades Chess.com, Queen Alice, ChessBase, FIDE | E2 | PARCIAL | Nome público (Chess.com, Lichess) e ID da FIDE existem ([`connections.rs`](../../crates/chess-store/src/connections.rs)); Queen Alice e ChessBase são só cartões de importação manual ([`catalog.rs`](../../crates/chess-net/src/catalog.rs)). |
| US-007 | FICS com transporte protegido | E5 | AUSENTE | Cartão "Em breve" ([`catalog.rs`](../../crates/chess-net/src/catalog.rs)); sem adaptador. |
| US-008 | Multicontas, permissões, estados, revogação | E3 | PARCIAL | Várias Conexões, papel, permissões, estados e revogação existem só para o Lichess ([`connections.rs`](../../crates/chess-store/src/connections.rs), [`connections/auth.rs`](../../crates/chess-studio/src/connections/auth.rs)). |
| US-009 | Stream de eventos e desafios Lichess | E4 | PARCIAL | Fluxo de eventos e desafios no adaptador ([`board/mod.rs`](../../crates/chess-providers/src/board/mod.rs)); sem interface. |
| US-010 | Jogar humano de ponta a ponta | E4 | PARCIAL | Domínio e Board API prontos ([`online.rs`](../../crates/chess-game/src/online.rs), [`board/session.rs`](../../crates/chess-providers/src/board/session.rs)); sem interface (7.1). |
| US-011 | Reconectar e recuperar estado | E4 | PARCIAL | Reconexão com backoff e reconciliação por retrato ([`board/link.rs`](../../crates/chess-providers/src/board/link.rs), [`online.rs`](../../crates/chess-game/src/online.rs)); sem interface. |
| US-012 | Ações de partida (empate, desistência, abort, chat) | E4 | PARCIAL | Abandono, empate, abortar e reclamar vitória na sessão ([`board_actions.rs`](../../crates/chess-providers/tests/board_actions.rs)); chat e reclamar empate por ausência ficam fora ([`partida-online.md`](../decisoes/2026-11-11-partida-online.md)); sem interface. |
| US-013 | PlayStrategy humano | E5 | AUSENTE | Cartão "Em breve" ([`catalog.rs`](../../crates/chess-net/src/catalog.rs)). |
| US-014 | Cliente FICS | E5 | AUSENTE | Idem. |
| US-015 | Conta BOT / Engine online | E5 | PARCIAL | Só a autorização separada da Engine online ([`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs)); a Bot API não foi feita ([`partida-online.md`](../decisoes/2026-11-11-partida-online.md)). |
| US-016 | Interface uniforme para várias Engines UCI | E1 | EXISTE | 3.1 e 3.2. |
| US-017 | Engine remota gratuita para análise | E3 | AUSENTE | 3.3. |
| US-018 | Importar e exportar PGN robustamente | E2 | EXISTE | Fluxo, encodings, tags e `.pgn.gz` (2.1 a 2.4). `[%clk]` e `[%eval]` ficam fora (2.6). |
| US-019 | Origem, direitos e deduplicação | E2 | EXISTE | [`origin.rs`](../../crates/chess-store/src/origin.rs), [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs) (2.5, 5.4). |
| US-020 | Explorar aberturas por posição | E2 | EXISTE (UNVERIFIED) | [`explorer/mod.rs`](../../crates/chess-studio/src/explorer/mod.rs), [`explorer.rs`](../../crates/chess-providers/src/explorer.rs) (4.3). |
| US-021 | Nomes de aberturas e ECO | E1 | EXISTE | 4.1. |
| US-022 | Pesquisa local por jogador, evento, posição | E1 | EXISTE | 5.2. |
| US-023 | Importar PGN Chess.com, Queen Alice, ChessBase | E2 | EXISTE (UNVERIFIED) | Importação por origem com nota de licença ([`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs)) e API pública do Chess.com ([`chesscom.rs`](../../crates/chess-providers/src/chesscom.rs)). |
| US-024 | Cloud Eval gratuito | E2 | EXISTE (UNVERIFIED) | [`eval.rs`](../../crates/chess-providers/src/eval.rs) e a linha "Em cache (Lichess)" ([`explorer/view.rs`](../../crates/chess-studio/src/explorer/view.rs)). |
| US-025 | Tablebase exata | E2 | EXISTE (UNVERIFIED) | [`tablebase.rs`](../../crates/chess-providers/src/tablebase.rs) e a aba Finais. |
| US-026 | Rating FIDE | E2 | EXISTE (UNVERIFIED) | O cliente consulta a FIDE pelo Lichess ([`profile.rs`](../../crates/chess-providers/src/profile.rs)); o cartão da FIDE e a página Perfil local mostram os três ritmos separados, com a Origem e o botão Atualizar ([`fide_profile.rs`](../../crates/chess-studio/src/fide_profile.rs)); o último valor fica por Perfil local ([`fide.rs`](../../crates/chess-store/src/fide.rs), migração 17). Decisões em [`2026-11-11-fide-caches-origem.md`](../decisoes/2026-11-11-fide-caches-origem.md). |
| US-027 | Cota, gratuidade e fallback | E1 | EXISTE | Limite de taxa, pausa do Provedor, cache e custo zero ([`limits.rs`](../../crates/chess-net/src/limits.rs), [`limits.rs`](../../crates/chess-net/tests/limits.rs)); sem cadeia automática de fallback, por decisão (CX-11). |
| US-028 | Fair play inviolável por vias de serviço | E4 | PARCIAL | Camada de rede travada ([`guard.rs`](../../crates/chess-net/src/guard.rs)); Engine local e análise ainda só na interface; sem gatilho de Partida online (7.4). |
| US-029 | Offline, falhas e acessibilidade | E1 | EXISTE (UNVERIFIED) | "Rede: offline" na barra de status, erros tipados, dado do cache com idade ([`status_bar.rs`](../../crates/chess-studio/src/status_bar.rs), [`status.rs`](../../crates/chess-net/tests/status.rs)). A aprovação visual e de teclado é do usuário. |
| US-030 | Observabilidade sem vazar segredo | E1 | EXISTE | Diário sanitizado ([`status.rs`](../../crates/chess-net/src/status.rs)); teste de ausência de segredo em log ([`static_secrets.rs`](../../crates/chess-vault/tests/static_secrets.rs)). |
| US-031 | Testes contratualizados | E1 | EXISTE | Servidor falso, respostas gravadas e fixtures reais ([`recorded.rs`](../../crates/chess-net/tests/recorded.rs), [`fixtures.rs`](../../crates/chess-providers/tests/fixtures.rs)). |
| US-032 | Parcerias futuras | Fora | PARCIAL | Cartões "Requer aprovação" no catálogo e na Central ([`catalog.rs`](../../crates/chess-net/src/catalog.rs)); nenhum código de parceria. |

## 12. Pontos de atenção para quem implementa

1. **Teste estático de rede.** [`no_network_without_consent.rs`](../../crates/chess-studio/tests/no_network_without_consent.rs) continua a trava do projeto. Uma nova porta de rede ou escuta precisa entrar nele de propósito, com a ADR-0006.
2. **Migrações são posicionais.** [`migrations.rs`](../../crates/chess-store/src/migrations.rs) é uma lista ordenada. Duas branches que criem a migração 17 vão conflitar; quem mesclar depois renumera.
3. **Log imutável.** Os Eventos da Partida online (`ClockReported`, `DrawOfferWithdrawn`, `GameAborted`) são aditivos e o `PAYLOAD_VERSION` continua 1 ([`event.rs`](../../crates/chess-game/src/event.rs), [`partida-online.md`](../decisoes/2026-11-11-partida-online.md), PO-02). Voltar a um binário anterior ao #230 com uma Partida online no banco não é suportado.
4. **Trava de fair play.** Antes da interface da Partida online, ligar a trava ao `online_human_in_progress` e fazer a Engine local e a análise consultá-la (7.4).
5. **Perfil de interface não é Perfil local.** As preferências de Conexão e do Explorador são do Perfil local.
6. **Licenças.** Todo crate novo passa por `cargo deny check licenses` (ADR-0004). Dados CC0 recebem linha em [`LICENSES.md`](../../crates/chess-studio/assets/LICENSES.md).
7. **Cobrir o serviço real.** A suíte nunca chama Lichess ou Chess.com. Antes de dar um Gate por fechado, o usuário percorre o [roteiro](../homologacao/integracoes.md).
