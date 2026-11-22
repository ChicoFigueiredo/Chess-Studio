# Evidência: Notação com a Linha de estudo (#323)

Capturas do Painel Notação em Modo de estudo, nos Temas Escuro e Claro, em 1280x720:
a Partida 1.e4 e5 2.Nf3, revista após 1.e4 e5, com a Linha de estudo 2.d4 d5 3.Nc3 (cursor no último Lance).

Geradas com:

```
chess-studio --screenshot <dir> --moves e2e4,e7e5,g1f3 --review 2 --study d2d4,d7d5,b1c3 \
  --panel --theme <dark|light> --dock-demo tallnotation --tabs Notation \
  --single linha-de-estudo-<tema> --size 1280x720
```

(`--study` e `--dock-demo tallnotation` são flags de captura novas deste ticket: a primeira move as peças por gestos e inicia a linha; a segunda põe a Notação no grupo alto do dock.)

O que as capturas mostram:

- os quatro botões no topo (próximo desabilitado: o cursor está no fim da linha; o botão de posição atual segue habilitado, porque sai do estudo);
- a lista "Linha de estudo" empilhada, em Brancas | Pretas, numerada a partir do Lance de partida (2.), com o Lance atual (Nc3) destacado e a mesma tipografia da Partida.

Limites: o painel nesta captura tem cerca de 340 px, não os 230 px do caso estreito (a largura de 230 px só é coberta pelo teste de desenho, sem imagem); a rolagem automática leva a vista ao Lance atual da linha, então a lista da Partida (acima, congelada no Lance 2) fica fora do quadro.
