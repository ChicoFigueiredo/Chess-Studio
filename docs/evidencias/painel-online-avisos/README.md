# Evidência: Painel da Partida online no dock padrão, avisos de desafio e de correspondência (#272)

Capturas nos Temas Escuro (`escuro/`) e Claro (`claro/`), geradas pelo modo `--screenshot` do app, com o **dock padrão** (o Painel vai sozinho para o grupo do Relógio, PO-22). As cenas usam dados **fictícios** (apelidos inventados, o visitante `GuestDEMO`, a Partida 142 do FICS): nenhuma conexão, nenhum banco.

## O Painel Partida online sem rolar (1600x900 e 1280x720)

Cada cena existe nos dois tamanhos: `*-1600.png` (1600x900) e `*-1280.png` (1280x720).

- `playing`: Lichess, em andamento. Os relógios do Provedor (o do adversário em cima, o seu embaixo, quem joga com contorno), **Enviando Ba4…**, **Propor empate** e **Abandonar**; abaixo, os selos **Lichess** e **Ao vivo**.
- `draw`: o adversário propôs empate: a linha da vez diz **Coruja_Azul propõe empate** e as Ações são **Aceitar**, **Recusar** e **Abandonar**.
- `fics-playing`: o mesmo Painel no FICS como visitante (**FICS · visitante**, sem rating).
- `fics-dropped`: a conexão caiu durante a Partida: **A conexão com o FICS caiu** e **Encerrar sem resultado**; abaixo, o aviso de que o FICS dá a Partida de visitante como perdida, de que a análise dela só volta ao encerrar, e **Descartar a sessão** (só fecha a conexão).
- `fics-unconfirmed`: a mesma Partida sem sessão (descartada ou reaberta da Biblioteca): **Sem resultado confirmado** e **Encerrar sem resultado**.
- `fics-closed`: depois de **Encerrar sem resultado**: "Encerrada no app, sem resultado…", **Analisar** e **Ver na Biblioteca**.

## Avisos e telas

- `fics-toast`: o aviso discreto de desafio do FICS no canto do tabuleiro (**TorreBranca (1480) desafia você no FICS**, blitz 3+2 · sem rating, **Ver** e **Fechar**).
- `fics-challenges`: aba **Desafios** do FICS com **Desafiar pelo nome** (nome, ritmo, **Desafiar**) acima dos desafios recebidos.
- `fics-challenge-sent`: o desafio por nome enviado (**Desafio enviado a TorreBranca · 3+2 · sem rating**, **Retirar desafio**).
- `list`: **Partidas em andamento** depois de Atualizar: a correspondência em que o adversário jogou vem primeiro, com contorno em destaque e o selo **Lance novo**.

## Como gerar de novo

```sh
cargo run -p chess-studio -- --screenshot <dir> --size 1600x900 \
  --online-tour playing,draw,fics-playing,fics-dropped,fics-unconfirmed,fics-closed,fics-toast,fics-challenges,fics-challenge-sent,list
cargo run -p chess-studio -- --screenshot <dir> --size 1280x720 \
  --online-tour playing,draw,fics-playing,fics-dropped,fics-unconfirmed,fics-closed
```
