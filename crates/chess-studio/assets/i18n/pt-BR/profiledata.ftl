# Dados do Perfil local e das Origens (CX-19, #242): rating FIDE, "Limpar dados guardados" e
# apagar Partidas importadas em lote. Linguagem de quem usa: Provedor, Conexão, Perfil local.

# --- Rating FIDE (cartão da FIDE na Central e página Perfil local) ---

fide-title = Rating FIDE
fide-feature = A consulta do rating FIDE
fide-feature-search = A busca de jogadores da FIDE
fide-not-linked = Nenhum ID FIDE vinculado. Busque o jogador pelo nome para achar o ID e vincular, ou vincule o ID abaixo, em Detalhes.
fide-not-linked-here = Nenhum ID FIDE vinculado a este Perfil local. Vincule o seu ID na Central de Conexões.
fide-open-central = Abrir a Central de Conexões
fide-never = O ID { $id } ainda não foi consultado. A consulta usa o Lichess, que reproduz os dados da FIDE.
fide-consult = Consultar rating
fide-refresh = Atualizar
fide-working = Consultando…

fide-rhythm-standard = Standard
fide-rhythm-rapid = Rápido
fide-rhythm-blitz = Blitz
fide-no-rating = A FIDE não tem rating neste ritmo para este jogador.

fide-title-hint = Título concedido pela FIDE.
fide-inactive = Inativo
fide-inactive-hint = A FIDE marca este jogador como inativo.
fide-federation = Federação { $code }
fide-id = ID { $id }

fide-origin = FIDE, via Lichess, consultado { $when }.
fide-explain = Cada ritmo é uma lista própria da FIDE e não se compara com ratings online. A data é a da consulta: o Lichess não informa de qual mês é a lista.
fide-offline = O Lichess está desligado: este é o último valor guardado neste computador.

fide-when-now = agora há pouco
fide-when-minutes = há { $count } min
fide-when-hours = há { $count } h
fide-when-days = em { $date } (há { $count } dias)

fide-note-updated = Rating atualizado e guardado neste Perfil local.
fide-note-same = Consultado agora: os ratings continuam os mesmos.
fide-note-not-found = A FIDE não tem um jogador com este ID.
fide-note-limited = O Lichess pediu uma pausa. Tente de novo daqui a pouco.
fide-note-not-turned-on = O Lichess continua desligado; nada foi consultado.
fide-note-save-failed = Consultado, mas não foi possível guardar o valor neste computador.
fide-note-search-short = Digite de 2 a 60 letras do nome.

fide-search-show = Buscar outro jogador pelo nome
fide-search-hide = Fechar a busca
fide-search-title = Buscar jogador da FIDE pelo nome
fide-search-hint = Sobrenome, nome (ao menos 2 letras)
fide-search-go = Buscar
fide-search-empty = Nenhum jogador da FIDE encontrado com este nome.
fide-search-more = Mostrando { $shown } de { $total } jogadores. Refine o nome para ver menos.
fide-hit-ratings = Standard { $standard } · Rápido { $rapid } · Blitz { $blitz } · ID { $id }
fide-hit-link = Vincular este ID
fide-hit-linked = Vinculado

# --- Limpar dados guardados (Central de Conexões) ---

stored-title = Dados guardados
stored-intro = As respostas dos Provedores ficam guardadas neste computador para as consultas serem rápidas e funcionarem sem rede.
stored-clear = Limpar dados guardados…
stored-done-none = Não havia nada guardado para limpar.
stored-done-one = Pronto: 1 resposta guardada foi apagada.
stored-done-many = Pronto: { $count } respostas guardadas foram apagadas.
stored-failed = Não foi possível limpar agora. Nada foi alterado.

stored-dialog-title = Limpar dados guardados?
stored-goes-title = O que some
stored-goes-1 = As respostas guardadas de todos os Provedores neste Perfil local: perfis, ratings, avaliações, Explorador e listas de Partidas.
stored-goes-2 = Nada além disso.
stored-stays-title = O que fica
stored-stays-1 = Suas Partidas jogadas no Chess Studio.
stored-stays-2 = As Partidas e os Exercícios importados.
stored-stays-3 = Suas Conexões e as autorizações.
stored-stays-4 = O último rating FIDE guardado no Perfil local.
stored-stays-5 = O que está ligado ou desligado em cada Provedor.
stored-after = Depois disso, cada consulta pergunta de novo ao Provedor, se ele estiver ligado.
stored-confirm = Limpar dados guardados
stored-cancel = Cancelar

# --- Importações da Biblioteca ---

library-tab-imports = Origens
bulk-intro = As Partidas que você trouxe de fora, por origem. As que você jogou no Chess Studio não aparecem aqui e nunca são apagadas.
bulk-empty = Você ainda não importou Partidas. Importe um arquivo PGN ou use "Importar online" para trazer Partidas de um Provedor.
bulk-by-provider = Tudo de um Provedor
bulk-provider-hint = Marca todas as importações deste Provedor.
bulk-by-import = Por importação
bulk-unnamed = Sem nome
bulk-import-line = { $mode } em { $date }
bulk-games-one = 1 Partida
bulk-games-many = { $count } Partidas
bulk-delete = Apagar marcadas…
bulk-clear-selection = Limpar marcação
bulk-nothing-marked = Marque um Provedor ou uma importação para apagar.

bulk-dialog-title = Apagar Partidas importadas?
bulk-dialog-nothing = Nenhuma Partida será apagada.
bulk-dialog-count-one = 1 Partida importada será apagada
bulk-dialog-count-many = { $count } Partidas importadas serão apagadas
bulk-scope-provider = Tudo de { $provider } ({ $games })
bulk-scope-batch = { $name }, de { $date } ({ $games })
bulk-dialog-kept-one = 1 Partida fica, porque também veio de outra origem.
bulk-dialog-kept-many = { $count } Partidas ficam, porque também vieram de outra origem.
bulk-dialog-safe = As Partidas que você jogou no Chess Studio não são tocadas.
bulk-dialog-undo = Não dá para desfazer, mas você pode importar de novo quando quiser.
bulk-dialog-confirm-one = Apagar 1 Partida
bulk-dialog-confirm-many = Apagar { $count } Partidas
bulk-dialog-cancel = Cancelar

bulk-done-one = Pronto: 1 Partida importada foi apagada.
bulk-done-many = Pronto: { $count } Partidas importadas foram apagadas.
bulk-done-kept-one = 1 Partida ficou, porque também veio de outra origem.
bulk-done-kept-many = { $count } Partidas ficaram, porque também vieram de outra origem.
bulk-failed = Não foi possível apagar agora. Nada foi alterado.
