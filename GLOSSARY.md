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

**FEN** (_en_: `from_fen` e `to_fen`):
Representação textual de uma Posição canônica; não substitui o histórico da Partida.

**PGN** (_en_: `PgnGame`):
Formato interoperável de exportação dos Lances legais e metadados de uma Partida.

## Lances

**Lance** (_en_: `Move`):
Movimento de uma peça por um lado, segundo as regras do xadrez.
_Evitar_: jogada, movimento

**Lance feito** (_en_: `MoveMade`):
Lance executado no tabuleiro (peça solta na casa de destino), antes de acionar o relógio.
_Evitar_: jogada feita

**Lance concluído** (_en_: `MoveCompleted`):
Lance feito e confirmado pelo acionamento do relógio, ou encerrado de imediato por mate ou afogamento.
_Evitar_: jogada concluída, lance confirmado

**Lance ilegal** (_en_: `IllegalMoveReleased`):
Lance que as regras não permitem na Posição canônica; pode estar feito ou concluído, e nunca entra no histórico legal.
_Evitar_: tentativa ilegal, jogada inválida

## Manipulação

**Seleção** (_en_: `SelectedSquare`):
Foco de uma peça ou casa na interface, sem consequência regulamentar própria.
_Evitar_: clique, toque

**Toque intencional** (_en_: `PieceTouched`):
Contato deliberado com uma peça que, numa Política competitiva, cria Obrigação de peça tocada.
_Evitar_: seleção, clique

**Ajuste** (_en_: `PieceAdjusted`):
Toque declarado apenas para acomodar a peça na casa (j'adoube), sem criar obrigação.
_Evitar_: j'adoube

**Obrigação de peça tocada** (_en_: `TouchedPieceObligation`):
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

**Infração** (_en_: `InfractionRecorded`):
Registro, feito pela Política de arbitragem, de um Lance ilegal concluído ou ato equivalente, com causa, instante, lado e consequência.
_Evitar_: erro, falta, incidente

**Reclamação de empate** (_en_: `ClaimDraw`):
Pedido do jogador para encerrar a Partida empatada por repetição tripla ou 50 lances, sujeito à verificação da Política de arbitragem.
_Evitar_: pedir empate, proposta de empate

**Proposta de empate** (_en_: `DrawOffered`):
Oferta de empate de um lado ao outro, que só encerra a Partida se aceita.

**Pausa** (_en_: `Paused`):
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
Conjunto nomeado de Apresentação, layout do dock (Painéis visíveis, abas, larguras), Painéis flutuantes e destacados com suas posições, estado do Ribbon (aba, modo simples ou recolhido), Perfil de auxílios, modo Foco e Vista da câmera. De fábrica: Jogo físico, Treinamento assistido, Estudo e análise, Competição e Mínimo; os outros são salvos pelo usuário. "Personalizado" é o estado atual quando não coincide com nenhum. Uma instalação nova ou um Perfil local novo abre em Jogo físico ("tabuleiro primeiro"). Trocar de perfil nunca altera a Partida nem o Tema. Não confundir com Perfil de auxílios (só os Auxílios) nem com Perfil de treino.
_Evitar_: tema, workspace, modo

**Perfil local** (_en_: `LocalProfiles`):
Um jogador deste computador: nome e, só dele, preferências, Perfis de interface, controles, Partidas e Resoluções. Vários podem coexistir; o seletor da abertura aparece quando há mais de um. Excluir apaga os dados do perfil; o último não se exclui. Não confundir com Perfil de interface, Perfil de auxílios nem Perfil de treino, que existem *dentro* de um perfil local.
_Evitar_: usuário, conta, login

**Ação** (_en_: `Action`):
Algo que o usuário pode mandar o aplicativo fazer (Nova Partida, Desfazer, Virar tabuleiro, Flutuar Painel…), com rótulo, ícone, atalhos e estado (habilitada ou marcada). É uma lista única: Ribbon, Barra de Acesso Rápido, KeyTips, Paleta de Ações e Mapa de controles são lugares onde a mesma Ação aparece.
_Evitar_: comando, command, botão (como sinônimo)

**Barra de Acesso Rápido** (_en_: `QuickAccessBar`):
Fileira de Ações fixas acima do Ribbon, escolhidas pelo usuário e guardadas no Perfil de interface.
_Evitar_: QAT, favoritos

**Paleta de Ações** (_en_: `ActionPalette`):
Busca por nome de qualquer Ação habilitada (Ctrl+K), que a executa ao confirmar.
_Evitar_: paleta de comandos, command palette

**Densidade** (_en_: `Density`):
Espaçamento da interface, Confortável ou Compacta; junto com a Escala da interface (80% a 200%), é preferência do Perfil local, como o Tema.
_Evitar_: zoom (para densidade)

**Mapa de controles** (_en_: `ControlMap`):
Tabela única ação -> atalhos (tecla, botão do mouse com modificadores, roda ou gesto de trackpad), consultada por todos os sistemas de entrada. O padrão é o da DEC-51; cada Perfil local guarda o seu. Um mesmo atalho não serve a duas ações (conflito).
_Evitar_: keybindings, hotkeys

**HUD** (_en_: `HudSettings`):
Sobreposição virtual de treino ou análise presa às casas e peças; não envolve câmera física.
_Evitar_: realidade aumentada

**Painel** (_en_: `Panel`):
Bloco da interface com um assunto só (Relógio, Notação, Análise, Aberturas, Súmula…), que pode estar encaixado no dock, flutuante ou destacado. A configuração do Painel vai junto quando ele muda de lugar. Cada Painel tem uma caixa de seleção no grupo Painéis da aba Exibir; a Ação "Ocultar todos os Painéis" (Ctrl+Shift+F12) esconde todos e, acionada de novo, devolve exatamente os que estavam abertos, sem tornar o Perfil de interface "Personalizado".
_Evitar_: quadro, card, gadget, widget

**Painel flutuante** (_en_: `Floating`):
Painel em janela própria *dentro* da janela principal, sobre o tabuleiro. Ação: Flutuar.
_Evitar_: janela interna, popup

**Painel destacado** (_en_: `Detached`):
Painel, ou um grupo de abas inteiro, numa janela do sistema operacional separada da principal, que pode ir para outro monitor; dentro dela as abas se reorganizam como no dock. Ação: Destacar; Encaixar devolve ao dock.
_Evitar_: janela externa, pop-out

**Conjunto de Painéis** (_en_: `PanelSet`):
Grupo nomeado de Painéis que se ligam e desligam juntos pela aba Exibir (ex.: Análise = Análise + Engines + Aberturas), sem mudar Apresentação, Ribbon nem Auxílios. Diferente do Perfil de interface, que troca tudo.
_Evitar_: workspace, layout, preset

**Ribbon** (_en_: `Ribbon`):
Menu superior em abas com grupos de Ações, no estilo Office; pode estar no modo clássico, no modo simples (uma linha) ou recolhido. A aba Arquivo abre os bastidores: Partidas, Perfis e Opções.
_Evitar_: toolbar, barra de ferramentas, menu

**Opções** (_en_: `OptionsDialog`):
Diálogo com as preferências persistentes por categoria (Aparência, Tabuleiro, Auxílios, Engines, Controles…). Ações de uso imediato ficam no Ribbon, não nas Opções.
_Evitar_: configurações, settings, preferências (como nome do diálogo)

**Tema** (_en_: `Theme`):
Aparência da interface (claro, escuro ou segue o sistema), preferência do Perfil local; independente do Perfil de interface.
_Evitar_: skin, estilo

**Tela cheia** (_en_: `ViewState::fullscreen`):
Janela sem bordas ocupando o monitor inteiro (F11 ou item de menu); estado de interface, independente do modo Foco.
_Evitar_: maximizar

**Modo Foco** (_en_: `ViewState::focus`):
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

**Anotação de estudo** (_en_: `StudyNotes`):
Comentário livre do usuário sobre um Lance, fora de Partida competitiva; não altera a Partida e sai no PGN como comentário. Edita-se no Painel Anotações ou, como nota rápida, ao lado do Lance na Notação: é o mesmo texto nas duas vistas.
_Evitar_: comentário (sozinho), nota rápida (como dado separado)

**Súmula manual** (_en_: `ManualScoresheet`):
Registro escrito pelo próprio jogador como exercício de competição; diferente da notação automática. É um registro à parte, por perfil local e por Partida, que nunca altera a Partida nem o PGN; enquanto está em anotação a notação automática fica oculta.
_Evitar_: planilha, scoresheet

**Entrega da Súmula** (_en_: `deliver`):
Ato de encerrar a anotação da Súmula manual, como entregá-la ao árbitro: o texto fica imutável e a notação automática volta a ser exibida.
_Evitar_: salvar, finalizar
