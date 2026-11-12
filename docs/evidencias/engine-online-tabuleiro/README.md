# Evidência: a Partida da Engine online no tabuleiro, só para ver (#302)

Capturas nos Temas Escuro (`escuro/`) e Claro (`claro/`), geradas pelo modo `--screenshot` do app, nos tamanhos 1600x900 e 1280x720 (o sufixo diz qual). As cenas usam dados **fictícios** (a conta `Motor_Demo`, os apelidos `TorreDeMadeira` e `Coruja_Azul`, a Partida `Hq3vT8aZ`): nenhuma conexão, nenhum banco. As capturas saíram pela GPU da máquina, com a qualidade padrão (sem renderização em software).

## Cenas

- `playing`: a tela da Engine online jogando duas Partidas. **Partidas em andamento** vem logo abaixo do estado (antes dos limites, que não se editam com a Engine ligada) e cada Partida tem **Ver no tabuleiro**.
- `watch-confirm`: a sua Partida local está em andamento (com relógio) e você pediu **Ver no tabuleiro**: a pergunta **Ver a Partida da Engine?** diz que a sua Partida fica guardada como está, volta ao tabuleiro ao sair e não é encerrada, e que o relógio dela fica em Pausa. **Ver a Partida da Engine** ou **Cancelar**.
- `spectator`: o modo espectador no 3D. No alto da área do tabuleiro, os selos **Engine online** e **Modo espectador** e **Sair do modo espectador** (sem espaço ao lado do tabuleiro, numa linha só, acima dele). O Painel **Partida online** vem à frente com os mesmos selos, os relógios do Lichess (quem joga com contorno), **A Engine joga de Brancas contra TorreDeMadeira (1720) no Lichess.**, o aviso de só leitura e **Sair do modo espectador**; nenhuma Ação de jogo. No Ribbon, **Abandonar**, **Propor empate**, **Desfazer**, **Refazer**, **Reclamar empate**, **Pausar relógio** e **Acionar o relógio** ficam desabilitados. A Notação e o relógio da mesa seguem a Partida vista.
- `spectator-2d`: o mesmo no 2D técnico. Com espaço ao lado do tabuleiro, a faixa vira um cartão ali (quem joga e o aviso de só leitura), sem cobrir nenhuma casa; a 1280x720 não há espaço e ela fica numa linha só, acima do tabuleiro.
- `spectator-ended`: a Partida vista terminou: a faixa e o Painel dizem **Partida terminada: 1-0.**, o rei caído na mesa, sem o diálogo de fim (que é da sua Partida).

## Como gerar de novo

```sh
cargo run -p chess-studio -- --screenshot <dir> --size 1600x900 \
  --engine-online-tour ready,playing,watch-confirm,spectator,spectator-2d,spectator-ended
cargo run -p chess-studio -- --screenshot <dir> --size 1280x720 \
  --engine-online-tour ready,playing,watch-confirm,spectator,spectator-2d,spectator-ended
# A primeira cena (`ready`) só aquece a cena 3D e não entra aqui.
# Sem memória de GPU: VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json e --quality economico.
```
