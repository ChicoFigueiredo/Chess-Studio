# Engine online no Lichess (#252, CX-13). Termo do glossário: "Engine online". "BOT" só aparece
# como o nome do tipo de conta do Lichess.

engine-online-title = Engine online no Lichess
engine-online-close = Fechar
engine-online-cancel = Cancelar
engine-online-intro = Deixe uma Engine instalada neste computador responder a desafios no Lichess, com a sua conta de Engine. Nada começa sozinho: ela só liga quando você clica em Ligar.

# Estado
engine-online-state-stopped = Parada
engine-online-state-waiting = Esperando desafios
engine-online-state-playing = Jogando { $count }
engine-online-state-stopping = Parando
engine-online-counts = Nesta execução: { $accepted } aceitos · { $declined } recusados · { $finished } terminadas
engine-online-stop-user = Parando: não aceita mais desafios e fecha quando as Partidas em andamento terminarem.
engine-online-stop-user-now = Parando já: as Partidas em andamento estão sendo abandonadas.
engine-online-stop-limit = Limite de Partidas atingido: não aceita mais desafios e fecha quando as Partidas em andamento terminarem.
engine-online-stop-lost = O Lichess encerrou a conexão ({ $why }). As Partidas em andamento terminam e a Engine fecha.

# Pré-requisitos
engine-online-needs-connection = Falta autorizar a Conexão de Engine do Lichess na Central de Conexões. Ela é separada da Conexão de jogador.
engine-online-provider-off = O Lichess está desligado. Ligue-o na Central de Conexões para a Engine responder a desafios.
engine-online-open-center = Abrir a Central
engine-online-already = A Engine online já está ligada.
engine-online-fairplay = A Engine online não liga agora. { $why }

# Limites
engine-online-settings = Limites
engine-online-engine = Engine
engine-online-no-engines = Nenhuma Engine instalada. Instale uma no gerenciador de Engines.
engine-online-speeds = Ritmos aceitos
engine-online-speed-bullet = Bullet
engine-online-speed-blitz = Blitz
engine-online-speed-rapid = Rápida
engine-online-speed-classical = Clássica
engine-online-rated = Valendo rating
engine-online-rated-both = Os dois
engine-online-rated-casual = Só sem rating
engine-online-rated-only = Só valendo
engine-online-opponents = Quem pode desafiar
engine-online-opp-people = Só pessoas
engine-online-opp-any = Pessoas e Engines
engine-online-concurrent = Partidas ao mesmo tempo
engine-online-max-games = Máximo de Partidas nesta execução
engine-online-max-none = Sem limite
engine-online-draw = Proposta de empate do adversário
engine-online-draw-decline = Recusar
engine-online-draw-accept = Aceitar

# Controles
engine-online-start = Ligar a Engine online
engine-online-start-hint = Escolha uma Engine e ao menos um ritmo.
engine-online-stop = Parar
engine-online-stop-now = Parar já…
engine-online-stop-now-warning = Parar já abandona as Partidas em andamento: o adversário recebe a vitória. Para deixar as Partidas terminarem, use Parar.
engine-online-stop-now-confirm = Abandonar e parar

# Partidas e histórico
engine-online-games = Partidas em andamento
engine-online-side-white = Brancas
engine-online-side-black = Pretas
engine-online-mode-rated = valendo rating
engine-online-mode-casual = sem rating
engine-online-game-reconnecting = reconectando
engine-online-game-over = terminada
engine-online-game-thinking = a Engine está pensando
engine-online-game-waiting = esperando o adversário
engine-online-game-line = { $plies } meios-lances · { $status }
engine-online-activity = Atividade recente
engine-online-act-accepted = Desafio de { $name } aceito.
engine-online-act-declined = Desafio de { $name } recusado: { $why }.
engine-online-act-started = Partida com { $name } começou.
engine-online-act-ended = Partida com { $name } terminou ({ $result }).
engine-online-act-accept-failed = O desafio de { $name } não pôde ser aceito: { $why } Confira se a conta já é uma conta BOT.
engine-online-act-move-rejected = A Engine propôs um Lance ilegal e ele não foi enviado.
engine-online-act-engine-failed = A Engine falhou e a Partida foi abandonada.
engine-online-act-engine-start-failed = A Engine não iniciou e a Partida foi abandonada.
engine-online-act-request-failed = Um pedido ao Lichess falhou: { $why }
engine-online-act-session-closed = Uma Partida parou de ser acompanhada: { $why }
engine-online-act-store-failed = Não foi possível guardar uma Partida na Biblioteca.
engine-online-act-fairplay-declined = Desafio de { $name } recusado: Trava de fair play (há uma Partida online contra uma pessoa em andamento).
engine-online-act-fairplay-resigned = A Partida com { $name } começou sob a Trava de fair play: a Engine não joga e a Partida é abandonada.
engine-online-decline-later = já há Partidas demais ou o limite foi atingido
engine-online-decline-pace = ritmo fora dos aceitos
engine-online-decline-rated = só aceita Partidas sem rating
engine-online-decline-casual = só aceita Partidas valendo rating
engine-online-decline-variant = só xadrez padrão
engine-online-decline-nobot = não joga contra outras Engines
engine-online-decline-onlybot = só joga contra outras Engines
engine-online-decline-generic = fora dos limites escolhidos

# Conversão da conta em BOT (irreversível)
engine-online-convert-title = A conta ainda não é uma conta BOT?
engine-online-convert-hint = Converter a conta de Engine em conta BOT (não tem volta).
engine-online-convert-sending = Enviando o pedido ao Lichess…
engine-online-convert-done = A conta foi convertida em conta BOT.
engine-online-convert-failed = O Lichess não converteu a conta. Ela precisa ser nova, sem Partidas jogadas.
engine-online-convert-understood = Entendi que isto não tem volta e que vou usar uma conta criada só para a Engine.
engine-online-convert-act = Converter em conta BOT

engine-online-footer = Fechar esta janela não para a Engine; use Parar. Se você fechar o Chess Studio, a Engine deixa de responder e as Partidas em andamento no Lichess perdem por tempo.
