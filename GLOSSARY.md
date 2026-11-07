# Chess Studio

Mesa pessoal de treinamento de xadrez, offline, para que o enxadrista aprenda a jogar presencialmente sem depender de auxílios digitais.

Os termos são em português; o equivalente em inglês (_en_) é o nome usado no código.

## Partida e posição

**Partida** (_en_: `Game`):
Sequência de Lances e eventos regulamentares entre Branco e Preto, com resultado e Política de arbitragem.
_Evitar_: jogo, sessão

**Posição canônica** (_en_: `CanonicalPosition`):
Estado oficial reconhecido pelo núcleo de regras: tabuleiro, lado a jogar, direitos de roque, en passant e contadores.
_Evitar_: estado do tabuleiro, posição atual

**Posição visual provisória** (_en_: `ProvisionalView`):
Disposição exibida durante um arrasto ou Lance ainda não aceito; nunca é a Posição canônica.
_Evitar_: preview, posição temporária

**FEN** (_en_: `Fen`):
Representação textual de uma Posição canônica; não substitui o histórico da Partida.

**PGN** (_en_: `Pgn`):
Formato interoperável de exportação dos Lances legais e metadados de uma Partida.

## Lances

**Lance** (_en_: `Move`):
Movimento de uma peça por um lado, segundo as regras do xadrez.
_Evitar_: jogada, movimento

**Lance feito** (_en_: `MadeMove`):
Lance executado no tabuleiro (peça solta na casa de destino), antes de acionar o relógio.
_Evitar_: jogada feita

**Lance concluído** (_en_: `CompletedMove`):
Lance feito e confirmado pelo acionamento do relógio, ou encerrado de imediato por mate ou afogamento.
_Evitar_: jogada concluída, lance confirmado

**Lance ilegal** (_en_: `IllegalMove`):
Lance que as regras não permitem na Posição canônica; pode estar feito ou concluído, e nunca entra no histórico legal.
_Evitar_: tentativa ilegal, jogada inválida

## Manipulação

**Seleção** (_en_: `Selection`):
Foco de uma peça ou casa na interface, sem consequência regulamentar própria.
_Evitar_: clique, toque

**Toque intencional** (_en_: `IntentionalTouch`):
Contato deliberado com uma peça que, numa Política competitiva, cria Obrigação de peça tocada.
_Evitar_: seleção, clique

**Ajuste** (_en_: `Adjustment`):
Toque declarado apenas para acomodar a peça na casa (j'adoube), sem criar obrigação.
_Evitar_: j'adoube

**Obrigação de peça tocada** (_en_: `TouchMoveObligation`):
Dever de mover (ou capturar) a peça tocada intencionalmente, quando houver Lance legal para isso.
_Evitar_: peça tocada é peça jogada

## Arbitragem

**Política de arbitragem** (_en_: `ArbitrationPolicy`):
Conjunto de regras que decide as consequências de cada ação na Partida: Assistido, Simulação FIDE ou Eliminação Imediata.
_Evitar_: modo, modalidade, nível de rigor

**Política competitiva** (_en_: `competitive policy`):
Simulação FIDE ou Eliminação Imediata; as Políticas em que há Infrações e não há desfazer.
_Evitar_: modo torneio, modo competição

**Assistido** (_en_: `Assisted`):
Política de arbitragem de aprendizado: Lances ilegais são recusados sem penalidade.

**Simulação FIDE** (_en_: `FideSimulation`):
Política de arbitragem que reproduz as Leis do Xadrez da FIDE para partidas clássicas.
_Evitar_: modo FIDE oficial, regulamentar

**Eliminação Imediata** (_en_: `SuddenElimination`):
Política de arbitragem personalizada de treinamento em que a primeira Infração encerra a Partida; nunca é apresentada como FIDE.
_Evitar_: morte súbita

**Infração** (_en_: `Infraction`):
Registro, feito pela Política de arbitragem, de um Lance ilegal concluído ou ato equivalente, com causa, instante, lado e consequência.
_Evitar_: erro, falta, incidente

**Reclamação de empate** (_en_: `DrawClaim`):
Pedido do jogador para encerrar a Partida empatada por repetição tripla ou 50 lances, sujeito à verificação da Política de arbitragem.
_Evitar_: pedir empate, proposta de empate

**Proposta de empate** (_en_: `DrawOffer`):
Oferta de empate de um lado ao outro, que só encerra a Partida se aceita.

**Pausa** (_en_: `Pause`):
Interrupção registrada da Partida em que os relógios param; perder o foco da janela não é Pausa.

## Apresentação

**Apresentação** (_en_: `Presentation`):
Forma visual de exibir a mesma Partida: 3D profissional, Mesa imersiva ou 2D técnico.
_Evitar_: visual, modo visual, tema

**Vista** (_en_: `CameraView`):
Enquadramento da câmera dentro de uma Apresentação: superior, oblíqua, padrão ou invertida.
_Evitar_: câmera, ângulo

**Auxílio visual** (_en_: `VisualAid`):
Marcação adicionada ao tabuleiro que não existiria na mesa física: destinos legais, fantasmas, destaques, setas, alertas.
_Evitar_: dica, ajuda, hint

**Perfil de auxílios** (_en_: `AidProfile`):
Conjunto nomeado de Auxílios visuais ligados e desligados, como Jogo físico (nenhum) ou Treinamento.
_Evitar_: preset de ajuda, nível de dificuldade

**Perfil de interface** (_en_: `InterfaceProfile`):
Conjunto nomeado de Apresentação, layout do dock (painéis visíveis, abas, larguras), Perfil de auxílios, modo Foco e Vista da câmera: Jogo físico, Treinamento assistido, Estudo e análise (de fábrica) ou um salvo pelo usuário; "Personalizado" é o estado atual quando não coincide com nenhum. Trocar de perfil nunca altera a Partida. Não confundir com Perfil de auxílios (só os Auxílios) nem com Perfil de treino.
_Evitar_: tema, workspace, modo

**HUD** (_en_: `Hud`):
Sobreposição virtual de treino ou análise presa às casas e peças; não envolve câmera física.
_Evitar_: realidade aumentada

**Tela cheia** (_en_: `Fullscreen`):
Janela sem bordas ocupando o monitor inteiro (F11 ou item de menu); estado de interface, independente do modo Foco.
_Evitar_: maximizar

**Modo Foco** (_en_: `FocusMode`):
Estado de interface que esconde o dock e a janela principal, deixando só tabuleiro, mesa, relógio e controles mínimos; nunca altera turno, relógio ou posição, e é independente da Tela cheia.
_Evitar_: modo zen, tela limpa

## Treinamento

**Exercício** (_en_: `Exercise`):
Posição inicial com objetivo e solução definidos, usada para treinar uma regra, um final ou a memória.
_Evitar_: puzzle, problema, desafio

**Resolução** (_en_: `ExerciseRun`):
Uma tentativa do usuário de resolver um Exercício, com resultado, Auxílios usados e duração.
_Evitar_: tentativa, attempt

**Perfil de treino** (_en_: `TrainingProfile`):
Sequência nomeada de Exercícios, cada passo com uma dificuldade (ex.: "Regras básicas"); o avanço é gravado por jogador. Não confundir com Perfil de auxílios.
_Evitar_: trilha, plano

## Engine

**Engine** (_en_: `Engine`):
Programa externo de xadrez, falando UCI, que propõe Lances ou análises e nunca decide legalidade.
_Evitar_: motor, IA, bot

**Súmula manual** (_en_: `ManualScoresheet`):
Registro escrito pelo próprio jogador como exercício de competição; diferente da notação automática.
_Evitar_: planilha, scoresheet
