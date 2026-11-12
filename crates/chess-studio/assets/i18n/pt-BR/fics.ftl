# FICS como visitante (#254). Linguagem de usuário: "Partida online", nunca "jogo". O aviso é
# honesto: a conexão com o FICS não tem criptografia, o app só entra como visitante e nunca
# pede nem envia senha, e a Partida de visitante que cai é perdida no FICS.

fics-title = Partida online no FICS
fics-provider-lichess = Lichess
fics-provider-fics = FICS · visitante
fics-badge = FICS · visitante
fics-consent-feature = A Partida online no FICS

# Aviso (tela da Partida online e cartão da Central)
fics-warning-plain = Conexão sem criptografia: os Lances, os comandos e o nome de visitante trafegam em texto puro até o FICS.
fics-warning-guest = Só como visitante: o FICS dá um nome GuestXXXX e só Partidas sem rating. O app nunca pede nem envia senha.
fics-warning-drop = Sem reconexão automática: se a conexão cair durante a Partida, o FICS a dá como perdida.
fics-warning-lag = Sem compensação de atraso: o atraso da rede entra no tempo do seu relógio.

# Central de Conexões
fics-auth-guest = Visitante, sem senha (o FICS dá o nome)
fics-card-how = Para jogar: Início > Nova Partida > Online, e escolha FICS.

# Desligado, entrada e saída
fics-off-title = O FICS está desligado
fics-off-text = Ligue o FICS para jogar como visitante. Nada sai do computador enquanto ele estiver desligado.
fics-turn-on = Ligar o FICS…
fics-guest-title = Jogar no FICS como visitante
fics-guest-text = Sem cadastro e sem senha: o FICS dá um nome de visitante para esta conexão. Antes de entrar:
fics-connect = Entrar como visitante
fics-connect-again = Entrar de novo como visitante
fics-connecting = Entrando como visitante
fics-connecting-text = Abrindo a conexão com o FICS. Nenhuma senha é enviada.
fics-logged-in = No FICS como { $handle }
fics-ready-note = Visitante: Partidas sem rating. Conexão sem criptografia.
fics-leave = Sair do FICS
fics-leave-warning = Sair agora é perder a Partida em andamento: o FICS a dá como perdida por desconexão. A Trava de fair play cai ao sair.
fics-leave-confirm = Sair e perder a Partida
fics-closed-title = Conexão com o FICS encerrada
fics-in-game = Há uma Partida em andamento no FICS: ela está no tabuleiro. Termine-a para buscar adversário, aceitar desafios ou observar.

# Abas
fics-tab-seek = Buscar adversário
fics-tab-challenges = Desafios
fics-tab-challenges-count = Desafios ({ $count })
fics-tab-observe = Observar

# Buscar
fics-pace-lightning = Relâmpago
fics-pace-blitz = Blitz
fics-pace-standard = Padrão
fics-seek-note = Visitante só joga sem rating. A Partida começa assim que alguém aceitar a busca.
fics-fairplay-note = Durante a Partida contra uma pessoa, a Trava de fair play desliga Engines, Explorador, finais e análise até o FICS confirmar o fim.
fics-seek-start = Buscar adversário
fics-seek-waiting = Procurando adversário no FICS
fics-seek-detail = { $pace } · sem rating
fics-seek-sent = Busca enviada; esperando o FICS publicar.
fics-seek-cancel = Cancelar busca

# Desafios
fics-challenges-empty-title = Nenhum desafio recebido
fics-challenges-empty = Outros jogadores do FICS podem desafiar você pelo nome { $handle }.

# Observar
fics-observe-label = Número da Partida no FICS
fics-observe-hint = Ex.: 77
fics-observe-invalid = Digite só o número da Partida.
fics-observe-start = Observar
fics-observe-stop = Parar de observar
fics-observe-note = Só para ver: a Partida observada fica na memória, não vai para a Biblioteca e não liga a Trava de fair play.
fics-observe-waiting = Pedido enviado; esperando o FICS.
fics-observe-now = Observando a Partida { $game }
fics-observe-ended = A observação da Partida { $game } terminou
fics-observe-moves = { $count ->
    [one] 1 Lance visto desde o começo da observação
   *[other] { $count } Lances vistos desde o começo da observação
}
fics-observe-last = Último Lance: { $san }

# Painel Partida online
fics-status-desync = A Partida no app não confere com o tabuleiro do FICS. Nada foi mudado e nenhum Lance foi inventado; aguardando um tabuleiro coerente do FICS.
fics-status-gone = Sem conexão com o FICS. No FICS, a Partida de visitante que cai é perdida; o resultado não pôde ser confirmado e o app não reconecta sozinho.
fics-dropped-title = A conexão com o FICS caiu
fics-dropped-text = A conexão com o FICS caiu durante a Partida. No FICS, a Partida de visitante que cai é dada como perdida; o resultado não pôde ser confirmado e o app não reconecta sozinho.
fics-dropped-lock = A Trava de fair play continua ligada até você descartar a sessão.
fics-discard = Descartar a sessão
fics-abort-tip = No FICS, abortar pede a concordância do adversário, salvo antes do primeiro Lance.
fics-fairplay-on = Trava de fair play ligada: Engines, Explorador, finais e análise ficam desligados até o fim confirmado pelo FICS.
fics-ended-aborted = A Partida foi abortada no FICS.
fics-ended-saved = A Partida está na Biblioteca, com a Origem FICS (visitante).
fics-resign-text = O FICS registra a derrota. Não dá para desfazer.
fics-panel-incoming-one = 1 desafio recebido no FICS
fics-panel-incoming-other = { $count } desafios recebidos no FICS
fics-panel-see-challenges = Ver desafios

# Avisos
fics-notice-logged-in = No FICS como { $handle }.
fics-notice-seek-posted = Busca publicada no FICS.
fics-notice-challenge = { $name } desafia você no FICS.
fics-notice-move-refused = O FICS recusou o Lance { $mv }: a peça voltou. Jogue de novo.
fics-notice-move-lost = O Lance { $mv } não foi confirmado pelo FICS e não é reenviado.
fics-notice-move-replaced = O FICS registrou outro Lance no lugar de { $mv }.
fics-notice-variant = Uma Partida em variante ({ $variant }) foi recusada: o app só joga xadrez padrão.
fics-notice-desync = A Partida no app não confere com o FICS. Nada foi reescrito.
fics-notice-resync = O FICS voltou a conferir com a Partida do app.
fics-notice-abort-requested = O adversário pediu para abortar. Use Abortar para aceitar.
fics-notice-observe-started = Observando a Partida { $game } do FICS.
fics-notice-observe-ended = A observação da Partida { $game } terminou.

# Erros em linguagem de usuário
fics-error-insecure = O FICS não oferece conexão cifrada: uma senha iria em texto puro. O app só entra como visitante.
fics-error-password = O servidor pediu senha na entrada como visitante. O app nunca envia senha e encerrou a conexão.
fics-error-login-timeout = O FICS não confirmou a entrada como visitante a tempo. Tente de novo.
fics-error-closed = A conexão com o FICS foi encerrada.
fics-error-variant = O app só joga xadrez padrão: o desafio em { $variant } não foi aceito.
fics-error-desync = A Partida no app não confere com o FICS: espere o próximo tabuleiro.
fics-error-not-allowed = Agora não: { $why }.
fics-error-rejected = Este Lance não vale agora.
fics-error-disabled = O FICS está desligado. Ligue-o para jogar como visitante.
fics-error-fair-play = A Trava de fair play está ligada por outra Partida em andamento: o FICS espera o fim dela.
fics-error-offline = Sem conexão com o FICS. Confira a internet e tente de novo.
fics-error-generic = O FICS recusou o pedido.
