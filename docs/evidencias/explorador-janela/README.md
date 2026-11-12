# Evidência: Explorador, janela temporal, aviso de amostra e privacidade (#265)

Capturas com o binário de desenvolvimento, dados fictícios, nenhum pedido de rede, em `escuro/` e `claro/`. Todas foram abertas e conferidas.

```
chess-studio --screenshot <pasta> --single masters|off --size 1920x1080 --panel --theme dark|light --explorer-demo masters|off --tabs Explorer --moves e2e4,e7e5,g1f3
```

| Arquivo | O que mostra |
|---|---|
| `masters.png` | Rodapé com Origem, **Período** (o Lichess não informa as datas; o app não filtra por data) e o aviso de que a estatística é histórica e não é avaliação. |
| `off.png` | Estado "O Lichess está desligado" com o texto corrigido: vão a Posição, a chave da conexão autorizada e, em "Um jogador", o nome digitado. |
