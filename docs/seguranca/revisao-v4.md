# Revisão de segurança do V4 (#273)

Revisão defensiva, feita em 11/11/2026 sobre o `main` em `c723e7f`, do que o V4 (#211) trouxe de rede e de segredo: o Cofre (`chess-vault`), o gate de rede (`chess-net`), os adaptadores dos Provedores (`chess-providers`) e o que o app (`chess-studio`) faz com tokens, nomes e textos vindos de fora. Valem a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md), a [ADR-0007](../adr/0007-partida-online-controlador-segue-o-provedor.md) e o [modelo de ameaça do Cofre](../decisoes/2026-11-11-cofre-e-autorizacao.md#modelo-de-ameaça-resumo).

## Resumo

| Severidade | Achados | Situação |
|---|---|---|
| Alta | 0 | — |
| Média | 3 (SEC-01 a SEC-03) | corrigidos neste PR, cada um com teste que falhava antes |
| Baixa | 6 (SEC-04 a SEC-09) | issues [#278](https://github.com/ChicoFigueiredo/Chess-Studio/issues/278) a [#282](https://github.com/ChicoFigueiredo/Chess-Studio/issues/282) e [#285](https://github.com/ChicoFigueiredo/Chess-Studio/issues/285), com `ready-for-agent` |
| Informativa | 8 (INF-01 a INF-08) | registradas aqui; nenhuma ação pedida |

Nenhum caminho leva o token a outro host ou Provedor, a disco fora do Cofre, a erro, a captura ou a PGN exportado; nada vindo de um Provedor vira comando de Engine, comando ICS fora do formato ou caminho de arquivo. Os três achados médios são um vazamento do token no log mais detalhado (só com `RUST_LOG=trace`) e duas formas de um servidor hostil esgotar a memória (bomba de descompressão nos problemas, despejo de linhas no FICS).

Fora da contagem: as brechas da Trava de fair play achadas pela [auditoria do V4](../integracoes/auditoria-v4.md) (Engine adversária local, busca por Posição na Biblioteca e Engine online sem consultar a Trava) já estão na [#261](https://github.com/ChicoFigueiredo/Chess-Studio/issues/261). São regra de produto que o próprio usuário contornaria, não entrada de um atacante, e não se repetem aqui.

## Método

Para cada área, primeiro a leitura do código (não dos relatórios), depois a tentativa de quebrar com teste: entrada maliciosa, resposta gigante, redirecionamento para fora do catálogo, `state` errado, bomba zstd, linha ICS infinita, caracteres de controle e ANSI em nomes, FEN e PGN malformados. Todos os testes rodam contra o servidor falso (`FakeServer`, `FakeIcsServer`) e o relógio falso; nenhum serviço real foi chamado e o cofre real do sistema não foi tocado.

Severidade:

- **Alta:** segredo exposto a terceiros, execução de código, token para outro host, autorização contornada.
- **Média:** segredo exposto em configuração realista fora do padrão; um servidor (ou alguém no caminho de um canal sem cifra) derruba, congela ou esgota a memória do app no build distribuído.
- **Baixa:** negação de serviço que exige processo local; pânico só no perfil de desenvolvimento; defesa em profundidade; custo de CPU sem teto que só um Provedor autenticado por TLS pode explorar.
- **Informativa:** risco aceito, já registrado no modelo de ameaça, ou observação sem ação.

## Achados de severidade média (corrigidos neste PR)

### SEC-01. Token, código e verificador PKCE no log em `TRACE`

- **Onde:** dependência `ureq-proto` 0.7, usada pelo `ureq` 3.5 do `chess-net`. Em `TRACE` ela despeja os bytes do pedido e da resposta em linhas de 16 bytes, em hexadecimal e em texto (`ureq-proto/src/util.rs`, `log_data`); a própria documentação do `ureq` avisa "TRACE - wire level debug. NOT REDACTED!". O `bevy_log` do app encaminha o `log` (`LogTracer`) e aceita `RUST_LOG`.
- **Ataque:** quem roda `RUST_LOG=trace chess-studio` (ou `RUST_LOG=ureq=trace`) para diagnosticar um problema de rede e cola a saída num relato publica o `Authorization: Bearer …`, o corpo da troca do código (código e `code_verifier`) e a resposta com o token novo. Contraria a ADR-0006 ("nunca … em log").
- **Evidência:** `crates/chess-net/tests/log_secrets.rs` liga um registrador que guarda tudo em `TRACE`, faz um pedido autenticado ao Explorador e a troca do código, e procura os segredos em claro e no despejo hexadecimal. Antes da correção: `lip_TOKEN_DO_EXPLORADOR apareceu no despejo hexadecimal do log`.
- **Correção:** o `chess-net` depende do `log` com `max_level_debug` e `release_max_level_debug`. É um teto em tempo de compilação, unificado em todo binário que tem o `chess-net`: nenhum `log::trace!` existe no app, e `RUST_LOG` não o devolve. O `DEBUG` do `ureq` continua, e ele só mostra cabeçalhos de uma lista segura (o teste confere que há registros e que nenhum tem segredo). Efeito colateral aceito: o `TRACE` de outras dependências que usam o `log` (o `wgpu`, por exemplo) também some do app; o `tracing` do Bevy não é afetado.

### SEC-02. Bomba de descompressão nos problemas do Lichess

- **Onde:** `import_lichess_csv` (`crates/chess-training/src/lichess.rs`), que lê o CSV descomprimido do `.csv.zst` baixado de `database.lichess.org` (`chess-providers/src/puzzles.rs`) ou escolhido pelo usuário, com `read_until(b'\n', …)` **sem limite**.
- **Ataque:** poucos KiB de Zstandard viram dezenas de GiB sem fim de linha (blocos RLE). O leitor guarda a "linha" inteira na memória até o processo morrer. A janela do `ruzstd` já é limitada (128 MiB); a linha não era. O módulo prometia "nada é carregado por inteiro".
- **Evidência:** `a_giant_line_from_a_decompression_bomb_stops_the_import_without_buffering_it` (`crates/chess-training/tests/lichess.rs`): 64 MiB sem `\n`; antes, a importação lia tudo para a memória e terminava `Ok` (`lines_read: 1, invalid: 1`). `bomba_de_descompressao_falha_sem_encher_a_memoria_nem_instalar` (`crates/chess-providers/tests/puzzles.rs`): um `.zst` de poucos KiB que vira 32 MiB de `a`, servido pelo servidor falso; antes, a baixada terminava `Finished` "sem problemas" depois de guardar os 32 MiB numa linha.
- **Correção:** `MAX_PUZZLE_LINE` (64 KiB; uma linha real tem uns 200 bytes). Cada linha é lida com `take(MAX_PUZZLE_LINE + 1)`; acima do limite, a leitura para com erro ("linha acima de 65536 bytes") e a baixada cai (nada é instalado). Linha longa abaixo do limite continua sendo só uma linha inválida (`a_long_line_below_the_limit_is_just_an_invalid_row`).

### SEC-03. Despejo de linhas no FICS: fila sem teto e consulta presa

- **Onde:** `Net::connect_ics` (`crates/chess-net/src/net/ics.rs`) entregava os eventos por `mpsc::channel()` sem teto; `FicsSession::poll` (`crates/chess-providers/src/fics/session.rs`), chamada a cada quadro na thread da interface, tratava **todos** os eventos da fila num `while`.
- **Ataque:** o FICS é texto puro (CX-18), então qualquer um no caminho (rede pública, DNS forjado) injeta linhas. Despejando linhas mais depressa do que a interface as trata (cada tabuleiro sem caminho custa uma busca de profundidade 2), a fila cresce sem fim (cada evento até 16 KiB) e o `poll` não volta nunca: o app congela e esgota a memória.
- **Evidência:** `a_flooding_server_cannot_fill_the_memory_while_nobody_reads` (`crates/chess-net/tests/ics.rs`): 1 MiB de linhas curtas sem ninguém lendo; antes, "524288 eventos esperavam na fila". `a_flood_of_lines_is_handled_a_slice_per_poll_without_loss` (`crates/chess-providers/tests/fics.rs`): 2 000 linhas; antes, "uma consulta tratou 2000 linhas". Com um despejo contínuo e linhas caras (tabuleiros sem caminho), essa consulta não voltaria.
- **Correção:** fila limitada (`MAX_ICS_PENDING`, 1 024 eventos): com ela cheia, a leitura do socket espera (o TCP segura o resto do outro lado), conferindo o cancelamento e o consentimento a cada fatia, então fechar o `IcsLink` ou desligar o Provedor continua fechando a conexão. E orçamento por consulta (`MAX_FICS_EVENTS_PER_POLL`, 256): o resto fica para os quadros seguintes. Nada se perde (`a_slow_reader_still_gets_every_line_in_order`, e o teste do FICS confere as 2 000 notificações).

## Achados de severidade baixa (issues)

| # | Achado | Issue |
|---|---|---|
| SEC-04 | Loopback da autorização: um processo local que manda um byte a cada 1,9 s prende a escuta por horas, além do prazo e do Cancelar (`read_head` só tem prazo por leitura); e qualquer processo local que acerte a porta gasta o único retorno (sem `state`, só nega o serviço). | [#278](https://github.com/ChicoFigueiredo/Chess-Studio/issues/278) |
| SEC-05 | Pânico por overflow com número vindo da rede no perfil `dev` (`cargo run`): `abs()` de `i64::MIN` num relógio do `style 12` do FICS (injetável por quem está no caminho, até antes do login), `Outcomes::total` do Explorador, `40 * inc` do `TimeControl` de PGN importado; e dois latentes (avaliação em cache, ano do Chess.com). O `release` não confere overflow e não cai. | [#279](https://github.com/ChicoFigueiredo/Chess-Studio/issues/279) |
| SEC-06 | O `game_id` do Provedor entra no caminho da URL sem `is_id` (os ids de desafio passam); e `parse_https` aceita `..` e `%2e`, então o catálogo classifica pelo prefixo um caminho que o servidor normaliza para outro. O host não muda e quem forja é o próprio Provedor. | [#280](https://github.com/ChicoFigueiredo/Chess-Studio/issues/280) |
| SEC-07 | Custo sem teto de entradas hostis: `ClockReported` e vaivém de proposta de empate sem Lance (no FICS, quem está no caminho enche o banco da Partida), fluxos NDJSON com fila sem teto e `Link::poll` que drena tudo na interface, deduplicação quadrática do Explorador, Engine remota que valida a linha inteira antes de cortar, busca do rastreador do FICS, lista de meses do Chess.com sem teto. | [#281](https://github.com/ChicoFigueiredo/Chess-Studio/issues/281) |
| SEC-08 | **Corrigido** em [#282](https://github.com/ChicoFigueiredo/Chess-Studio/issues/282). Higiene na interface: Esc no diálogo da senha-mestra e troca de Perfil não esquecem o estado do `egui` dos campos de segredo; token colado no campo de nome público fica em claro; erro de importação mostra texto cru do `serde_json`; nomes e textos do Provedor sem filtro de caracteres de formatação (bidi). Correção: Esc e troca de Perfil esquecem os campos de segredo; `identifier_looks_like_token` no `chess-store` recusa token como nome público (e o campo é limpo); erros crus viram mensagem fixa, com o detalhe só no log; `display_text::clean_display`/`clean_argument` (controles, bidi e invisíveis fora, corte em 64) são aplicados em todo argumento do Fluent (`I18n::t_with`) e nos nomes desenhados direto, com varredura em `display_text.rs`. | [#282](https://github.com/ChicoFigueiredo/Chess-Studio/issues/282) (corrigido) |
| SEC-09 | `cargo deny check advisories` falha: `zvariant`, `zbus_names` e `zvariant_derive` (pilha do cofre do sistema no Linux, `keyring` → `zbus` 5) estão sem manutenção (RUSTSEC-2026-0370 a 0372; foram incorporados ao `zbus` 6, sem versão segura ainda). Não são vulnerabilidades; `cargo deny check licenses` passa. | [#285](https://github.com/ChicoFigueiredo/Chess-Studio/issues/285) |

## Informativos

- **INF-01. FICS sem cifra.** Aceito por CX-18: só visitante, senha nunca sai (testes `a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves` e `a_server_asking_for_a_password_gets_none_and_the_connection_closes`). Quem está no caminho vê e altera a Partida de visitante; SEC-03 e SEC-07 tratam do que isso custa ao app.
- **INF-02. Custo do Argon2id lido do arquivo.** Ao abrir, aceita-se até 1 GiB e 64 passadas (`own.rs`, `MAX_MEMORY_KIB`/`MAX_ITERATIONS`): um `own.vault` adulterado por quem escreve nos arquivos do usuário pode custar minutos e 1 GiB por tentativa de desbloqueio. Acima disso é "corrompido" (teste `custo absurdo`). Não há limite de tentativas de senha; o Argon2id (64 MiB, 3 passadas) já as encarece.
- **INF-03. Permissões.** No Unix o diretório do Cofre nasce 0700 e os arquivos 0600, inclusive ao regravar (`the_vault_directory_and_files_are_private_to_the_user`), mas um diretório que já exista com permissão mais aberta não é apertado (os arquivos continuam 0600). No Windows e no macOS valem as permissões do perfil do usuário; o app não define ACL própria. **UNVERIFIED** fora do Linux.
- **INF-04. Cópias fora do alcance do `zeroize`.** Realocações da `String` do `egui` enquanto se digita, buffers do `serde_json` e do `ureq`/`rustls`. Já no modelo de ameaça do Cofre.
- **INF-05. URL de autorização na linha de comando.** O `xdg-open` recebe a URL como argumento, visível a outros usuários em `/proc/<pid>/cmdline`. Ela só leva o `state` e o desafio PKCE; sem o verificador, um código injetado é recusado na troca.
- **INF-06. Cache sem teto total.** Cada resposta guardada tem no máximo o limite do pedido (16 MiB; 64 MiB no mês de Partidas do Chess.com), mas o número de entradas não tem teto; só pedidos que o usuário fez entram, e "Limpar cache" apaga.
- **INF-07. Nada encontrado em `release` que entre em pânico com entrada da rede.** Os pânicos de SEC-05 dependem da conferência de overflow do perfil `dev`; nenhum `unwrap`/`expect` fora de teste nos adaptadores.
- **INF-08. Fora do teste.** A validação de certificado é a do `rustls` com `webpki-roots` (não testada contra um MITM com certificado inválido); o cofre real do sistema (Secret Service, Credential Manager, Keychain) não foi exercitado (CX-17). Ambos **UNVERIFIED**.

## O que foi tentado e não quebrou

| Área | Tentativa | Resultado | Evidência |
|---|---|---|---|
| Cofre | Arquivo adulterado (conteúdo, etiqueta, nonce, sal, assinatura, versão, custo absurdo) | recusado, nada lido | `own_vault.rs`: `a_tampered_truncated_or_unknown_file_is_refused_and_nothing_is_read` |
| Cofre | Arquivo cortado em **cada** comprimento, de 0 ao total | `Corrupted`, sem pânico | `own_vault.rs`: `a_file_cut_at_any_length_is_refused_without_panic` (novo) |
| Cofre | Permissões depois de regravar; temporário esquecido | 0700/0600, nenhum `.tmp` | `own_vault.rs`: `the_vault_directory_and_files_are_private_to_the_user` (novo) |
| Cofre | Índice adulterado redireciona token | não: o vínculo está dentro do cofre | `vault.rs` (CF-05); `secrets.rs` abre um Cofre fechado e mostra o erro |
| Segredo | `Debug`, erro, diário de operações, URL, cache, disco | nada | `gate.rs`: `no_secret_shows_up_in_debug_errors_or_the_operation_log`; `static_secrets.rs`; `auth.rs` varre o diretório de dados |
| Segredo | Log em `TRACE` | **vazava** (SEC-01), corrigido | `log_secrets.rs` (novo) |
| Gate | Endereço fora do catálogo, porta, `http`, userinfo | recusado antes da rede | `gate.rs`: `an_address_outside_the_catalog_is_refused_before_the_network` |
| Gate | Token para outro Provedor ou para endereço sem `takes_token`, mesmo com Cofre defeituoso | não sai | `gate.rs`: `a_token_of_one_provider_never_reaches_another_provider_even_from_a_faulty_vault`, `the_authorization_goes_only_to_the_authenticated_endpoints_of_its_provider` |
| Gate | `Authorization`, `Cookie`, `Host` à mão | recusados | `gate.rs`: `authorization_and_cookies_cannot_be_set_by_hand` |
| Gate | Redirecionamento de consulta | não seguido | `gate.rs`: `a_redirect_to_another_host_is_not_followed`; `download.rs`: `redirecionamento_nao_vale_para_consulta_comum` |
| Download | Redirecionamento para fora do catálogo, outra capacidade, `http`, porta, consulta, laço | recusado | `download.rs`: `recusa_redirecionamento_*`, `laco_de_redirecionamentos_para_depois_do_limite_de_saltos` |
| Gate | Resposta de 2 MiB com limite de 1 MiB; corpo sem tamanho que nunca acaba | `BodyTooLarge`, nada no cache | `limits.rs`: `a_giant_response_is_cut_at_the_limit_and_never_cached`, `an_endless_body_is_cut_at_the_limit` (novos) |
| Gate | Descompressão HTTP (gzip/brotli) | não existe: `ureq` sem essas features, nenhum `Accept-Encoding` | `chess-net/Cargo.toml` |
| Gate | Consentimento desligado, Trava de fair play, limites e `Retry-After` | recusado/respeitado | `consent.rs`, `gate.rs`, `limits.rs` |
| NDJSON | Linha acima do limite, UTF-8 inválido, fluxo cortado | erro tipado, ressincroniza | `ndjson.rs` |
| PGN da rede | Variações aninhadas, linha e Partida gigantes | `MAX_DEPTH` 128, linha 1 MiB, Partida 16 MiB; gz local até 4 GiB | `chess-pgn/src/parser.rs`, `stream.rs`; `chess-store/src/imported.rs` |
| OAuth | `state` errado, repetido ou ausente; recusa sem `state`; código reutilizado; prazo; cancelar | recusado, nada trocado nem guardado | `auth.rs`: `a_wrong_state_*`, `state_repeated_or_missing_*`, `the_user_refusing_*`, `a_reused_code_*`, `the_deadline_*`, `cancelling_*` |
| Loopback | Outro caminho, método ou `Host`; segunda requisição; outra interface | não conta / porta fechada / inalcançável | `loopback.rs` |
| Escopo | Escopo do outro papel; token sem `X-OAuth-Scopes` | recusado, nada guardado | `auth.rs`: `an_engine_scope_on_a_player_connection_*` |
| ICS | Bytes hostis, telnet em qualquer lugar, linha gigante, controle | sem pânico, sem controle na linha | `ics_reader.rs`; `ics.rs`: `hostile_input_neither_hangs_nor_kills_the_connection` |
| ICS | Comando com `\n`, byte de controle ou acima de 1 KiB | recusado | `ics.rs`: `a_command_never_hides_another_line_or_a_control_byte` |
| ICS | Despejo de linhas sem leitor | **fila sem teto** (SEC-03), corrigido | `ics.rs`, `fics.rs` (novos) |
| FICS | ANSI/controle em nomes; nome do adversário de volta em comando | nomes só `[A-Za-z0-9_-]` (`fics/parse.rs`, `valid_name`); `accept`/`decline` só com nome validado | `fics.rs`: `hostile_input_neither_hangs_nor_breaks_the_session` |
| Engine online | FEN, Lances, relógio e ids do Lichess no `stdin` da Engine | FEN refeito do `CanonicalPosition`, Lances tipados, relógio numérico; `Engine::send` recusa `\n`/`\r` e `set_option` recusa controle; opções UCI só do banco local | `board/engine.rs`, `chess-uci/src/engine.rs` |
| Lances | Lance ilegal ou fora de ordem do Provedor; FEN inicial inválido | dessincroniza sem gravar; FEN validado pelo `chess-core` | `chess-game/src/online.rs`, `setup.rs`; testes de `board_session.rs` e `fics.rs` |
| Arquivos | Nome de arquivo ou caminho vindo da rede | nenhum: PGN e súmula por data, pacote por slug local `[a-z0-9-]`, Engine pelo nome fixo do manifesto | `fen_pgn.rs`, `training_ui/download.rs`, `chess-training/src/packs.rs`, `chess-uci/src/install.rs` |
| Navegador | URL de autorização forjada; argumento com espaço ou controle | só `https://`, sem shell, URL do Lichess conferida | `connections/browser.rs` |
| Capturas e PGN | Token em captura (`--screenshot`) ou PGN exportado | Cofre falso e temporário nas capturas; PGN sem URL nem token | `secrets.rs`, `chess-game/src/pgn.rs` |

## Itens de homologação

[CX-HOM-77](../homologacao/integracoes.md#cx-hom-77-log-detalhado-sem-segredo) (log detalhado sem segredo) e [CX-HOM-78](../homologacao/integracoes.md#cx-hom-78-problemas-do-lichess-depois-do-limite-de-linha) (a base real de problemas continua sendo lida com o limite de linha).
