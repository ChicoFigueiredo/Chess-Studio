# Estado canônico, apresentação e arbitragem são autoridades separadas

A Partida tem **uma única autoridade de escrita**: o controlador da sessão, que só aplica um Lance depois de o núcleo de regras validá-lo e de a Política de arbitragem decidir a consequência. A entrada (mouse, teclado) produz apenas intenções; as Apresentações (3D profissional, Mesa imersiva, 2D técnico) leem a Posição canônica e desenham por cima a Posição visual provisória (arrasto, peça solta numa casa ilegal, animações), que nunca escreve na Partida. Legalidade, interface de movimentação e arbitragem são módulos distintos e testados em separado.

Escolhemos isso em vez de deixar o mundo gráfico (ECS do Bevy) ser o estado do jogo, que é o caminho natural num motor de jogos, porque a Simulação FIDE exige que uma peça fique visualmente numa casa ilegal sem que a Partida a aceite, e porque trocar de Apresentação, perder o foco ou cancelar um arrasto não pode criar lances fantasmas (INT-03, INT-04, VIS-05).

## Consequências

- O núcleo de regras, a arbitragem, o relógio e o treino não dependem do Bevy e são testáveis sem janela.
- O Bevy recebe o estado por leitura (eventos/recursos de saída) e devolve intenções; nenhum sistema do Bevy muta a Partida diretamente.
