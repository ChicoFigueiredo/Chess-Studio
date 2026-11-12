## Painel Análise (C8): avaliação contínua da Engine em estudo (US-17, UI-08).

analysis-toggle-on = Ligar análise
analysis-toggle-off = Desligar análise
analysis-engine-label = Engine
analysis-engine-default = Engine padrão
analysis-lines-label = Linhas
analysis-threads-label = Núcleos do processador
analysis-hash-label = Memória da Engine (MB)
analysis-pause-while-thinking = Pausar enquanto a Engine adversária pensa
analysis-load-note = A análise usa uma segunda Engine (outro processo) e consome CPU; ela nunca mexe no relógio nem no lance do adversário.
analysis-status-off = Análise desligada.
analysis-status-starting = Iniciando a Engine…
analysis-status-thinking = Calculando…
analysis-status-paused = Pausada: a Engine adversária está pensando.
analysis-status-no-moves = Sem lances: a Partida termina nesta posição.
analysis-status-no-engine = Nenhuma Engine encontrada. Instale ou escolha uma no painel Engines.
analysis-status-failed = A Engine de análise falhou
analysis-retry = Tentar de novo
analysis-locked-training = Indisponível durante um Exercício de Treino.
analysis-white-pov = Avaliação do ponto de vista das Brancas.
analysis-depth = Profundidade { $depth } (seletiva { $seldepth })
analysis-depth-plain = Profundidade { $depth }
analysis-speed = { $nodes } nós · { $nps } nós/s
analysis-line-hint = Clique num lance para ver a linha no tabuleiro; a Partida não muda.
analysis-back-to-game = Voltar à Partida
analysis-showing-line = Mostrando uma linha da análise (somente leitura).
analysis-mate-white = Mate das Brancas em { $n }
analysis-mate-black = Mate das Pretas em { $n }

## Controle de carga e backend (C10, UCI-06).

analysis-load-label = Carga
analysis-load-max = Máximo
analysis-load-balanced = Equilibrado
analysis-load-light = Leve
analysis-load-max-hint = Usa tudo o que está configurado, mesmo que o FPS caia abaixo de { $fps }.
analysis-load-balanced-hint = Reduz a análise sozinha quando o FPS cai abaixo de { $fps } e a devolve quando o app se recupera.
analysis-load-light-hint = Já começa reduzida e reage cedo; o app fica fluido (meta de { $fps } FPS).
analysis-load-reduced = Análise reduzida para manter o FPS.
analysis-load-paused = Análise pausada para manter o FPS; ela volta quando o app se recuperar.
analysis-load-below-target = O FPS está abaixo da meta e o modo Máximo não reduz a análise. Escolha Equilibrado ou Leve para protegê-lo.
analysis-backend-label = Backend
analysis-backend-auto = Automático
analysis-backend-unsupported = indisponível neste sistema
analysis-backend-active = Em uso: { $backend }
analysis-locked-fair-play = Indisponível: a Trava de fair play está ligada. A análise (local e remota) volta quando a Partida online terminar.

# Engine remota (#252, CX-11)
remote-engine-entry = Stockfish (Chess-API.com) · Remota
remote-badge = Remota · Chess-API.com
remote-warning-title = Usar a Engine remota?
remote-warning-lead = A Engine remota analisa no servidor do { $provider }, não neste computador. Para isso, a Posição que você está analisando sai da sua máquina e vai pela internet.
remote-warning-sends-label = O que é enviado
remote-warning-sends = Só a Posição (onde estão as peças, de quem é a vez e os direitos de roque) e a profundidade pedida. Nunca nomes, histórico da Partida, Perfil local ou Conexão.
remote-warning-footer = A Engine remota nunca é a padrão e só consulta o serviço enquanto você a mantiver escolhida. Para voltar, escolha outra Engine na lista. Ela não funciona durante uma Partida online contra uma pessoa (Trava de fair play).
remote-warning-continue = Continuar
remote-warning-cancel = Cancelar
remote-active-note = A Posição analisada sai deste computador para o { $provider }.
remote-origin = Origem: { $provider }, consultado { $when }.
remote-origin-pending = Origem: { $provider }; ainda sem resposta para esta Posição.
remote-act-turn-on = Ligar o Chess-API.com
remote-problem-disabled = O Chess-API.com está desligado, então nada foi enviado. Ligue-o para usar a Engine remota.
remote-problem-fairplay = Trava de fair play: a Engine remota fica bloqueada enquanto a Partida online contra uma pessoa estiver em andamento. Nada foi enviado.
remote-problem-limit = O Chess-API.com pediu uma pausa. Tente de novo em { $seconds } s; a análise local continua disponível.
remote-problem-invalid = A resposta da Engine remota não pôde ser usada, então nada foi mostrado. Tente de novo ou use uma Engine local.
remote-problem-offline = A Engine remota não respondeu agora. Verifique a conexão e tente de novo; a análise local continua disponível.
remote-problem-generic = A Engine remota não pôde analisar esta Posição. Tente de novo ou use uma Engine local.
