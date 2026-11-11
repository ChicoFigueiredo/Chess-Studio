## Painel Explorador (CX-9): aberturas, finais exatos e a avaliação em cache da Análise.

dock-panel-explorer = Explorador
explorer-feature-name = O Explorador

explorer-tab-openings = Aberturas
explorer-tab-endgames = Finais

explorer-source-label = Fonte
explorer-source-library = Minha Biblioteca
explorer-source-masters = Mestres (Lichess)
explorer-source-lichess = Todos os jogadores (Lichess)
explorer-source-player = Um jogador (Lichess)
explorer-player-hint = Nome do jogador no Lichess
explorer-color-white = Como Brancas
explorer-color-black = Como Pretas
explorer-pace-label = Ritmos (nenhum marcado: todos)
explorer-pace-bullet = Bullet
explorer-pace-blitz = Blitz
explorer-pace-rapid = Rápida
explorer-pace-classical = Clássica
explorer-pace-correspondence = Correspondência
explorer-rating-label = Faixas de rating (nenhuma marcada: todas)
explorer-rating-below = Até 999
explorer-rating-above = 2500+
explorer-rating-tip-below = Jogadores com rating até { $to }
explorer-rating-tip = Jogadores com rating de { $from } a { $to }
explorer-rating-tip-above = Jogadores com rating { $from } ou mais

explorer-opening = { $eco } · { $name }
explorer-opening-unnamed = Posição fora do livro de aberturas
explorer-games-one = { $count } Partida
explorer-games-other = { $count } Partidas

explorer-col-move = Lance
explorer-col-games = Partidas
explorer-col-result = Resultado
explorer-col-rating = Rating
explorer-legend-white = Brancas
explorer-legend-draws = Empates
explorer-legend-black = Pretas
explorer-bar-tip = Brancas { $white }% · Empates { $draw }% · Pretas { $black }% ({ $decided } com resultado)
explorer-more-coming = Recebendo mais resultados…
explorer-references = Partidas de referência
explorer-reference-tip = Procurar esta Partida na Biblioteca
explorer-reference-white = Brancas { $rating }
explorer-reference-black = Pretas { $rating }

explorer-showing = Mostrando uma variante do Explorador, só para ver. A Partida não muda.
explorer-back = Voltar à Partida
explorer-library-error = Não foi possível ler a Biblioteca: { $error }

## Origem (rodapé)
explorer-origin-local = Origem: sua Biblioteca, sem internet
explorer-origin-queried-now = { $provider } · consultado agora · { $license }
explorer-origin-queried-minutes = { $provider } · consultado há { $count } min · { $license }
explorer-origin-queried-hours = { $provider } · consultado há { $count } h · { $license }
explorer-origin-queried-days = { $provider } · consultado há { $count } d · { $license }
explorer-origin-cached = { $provider } · do cache, de { $date } · { $license }
explorer-ago-now = agora há pouco
explorer-ago-minutes = há { $count } min
explorer-ago-hours = há { $count } h
explorer-ago-days = há { $count } d
explorer-stale = Sem conexão com a internet. Mostrando o que estava salvo, de { $date } ({ $ago }).

## Estados do Painel
explorer-fairplay-title = Indisponível durante a Partida
explorer-fairplay-text = A Trava de fair play desliga o Explorador, os finais e a avaliação em cache enquanto uma Partida competitiva está em andamento. Nada é consultado. Eles voltam quando a Partida terminar.
explorer-training-title = Indisponível durante um Exercício
explorer-training-text = Termine ou saia do Exercício para consultar o Explorador.
explorer-off-title = O { $provider } está desligado
explorer-off-text = Ligue o { $provider } para consultar esta fonte. Só a Posição do tabuleiro é enviada; nenhum dado seu.
explorer-off-button = Ligar { $provider }
explorer-signin-title = Requer autorizar sua conexão do { $provider }
explorer-signin-text = O Explorador de aberturas do { $provider } só responde a quem autorizou a conexão. Autorize na Central de Conexões e esta fonte passa a funcionar.
explorer-signin-center = Abrir a Central de Conexões
explorer-use-library = Usar Minha Biblioteca
explorer-player-title = Digite o nome de um jogador
explorer-player-text = Use o nome público no Lichess para ver como essa pessoa jogou a partir desta Posição.
explorer-loading = Consultando…
explorer-nodata-title = Sem dados para esta Posição
explorer-nodata-openings = Ninguém jogou esta Posição nesta fonte. Experimente outra fonte ou volte um Lance.
explorer-nodata-endgames = Os finais exatos não têm esta Posição.
explorer-limited-title = O serviço pediu uma pausa
explorer-limited-text = O Explorador volta a consultar sozinho em cerca de { $minutes } min.
explorer-limited-until = Pausa até { $time }
explorer-offline-title = Sem conexão com a internet
explorer-offline-text = Não há nada salvo para esta Posição. Quando a conexão voltar, tente de novo.
explorer-failed-title = Não foi possível consultar agora
explorer-failed-text = Tente de novo em alguns instantes.
explorer-retry = Tentar de novo
explorer-pieces-title = Disponível com 7 peças ou menos
explorer-pieces-text = Esta Posição tem { $pieces } peças. Os finais exatos aparecem quando restam 7 ou menos.
explorer-empty-title = Nenhuma Partida sua passou por aqui
explorer-empty-text = Importe Partidas na Biblioteca, inclusive as suas do Lichess e do Chess.com, para ver aqui como você costuma jogar esta Posição.
explorer-empty-button = Abrir a Biblioteca

## Finais
explorer-end-win-white = Vitória das Brancas
explorer-end-win-black = Vitória das Pretas
explorer-end-draw = Empate
explorer-end-nuance-cursed = A vitória existe, mas a regra dos 50 Lances a transforma em empate.
explorer-end-nuance-blessed = Estaria perdido, mas a regra dos 50 Lances salva o empate.
explorer-end-nuance-probable = Resultado provável: as bases usadas não o confirmam.
explorer-end-checkmate = Xeque-mate
explorer-end-stalemate = Empate por afogamento
explorer-end-insufficient = Empate por material insuficiente
explorer-end-moves = Lances, do melhor ao pior
explorer-end-result-win = Vitória
explorer-end-result-draw = Empate
explorer-end-result-loss = Derrota
explorer-end-result-fifty = Empate (50 Lances)
explorer-end-result-probable-win = Provável vitória
explorer-end-result-probable-loss = Provável derrota
explorer-end-mate-one = Mate em { $count } Lance
explorer-end-mate-other = Mate em { $count } Lances
explorer-end-dtm-tip = Quantos Lances faltam para o xeque-mate com jogo perfeito dos dois lados.
explorer-end-dtz = DTZ { $count }
explorer-end-dtz-tip = Distância até o próximo Lance de Peão ou captura (zeramento da regra dos 50 Lances), em meios-Lances.

## Avaliação em cache (Painel Análise)
explorer-cloud-title = Em cache (Lichess)
explorer-cloud-none = Sem avaliação em cache para esta Posição.
explorer-cloud-loading = Consultando o cache do Lichess…
explorer-cloud-off = O Lichess está desligado. Ligue para ver avaliações já calculadas por outros.
explorer-cloud-fairplay = Indisponível durante a Partida.
explorer-cloud-limited = O serviço pediu uma pausa. Volta sozinho em instantes.
explorer-cloud-failed = Não foi possível consultar agora.
explorer-cloud-depth = Profundidade { $depth }
explorer-cloud-pov = Do ponto de vista das Brancas
