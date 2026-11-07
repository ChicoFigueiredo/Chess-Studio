## Biblioteca (C7): pesquisa de Partidas, importação de PGN, Posições favoritas e Exercícios por tipo.

dock-panel-library = Biblioteca

library-tab-games = Partidas
library-tab-favorites = Favoritas
library-tab-exercises = Exercícios
library-unavailable-training = Abrir itens fica indisponível durante um Exercício de Treino.
library-needs-database = Sem banco de dados: não é possível reabrir Partidas guardadas.

library-any = Todos
library-any-policy = Qualquer Política
library-source-any = Jogadas e importadas
library-source-played = Jogada
library-source-imported = Importada
library-result-white = Brancas vencem (1-0)
library-result-black = Pretas vencem (0-1)
library-result-draw = Empate (1/2-1/2)
library-result-unfinished = Sem resultado (*)

library-search-hint = Pesquisar: jogador, evento, abertura (e4 c5)…
library-filters = Filtros
library-filter-player = Jogador ou Engine
library-filter-date = Data
library-filter-date-hint = AAAA.MM.DD
library-filter-position = Posição (FEN) — acha Partidas que passam por ela
library-position-current = Usar a posição do tabuleiro
library-filters-clear = Limpar filtros
library-count = { $count } Partidas
library-empty = Nenhuma Partida encontrada.
library-plies = { $count } meios-lances
library-open = Abrir
library-matched-ply = Posição no meio-lance { $ply }

library-import-heading = Importar PGN…
library-import-help = Informe o caminho de um arquivo .pgn (ou escolha um da pasta de documentos). Partidas com erro são puladas e listadas.
library-import-hint = /caminho/para/arquivo.pgn
library-import-button = Importar
library-import-suggestions = Arquivos .pgn nos documentos:
library-import-done = { $file }: { $count } Partidas importadas.
library-import-errors = { $count } Partidas puladas por erro:
library-import-failed = Não foi possível importar: { $detail }

library-back = Voltar à lista
library-delete-imported = Excluir da biblioteca
library-tags = Informações do PGN
library-mainline = Linha principal
library-variation = Variação em { $move }
library-tree-summary = { $plies } meios-lances, { $variations } variações
library-study-position = Estudar esta posição
library-study-position-hover = Abre a posição numa Partida Assistida de estudo (substitui a Partida atual no tabuleiro; a anterior continua guardada).
library-study-mainline = Estudar a linha principal
library-study-mainline-hover = Reproduz a linha principal numa Partida Assistida, já na revisão deste Lance. As variações ficam só na biblioteca.
library-opened-study = Posição aberta numa Partida Assistida de estudo.
library-opened-played = Partida reaberta em revisão (continua guardada; retome pela Notação).

library-fav-hint = Nome (padrão: lance { $ply })
library-fav-mark = Marcar posição
library-fav-mark-hover = Marca como favorita a posição que está no tabuleiro.
library-fav-mark-this = Favoritar
library-fav-marked = Posição marcada como favorita.
library-fav-default-name = Posição após o meio-lance { $ply }
library-fav-search = Pesquisar nas favoritas (nome ou nota)…
library-fav-note-hint = Nota
library-fav-empty = Nenhuma posição favorita ainda. Marque a posição do tabuleiro com "Marcar posição".
library-fav-open = Estudar
library-fav-source = Partida de origem
library-fav-source-hover = Reabre a Partida de onde a posição foi marcada (meio-lance { $ply }).
library-fav-delete = Remover

library-exercise-search = Pesquisar Exercícios…
library-exercise-count = { $count } Exercícios
