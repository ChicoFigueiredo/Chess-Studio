# Especificacao de Integracao - Ecossistema Online de Xadrez

> **Versao:** 1.1 | **Data-base:** 11/11/2026 | **Status:** proposta para implementacao incremental | **Idioma:** pt-BR
>
> **Diretriz de produto:** **FREE FIRST**. Priorizar provedores gratuitos que nao exigem cadastro ou que exigem apenas cadastro gratuito. Integracoes pagas, dependentes de acordo comercial ou de endpoints nao documentados NAO bloqueiam o produto.
>
> **Contexto:** o software de xadrez JA EXISTE e esta em desenvolvimento. Esta especificacao nao determina uma reescrita, nao presume uma stack e nao afirma que qualquer funcionalidade ja foi implementada. A equipe deve primeiro inventariar o codigo, adaptar contratos ao dominio existente e preservar retrocompatibilidade.

## 0. Como usar este documento

Este documento e simultaneamente: (i) pesquisa verificavel de provedores, (ii) especificacao funcional, (iii) arquitetura-alvo, (iv) backlog com historias de usuario, (v) plano de testes, (vi) criterios de aceite, qualidade e superacao e (vii) checklist de liberacao. Pode ser entregue diretamente a um agente de programacao.

**Legenda de situacao de integracao:**

- `READY_PUBLIC`: API oficial ou formato aberto documentado, uso sem cadastro confirmado.
- `READY_SIGNUP`: API oficial documentada, cadastro gratuito e autorizacao normal da conta.
- `FREE_LIMITED`: acesso gratuito com cota, registro, limite de produto ou condicao operacional a validar.
- `MANUAL_IMPORT`: baixar PGN no proprio site e importar localmente; nao equivale a API automatizada.
- `NEEDS_APPROVAL`: requer permissao especifica, contrato, parceria ou licenca.
- `NOT_CONFIRMED`: API publica externa nao comprovada; NAO implementar scraping de substituicao.
- `PAID_OPTIONAL`: alternativa paga, desabilitada por padrao e fora do caminho critico.

**Legenda de prioridade:** `P0` = obrigatoria para o primeiro marco; `P1` = altamente desejavel; `P2` = posterior; `P3` = estudo/parceria. `P0` nao significa que todas as fontes devam ser integradas simultaneamente: priorizar entrega vertical comprovada.

**Regra para evidencia:** links de documentacao e disponibilidade descritos abaixo foram verificados em paginas oficiais ou nos proprios projetos em 11/11/2026; precos, cotas e termos podem mudar. Valores externos sao referencias, nunca constantes de codigo. Metas de engenharia neste documento sao **propostas de aceite**, nao SLAs prometidos pelos provedores.

## 1. Objetivo, principios e fronteiras

### 1.1 Objetivo de negocio

Evoluir o aplicativo existente para um **cliente de xadrez independente e multiprovedor**, capaz de: jogar partidas humanas online quando houver API autorizada; conectar motores UCI em modos permitidos; analisar posicoes com motores locais/remotos; explorar aberturas, partidas historicas e finais; importar/exportar PGN; sincronizar bibliotecas; operar parcialmente offline; e apresentar origem, custo e licenca dos dados.

### 1.2 Metas de produto

1. **Custo de API igual a zero no modo padrao.** Nenhum provedor pago habilitado sem consentimento expresso, configuracao e orcamento.
2. **Cadastro simples basta quando a plataforma permitir.** Nao inventar acesso desenvolvedor, OAuth ou jogos remotos em sites cuja API nao o documenta.
3. **Cliente, nao copia de plataforma.** UX propria, tabuleiro e dominio local desacoplados de nomes, marcas e modelos externos.
4. **Fair play por desenho.** Jogo humano competitivo completamente isolado da assistencia de engine; analise remota/local so nos contextos permitidos.
5. **Resiliencia.** Perda de internet, 429, resposta atrasada ou revogacao de token nao corrompem partidas nem travam o software offline.
6. **Dados auditaveis.** Toda partida, avaliacao, estatistica ou rating registra provedor, timestamp, escopo, licenca e versao/qualidade quando aplicavel.
7. **Sem scraping como estrategia principal.** Permitir somente APIs e exports documentados ou importacao manual autorizada.
8. **Incrementalidade.** Usar adaptadores e feature flags no sistema que ja existe, com testes de regressao.

### 1.3 Fora do escopo do primeiro marco

- Construir rede social de xadrez, servidor de rating ou marketplace pago proprio.
- Replicar integralmente Chess.com, ChessBase, Queen Alice ou qualquer UX/marca protegida.
- Executar engines durante partidas humanas ativas onde as regras proibem assistencia.
- Baixar os 8+ bilhoes de partidas Lichess para o disco do usuario como requisito inicial.
- Criar conexoes nao oficiais com ChessBase/Queen Alice usando endpoints privados ou automacao da interface.
- Fazer compras, assinar planos ou criar contas externas automaticamente em nome do usuario.

## 2. Politica de priorizacao: gratuito primeiro

A ordenacao para implementacao deve considerar quatro eixos nesta ordem: **(1) permissao explicita e integracao documentada; (2) custo de API zero; (3) cadastro normal, sem aprovacao comercial; (4) valor para o usuario e custo tecnico**. Uma fonte gratuita apenas na interface web nao ultrapassa uma API aberta bem documentada.

| Faixa | Regra | Exemplos | Decisao |
|---|---|---|---|
| A: gratis e aberto, sem conta | API/download publico documentado, sem chave | Lichess Opening Explorer, Cloud Eval, Tablebase, database CC0, aberturas CC0, Chess.com PubAPI (consulta publica), Chessigma, FIDE (com ressalva de direitos) | Fazer primeiro onde a licenca permitir |
| B: gratis com cadastro | API oficial + conta comum/OAuth, sem contrato especial | Lichess Board/Bot, PlayStrategy Board/Bot, FICS por cliente externo | Fazer no fluxo de jogo online |
| C: gratis com cota/chave | Plano gratuito ou servico compartilhado, limites explicitos | Chess-API.com, StockfishAPI.com, Chessify free no site | Adotar como complemento, verificar termos |
| D: gratis para uso humano/manual | Exportacao PGN, web-app e conta gratuitos, sem API externa comprovada | Queen Alice, ChessBase free, PGN Mentor, TWIC pessoal | Importar por arquivo, nao automatizar acesso |
| E: pago/parceria | Licenca/contrato/compra necessaria ou API fechada | Chess.com interativo, ChessBase API externa, TWIC comercial, Mega Database, Chessify enterprise | Somente estudo ou fase futura |

**Politica de selecao automatica de engine** (modo analise autorizado): `UCI_LOCAL -> CACHE_LOCAL -> LICHESS_CLOUD_CACHE -> CHESS_API_FREE -> SERVICO_FREE_COM_CHAVE -> REMOTO_PROPRIO_OPCIONAL -> PAGO_OPT_IN`. A ordem exata entre motor local e cache pode ser configurada pelo usuario; o valor em cache deve indicar sua procedencia e profundidade. **Nao fazer fallback de engine em modo de partida humana competitiva.**

### 2.1 Matriz executiva dos provedores

| Provedor/recurso | Jogar humano pelo app | Bot/engine em partidas | Dados/analise | Entrada e custo | Status | Prioridade |
|---|---|---|---|---|---|---|
| **Lichess Board API** | Sim | Nao neste fluxo | Partidas/relógios/desafios | Conta gratuita + OAuth `board:play` | READY_SIGNUP | P0 |
| **Lichess Bot API** | Nao | Sim, conta BOT separada | Jogos de engine | Conta BOT gratuita + `bot:play` | READY_SIGNUP | P1 |
| **Lichess Opening Explorer** | - | - | Masters/lichess/player por posicao | API publica gratuita | READY_PUBLIC | P0 |
| **Lichess Cloud Eval** | - | - | Avaliacoes previamente calculadas | API publica gratuita | READY_PUBLIC | P0 |
| **Lichess Tablebase** | - | - | Finais exatos Syzygy, ate sete pecas | API publica gratuita | READY_PUBLIC | P0 |
| **Lichess DB / puzzles / openings** | - | - | PGN, puzzles, avaliacoes, nomes ECO | Downloads CC0 | READY_PUBLIC | P0/P1 |
| **PlayStrategy** | Sim | Sim, via conta BOT | Jogos, variantes, estudos | Conta gratuita + OAuth/API | READY_SIGNUP | P1 |
| **FICS** | Sim, protocolo ICS | Possivel em contas/regras proprias | Historico disponivel no servidor | Cadastro gratuito; interface externa | READY_SIGNUP | P1/P2 |
| **Chess.com PubAPI** | Nao | Nao | Perfil, partidas, clubes, torneios, PGN | Leitura publica gratuita | READY_PUBLIC | P1 |
| **Chess.com integracao de jogo** | Depende de avaliacao | Nao confirmado genericamente | OAuth/connected board especifico | Solicitar acesso | NEEDS_APPROVAL | P3 |
| **Chess-API.com** | - | - | Stockfish remoto REST/WS | Uso gratuito sujeito a limites | FREE_LIMITED | P1 |
| **StockfishAPI.com** | - | - | Analise remota, jobs, versoes de engine | Teste sem chave e conta Free | FREE_LIMITED | P2 |
| **Stockfish.online** | - | - | Stockfish REST remoto | Acesso no site; limites/termos a validar | FREE_LIMITED | P2 |
| **Chessigma** | - | - | API de partidas, metadados e puzzles; calculo no navegador | Gratis, sem conta, limite compartilhado | READY_PUBLIC | P2 |
| **Queen Alice** | Apenas pelo site, sem API externa confirmada | Nao, regras impedem engine em jogos ativos | PGN manual, aberturas, correspondencia | Cadastro gratuito | MANUAL_IMPORT | P1 |
| **ChessBase Web/Live DB** | PlayChess no ecossistema proprio | Engine Cloud no ecossistema proprio | Pesquisa e PGN manual | Conta gratis com limitacoes | MANUAL_IMPORT | P1 |
| **ChessBase produtos pagos** | Nao garantido em cliente externo | Nao garantido em cliente externo | Bases e engines proprietarios | Licenca comercial | PAID_OPTIONAL | P3 |
| **TWIC** | - | - | Partidas de torneios e PGN | Download gratis **para uso pessoal** | MANUAL_IMPORT | P2 |
| **PGN Mentor** | - | - | PGNs por jogador, evento e abertura | Download publico; direitos de redistribuicao a confirmar | MANUAL_IMPORT | P2 |
| **FIDE Rating Lists** | - | - | Listas oficiais, nao partidas | Download gratuito; reproducao restringida | MANUAL_IMPORT | P2 |
| **PGNBase / ChessTempo** | - | - | Bases consultaveis no site | API externa nao confirmada | NOT_CONFIRMED | P3 |
| **Chessify** | - | - | Engines em nuvem no proprio servico | Plano free no site; API externa nao comprovada | NOT_CONFIRMED | P3 |
| **ChessUrge** | - | - | Analise remota anunciada/beta | Servico/cobranca precisam revalidacao | NOT_CONFIRMED | P3 |
| **PyChess Variants** | Plataforma de variantes | Infraestrutura open source para engines | Variantes | Gratis; protocolo externo a estudar | NOT_CONFIRMED | P3 |
| **ICC** | Clientes proprios/tradicionais | Condicoes especificas | Jogos comerciais | Comercial, sem API geral confirmada | PAID_OPTIONAL | P3 |

> **Decisao obrigatoria:** classificacao de cadastro gratuito nao significa direito de jogar via cliente terceiro. ChessBase e Queen Alice sao prioridades para **compatibilidade de dados e referencia funcional**, nao conectores de lances automaticos neste momento.

## 3. Fontes, documentacao oficial e condicoes de uso

### 3.1 Lichess - servidor e APIs (P0/P1)

- **API oficial/OpenAPI:** https://lichess-org.github.io/api/
- **Regras de fair play:** https://lichess.org/page/fair-play
- **Termos (versao modificada em 01/08/2026):** https://lichess.org/terms-of-service
- **Base CC0:** https://database.lichess.org/
- **Exemplo oficial OAuth e jogabilidade:** https://github.com/lichess-org/api-demo
- **Exemplo de BOT UCI:** https://github.com/lichess-bot-devs/lichess-bot
- **Opening Explorer (codigo Rust, AGPL):** https://github.com/lichess-org/lila-openingexplorer
- **Dataset de nomes de aberturas (CC0):** https://github.com/lichess-org/chess-openings

**Cadastro:** conta pessoal gratuita, autorizacao OAuth 2.0 com PKCE S256 e escopo minimo. Tokens de longa duracao, sem refresh token: guardar revogacao e reautorizacao. Conta BOT e distinta, e a conversao para BOT tem restricoes e nao deve usar a conta pessoal de jogo humano. Verificar no endpoint atual antes do cadastro/conversao. Nunca pedir a senha Lichess dentro do aplicativo.

**Board API (jogo humano):** stream de eventos, desafios, partida e envio de lances. API documentada para rapid, classical, correspondence e excecoes de blitz por desafio direto. Engine nao autorizada em jogo humano, inclusive casual e contra BOT adversario. Correspondencia permite consultar livros/explorer, nao engines nem tablebases.

**Bot API:** autoriza motor, partidas por desafios e stream, respeitando limitacoes de emparelhamento, ritmo e conduta. Conta BOT irreversivel conforme documentacao; requer confirmacao explicita antes de converter. O app deve tratar o modo BOT como identidade separada.

**Endpoints oficiais importantes (base `https://lichess.org`):**

| Funcao | Verbo e path | Escopo/modalidade |
|---|---|---|
| Perfil autenticado | `GET /api/account` | Conta OAuth |
| Eventos da conta | `GET /api/stream/event` | OAuth; streaming NDJSON |
| Criar desafio direto | `POST /api/challenge/{username}` | Permissao de desafio adequada |
| Procurar partida (quando permitido) | `POST /api/board/seek` | Board; stream e eventos ja conectados |
| Estado da partida humana | `GET /api/board/game/stream/{gameId}` | Board; NDJSON |
| Lance humano escolhido no tabuleiro | `POST /api/board/game/{gameId}/move/{move}` | Board; UCI |
| Estado da partida de BOT | `GET /api/bot/game/stream/{gameId}` | Bot; NDJSON |
| Lance calculado pelo BOT | `POST /api/bot/game/{gameId}/move/{move}` | Bot; UCI |
| Exportar partidas de usuario | `GET /api/games/user/{username}` | PGN/NDJSON conforme Accept |
| Exportar partida | `GET /game/export/{gameId}` | PGN |
| Cloud eval (cache) | `GET /api/cloud-eval` | Analise, NAO calculo garantido |

**Hostnames diferentes:** Opening Explorer usa `https://explorer.lichess.org/masters`, `/lichess`, `/player`; Tablebase usa `https://tablebase.lichess.org/standard?fen=...` conforme host indicado pela OpenAPI (revalidar em deploy). O cliente deve suportar URL base por servico. **Nao** supor que API REST de jogo, base PGN e Explorer compartilhem host.

**Limites:** politica de uso razoavel e rate limiting dinamico; requisicoes conservadoras, sem paralelismo agressivo; se `429`, esperar o periodo anunciado em `Retry-After` ou, na ausencia, adotar no minimo ~60 s antes de retomar aquele provedor. Streaming deve ser mantido com backpressure, sem reconnect em loop.

### 3.2 Lichess Database / Opening Explorer (P0)

- **Standard rated games:** 8.315.708.510 partidas acumuladas ate outubro de 2026 (dados no portal em 11/11/2026); outubro/2026, 95.395.628 partidas, arquivo `.pgn.zst` ~31 GB. Arquivos mensais nao cumulativos. NAO baixar tudo automaticamente.
- **Avaliacoes em arquivo:** 417.862.375 posicoes avaliadas, conjunto atualizado em 09/11/2026, JSONL compactado.
- **Puzzles:** 6.193.530 problemas no conjunto indicado em 11/11/2026, CSV compactado.
- **Licenca:** os exports gerais do portal sao CC0, mas broadcasts/relays e codigo dos servidores podem ter licencas diferentes; verificar por dataset antes de republicar. O codigo do Opening Explorer e AGPL-3.0-or-later, e os dados de aberturas `chess-openings` sao CC0. Nunca confundir licenca de dado com licenca de codigo.
- **Opening Explorer:** estatisticas por posicao, filtros de rating/ritmo/periodo, partidas exemplares e tres bases (masters, lichess, player); o cliente deve mostrar qual universo foi consultado e a janela temporal. O corpus de masters nao deve ser automaticamente declarado CC0 sem prova da licenca individual.

**Consultas de referencia (sem token nos exemplos):**

```bash
# Estatisticas de abertura por lances UCI.
curl -G 'https://explorer.lichess.org/masters' \
  --data-urlencode 'play=e2e4,e7e5,g1f3'

# Avaliacao previamente existente. 404/miss e um resultado valido.
curl -G 'https://lichess.org/api/cloud-eval' \
  --data-urlencode 'fen=rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

# Consultar um final de poucas pecas; encode sempre o FEN.
curl -G 'https://tablebase.lichess.org/standard' \
  --data-urlencode 'fen=8/8/8/8/8/8/4K3/6kQ w - - 0 1'
```

**Regra do Explorer:** agregacao de resultados nao implica qualidade de jogada; mostrar `amostra`, `vitorias brancas/empates/vitorias pretas`, fonte, filtros e data. Nao afirmar que a melhor pontuacao historica equivale a melhor lance de engine.

### 3.3 PlayStrategy (P1; gratis com cadastro)

- Documentacao: https://playstrategy.org/api
- Pagina para desenvolvedores: https://playstrategy.org/developers
- API baseada em padroes semelhantes aos Lichess, incluindo Board, Bot, OAuth, streams e importacao/exportacao, com diferencas de variantes e contratos.
- Conta pessoal gratuita para Board, conta BOT separada para motor; engine proibida em Board.
- Endpoints representativos: `GET /api/stream/event`, `GET /api/board/game/stream/{gameId}`, `POST /api/board/game/{gameId}/move/{move}`, `GET /api/bot/game/stream/{gameId}`, `POST /api/bot/game/{gameId}/move/{move}`, `GET /api/games/user/{username}`.
- Limite declarado: uma requisicao de cada vez; apos `429`, esperar um minuto. Implementacao deve possuir adaptador proprio e testes de contrato, nao apenas trocar hostname do Lichess.
- Suporte a variantes somente quando o motor de regras do app realmente entende cada variante.

### 3.4 FICS (P1/P2; gratis com cadastro)

- Site: https://www.freechess.org/
- Guia de clientes externos: https://www.freechess.org/interfaces.html
- Permite clientes de xadrez externos e registro gratuito. A integracao exige adaptador ao protocolo tradicional ICS e seu fluxo de autenticao/eventos, **nao REST JSON**.
- Validar suporte a prompt, relogio, formula/seek, board style, lances, reconexao, comandos e eventual jogo com contas de computador conforme regras oficiais antes de ativar modo BOT.
- Segredos de login FICS nao podem ser transmitidos para outros provedores. Testar com uma conta voluntaria dedicada.

### 3.5 Chess.com PubAPI (P1; gratis sem cadastro para leitura)

- Documentacao: https://www.chess.com/news/view/published-data-api
- Ajuda oficial: https://support.chess.com/en/articles/9650547-what-is-the-pubapi-and-how-do-i-use-it
- Developer Community / pedido de OAuth & Connected Board: https://www.chess.com/club/chess-com-developer-community
- Base: `https://api.chess.com/pub/`; endpoints de perfil, partidas por mes, clubes e eventos.
- **A PubAPI e somente de leitura.** E vedado enviar lances por ela; nao simular logins, nao usar endpoints privados, nao automatizar GUI.
- `GET /pub/player/{username}`, `GET /pub/player/{username}/games/archives`, `GET /pub/player/{username}/games/{YYYY}/{MM}` e `GET .../{MM}/pgn` quando documentado; validar exemplos na documentacao atual.
- Limites: acesso serial conservador, paralelismo pode causar 429; CDN pode servir cache de ate 12 h. Usar `User-Agent` descritivo, `ETag`/`Last-Modified`/`Cache-Control`, 304 e exponential backoff.
- Termos e propriedade intelectual: respeitar marca, UX, sons e recursos exclusivos; nao construir espelho/mineracao macica ou produto concorrente a partir de conteudo sem direito apropriado. Exibir aviso de nao afiliacao quando requerido. Imports pessoais autorizados e visualizacao de dados publicos documentados sao o escopo inicial.
- OAuth/connected boards dependem de processo de aprovacao. Sem aprovacao real -> status `NEEDS_APPROVAL`, nenhum botao 'Jogar Chess.com' que sugira conectividade existente.

### 3.6 Queen Alice (P1 para experiencia de correspondencia e PGN; NAO API externa)

- Site e cadastro gratuito: https://www.queenalice.com/
- Descricao, controles e fair play: https://www.queenalice.com/about.php
- Opening Browser: https://www.queenalice.com/opening.php
- Exemplo de listagem com opcao de exportacao: https://queenalice.com/playergames.php?id=22282
- Xadrez por correspondencia; notificacao de lance por e-mail; controles publicados de 1/3/7/14 dias por lance ou 10/30/60/90 dias por jogador/partida.
- Exportacao de **ate 25 partidas concluidas por pedido**, feita pela propria pagina. Cadastro gratuito nao concede API externa de envio de lances; `getpgn.php` e outros caminhos internos nao sao contrato de integracao.
- Regras do Queen Alice **proibem utilizar programas para avaliar/analisar partidas em andamento**, mas permitem livro/base de abertura e uso do computador como registro de lances. Aplicar isolamento equivalente ao modo humano de Lichess.
- Integracao aprovada agora: `Importar arquivo PGN baixado pelo usuario` + metadados `source=queen_alice` + modo de estudo depois de encerrada a partida. **Nao** automatizar polling, login ou lance via HTML/endpoint nao documentado.
- Roadmap eventual: parceria por escrito autorizando API, fluxo externo e politicas de rate limit. Ate la, conector `MANUAL_IMPORT`.

### 3.7 ChessBase (P1 para compatibilidade e referencia; sem API externa confirmada)

- Conta gratuita: https://learn.chessbase.com/en/account/create
- Live Database: https://help.chessbase.com/apps/en/the_live_database.htm
- Ajuda com exportacao PGN: https://support.chessbase.com/en/content/details/CBAccount/30983b7b-118e-4328-a87d-3e0854ce9fb0
- Comparacao de plano gratuito/Premium: https://shop.chessbase.com/en/products/chessbase_account_premium_month
- Ecossistema: Live Database, LiveBook, Let's Check, Engine Cloud, PlayChess, Mega/Big/Corr Databases e softwares comerciais.
- Cadastro ChessBase e gratuito e certas funcoes web, buscas e exportacoes PGN funcionam no proprio produto; resultados e capacidade tem limites por plano. Exemplo de anuncio da versao mobile: Free com ate 50 partidas nos resultados; Premium amplia limites. **Isso e limite do produto, nao franquia de API externa.**
- Nao foi localizada API publica REST/JSON que autorize acesso programatico completo a Live Database, Engine Cloud ou PlayChess a partir de cliente proprio.
- Integracao autorizada por padrao: **importar PGN que o usuario exportou legalmente**; tratar formatos proprietarios `.cbh/.cbv` apenas com biblioteca/licenca permitida e apos estudo juridico. Nao converter automaticamente Mega Database em servico publico.
- Produtos pagos e dados licenciados ficam `PAID_OPTIONAL`/`NEEDS_APPROVAL`. Uma assinatura Premium pessoal **nao** equivale a licenca de integracao nem redistribuicao.
- Engenharia deve tomar ChessBase como benchmark para pesquisa por posicao, repertorio, banco local, anotacoes, classificacao de lances e multiengine, mas criar identidade visual/UX proprias.

### 3.8 APIs gratuitas de engines e posicoes (P1/P2)

| Opcao | Documentacao | Alcance e restricoes |
|---|---|---|
| Chess-API.com | https://chess-api.com/ | `POST https://chess-api.com/v1`, `wss://chess-api.com/v1`; Stockfish 18, depth padrao 12/max 18 no acesso comum, ate 5 variantes, limite de tempo; gratuito, sem SLA publico confirmado; patrocinadores podem ter limites superiores |
| StockfishAPI.com | https://stockfishapi.com/pricing e https://stockfishapi.com/docs/limits | Tem teste sem chave e conta Free; pagina de precos anuncia ate 2.000 unidades/dia e profundidade 14; **documentacao de limites diverge sobre exigencia de cartao e excedentes**: validar em onboarding antes de habilitar e impedir qualquer cobranca nao autorizada |
| Stockfish.online | https://stockfish.online/ | API REST divulgada, engine remota; validar termos, estabilidade, cotas e esquema antes de homologacao |
| Chessigma | https://www.chessigma.com/developers | API publica sem conta/chave, 30 requisicoes/minuto global, analise de engine executada no navegador e nao computacao remota garantida; exige atribuicao |
| Lichess Cloud Eval | https://lichess-org.github.io/api/ | Posicoes ja calculadas em cache; miss nao deve acionar calculo remoto ilimitado |
| Lichess Tablebase | https://lichess-org.github.io/api/ | Finais deterministas ate sete pecas; nao avaliar posicoes fora do dominio de tablebase |
| Chessify free | https://chessify.me/pricing | 13 engines em servidor compartilhado e limite de armazenamento no proprio aplicativo; **API externa nao identificada**, nao tratar conta gratuita como API |

**Chamada exemplificativa da Chess-API.com (somente em estudo/analise e BOT autorizado):**

```bash
curl 'https://chess-api.com/v1' \
  -H 'Content-Type: application/json' \
  -d '{"fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1","depth":12,"variants":3}'
```

**Contrato interno de analise:** resposta deve diferenciar `cached`, `computed`, `tablebase_exact`, `partial`, `unavailable`, `forbidden`, `rate_limited`; jamais confundir pontuacao centipawn com probabilidade real de vitoria, nem calcular numero de mate a partir de heuristica.

### 3.9 Outras fontes de PGN, ratings e variantes (P2/P3)

| Fonte | URL oficial | Permissao/escopo pratico |
|---|---|---|
| The Week in Chess (TWIC) | https://theweekinchess.com/twic | Arquivos PGN/CBV por edicao, mas **edicao gratuita declarada para uso pessoal apenas**; publicacao, banco compartilhado ou uso comercial necessitam autorizacao |
| PGN Mentor | https://www.pgnmentor.com/files.html | PGNs gratuitos por mestre, evento e abertura; direitos de redistribuicao/uso comercial **nao confirmados**. Priorizar importacao manual e solicitar licenca antes de republicar |
| FIDE | https://ratings.fide.com/download_lists.phtml | TXT/XML de ratings oficiais; nao e base de partidas. Rodape do site restringe reproducao/armazenamento sem permissao; habilitar apenas uso licenciado ou pessoal compativel |
| PGNBase | https://www.pgnbase.com/ | Pesquisas online, API publica nao confirmada; sem scraping |
| ChessTempo | https://chesstempo.com/game-database/ | Base consultavel no navegador, API publica de reutilizacao nao confirmada |
| PyChess Variants | https://github.com/pychess/pychess-variants | Servidor open source AGPL focado em variantes; reuso de codigo e protocolo exigem estudo separado |
| ICC | https://www.chessclub.com/ | Servico historico/comercial; cliente externo/automacao so se documentado/contratado |
| Chessify | https://chessify.me/pricing | Plano no site nao e contrato de consumo por API |
| ChessUrge | https://chessurge.com/ | Beta e condicoes comerciais/tecnicas requerem revalidacao; nao dependencia de producao |

**Fronteira juridica:** fatos de partidas e lances nao substituem o exame da protecao de compilacoes, anotacoes, fotos, software, marcas, contratos e termos dos fornecedores. Toda licenca vai ao catalogo do produto; dados sem direitos claros nao entram em um banco compartilhado.

## 4. Central de Credenciais e Conexoes (REQUISITO P0)

> **Requisito acrescentado pelo proprietario do produto em 11/11/2026:** deve existir uma **Central de Credenciais e Conexoes** para administrar, em uma unica interface, contas, autorizacoes, chaves, IDs publicos e estado das integracoes. O usuario citou Chess.com, Queen Alice e um provedor pronunciado como "Feed"; para garantir cobertura sem presumir a grafia, o catalogo contempla **FIDE e FICS** separadamente. **Nenhuma credencial, por si so, libera uma API inexistente ou nao autorizada.**

### 4.1 Experiencia de usuario obrigatoria

A central deve ser acessivel em `Configuracoes > Integracoes > Credenciais e Conexoes` (ou caminho equivalente no produto existente), sem exigir infraestrutura comercial. Seu catalogo de provedores deve apresentar:

1. **Provedor / categoria:** jogo online, bot, dados, engine, ratings ou importacao de arquivos.
2. **O que o cliente consegue fazer agora**, sem prometer funcionalidade futura: `Jogar`, `Bot`, `Consultar`, `Importar PGN`, `Analisar`, `Apenas cadastro de identidade`, `Em negociacao`.
3. **Metodo real de identificacao/autenticacao:** `NONE`, `PUBLIC_ID`, `OAUTH2_PKCE`, `PERSONAL_ACCESS_TOKEN`, `API_KEY`, `USERNAME_PASSWORD_APPROVED_PROTOCOL`, `MANUAL_IMPORT_ONLY`, `PARTNER_APPROVAL`.
4. **Custo e elegibilidade:** gratuito sem cadastro, gratuito com cadastro, gratuito limitado, pago opcional, requer aprovacao, nao confirmado. O padrao deve filtrar e ordenar **gratuitos primeiro**.
5. **Status da conexao:** `NAO_CONFIGURADO`, `DISPONIVEL_SEM_CREDENCIAL`, `AGUARDANDO_AUTORIZACAO`, `CONECTADO`, `LIMITADO`, `EXPIRADO`, `REVOGADO`, `FALHA`, `NAO_SUPORTADO`, `IMPORTACAO_MANUAL`.
6. **Escopos/permissoes concedidos** e modo autorizado (`HUMANO`, `BOT`, `LEITURA`, `ANALISE`); nao misturar contas com papeis diferentes.
7. **Ultima verificacao**, ultima sincronizacao, diagnostico sanitizado, proxima acao recomendada, cota e custo conhecido (quando o fornecedor disponibilizar tais dados).
8. **Acoes adequadas ao metodo:** `Conectar`, `Autorizar no navegador`, `Inserir token`, `Vincular nome/ID publico`, `Testar conexao`, `Reautorizar`, `Trocar credencial`, `Desconectar`, `Revogar`, `Abrir instrucoes` ou `Importar PGN`.
9. **Contas multiplas:** permitir varias identidades por provedor, com apelido local e conta-padrao para cada modo; impedir troca silenciosa da identidade que participa de uma partida.
10. **Privacidade:** senhas e tokens nunca aparecem em texto, dialogos de suporte, URL, logs, exportacoes de configuracao ou telemetria; apenas informacao parcial segura de identificacao quando necessario.

**Fluxo basico:** abrir central -> escolher provedor -> visualizar capacidade e requisitos -> informar identificador publico ou autorizar/fornecer segredo quando realmente exigido -> executar teste minimo de permissao -> apresentar o que foi habilitado -> oferecer desconexao/revogacao. Se nao houver API oficial de autenticacao, mostrar apenas identificacao e importacao legalmente permitida, nunca solicitar senha como falsa promessa de integracao.

### 4.2 Matriz de credenciais por provedor (fonte da verdade da UI)

| Provedor | O que usuario informa na central | Forma correta de autenticar | Funcao liberada imediatamente | O que NAO libera |
|---|---|---|---|---|
| **Lichess - humano** | Conta autorizada pelo navegador; token pessoal opcional para uso avancado | OAuth2 PKCE/S256 com `board:play` e escopos minimos; PAT em fluxo explicito | Board API, desafios, jogos e sincronizacao permitidos | Engine em jogo humano |
| **Lichess - BOT** | **Conta BOT separada** autorizada pelo navegador ou PAT | OAuth2/PAT com `bot:play`, nunca token da conta humana | Jogo de engines pela Bot API | Acesso a torneios/pools nao permitidos |
| **Lichess Explorer, Cloud Eval, Tablebase** | Nada | API publica | Consultas de abertura, cache de avaliacao e finais | Computacao remota garantida sob demanda |
| **PlayStrategy - humano/BOT** | Conta OAuth; perfil BOT separado | OAuth nos escopos documentados no servico | Board/Bot conforme papeis e regras | Livre auxilio de engine em jogo humano |
| **FICS** | Identificador/usuario; senha **somente** para protocolo aprovado e transporte seguro | Login ICS com implementacao validada; visitante sem senha quando disponivel | Jogar pelo cliente externo, depois de homologar protocolo e transporte | Compartilhar senha ou executar engine fora das regras FICS |
| **Chess.com - PubAPI** | Nome de usuario publico, opcional | **Nenhuma senha ou token** para consultas publicas | Perfil e arquivos publicos documentados | Enviar lances, ler dados privados, controlar conta |
| **Chess.com - OAuth/connected board** | Somente se parceria oficial aprovada | OAuth oficial fornecido no acordo, com escopos aprovados | Apenas capacidades expressamente contratadas | Acesso automatico por ter assinatura Premium |
| **Queen Alice** | Nome/ID de usuario, opcional, e arquivos PGN do proprio usuario | **Nenhum login automatizado** ate existir API e autorizacao oficial | Identificar origem, associar partidas, importar PGN manualmente | Ler partidas privadas, lancar jogadas ou fazer scraping usando senha |
| **ChessBase / PlayChess** | Apelido/conta opcional e arquivo PGN exportado | Nenhuma autenticacao externa confirmada | Associacao local de perfil e importacao PGN legal | Usar Live Database/Engine Cloud/PlayChess como API de terceiro |
| **FIDE** | FIDE ID publico (se aplicavel) | **Nenhuma senha** para lista publica; fonte autorizada | Buscar/vincular rating ou importar dataset permitido | Autenticacao de jogador ou acesso a dados restritos |
| **Chess-API.com** | Nada, enquanto plano publico permitir; chave apenas se documentada | Acesso REST/WS publico; eventual contrato separado | Analise remota nos limites free | Capacidade ou disponibilidade garantida |
| **StockfishAPI.com** | Chave de API **se e somente se** exigida no plano habilitado | API key conforme documentacao atual | Analise com cota contratada e custo limitado | Cobranca automatica sem consentimento |
| **Stockfish.online / outros** | Chave somente se exigida pela API oficial | Metodo documentado e homologado | Apenas capacidades verificadas | Varredura de endpoints privados |
| **PGN Mentor, TWIC** | Nada | Importacao manual | Importar arquivo permitido, respeitando licenca | Coleta automatica ou redistribuicao em massa |

**Decisao de seguranca:** a interface pode exibir Chess.com, Queen Alice, ChessBase e FIDE na mesma Central, mas **nao pode pedir senha** desses provedores para simular integracao. O unico caminho para API privada futura e registrar o status `NEEDS_APPROVAL` e habilitar um fluxo oficial depois de convenio real. A central deve informar com clareza: `Conta vinculada para importacao/consulta publica; jogo remoto nao disponivel` quando for o caso.

**Recurso gratuito extra:** a documentacao Lichess lista API de jogadores FIDE (`GET /api/fide/player/{playerId}`, `GET /api/fide/player/{playerId}/ratings` e `GET /api/fide/player`). Considerar este adaptador somente leitura como alternativa util, respeitando direitos da fonte e registrando que os dados sao reproduzidos pelo Lichess. Referencia: https://lichess-org.github.io/api/ (secao FIDE).

### 4.3 Armazenamento seguro e ciclo de vida do segredo

- `SecretRef` guarda uma referencia opaca; a tabela/banco de configuracao **nao** armazena token ou senha em texto puro, nem hash reversivel sem criptografia autenticada.
- Desktop: usar armazenamento nativo do sistema operacional **quando disponivel e suportado** (Windows Credential Manager/DPAPI, macOS Keychain, Linux Secret Service/libsecret); em Linux sem servico seguro, exigir cofre criptografado com desbloqueio explicito, ou manter segredo apenas em memoria. Nunca fallback silencioso para texto puro.
- Servidor/web, se existir: usar cofre gerenciado ou envelope encryption com KMS/HSM/segredo mestre fora da base; TLS em transito; politica de minimo privilegio; segmentacao por usuario/tenant; nao enviar tokens do usuario para frontend alem do necessario ao fluxo de autenticacao.
- Chaves de criptografia com versao; rotacao sem perder contas; snapshots, backup e export de preferencias devem excluir segredos por padrao. Exportar segredos apenas em fluxo excepcional criptografado e com advertencia, se houver requisito explicito futuro.
- OAuth: usar navegador do sistema, PKCE/S256, `state` imprevisivel, verificacao de callback, URL/redirect confiavel, escopo minimo e protecao contra CSRF. Em desktop, callback loopback/deep link seguro conforme plataforma; nao capturar senha no app.
- PAT/API key: entrada por campo mascarado, validacao da permissao sem exibir segredo, substituicao atomica e descarte seguro do valor anterior.
- Nao reutilizar token de uma conta em outro provedor, compartilhar segredo entre perfil humano e BOT, registrar cabecalho `Authorization`, gravar PGN privado em logs ou usar clipboard automaticamente.
- Falha de permissao `401/403`: interromper a operacao afetada, informar a causa, oferecer reautorizacao, preservar dados locais. Revogacao local deve destruir a referencia secreta; revogacao remota quando documentada, sem afirmar sucesso remoto se a chamada falhar.
- FICS: **validar se o transporte real autentica com criptografia ponta a ponta**. Se a implementacao exigir login/senha em conexao sem TLS ou tunel seguro, bloquear o envio no fluxo padrao, sinalizar claramente a limitacao e priorizar modo visitante ou configuracao de tunel verificado. Nao presumir suporte TLS pelo servidor sem ensaio tecnico.
- Controle opcional de desbloqueio do cofre ao iniciar o app; para contas em dispositivos compartilhados, isolamento por usuario local e bloqueio ao sair.
- Auditoria sanitizada: evento, provedor, identificador local, conta parcialmente mascarada, instante, resultado e categoria de erro; **nunca** segredo, token, senha, payload privado ou PGN integral.

### 4.4 Modelo de dominio e contrato transversal

Modelo minimo recomendado, adaptavel ao codigo existente:

```text
ProviderDefinition {
  provider_id, name, official_docs_url, provider_status,
  supported_capabilities[], auth_methods[], permission_scopes[],
  cost_tier, terms_url, data_licenses[], restrictions[], last_reviewed_at
}
ProviderConnection {
  connection_id, local_profile_id, provider_id, display_label,
  external_user_id?, public_username?, fide_id?, secret_ref?,
  auth_method, granted_scopes[], enabled_capabilities[], mode,
  state, last_checked_at?, last_sync_at?, last_error_code?,
  created_at, updated_at, selected_by_default
}
ProviderSecret { /* apenas no cofre do SO/gerenciado */
  secret_ref, secret_type, encrypted_material, key_version, created_at
}
ProviderOperationLog {
  operation_id, connection_id, capability, started_at, ended_at,
  outcome, http_status?, provider_code?, retry_after?, cost_units?
  // SEM segredo, senha, cookie, PGN integral ou dados privados
}
```

**Invariantes:** (`provider_id`, `connection_id`, `local_profile_id`) sempre explicitamente vinculados; `secret_ref` nao e serializado na telemetria nem em exportacao; `enabled_capabilities` e intersecao entre contrato oficial, modo, autorizacao real, escopos e compliance; estado `CONECTADO` nao substitui o teste da capacidade especifica; status alterado por verificacao real, nunca por clique no botao.

Contrato de adapter por responsabilidade (pseudocodigo, nao pressupoe linguagem):

```text
IProviderCatalog: list(), get(providerId), getCapabilities(providerId)
ICredentialVault: put(localProfileId, providerId, secret), read(ref), delete(ref), rotate(ref)
IProviderConnection: connect(), test(capability), disconnect(), health(), capabilities()
IOnlineBoardProvider: watchEvents(), watchGame(id), createChallenge(), sendHumanMove()
IBotProvider: watchEvents(), watchBotGame(id), sendEngineMove()
IGameArchiveProvider: search(), getGame(), syncIncremental(), exportPgn()
IPositionExplorer: lookup(fen, filters), listTopMoves(), exampleGames()
IAnalysisProvider: evaluate(fen, options), supportsMultiPv(), quota()
IEndgameProvider: probe(fen), supportedVariants(), maxPieces()
IPlayerRatingProvider: getPlayer(id), getRatingHistory(id)
IManualImportProvider: validateFile(), preview(), importPgn()
```

Cada `ProviderConnection` usa capacidades verificadas, e nao um booleano generico `authenticated`. Um provedor sem segredo pode existir com `secret_ref=null`. Um conector `MANUAL_IMPORT_ONLY` **nao implementa** `sendHumanMove` nem `evaluate` por engano.

### 4.5 Historias de usuario especificas da central

**US-CRED-01 (P0) - Descobrir provedores gratuitos.** Como jogador, quero ver todos os provedores ordenados por custo e possibilidade real de integracao.
- **Aceite:** filtros `Gratis/Com cadastro/Manual/Pago opcional`; informacao documentada de capacidade, custo, necessidade de cadastro e fonte; `Chess.com PubAPI` aparece como consulta sem login; Queen Alice e ChessBase aparecem como importacao manual.
- **Teste:** contrato do catalogo por provedor; snapshot sem estados ficticios; teste de acessibilidade com teclado.

**US-CRED-02 (P0) - Autorizar Lichess com seguranca.** Como jogador, quero conectar minha conta Lichess sem revelar minha senha ao programa.
- **Aceite:** OAuth PKCE/S256 no navegador; `state` validado; escopos minimos; perfil recuperado via API; token no cofre; estado `CONECTADO` apenas apos teste positivo; logout/revogacao.
- **Teste:** sucesso, usuario cancela, callback invalido, token revogado, permissao insuficiente, cofre indisponivel e repeticao de callback.

**US-CRED-03 (P0) - Identidades publicas sem senha.** Como usuario, quero vincular meu nome Chess.com, meu FIDE ID e meu perfil Queen Alice sem fornecer senhas desnecessarias.
- **Aceite:** Chess.com usa PubAPI apenas para dados publicos; FIDE salva ID e fonte; Queen Alice habilita associacao de PGN e atalho para site, mas nao requer/aceita senha; cada cartao explica capacidades nao suportadas.
- **Teste:** formulario recusa `password` para conectores sem autenticacao oficial; consulta de Chess.com nao chama endpoints logados; importacao de Queen Alice e offline.

**US-CRED-04 (P1) - FICS com isolamento de segredo.** Como jogador, quero usar minha conta FICS no protocolo ICS ou conectar como visitante.
- **Aceite:** conta de visitante sem segredo; login de conta registrada apenas apos validacao do transporte seguro; credencial isolada; nenhuma senha exibida; mensagens claras quando transporte nao e seguro.
- **Teste:** login valido, timeout, desconexao, prompt alterado, resposta maliciosa, TLS/tunel ausente, senha incorreta e reconnect.

**US-CRED-05 (P0) - Gerenciar e revogar conexoes.** Como usuario, quero testar, trocar, escolher e desconectar cada conta.
- **Aceite:** multiplas contas por provedor, modo humano/BOT isolado, teste de escopo, revogacao local efetiva, tentativa documentada de revogacao remota quando suportada e alertas de validade; desconectar nao apaga partidas pessoais importadas.
- **Teste:** duas contas do mesmo provedor, mudanca durante partida ativa rejeitada, revogacao com rede indisponivel, secret removido e dados locais preservados.

**US-CRED-06 (P0) - Zero cobranca silenciosa.** Como usuario, quero impedir que um conector free incorra em cobranca ao terminar a cota.
- **Aceite:** sem chave paga por padrao, limites locais, stop em quota atingida, sem auto-upgrade, registro de custo quando disponivel, notificacao de plano/termo alterado.
- **Teste:** resposta 402/429, cota zerada, custo desconhecido, key premium configurada sem opt-in, falha do servico de contagem.

### 4.6 Criterios de aceite integrados da central - nao negociaveis

- [ ] O catalogo lista Lichess humano e BOT, PlayStrategy, FICS, Chess.com PubAPI, Queen Alice, ChessBase, FIDE e motores free, mesmo quando a integracao e `MANUAL_IMPORT` ou `NEEDS_APPROVAL`.
- [ ] 100% dos segredos persistentes ficam fora de arquivos de preferencias, SQLite comum, PGN, logs, stack traces, bundles e backup nao criptografado.
- [ ] Em inspeccao automatizada dos artefatos e logs de teste, **zero** tokens/senhas em claro; usar scanners de segredos e fixtures sinteticamente identificaveis.
- [ ] OAuth Lichess passa em sucesso, cancelamento, expiracao/revogacao, escopo insuficiente e callback adulterado.
- [ ] Queen Alice, ChessBase, Chess.com PubAPI e FIDE nao solicitam senha para funcionalidades publicas/manuais.
- [ ] FICS nao envia senha em canal sem protecao validada pelo fluxo padrao.
- [ ] Todas as conexoes exibem capacidade efetiva, motivo de indisponibilidade e ultimo teste; `CONECTADO` nao equivale a `JOGAR DISPONIVEL`.
- [ ] Remover credencial interrompe novas chamadas autenticadas; jogo em andamento exibe transicao segura e possibilidade de reautorizar, sem corromper estado.
- [ ] Uma conta humana nunca pode ser silenciosamente promovida a BOT; mudanca irreversivel exige confirmacao informada e cadastro dedicado.
- [ ] A central respeita a diretriz **FREE FIRST**, sem criar assinaturas ou comprar credito.

## 5. Arquitetura de integracao no sistema existente

### 5.1 Fase zero: inventario tecnico obrigatorio

Antes de mudar codigo, a equipe deve mapear: linguagens e frameworks do produto, arquitetura, persistencia, UI de tabuleiro, modelo de partida, motor de regras, FEN/PGN/UCI existentes, gerenciamento de engines, suporte a variantes, plataforma de deploy, CI/testes, mecanismo de configuracao, logs e credenciais ja utilizadas. Registrar em `docs/chess-integration/current-state.md` quais capacidades sao `EXISTE`, `PARCIAL`, `AUSENTE` ou `DESCONHECIDA` com links para arquivos e testes. **Nao substituir componentes existentes funcionais por novos somente por esta especificacao.**

### 5.2 Arquitetura alvo modular

```text
                  [ Cliente de xadrez JA EXISTENTE ]
           Tabuleiro | Relogios | Historia | Estudos | UI
                               |
              [ Estado de modo / Fair Play Guard ]
                               |
            [ Provider Registry + Capability Router ]
             /                |                  \
    [Credenciais]        [Jogo Online]        [Dados/Analise]
    Vault/Accounts     Board API | ICS       PGN | Explorer
    PKCE/Scopes        Bot API | Streams     UCI | Cloud Eval
             \                |                  /
             [ Normalizacao, Cache, Auditoria, Limites ]
                               |
                 [ Persistencia local existente ]
```

**Isolamentos:** UI nunca armazena segredos; transport/stream separado de dominio do jogo; engines nao recebem eventos de partidas humanas ativas; respostas de provedores nunca sobrescrevem o estado autoritativo de partida sem verificacao; caches sao segregados por classe de dado, permissao e licenca; consultas de dados podem funcionar sem contas conectadas.

### 5.3 Normalizacao do dominio

- **Posicao:** FEN canonico + variante, lado a mover, direitos de roque, en passant e contador de meio-lance quando relevante. Diferenciar posicao de repeticao e situacao de empate de estado de partida.
- **Lance:** UCI para transporte quando exigido; SAN para apresentacao; preservar promocao, roque (inclusive UCI Chess960), `en passant`, movimentos ilegais e recusas do servidor.
- **Partida:** PGN estruturado com tags `[Event]`, `[Site]`, `[Date]`, `[Round]`, `[White]`, `[Black]`, `[Result]`, `[FEN]`/`[SetUp]` quando apropriado, clocks/anotacoes quando licito; preservar original para auditoria.
- **Identidade e identificadores:** `globalGameRef = (providerId, remoteGameId)`; `globalPlayerRef = (providerId, remoteUsernameOrId)`; FIDE ID opcional, nao e login; nao colapsar jogadores diferentes so por nome igual.
- **Rating:** Elo FIDE standard/rapid/blitz vs classificacoes e sistemas online separados, com data e origem; nao comparar valores como se fossem escala equivalente ou pontuacao de engine.
- **Analise:** centipawns do ponto de vista explicito, mate-in-N, profundidade, nos, tempo, MultiPV, hash, versao e parametros da engine; diferenciar cache de calculo novo.
- **Proveniencia:** registrar `source`, `license`, `retrieved_at`, `terms_snapshot_reference`, `rights_scope`, `import_method`, `user_owned` e `sharing_allowed`.

### 5.4 Controle de estados de partida e concorrencia

Estados de alto nivel: `OFFLINE_STUDY`, `ONLINE_HUMAN_ACTIVE`, `ONLINE_BOT_ACTIVE`, `ONLINE_SPECTATOR`, `CORRESPONDENCE_HUMAN_ACTIVE`, `POST_GAME_ANALYSIS`. Transicoes exigem evento real do provedor e autorizacao correspondente. Toda operacao leva `gameId`, `connectionId`, `mode`, versao/revisao do estado e correlacao.

**Lance humano:** selecionar no tabuleiro -> validar localmente -> confirmar condicoes da conexao -> enviar uma vez com controle de duplicidade -> aguardar resposta/stream -> reconciliar estado remoto -> exibir lance confirmado. Em perda de confirmacao, **nao repetir cegamente** um POST de lance: consultar estado da partida antes de decidir reenvio. Servidores sao fonte autoritativa de clocks, resultados e lances. Premove (se suportado) e apenas intencao local, nunca partida confirmada.

**Reconexao:** reabrir stream com jitter/backoff; buscar snapshot atualizado; deduplicar eventos; detectar partidas terminadas enquanto desconectado; reconstruir pilha de lances sem duplicatas; reconciliar clocks; registrar gap temporal. Eventos fora de ordem nao podem regredir partida.

**FICS:** protocolo ICS deve ganhar parser separado de mensagens com escapes/variacoes e parser de `style 12` (ou estilo homologado) e nao reutilizar desserializacao JSON. O parser deve ser testado contra respostas parciais e entradas hostis.

### 5.5 Fair Play Guard: uma regra arquitetural, nao opcao de interface

- Se modo `ONLINE_HUMAN_ACTIVE` ou `CORRESPONDENCE_HUMAN_ACTIVE`, bloquear calculo, sugestao de lance, tablebase, cloud-eval e automacao de lance para a mesma partida/posicao; padrao conservador: nao expor paines de auxilio de engine durante a sessao humana competitiva.
- Isolar fila de tarefas de engines e de analise; nao manter job de analise estrategica associado a partida humana que tenha acabado de iniciar.
- Nao habilitar chave `debug`, plugin externo ou endpoint interno que contorne o bloqueio. Todo provider de engine valida o contexto autorizado no backend/camada de servico, nao somente oculta botoes na UI.
- Explorar livros e aberturas em correspondencia somente onde regras expressamente permitem; nao generalizar essa permissao para blitz/rapid ou outros sites.
- Jogar como BOT somente por integracao e identidade de bot autorizadas, com etiqueta visivel de bot. Engines locais e remotas sao equivalentes para fins de restricao durante partidas humanas.
- Liberar analise pos-partida apenas apos confirmacao confiavel de encerramento, nunca no momento em que usuario clica `Desistir` antes do servidor confirmar.

### 5.6 Redes, cache, cotas e privacidade

- HTTP client por provedor com timeout, cancelamento, circuit breaker, backoff e tratamento de `Retry-After`, `ETag`, `Last-Modified` e `304`. Nao compartilhar cookies/tokens entre hosts.
- Streams NDJSON com decoder incremental, tamanho maximo de linha, tratamento de heartbeat/linhas vazias, encerramento controlado e reconexao exponencial com jitter.
- Cache Lichess Explorer e Cloud Eval por FEN/posicao + conjunto completo de filtros relevantes, versao/fonte e TTL configuravel; cache de PGN com provedores, direitos e status de privacidade; nao servir dados privados a outra conta.
- PGN `.pgn`, `.pgn.gz`, `.pgn.zst` (se biblioteca disponivel e testada), UTF-8 e normalizacao de encodings conhecidos, sem carregar arquivos gigantes inteiros em RAM. Arquivo suspeito nao executa comandos nem grava em caminho arbitrario.
- Guardar somente dados necessarios; nao registrar PII ou cookies de terceiros em observabilidade; permitir apagar vinculos, tokens e cache do usuario separadamente dos PGNs que ele escolheu manter.
- Modo offline: tabuleiro, engine local, PGNs ja importados, repertorio e notas continuam acessiveis sem logar em nenhum provedor.

## 6. Especificacao funcional por fluxo

### 6.1 Jogar online com Lichess / PlayStrategy (humano)

**Historia:** como jogador humano, quero usar meu tabuleiro atual para jogar uma partida real, receber desafio, fazer lances e ver resultado sem usar o website do provedor.

**Comportamento requerido:** autorizacao via central; conectar stream de eventos; listar convites; aceitar/recusar desafio; iniciar partida nas modalidades permitidas; carregar jogadores, cores, classificacao, incremento e relogios; sinalizar vez de jogar; confirmar lance via servidor; permitir empate/desistencia/abort apenas quando endpoint e estado permitirem; notificar desconexao e game over. Mostrar link de origem e identidade do provedor.

**Nao fazer:** envio de lance por Chess.com PubAPI; contabilizar lance antes de confirmado; assistencia de engine; transformar um PGN importado em partida oficial; assumir mesmas regras de blitz em todos os servidores.

### 6.2 BOT e engines na internet

**Historia:** como desenvolvedor de engines, quero colocar Stockfish e outros motores UCI para jogar em partidas identificadas como bots.

**Comportamento requerido:** conta dedicada BOT, UCI local/remoto homologado, parametros de tempo com margem contra latencia, orquestracao de turnos, sanitizacao de log, recusa de lances ilegais, encerramento correto, isolamento dos tokens e registros de resultados por engine/versao.

**Nao fazer:** converter conta humana sem aviso; usar engine em partida pessoal; enviar lances de bot a API humana; inventar acesso a torneios comuns.

### 6.3 Biblioteca, estatistica de aberturas e historico

**Historia:** como estudante, quero abrir uma posicao e descobrir aberturas, lances frequentes e partidas de referencia.

**Comportamento requerido:** consulta FEN/jogadas no Explorer, escolha entre bases masters/lichess/player, filtros aplicaveis, barras de resultado por cor, total da amostra, timestamp, link do jogo original; ECO/nome da abertura a partir de dados CC0; PGN de fontes permitidas com deduplicacao.

**Nao fazer:** declarar estatistica como avaliacao de engine; espelhar Chess.com/TWIC/ChessBase comercial; importar corpus integral por padrao; afirmar cobertura de todos os torneios historicos.

### 6.4 Analise local, remota e finais

**Historia:** como estudante, quero avaliar uma posicao com custo zero quando possivel e escolher engine local ou remota.

**Comportamento requerido:** escolher provider local/Cloud Eval/Chess-API; progresso e cancelamento; profundidade, PV, fonte, latencia e limite; fallbacks somente em modo autorizado; cache com TTL; Tablebase quando posicao e variante suportadas; protecao contra resposta antiga.

**Nao fazer:** mostrar cache como novo calculo; supor disponibilidade sem SLA; aceitar linha ilegal; confundir `cp` com chance objetiva de vitoria; cobrar se a cota free acabou.

### 6.5 Importacao/exportacao PGN de todas as fontes

**Historia:** como usuario, quero importar arquivos de Lichess, Chess.com, ChessBase e Queen Alice em uma biblioteca unificada.

**Comportamento requerido:** seletor de arquivos/arrastar-soltar (se UI ja suporta), preview de numero de jogos e tags, deteccao parcial de fonte, confirmacao de direitos, preservacao de original, validacao de lances e posicoes, deduplicacao por `providerId + gameId` e por assinatura canonicamente justificada, relatorio de erros por partida. Exportar somente conteudo que usuario tem direito de exportar.

**Nao fazer:** anexar anotacoes pagas de ChessBase a export publico; baixar jogos privados com cookies ou senhas; excluir todo arquivo se uma unica partida tiver erro; alterar silenciosamente resultado e PGN original.

### 6.6 Correspondencia com Queen Alice como referencia de produto

**Historia:** como jogador, quero organizar jogos longos, avisos de vez e historico de correspondencia no meu cliente.

**Comportamento requerido agora:** cadastro publico da identidade Queen Alice sem senha, importacao legal de PGN ja encerrado, classificacao por tempo e origem, links de acesso ao site e estudos pos-partida. Para partidas em andamento, permitir no maximo registro manual nao automatizado e sem engine, se compativel com os termos vigentes; **nao** apresentar tabuleiro de lance sincronizado com Queen Alice.

**Visao futura:** um modulo de correspondencia proprio, independente do Queen Alice, com status persistente, notificacoes, multi-partidas, relogio por lance/por jogo e regras antifraude, somente se entrar no escopo de produto futuro.

### 6.7 Pesquisa e benchmark ChessBase

**Historia:** como pesquisador, quero buscar por nome, evento, FEN, abertura, data e rating e manter repertorio proprio.

**Comportamento requerido:** index local de PGN importado legitimamente; anotacoes do usuario; categorias/tags; linhas e variantes; comparacao de engines; busca por posicao; export controlado. ChessBase inspira recursos, **nao** determina API externa, direito a dados comerciais ou copia de interface.

### 6.8 Perfis, FIDE e identidade federativa

**Historia:** como usuario, quero vincular meu FIDE ID e consultar historico de rating permitido sem misturar Elo federativo com rating online.

**Comportamento requerido:** campo de FIDE ID publico, nome, federacao e rating por modalidade com data/fonte; via dados oficiais autorizados ou API publicamente documentada de Lichess; preferencias de armazenamento, politica de retencao, correcoes e remocao.

**Nao fazer:** pedir senha FIDE para download publico; afirmar que FIDE permite republicacao irrestrita; criar identidade federativa baseada apenas em nome igual.

## 7. Backlog executavel - historias, criterios de aceite e evidencias

**Padrao de entrega:** cada historia deve ter ticket, link para alteracao, testes automatizados, documentacao, evidencia de funcionamento, impacto no modelo de dados e status `NOT_STARTED / IN_PROGRESS / BLOCKED / ACCEPTED`. O aceite exige resultado **observado**, nao promessa do agente. IDs estaveis devem aparecer em testes e release notes.

### EPICO A - Compatibilidade com o produto existente

**US-001 [P0] Inventariar arquitetura e funcionalidades ja implementadas**
- Como responsavel pelo produto, quero saber o que existe antes de evoluir os conectores.
- CA-001.1: documentar stack, modulos, testes, contratos PGN/FEN/UCI e plataformas suportadas, com paths/commits.
- CA-001.2: cada historia e mapeada para `REUSE / EXTEND / ADD / DEFER`, com justificativa.
- CA-001.3: nenhum fluxo offline existente piora no benchmark ou falha em teste de regressao.
- Evidencia: matriz de inventario, diff, testes baseline e aprovacao de revisao tecnica.

**US-002 [P0] Registro e roteamento de provedores por capacidade**
- Como sistema, quero ativar um provedor sem codificar suas regras na interface.
- CA-002.1: catalogo declara licenca, custo, `auth_method`, `capabilities`, status e URLs de fonte, com validacao de schema.
- CA-002.2: conectar um provedor nao habilita metodos ausentes (ex.: `sendMove` no Chess.com PubAPI gera `UNSUPPORTED_CAPABILITY` local, sem requisicao remota).
- CA-002.3: configuracao desabilitada bloqueia qualquer trafego do adapter, mas preserva arquivos locais.
- Evidencia: suite de contrato por provider + teste de flags.

### EPICO B - Central de Credenciais e Conexoes

**US-003 [P0] Catalogo gratuito primeiro**
- CA-003.1: sem cadastro: Explorer, Cloud Eval, Tablebase, Chess.com PubAPI e dados CC0 acessiveis conforme limites; com cadastro: Lichess e PlayStrategy; manuais: Queen Alice e ChessBase.
- CA-003.2: interfaces de cadastro mostram custos reais conhecidos e aviso `custo nao confirmado` quando necessario, sem oferecer upgrade automatico.
- CA-003.3: busca por nome e filtro por capacidades, status, gratuidade e provider.
- Evidencia: teste visual/acessibilidade e contrato do catalogo.

**US-004 [P0] Cofre de segredos por usuario e provedor**
- CA-004.1: nenhuma credencial persistente aparece fora do cofre; backup de preferencias nao inclui material secreto.
- CA-004.2: revogar/deletar secret impede nova requisicao autenticada; submeter chave substituta nao deixa token anterior acessivel.
- CA-004.3: usuario local B nao consegue listar, ler nem usar segredo do usuario A.
- Evidencia: teste de fronteira de usuario, scan de arquivos/logs, teste de rotacao.

**US-005 [P0] OAuth seguro no Lichess**
- CA-005.1: fluxo PKCE/S256 e `state`; callback indevido e recusado; token e protegido; escopo refletido na tela.
- CA-005.2: `GET /api/account` identifica conta; conectar falha com escopo inadequado e fornece passo de correcao.
- CA-005.3: desconectar remove segredo local e tenta revogacao remota se API documenta, registrando dois resultados separados.
- Evidencia: mock OAuth, smoke test com conta dedicada autorizada, testes de revogacao e cancelamento.

**US-006 [P0] Identidades Chess.com, Queen Alice, ChessBase e FIDE**
- CA-006.1: `Chess.com`: username publica arquiva e le PubAPI sem senha; nao habilita jogos.
- CA-006.2: `Queen Alice`: vincula identidade e associa PGNs importados, mas nao coleta senha/cookies e nao diz estar sincronizado automaticamente.
- CA-006.3: `ChessBase`: perfil opcional e importacao PGN; nao habilita Engine Cloud, PlayChess ou Live Database como API.
- CA-006.4: `FIDE`: usa FIDE ID; consulta publicamente permitida, sem senha, e indica modalidade, data, fonte e uso autorizado.
- Evidencia: testes negativos de campos proibidos, capture de requests para confirmar zero login privado.

**US-007 [P1] Conta FICS ou convidado com transporte protegido**
- CA-007.1: guest conecta sem cofre; conta registrada so usa credencial se transporte for comprovadamente seguro.
- CA-007.2: variacoes/fragmentacao do protocolo ICS nao causam parse incorreto ou vazamento; reconexao preserva sessao local de maneira segura.
- CA-007.3: se nao houver TLS/tunel homologado, UI bloqueia login com senha e informa o motivo.
- Evidencia: testes com servidor/fixture ICS, captura de rede para checar nao exposicao de senha, manual de configuracao.

**US-008 [P1] Multicontas, permissoes, estados e revogacao**
- CA-008.1: conectar duas contas de um mesmo provedor preserva isolamento e associacao correta de partidas.
- CA-008.2: conta BOT e humana nao compartilham token, modos, escopos ou fila de movimentos.
- CA-008.3: transicao de estado deriva de resposta real; erro de teste e representado como tal, sem falsa confirmacao.
- Evidencia: suite de multicontas com revogacao durante operacao e consulta offline posterior.

### EPICO C - Jogo online autorizado

**US-009 [P0] Stream de eventos e desafios Lichess**
- CA-009.1: eventos `challenge`, `gameStart`, `gameFinish`, cancelamentos e stream quebrado aparecem uma unica vez.
- CA-009.2: usuario pode aceitar/rejeitar desafio por endpoint oficial e recebe resultado autoritativo.
- CA-009.3: stream longo nao aumenta memoria indefinidamente nem bloqueia UI.
- Evidencia: integracao com eventos simulados + smoke test real em conta de teste.

**US-010 [P0] Jogar uma partida humana de ponta a ponta**
- CA-010.1: cor, turno, relogio, controle de tempo, resultado e lances refletem servidor.
- CA-010.2: jogador envia apenas lance legal selecionado por humano; ACK/stream define confirmacao, nao clique.
- CA-010.3: request com atraso/timeout nao duplica o lance; reconcilia antes de retry.
- CA-010.4: UI recusa enviar lance por token BOT em endpoint Board humano.
- Evidencia: partida completa em ambiente de teste com dois clientes/contas consentidas; log sanitizado e gravacao de teste.

**US-011 [P0] Reconectar e recuperar estado de partida**
- CA-011.1: perda de 5 s, 30 s e 2 min durante partida nao causa duplicidade de lances nem clocks negativos.
- CA-011.2: reconexao deduplica eventos, busca snapshot e recupera encerramento ocorrido offline.
- CA-011.3: se remota indisponivel, UI informa `estado nao confirmado` e bloqueia envio irresponsavel.
- Evidencia: fault injection de rede e testes deterministas em stream NDJSON.

**US-012 [P1] Acoes de partida (empate, desistencia, abort, chat)**
- CA-012.1: so mostrar acao quando servidor e estado a suportam; confirmacao para ato irreversivel.
- CA-012.2: resultado de empate/desistencia so muda apos estado remoto autoritativo.
- CA-012.3: chat, se implementado, respeita privacidade, bloqueio e regras do provedor; conteudo nao entra em logs.
- Evidencia: testes por estados validos/invalidos e respostas 403/409/429.

**US-013 [P1] Jogo humano PlayStrategy**
- CA-013.1: usa API e permissoes do PlayStrategy, com adapter e testes independentes.
- CA-013.2: so habilitar variantes cujo motor local de regras esteja validado.
- CA-013.3: falha no PlayStrategy nao degrada a conta Lichess conectada.
- Evidencia: teste end-to-end de uma partida standard e casos negativos de variantes.

**US-014 [P1/P2] Cliente FICS pelo protocolo ICS**
- CA-014.1: suportar login conforme seguranca da US-007, busca ou desafio, lances e resultados.
- CA-014.2: parser de prompt, notificacoes assincronas e estado de tabuleiro opera com dados parciais.
- CA-014.3: permitir cancelar conexao e restaurar UI sem reiniciar aplicativo.
- Evidencia: fixtures do protocolo + sessao supervisionada com servidor/conta de teste.

### EPICO D - BOT e engines

**US-015 [P1] Conta BOT Lichess e partida de engine**
- CA-015.1: conta BOT verificada, nunca convertida silenciosamente; token possui `bot:play`.
- CA-015.2: engine UCI recebe posicao correta e limite de tempo, gera lance legal e envia no endpoint BOT.
- CA-015.3: queda da engine nao resulta em lance humano artificial nem envio para conta errada.
- Evidencia: pelo menos duas partidas BOT completas e cenarios de perda de engine.

**US-016 [P1] Interface uniforme para varias engines UCI**
- CA-016.1: engine local selecionavel; `uci`, `isready`, `position`, `go`, `stop`, `bestmove` tratados conforme protocolo.
- CA-016.2: engines com parametros diferentes nao travam UI; timeout e encerramento do processo respeitados.
- CA-016.3: historico mostra engine, hash de executavel opcional, versao, tempo e profundidade.
- Evidencia: suite com Stockfish e pelo menos uma engine UCI alternativa compativel, ou mock UCI se instalacao nao for reproduzivel.

**US-017 [P1] Engine remota gratuita como provider de analise**
- CA-017.1: integrar primeiro provider gratuito com REST ou WebSocket publico documentado.
- CA-017.2: respostas de PV, score, depth e erro normalizadas; atraso ou resposta antiga nao sobrescreve posicao atual.
- CA-017.3: cota excedida interrompe automaticamente uso remoto e oferece alternativa local, sem pagamento.
- Evidencia: contrato versionado com mock e teste de resposta real sem SLA assumido.

### EPICO E - Bases, importacao e estudos

**US-018 [P0] Importar e exportar PGN robustamente**
- CA-018.1: multi-game, comentarios, NAGs, variantes, roque, promocao, en passant e tags arbitrarias tratados.
- CA-018.2: PGN invalido tem erro localizado por partida/linha; importacao parcial preserva jogos validos.
- CA-018.3: export/import roundtrip preserva lances, variacoes e informacao que o formato suporta, exceto normalizacoes documentadas.
- Evidencia: fixtures abertas e geradas; property-based tests e relatorio de corrupcao por caso.

**US-019 [P0] Origem, direitos e deduplicacao de partidas**
- CA-019.1: manter fonte, ID remoto, data de coleta, termos/licenca e permissao de compartilhar.
- CA-019.2: importar duas vezes o mesmo PGN nao cria duas partidas duplicadas; variantes comentadas por usuarios diferentes nao se perdem.
- CA-019.3: dados ChessBase/TWIC sem permissao de distribuicao ficam locais e indisponiveis para backend multiusuario.
- Evidencia: matriz de licenca por dataset, casos de dedupe e teste de isolamento entre usuarios.

**US-020 [P0] Explorar aberturas por posicao**
- CA-020.1: consultar `masters`, `lichess`, `player` conforme disponibilidade, apresentar denominador, filtros e fonte.
- CA-020.2: uma posicao sem resultado nao causa sugestao falsa; rotulo `sem dados`.
- CA-020.3: reconhecer que vitorias por cor em amostra nao significam vantagem objetiva.
- Evidencia: fixtures JSON conhecidos, asserts das somas e snapshot de UI.

**US-021 [P0] Nomes de aberturas e ECO de fonte aberta**
- CA-021.1: nome ECO e linha calculados a partir de fonte CC0 versionada e licenca registrada.
- CA-021.2: transposicao e variantes sem linha conhecida nao geram identificacao inventada.
- CA-021.3: atualizacao do dataset nao apaga repertorios criados pelo usuario.
- Evidencia: testes de abertura espanhola, italiana, siciliana, defesa francesa, transposicoes e posicoes desconhecidas.

**US-022 [P1] Pesquisa por jogador, evento e posicao nos PGNs locais**
- CA-022.1: indice incremental atualiza ao importar/remover PGN, com busca por campos e posicao.
- CA-022.2: cancelamento de indexacao e reinicio retomam sem corromper banco.
- CA-022.3: resultados carregam descricao da fonte e classificacao de direitos.
- Evidencia: teste em lote e benchmark no hardware de referencia documentado.

**US-023 [P1] Importar PGNs Chess.com, Queen Alice e ChessBase**
- CA-023.1: Chess.com por download PubAPI documentado e por arquivo; Queen Alice e ChessBase por exportacao que o usuario fez.
- CA-023.2: nenhum conector Queen Alice/ChessBase envia login, coleta cookies, consulta endpoint interno ou roda automacao de browser.
- CA-023.3: preview e resumo de importacao identificam partidas bem-sucedidas, duplicadas, rejeitadas e com licenca restrita.
- Evidencia: fixture por fonte e teste de ausencia de chamadas de rede nos modos manuais.

### EPICO F - Analise, tablebase, rating e custos

**US-024 [P0] Consulta gratuita Cloud Eval**
- CA-024.1: diferenciar hit de cache, miss/404, profundidade/versao e falta de permissao.
- CA-024.2: consulta so ocorre em contexto autorizado; um miss nao dispara calculo remoto pago.
- CA-024.3: cancelamento de consulta antiga nao altera avaliacao da posicao atual.
- Evidencia: contrato HTTP com 200/404/429/5xx e teste de contexto de Fair Play.

**US-025 [P0] Tablebase Syzygy exata**
- CA-025.1: habilitar apenas final/variante cobertos pelo endpoint; registrar WDL/DTZ e categoria conforme retorno.
- CA-025.2: numero de pecas superior ao limite, FEN invalido e variante nao coberta nao sao anunciados como prova matematica.
- CA-025.3: UI distingue resultado exato de estimativa Stockfish.
- Evidencia: fixtures de finais ganhos, empatados, perdidos, regra dos 50 lances e posicoes invalidas.

**US-026 [P1] Vinculo e atualizacao de ratings FIDE**
- CA-026.1: FIDE ID nunca se confunde com usuario e senha; modalidade e data sao obrigatorias no registro de rating.
- CA-026.2: fonte de dados e direitos registrados; atualizar nao altera retroativamente rating historico.
- CA-026.3: caso pessoa nao exista, UI informa `nao encontrado`, sem criar dados inferidos.
- Evidencia: fixture de tabela/licenca e contrato Lichess/FIDE autorizado.

**US-027 [P0] Controle de cota, gratuidade e fallback**
- CA-027.1: `429` pausa o provedor pelo `Retry-After` ou janela conservadora; nao faz loop de retry.
- CA-027.2: `402`, falta de creditos e preco nao autorizado bloqueiam provider; nao aceitar pagamento nem upgrade automatico.
- CA-027.3: fallback usa somente provedores habilitados e autorizados; modo de jogo humano nunca e redirecionado a engine.
- Evidencia: fault injection de quota, conta sem pagamento e teste de cascata de provedores.

### EPICO G - Qualidade, privacidade e operacao

**US-028 [P0] Fair Play inviolavel por vias de servico**
- CA-028.1: qualquer chamada de engine em sessao humana ativa e rejeitada por guard central, inclusive via UI, plugin, RPC e job de background.
- CA-028.2: partida Queen Alice em andamento nao recebe analise automatizada ou recomendacao.
- CA-028.3: analise somente liberada apos encerramento confirmado; testes negativos sao automatizados no CI.
- Evidencia: teste de penetracao de fronteira funcional e matriz de todos os endpoints de analise.

**US-029 [P0] Offline, falhas e acessibilidade**
- CA-029.1: sem internet, abrir tabuleiro, estudos, biblioteca e engine local sem bloqueio.
- CA-029.2: erros de API nao encerram o aplicativo nem apagam PGNs; reconexao oferece acao recuperavel.
- CA-029.3: Central de Credenciais navegavel por teclado/leitor de tela nos controles aplicaveis; erros nao dependem apenas de cor.
- Evidencia: testes e2e offline, recuperacao de corrupcao e auditoria de acessibilidade.

**US-030 [P0] Observabilidade sem vazamento de segredo**
- CA-030.1: eventos correlacionados por operacao e provider; status e duracao monitorados sem logging de token ou dados privados.
- CA-030.2: painel local permite visualizar ultima sincronizacao, falha e cota sem ler segredos.
- CA-030.3: telemetria remota, se existir, e opcional e respeita privacidade/consentimento.
- Evidencia: scan automatico de logs, testes de redaction e dashboard em ambiente de homologacao.

**US-031 [P1] Testes contratualizados e evolucao sem regressao**
- CA-031.1: fixtures congeladas por versao de API e teste contra schema atual na CI com jobs externos separadamente.
- CA-031.2: quebra externa nao publica nova versao sem diagnostico; testes locais offline continuam reproduziveis.
- CA-031.3: contratos de provedores ficam desacoplados da UI e podem ser removidos/pausados por flag.
- Evidencia: pipeline CI, suite de contratos e plano de rollback executado.

**US-032 [P2/P3] Parcerias futuras Chess.com / Queen Alice / ChessBase**
- CA-032.1: so com acordo oficial e contrato tecnico acessivel, nova capacidade passa de `NEEDS_APPROVAL` a `READY_SIGNUP/PAID_OPTIONAL`.
- CA-032.2: se liberada, requer adapter novo, teste de seguranca, revisao juridica e atualizacao da Central de Credenciais.
- CA-032.3: sem acordo, permanece importacao manual/consulta publica; nenhuma feature flag secreta usa endpoint privado.
- Evidencia: registro documental de autorizacao, contrato de API, revisao de termos e aprovacao de release.

## 8. Casos de teste de aceite em comportamento (BDD / Gherkin)

Os cenarios abaixo sao obrigatorios independentemente do framework de testes escolhido.

```gherkin
Funcionalidade: Conectar conta Lichess com seguranca
  Cenario: OAuth autorizado com escopos minimos
    Dado que o usuario abriu a Central de Credenciais
    Quando autoriza sua conta Lichess no navegador por PKCE/S256
    E o provedor confirma a identidade e o escopo board:play
    Entao a conexao fica CONECTADO para a capacidade BOARD_HUMAN
    E o token fica no cofre protegido
    E nenhuma senha ou token aparece em arquivo de configuracao ou log

  Cenario: Token revogado
    Dado que havia conexao Lichess valida
    Quando o servidor retorna 401
    Entao o cliente interrompe operacoes protegidas desta conexao
    E apresenta a opcao Reautorizar
    E preserva partidas locais e anotacoes

Funcionalidade: Capacidades reais de terceiros
  Cenario: Vincular usuario Chess.com
    Quando o usuario informa apenas o nome publico Chess.com
    Entao o cliente consulta somente a PubAPI
    E habilita importar partidas publicas documentadas
    Mas nao habilita Jogar via Chess.com

  Cenario: Vincular Queen Alice
    Quando o usuario seleciona Queen Alice
    Entao o cliente mostra Vincular perfil e Importar PGN
    E nao solicita nem armazena a senha do site
    E nao envia lances por HTTP ou browser automation

  Cenario: Vincular jogador FIDE
    Quando o usuario informa um FIDE ID valido
    Entao o cliente consulta apenas a fonte de rating autorizada
    E registra fonte, modalidade e data
    E nao pede senha da FIDE

Funcionalidade: Isolamento de fair play
  Cenario: Analise proibida durante partida humana online
    Dado que existe uma partida humana ativa no Lichess
    Quando UI, plugin, API interna ou tarefa pede evaluacao da posicao
    Entao o Fair Play Guard recusa toda tentativa
    E nenhum provider de engine remoto ou local e chamado

  Cenario: Engine autorizada somente em BOT
    Dado que a identidade e uma conta BOT oficial
    E o endpoint de partida e da Bot API
    Quando a engine UCI produz um lance legal para sua vez
    Entao o cliente envia o lance pela Bot API
    E nunca usa o token de uma conta humana

Funcionalidade: Continuidade e integridade
  Cenario: Timeout apos envio de lance humano
    Dado que o servidor recebeu o lance mas a resposta se perdeu
    Quando o cliente reconecta
    Entao consulta o estado remoto antes de qualquer reenvio
    E nao duplica a jogada

  Cenario: Limite de cota gratuita
    Dado que um provider gratuito responde HTTP 429
    Quando o cliente processa Retry-After
    Entao pausa apenas o provider afetado
    E nao faz chamadas adicionais antes do limite
    E nunca ativa plano pago automaticamente

  Cenario: PGN com jogo invalido entre jogos validos
    Dado um arquivo com tres partidas, sendo a segunda invalida
    Quando eu importo o arquivo
    Entao as partidas validas sao preservadas
    E a segunda tem erro de linha, lance e causa
    E um novo import do arquivo nao cria duplicatas
```

## 9. Requisitos nao funcionais, qualidade e seguranca

Os alvos seguintes sao **propostos para homologacao interna**, nao promessas de fornecedores externos. A equipe deve registrar maquina de referencia, SO, tamanho de corpus, velocidade de rede, versao de app e metrica de medicao; se demonstrar inviabilidade objetiva, abrir ADR com nova meta antes de alterar o aceite.

| ID | Aspecto | Criterio minimo de aceite | Evidencia | Criterio de superacao (stretch) |
|---|---|---|---|---|
| NFR-01 | Nao regressao | 100% dos testes criticos preexistentes aprovados | CI baseline vs atual | Zero regressao em benchmark principais e melhorias documentadas |
| NFR-02 | Fair play | 100% dos caminhos de engine testados negativamente em modo humano | Matriz de guard + ataques de bypass | Guard central provado tambem via teste de integracao de plugin/RPC |
| NFR-03 | Segredos | Zero segredos em claro em logs/backup/config/bundle | secret scanning, inspecao de armazenamento | Rotacao e revogacao verificadas com teste de restore e multicontas |
| NFR-04 | Confiabilidade de jogo | Nenhum lance duplicado ou estado regressivo nos cenarios de fault injection | Suite determinista com 5 s/30 s/2 min | Recuperar serie de 10 falhas consecutivas com snapshot reconciliado |
| NFR-05 | Consumo de API | Respeito a 429, backoff, ETag, sem chamadas proibidas | Mocks, traces sanitizados | Cache hit >= 80% no conjunto repetitivo de estudo especificado |
| NFR-06 | Responsividade local | Interacoes locais do tabuleiro sem bloqueio de rede, meta p95 < 100 ms em ambiente de referencia | Medicao automatizada de interacao | p95 < 50 ms no mesmo equipamento |
| NFR-07 | Reacao ao stream | Atualizar UI p95 < 500 ms apos receber evento valido localmente (exclui latencia externa) | Trace com timestamps locais | p95 < 250 ms em teste controlado |
| NFR-08 | PGN e integridade | 100% dos fixtures de regras e roundtrips sem corrupcao silenciosa | Property tests + fixtures | Corpus adversarial ampliado, fuzz >= 24 h cumulativas sem crash |
| NFR-09 | Importacao | Streaming: arquivo grande nao e carregado inteiro em RAM; limite de memoria medido | Profiler + corpus de referencia >= 1 GiB | Import incremental, checkpoints e retomada de arquivos grandes |
| NFR-10 | Cobertura de codigo novo | >= 85% de linhas nos adapters, parsers e central; **100% dos ramos de seguranca e fair play mapeados por teste** | Coverage + lista de ramos | >= 95% em modulos criticos com mutation testing |
| NFR-11 | Acessibilidade | Teclado e leitor de tela funcionam nas principais acoes da central e tabuleiro, foco visivel | Testes manuais/automaticos | Alinhamento WCAG 2.2 AA para novas telas web, quando aplicavel |
| NFR-12 | Multiplataforma | Todos os SOs efetivamente suportados pelo app tem cofre e fluxo de autenticacao testados | Matrix CI + smoke por SO | Reproducao automatizada ponta a ponta em cada SO de entrega |
| NFR-13 | Independencia | Desligar qualquer provider externo nao impede estudo offline | Chaos test desligando endpoints | Troca hot-swap de provider analitico sem reiniciar aplicacao |
| NFR-14 | Privacidade | Dados e segredos separados por usuario; exclusao de vinculo independente dos PGNs mantidos | Tests multiuser/retention | Exportacao portavel de dados locais com licencas preservadas |
| NFR-15 | Custo | Custo automatico de API **R$ 0 por padrao**; nenhuma compra/upgrade implicito | Simulacoes de quota e configuracao | Monitor de consumo por provider com previsao de custo e hard cap |
| NFR-16 | Evolucao da API | Quebra de contrato externa gera erro controlado/feature flag, sem crash | Teste de schema versao N-1/N | Alertas de drift + contract diff automatico |

**Metas de disponibilidade:** nao atribuir SLA a Lichess, Chess-API ou outros servicos gratuitos. O compromisso do produto e **degradacao graciosa**: servico de terceiros fora do ar -> modulo sinaliza indisponibilidade e o app local continua utilizavel. Nao medir latencia da rede do usuario como se fosse desempenho interno do software.

### 9.1 Regras de seguranca de integracao

1. Nunca guardar ou transmitir token em query string, analytics ou ferramenta de captura de tela; usar cabecalhos e cofre quando permitido.
2. Usar TLS 1.2+ para HTTPS e WebSocket `wss` onde aplicavel; validar certificados; proibir URL de provedor customizada com esquema `http` para envio de segredo, salvo ambiente controlado de desenvolvimento local explicitamente isolado.
3. Bloquear SSRF por hosts configurados: allowlist para APIs oficiais, protecao de redirects, limitacao de tamanho e de resposta e politica de proxy configurada.
4. Proteger callback OAuth contra CSRF/code injection/redirect interception; nunca confiar em identificador de usuario recebido do frontend sem vinculacao com token.
5. Rejeitar HTML ativo, scripts, caminhos de arquivos ou comandos embutidos em PGN; renderizar anotacoes como texto escapado e sanitizado.
6. Isolar logs por usuario; mascarar segredos e cookies em erros, traces e crash reports.
7. Credencial comprometida deve permitir revogacao e reautorizacao sem perder notas, puzzles e biblioteca.
8. Exigir autorizacao e teste contratual para habilitar novos adapters que consumam dados protegidos; documentar modelo de ameacas por autenticacao e por dados.
9. Fazer revisao de dependencias, SCA, CVEs, secret scanning, licencas de bibliotecas (inclusive AGPL quando aplicavel) e politica de atualizacao de engines UCI.
10. Conformidade LGPD para perfis e historicos persistidos em servidor: finalidade, minimizacao, retencao, direitos do titular, isolamento e politica de exclusao; documento juridico quando a implantacao efetiva exigir.

### 9.2 Casos de limite e seguranca (obrigatorios)

- Token truncado, token com 512+ caracteres, token revogado, chave trocada no meio da requisicao, cofre bloqueado, clock local incorreto e perfil removido.
- HTTP 200 com corpo malformado, 204, 304, 400, 401, 403, 404, 410, 429, 500/502/503, timeout, host inexistente, rede offline, DNS instavel e stream encerrado sem motivo.
- NDJSON cortado em qualquer byte, varias mensagens no mesmo fragmento, linhas vazias/heartbeat, evento duplicado, atualizacao fora de ordem e corpo gigante.
- FEN invalido, posicao impossivel, Chess960, promocao, roque para os dois lados, `en passant`, 50/75 lances, repeticoes, posicao de mate/afogamento, fim de partida e empate.
- PGN com multiplos jogos, variacoes aninhadas, NAGs, comentarios em outros alfabetos, unicode invalido, arquivo compactado suspeito, zip-bomb, tabs e linha muito longa.
- Dois provedores com mesmo `gameId`, dois usuarios com mesmo username em plataformas distintas, usuario com multiplas contas e concorrencia de updates.
- Arquivo TWIC/ChessBase com termos restritivos: importacao local permitida se usuario autorizado, **nenhuma sincronizacao para catalogo publico**.
- Uma posicao analisada localmente antes de um jogo nao pode fazer sua avaliacao aparecer automaticamente como dica durante partida humana ativa.

## 10. Plano de validacao e matrizes de teste

### 10.1 Camadas de testes obrigatorias

| Camada | Objetivo | Politica de execucao |
|---|---|---|
| Unitario | PGN/FEN/UCI, guard, dominio, status, validacoes de credencial | Toda PR, 100% offline |
| Property-based / fuzz | robustez parser PGN/NDJSON/ICS e normalizador de posicao | PR relevante + nightly ampliado |
| Contract test | request/response de cada provider, limites, schemas e erro | Toda PR com mock determinista; real apenas job autorizado |
| Integracao local | Vault, DB, fila, cache, engine local e adapters | Toda PR ou pipeline de integracao |
| End-to-end de UI | Cadastro, OAuth simulado, sincronizacao, partida e import PGN | PR critica + release |
| Smoke real supervisionado | Lichess/PlayStrategy/FICS/engine free com conta de teste propria | Sob controle de quota, sem sobrecarregar APIs, antes de release |
| Seguranca | redaction, injection, secret scan, TLS, permissao cruzada, fair play | Toda PR critica + release |
| Desempenho e resiliencia | arquivo grande, interrupcoes, memoria, fluxo de jogo | Nightly / marco de release |
| Compliance | licencas, termos, usos permitidos, inventario de fontes | Cada nova fonte/versao significativa |

### 10.2 Matriz de casos concretos

| Teste | Entrada/simulacao | Resultado esperado | Historia |
|---|---|---|---|
| T-001 | Sem internet e sem conexoes | estudo local e tabuleiro funcionais | US-001,029 |
| T-002 | OAuth Lichess correto | conta visivel, token no cofre | US-005 |
| T-003 | OAuth cancelado, `state` adulterado | nenhuma conexao e criada | US-005 |
| T-004 | Lichess OAuth sem `board:play` | recurso de jogo desabilitado com explicacao | US-005 |
| T-005 | Revogar token | novo pedido autenticado nao sai | US-004,008 |
| T-006 | Vincular Chess.com publico | consulta PubAPI, zero password | US-006 |
| T-007 | Tentar `sendMove` no Chess.com | erro `UNSUPPORTED_CAPABILITY`, zero HTTP | US-002,006 |
| T-008 | Vincular Queen Alice | perfil e PGN manual, zero login | US-006,023 |
| T-009 | Vincular ChessBase | somente importacao PGN | US-006,023 |
| T-010 | FIDE ID e rating de modalidades | modalidades e data corretas, sem senha | US-006,026 |
| T-011 | FICS sem canal seguro | nao transmite senha | US-007 |
| T-012 | NDJSON quebrado e eventos repetidos | um unico estado coerente | US-009,011 |
| T-013 | Lance confirmado remotamente, resposta perdida | sem repeticao/duplicacao | US-010,011 |
| T-014 | Jogo termina enquanto offline | estado terminal recuperado | US-011 |
| T-015 | Partida humana com engine ligada | calculo bloqueado em servico e UI | US-028 |
| T-016 | Mesmo request por plugin/RPC | calculo bloqueado sem tocar engine | US-028 |
| T-017 | BOT oficial com UCI | lances so no endpoint BOT | US-015 |
| T-018 | Motor UCI trava | timeout e recuperacao sem UI congelada | US-016 |
| T-019 | PGN com roque/en passant/promocao/variantes | importacao/roundtrip corretos | US-018 |
| T-020 | PGN com jogo invalido no meio | importacao parcial + erros precisos | US-018 |
| T-021 | Reimportar mesmo PGN | nenhuma duplicacao indevida | US-019 |
| T-022 | Arquivo com direitos restritos | compart. publico negado | US-019 |
| T-023 | Lichess Explorer com filtro e sem dados | denominador/fonte coerentes | US-020 |
| T-024 | Transposicao de abertura | ECO sem afirmacao inventada | US-021 |
| T-025 | Cloud Eval miss/404 | `sem avaliacao em cache`, sem gasto | US-024 |
| T-026 | Final Syzygy nao coberto | sem etiqueta de prova exata | US-025 |
| T-027 | 429 com Retry-After | espera e backoff, sem loop | US-027 |
| T-028 | API key free expirou | fallback local e aviso | US-017,027 |
| T-029 | Duas contas Lichess | identidade isolada e jogo sem troca | US-008,010 |
| T-030 | Scan de logs e backups | zero segredos | US-004,030 |
| T-031 | Desligar Chess-API e Lichess | dados locais e UCI local funcionais | US-029 |
| T-032 | PlayStrategy com variante nao suportada | seletor bloqueia partida | US-013 |
| T-033 | ICS mensagens hostis/fragmentadas | parser sem execucao/crash | US-014 |
| T-034 | Dataset FIDE alterou formato | erro de import recuperavel, versao preservada | US-026,031 |
| T-035 | Reinstalacao/migracao de configuracao | segredos nao entram em backup aberto | US-004,029 |

### 10.3 Definicao de pronto (DoD) por historia

Uma historia so esta `ACCEPTED` quando TODOS os itens se aplicam:

- [ ] Codigo implementado na arquitetura ja existente, com descricao de adaptacao e sem reescrita desnecessaria.
- [ ] Criterios de aceite `CA-*` evidenciados por testes verdes e registro do resultado.
- [ ] Caso negativo e falha de rede/credencial testados quando a historia usa API.
- [ ] Erros e mensagens amigaveis, em portugues, sem revelar segredo.
- [ ] Controle de fair play e de licenciamento revistos se a funcionalidade depende de dados, posicao ou engine.
- [ ] Telemetria, cache e validacao de quota/custo tratados quando aplicaveis.
- [ ] Suite atual e de regressao executada; performance e memoria medidos se alterados.
- [ ] Novas capacidades documentadas com endpoints, exemplos e limites, linkados ao provider.
- [ ] Feature flag/rollback e migracao reversivel (ou plano de recuperacao) entregues quando necessario.
- [ ] Responsavel tecnico e responsavel pelo produto validam demonstracao objetiva do fluxo.

## 11. Implantacao incremental - plano de marcos e dependencias

**Regra:** priorizacao `FREE FIRST` + menor risco + maior valor. Nao iniciar negociacoes comerciais ou espelhamento de bancos proprietarios antes de entregar o nucleo aberto. Cada marco deve funcionar como release independente, demonstravel e reversivel.

| Marco | Entregas | Dependencias | Criterio de demonstracao |
|---|---|---|---|
| **M0 - Diagnostico** | Inventario do produto, testes baseline, ADRs, catalogo de provedores e matriz de permissoes | Repositorio atual e acesso a CI | Documento `current-state.md` com rastreabilidade de codigo real |
| **M1 - Central de Credenciais** | UI da central, cofre, OAuth Lichess, perfis publicos Chess.com/Queen Alice/ChessBase/FIDE e status | M0 | Conectar Lichess; vincular os demais SEM pedir senha; revogar; scan de segredos limpo |
| **M2 - Biblioteca gratuita** | PGN robusto, deduplicacao, ECO, Opening Explorer, Cloud Eval, Tablebase, busca basica | M0/M1 parcialmente; APIs publicas independem do OAuth | Abrir posicao e obter abertura/estatistica/avaliacao/final, importar PGNs das fontes manuais |
| **M3 - Online humano Lichess** | Board API, desafios, stream, lances, relogio, reconexao, fair play guard | M1, modelo de partida estavel | Partida real completa com reconexao e zero auxilio de engine |
| **M4 - Bots e multiengine** | Bot API Lichess, UCI modular, engine remota free, quota/fallback | M1/M3 + guard | Partidas BOT completas, analise remota somente em modo permitido |
| **M5 - Mais redes gratuitas** | PlayStrategy Board/Bot, FICS quando transporte seguro for homologado | M3/M4, protocolo ICS | Partida real em cada provider habilitado, sem regressao Lichess |
| **M6 - Refinamento** | Indice local avancado, ratings FIDE, estudos, corpus CC0 selecionado, relatorios e UX | M2 | Pesquisa local por posicao, origem/licenca e rating segregado |
| **M7 - Parcerias opcionais** | Chess.com interativo, Queen Alice automatizado, ChessBase API, engines comerciais | **Acordos/contratos oficiais** | Nao iniciar implementacao de lance/login sem autorizacao; P3 nao bloqueia releases |

**Sequencia alternativa permitida:** biblioteca publica M2 pode ser implementada em paralelo ao OAuth M1 se nao houver dependencia de credenciais. M3 deve esperar Fair Play Guard e cofre homologados. FICS nao precisa atrasar PlayStrategy. O objetivo do MVP e ter valor mesmo se nenhum provedor P3 responder a propostas comerciais.

### 11.1 Marcos de interface para o jogador

- **Release gratuito util:** biblioteca PGN, abertura ECO, Explorer e engines locais, sem login obrigatorio.
- **Release gratuito conectado:** central de credenciais e partida Lichess com conta comum.
- **Release de engines:** conta BOT separada, nuvem gratuita opcional, analises cache e finais Syzygy.
- **Release de multiplataforma:** PlayStrategy e FICS, condicionados a teste real, mais perfis Chess.com/FIDE/Queen Alice/ChessBase.

### 11.2 Ordem de execucao sugerida ao agente de programacao

1. Ler codigo do software atual e gerar `docs/chess-integration/current-state.md`.
2. Montar backlog/issue tracker com US-001..US-032 e dependencias reais.
3. Criar contratos e catalogo sem refatorar o tabuleiro existente desnecessariamente.
4. Implementar `FairPlayGuard`, `CredentialVault` e schema de `ProviderConnection` antes de mover credenciais reais.
5. Criar UI da Central + OAuth Lichess + perfis publicos e manual-only.
6. Adicionar tests de PGN/FEN/UCI e Explorer/Cloud Eval/Tablebase.
7. Implementar Board API e reconciliacao de rede.
8. Implementar Bot API, UCI e providers de engine remota gratuita.
9. Implementar PlayStrategy e FICS (seguranca de canal confirmada).
10. Concluir indice local, ratings e comparativos de analise.
11. Rodar suites de seguranca, compliance, regressao, stress e UX.
12. Entregar relatorio `ACCEPTED / FAILED / DEFERRED` com evidencias por criterio; manter pendencias P3 fora do gate.

## 12. Tabela de custo de referencia e politicas financeiras

**Objetivo financeiro:** operacao inicial com custo direto de acesso a APIs **zero**, tirando infraestrutura/rede/armazenamento do proprio produto e eventuais custos internos de cloud. `Gratis` nao significa capacidade ilimitada nem permissao irrestrita de redistribuicao.

| Recurso/plano | Referencia publica em 11/11/2026 | Impacto de integracao | Decisao |
|---|---|---|---|
| Lichess API, Explorer, Cloud Eval, Tablebase e dados CC0 | Sem tarifa de API anunciada; limites dinamicos e licencas por dataset | API publica oficial | **Padrao** |
| Lichess Board/BOT | Conta gratuita; OAuth apropriado | Integracao de jogo oficial | **Padrao** |
| PlayStrategy e FICS | Servidores de jogo gratuitos; conta quando pertinente | API/ICS com adaptador | **Gratuito posterior** |
| Chess.com PubAPI | Consulta publica gratis, read-only | Importacao de partidas/perfis publicos | **Padrao de leitura** |
| Queen Alice | Cadastro gratuito, PGN de partidas concluidas pelo site | Importacao manual | **Sem API automatizada** |
| ChessBase - conta e web apps | Conta gratuita com limites variaveis; plano Starter **EUR 3,49/mes**, Premium **EUR 4,99/mes** ou **EUR 49,90/ano** | Plano de uso pessoal NAO compra API | **Manual/P3** |
| ChessBase Big Database 2026 | **EUR 99,90**, compra individual | Banco proprietario nao redistribuivel por presuncao | **P3** |
| ChessBase Corr Database 2026 | **EUR 199,90**, compra individual | Importacao/distribuicao sob licenca | **P3** |
| ChessBase Mega Database 2026 | **EUR 229,90**, compra individual | Banco proprietario, nao API | **P3** |
| ChessBase 26 - Single Program | **EUR 199,90**, compra individual | Software licenciado, nao SDK de livre uso | **P3** |
| Chess-API.com | API gratuita com limites operacionais anunciados | Analise remota, sem SLA garantido | **Testar P1** |
| StockfishAPI.com - Free | **US$ 0**, pagina comercial declara **2.000 unidades/dia** e profundidade ate 14 | Docs de limite mencionam condicoes de cartao; **resolver divergencia em onboarding** | **Testar P2 com bloqueio de cobranca** |
| StockfishAPI.com - Hobby/Pro | **US$ 9/mes** e **US$ 49/mes** | Planos pagos com quotas e eventuais condicoes de excedente | **Desligados por padrao** |
| Chessify Free | **US$ 0** no site, servidor compartilhado | API de integracao externa nao confirmada | **Nao usar como backend** |
| Chessify Amateur anual | **US$ 6,67/mes equivalentes**, cobranca anual, conforme pagina | Assinatura de uso no ecossistema, nao API | **P3** |
| TWIC / PGN Mentor | Downloads gratuitos com condicoes de uso e direitos a confirmar | Uso local/manual conforme licenca | **Manual** |
| FIDE Ratings | Listas publicas para download; direitos de uso/reproducao limitados | Apenas rating/identidade, nao jogo online | **Uso permitido/sem login** |

**Fontes de preco e ressalvas:** https://shop.chessbase.com/en/products/chessbase_account_premium_month ; https://shop.chessbase.com/en/products/mega_database_2026 ; https://shop.chessbase.com/en/categories/chessbase_database ; https://shop.chessbase.com/en/categories/chessbase ; https://stockfishapi.com/pricing ; https://stockfishapi.com/docs/limits ; https://chessify.me/pricing . Precos sao referencia, podem variar por moeda, pais, imposto e periodo. **Se duas paginas oficiais divergirem, nao presumir a interpretacao mais favoravel: manter o provider bloqueado para consumo faturavel ate validar o plano especifico da conta.**

### 12.1 Regras de orcamento e quota

- `default_monthly_external_spend = 0` para qualquer perfil novo, com hard cap.
- Provedor pago requer escolha consciente, tela de preco/unidade, politica de uso, teto mensal e confirmacao; nao ativar apenas pela presenca de API key.
- Uma chave com faturamento nao deve ser testada com workload grande durante onboarding; usar `health/test` minimo documentado e registrar custo se possivel.
- Cota esgotada: pausar provider; nao contornar limite criando varias contas, rotacionando chaves fraudulentamente ou espalhando requests em IPs.
- Indicador de `free_calls_remaining` apenas quando provedor retorna numero confiavel; caso contrario, mostrar `cota nao informada` e limitar localmente.
- Se uma API gratuita exigir cartao de credito para habilitar, deixa de ser fluxo `somente cadastro` e exige autorizacao separada; nao entra em P0.

## 13. Criterios de superacao - como o produto se destaca

O primeiro alvo e ter um cliente correto, licito, barato e confiavel. Os itens abaixo caracterizam **superacao da especificacao minima**, nao obrigacoes do MVP. So iniciar apos P0 estavel.

| Dimensao | Aceite basico | Superacao desejada | Como comprovar |
|---|---|---|---|
| Inteligencia de aberturas | ECO + Explorer + partidas de exemplo | Painel conjunto com estatisticas por rating/periodo, transposicoes e repertorio pessoal | Estudo de caso com linhas transpostas e filtros reproduziveis |
| Independencia online | Lichess humano + Bot | Lichess + PlayStrategy + FICS testados, com contratos comuns e configuracoes isoladas | Mesma suite e2e executada por provider elegivel |
| Biblioteca | PGN local, busca por metadados e duplicata | Busca por FEN, transposicoes, diferencas de anotacoes, importacao incremental CC0 e streaming ZST | Benchmark de corpus de referencia e reconciliacao sem perda |
| Qualidade de engine | Stockfish local + Cloud Eval/Tablebase | MultiPV multiengine com comparacao de versoes e calculo remoto gratuito controlado | Testes de consistencia e metadados completos |
| Disponibilidade | Offline preservado, erros controlados | Operacao com provider alternate e cache em modo degradado sob chaos testing | 10 falhas consecutivas simuladas sem corrupcao de tabuleiro |
| Seguranca | OAuth correto, cofre, zero vazamento | Rotacao automatizavel, threat modeling, pentest independente e relatorio de conformidade | Relatorio reproducivel sem vulnerabilidade critica/alta aberta |
| Fair play | Guard central impede engine no jogo humano | Guard resistente a extensoes, RPC, imports e jobs concorrentes com prova automatizada | Matriz completa de bypass e mutation testing de seguranca |
| Experiencia de usuario | Central indica status e acao real | Onboarding orientado, diagnostico, conta multipla e teste de conexao em um passo | Teste com usuarios: tarefas centrais concluidas sem ajuda |
| Custos | R$ 0 automatico | Relatorio de economias/cache e politica de cotas por perfil | Relatorio de consumo e simulacao de cota zero |
| Interoperabilidade | PGN/FEN/UCI e APIs oficiais | Perfis moveis, import/export amplo, protocolo plugavel e suporte a novas engines | Adicionar novo adapter sem alterar tabuleiro, guard ou vault |

**Pontuacao de superacao (somente depois dos gates P0):** avaliar 100 pontos: confiabilidade de jogo 20; segredos/fair play 25; biblioteca e interoperabilidade 20; UX/acessibilidade 15; resiliencia/custo 10; documentacao/testes 10. `>= 90` e excepcional se **todos** os gates de seguranca e compliance estiverem verdes. Qualquer falha critica de fair play, vazamento de segredo ou violacao de licenca implica **reprovacao da entrega**, independentemente da pontuacao. A pontuacao nao substitui testes de aceite por historia.

## 14. Riscos, limitacoes e decisoes explicitas

| Risco / incerteza | Impacto | Tratamento obrigatorio | Responsavel sugerido |
|---|---|---|---|
| Supor API privada a partir de login normal | Alto: quebra de integracao/termos | Registrar `NEEDS_APPROVAL`; sem scraping | Produto + juridico + integracoes |
| Solicitar senha Queen Alice/ChessBase/Chess.com PubAPI sem API de login | Critico: falsa promessa e risco de dados | UI somente identidade publica e PGN manual | UX + seguranca |
| Transporte ICS antigo sem criptografia validada | Alto: vazamento de senha FICS | Bloquear login padrao ate canal seguro homologado | Seguranca + integracoes |
| Engine sugerir lance em jogo humano | Critico: violacao fair play | Guard central + testes negativos e nao-bypass | Dominio + qualidade |
| Lichess 429/downtime | Medio: experiencia degradada | Token bucket, backoff, cache e offline | Integracoes |
| Chess.com PubAPI read-only | Medio: expectativa de jogo pela API | Explicar no cartao, parcerias P3 | Produto |
| Licencas diferentes de dados e codigo | Alto: bloqueio de redistribuicao | Catalogo de licenca por fonte/corpus | Juridico + dados |
| Planos de engine gratuitos mudarem | Medio: custo/limite | Flags + budget zero e fallback local | Operacao |
| UI atual possuir arquitetura restritiva | Medio/alto: retrabalho | Inventario M0, seams/adapters incrementais | Arquitetura |
| Base PGN grande corromper memoria | Medio/alto: indisponibilidade local | Parser streaming e processamento por lotes | Dados |
| FIDE ID confundido com login/credencial | Medio: UX incorreta e vazamento | Campo de identificacao publica, nao senha | Produto + seguranca |
| ChessBase/Queen Alice nunca liberarem API | Baixo no desenho recomendado | Produto entrega valor completo por Lichess + PGN | Produto |

### 14.1 Decisoes arquiteturais obrigatorias (ADRs)

- `ADR-001`: adaptadores por capacidade em vez de integracao acoplada a tela.
- `ADR-002`: central de conexoes com cofre de segredos do SO/gerenciado.
- `ADR-003`: isolamento de modo humano e BOT atraves de Fair Play Guard.
- `ADR-004`: Lichess e ecossistema publico como base FREE FIRST.
- `ADR-005`: ausencia de scraping/login nao documentado e verificacao de licencas por fonte.
- `ADR-006`: PGN/FEN/UCI como intercambio; servidor remoto como autoridade do jogo.
- `ADR-007`: quota zero-custo com circuito de fallback e cancelamento.
- `ADR-008`: FICS autenticado somente apos verificacao de transporte seguro.

## 15. Artefatos esperados de quem implementar

O agente/equipe deve entregar, **alem do codigo**, os seguintes artefatos no proprio repositorio:

```text
docs/chess-integration/
  current-state.md                 # Inventario do codigo existente
  provider-catalog.md              # Capacidades e documentos oficiais
  provider-licensing.md            # Fonte/licenca/uso permitido
  integration-architecture.md      # Diagrama, contratos, estados
  credentials-security.md          # OAuth, Vault, scopes, revogacao, ameacas
  fair-play-policy.md              # Restricoes por modo, provider e teste
  migration-plan.md                # Migracoes, compatibilidade e rollback
  integration-test-plan.md         # Matriz US/CA/teste/evidencia
  api-contracts/                   # Schemas e fixtures de requests/responses
  evidence/                        # CI, capturas sem segredo, benchmarks
  adrs/                             # ADR-001..ADR-008
```

**Formato de evidencias por requisito:** `US-ID | CA-ID | arquivo alterado | teste executado | resultado | data | commit | observacoes`. Quando nao houver API oficial/parceria, marcar `BLOCKED_BY_PROVIDER` sem criar workaround ilegal. Se nao for possivel executar smoke test externo, marcar `NOT_VERIFIED_EXTERNALLY`, nunca `PASS` somente com mock.

### 15.1 Gate final para publicar o MVP

- [ ] Catalogo FREE FIRST e Central de Credenciais em producao, sem solicitar senha a provedor sem API oficial.
- [ ] OAuth Lichess, cofre e revogacao testados no(s) SO(s) alvo.
- [ ] Board API humana funcional com jogos reais e reconexao.
- [ ] Fair Play Guard bloqueia engine em partidas humanas e correspondencia conforme regras.
- [ ] PGN multiformato, ECO, Explorer, Cloud Eval e Tablebase demonstraveis.
- [ ] Chess.com PubAPI apenas leitura; Queen Alice/ChessBase por PGN manual; FIDE por dados publicos autorizados.
- [ ] Funcionalidade offline existente preservada e regressao verde.
- [ ] Sem cobranca automatica, endpoints privados, scraping ou violacao conhecida de licencas.
- [ ] T-001..T-035 aplicaveis aprovados; `N/A` explicado e aprovado por revisao.
- [ ] Documentacao, evidencias, versoes de dependencia, warnings e plano de rollback disponiveis.
- [ ] Nenhuma falha critica ou alta de seguranca aberta, e zero segredo em logs e artefatos.

## 16. Fontes primarias e links essenciais para quem vai implementar

**Provedores, API e autenticacao**

1. Lichess OpenAPI: https://lichess-org.github.io/api/
2. Lichess API Demo/OAuth: https://github.com/lichess-org/api-demo
3. Lichess fair play: https://lichess.org/page/fair-play
4. Lichess bots e exemplo UCI: https://github.com/lichess-bot-devs/lichess-bot
5. PlayStrategy OpenAPI: https://playstrategy.org/api
6. FICS e clientes externos: https://www.freechess.org/interfaces.html
7. Chess.com PubAPI (documentacao): https://www.chess.com/news/view/published-data-api
8. Chess.com PubAPI (ajuda e restricoes): https://support.chess.com/en/articles/9650547-what-is-the-pubapi-and-how-do-i-use-it
9. Chess.com desenvolvedores e pedido de OAuth: https://www.chess.com/club/chess-com-developer-community
10. Chess.com formulario de OAuth/connected board: https://www.chess.com/blog/CHESScom/chess-com-oauth-login-connected-board-application

**Dados, PGN, abertura, analise e rating**

11. Lichess Database: https://database.lichess.org/
12. Lichess Opening Explorer: https://explorer.lichess.org/
13. Codigo do Opening Explorer: https://github.com/lichess-org/lila-openingexplorer
14. Nomes de abertura CC0: https://github.com/lichess-org/chess-openings
15. Lichess Cloud Eval/Tablebase/FIDE: https://lichess-org.github.io/api/ (secoes Analysis, Tablebase e FIDE)
16. Queen Alice: https://www.queenalice.com/ e https://www.queenalice.com/about.php
17. Queen Alice Opening Browser: https://www.queenalice.com/opening.php
18. ChessBase Live Database: https://help.chessbase.com/Apps/en/summary_live_database.htm
19. ChessBase loja e planos: https://shop.chessbase.com/en/
20. Chess-API.com: https://chess-api.com/
21. StockfishAPI.com: https://stockfishapi.com/pricing e https://stockfishapi.com/docs/limits
22. Stockfish.online: https://stockfish.online/
23. Chessigma: https://www.chessigma.com/developers
24. Chessify: https://chessify.me/pricing
25. The Week in Chess: https://theweekinchess.com/twic
26. PGN Mentor: https://www.pgnmentor.com/files.html
27. FIDE Ratings, arquivo mensal: https://ratings.fide.com/download_lists.phtml
28. PGNBase: https://www.pgnbase.com/
29. ChessTempo: https://chesstempo.com/game-database/
30. PyChess Variants: https://github.com/pychess/pychess-variants
31. Internet Chess Club: https://www.chessclub.com/

**Padroes e seguranca**

32. Portable Game Notation (PGN), revisao de referencia: https://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm
33. UCI (Universal Chess Interface): https://www.wbec-ridderkerk.nl/html/UCIProtocol.html
34. OAuth 2.0 for Native Apps (RFC 8252): https://www.rfc-editor.org/rfc/rfc8252
35. Proof Key for Code Exchange (RFC 7636): https://www.rfc-editor.org/rfc/rfc7636
36. OAuth 2.0 Security Best Current Practice (RFC 9700): https://www.rfc-editor.org/rfc/rfc9700
37. OWASP Secrets Management: https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html
38. OWASP OAuth 2.0: https://cheatsheetseries.owasp.org/cheatsheets/OAuth2_Cheat_Sheet.html
39. WCAG 2.2: https://www.w3.org/TR/WCAG22/

**Nota de atualizacao:** URLs, versoes de APIs, quotas, precos e termos devem ser revisitados em cada release e a cada alteracao relevante comunicada pelos provedores. Links externos sao referencias para a equipe, nao autorizacoes contratuais automaticas. A selecao e o uso de dados devem considerar a jurisdicao e os direitos de cada fonte.

## 17. Matriz de decisao final

| Objetivo do produto | Primeiro provedor/recurso | Conta/senha necessaria? | Prioridade | Condicao decisiva |
|---|---|---|---|---|
| Jogar contra pessoas com o cliente atual | **Lichess Board API** | Conta gratuita + OAuth (sem senha no cliente) | P0 | Fair Play Guard |
| Coletar partidas publicas de conta Chess.com | **Chess.com PubAPI** | Nome publico, sem senha | P1 | Read-only / termos |
| Competir com Stockfish/engines online | **Lichess Bot API** | Conta BOT gratuita dedicada + OAuth/PAT | P1 | Perfil BOT autorizado |
| Explorar linhas de abertura e mestres | **Lichess Opening Explorer** | Nao | P0 | Cache e fonte |
| Descobrir ECO e nome de abertura | **Lichess chess-openings CC0** | Nao | P0 | Dataset versionado |
| Consultar avaliacoes em cache | **Lichess Cloud Eval** | Nao | P0 | Pode nao haver dados |
| Consultar finais matematicos | **Lichess Tablebase** | Nao | P0 | Posicao coberta |
| Analise remota executada de fato | **Chess-API.com** | Gratuito nos limites, sem chave no fluxo publico | P1 | Cota e indisponibilidade |
| Importar partidas proprias Queen Alice | **PGN manual** | Conta no site; senha nao entra no app | P1 | Sem API publica de lances |
| Importar partidas proprias ChessBase | **PGN manual** | Conta no site; senha nao entra no app | P1 | Licenca e importacao |
| Associar rating FIDE | **FIDE ID + fonte publica autorizada** | Identificador publico, sem senha | P1/P2 | Direitos da fonte e modalidades |
| Segundo servidor online aberto | **PlayStrategy** | Conta gratuita + OAuth | P1 | Adapter proprio |
| Cliente historico de xadrez online | **FICS / ICS** | Guest ou conta gratuita; senha so em canal seguro homologado | P1/P2 | Seguranca de transporte |
| Bancos profissionais/comerciais | **ChessBase, TWIC licenciado** | Planos/contratos externos | P3 | Nao bloqueia versao gratis |
| Jogar Chess.com via cliente proprio | **Solicitar acesso oficial** | OAuth/parceria, se aprovado | P3 | PubAPI nao envia lances |

---

## 18. Instrucao final de execucao para agentes de programacao

> **Implemente a evolucao incremental do programa de xadrez existente conforme esta especificacao, adotando FREE FIRST como politica de produto. Antes de modificar codigo, identifique e preserve recursos existentes. Crie uma Central de Credenciais e Conexoes centralizada, segura e multiusuario, com OAuth oficial onde disponivel, cofre para segredos, vinculacao de perfis publicos e fluxo de PGN manual quando nao houver API de autenticacao. Inclua explicitamente Lichess, PlayStrategy, FICS, FIDE, Chess.com, Queen Alice, ChessBase e provedores de engines. Nao solicite nem armazene senha de Queen Alice, ChessBase, FIDE ou Chess.com PubAPI para fingir acesso automatizado. Separe inteiramente jogo humano de BOT/analise com Fair Play Guard impossivel de contornar pelas APIs internas. Cada historia so pode ser aceita com teste e evidencia; execute primeiro M0-M3 e avance para M4-M7 sem iniciar custos ou usar endpoints privados. Registre limites, licencas, riscos, criterios de qualidade e divergencias entre plano e realidade. Entregue codigo, testes, documento de arquitetura, matriz de rastreabilidade e relatorio de conformidade.**

**Status do documento:** pronto como especificacao para planejamento e implementacao; **nao** afirma que estas integracoes ja foram feitas no software existente. APIs gratuitas e recursos P3 devem ser revalidados no momento da implementacao.
