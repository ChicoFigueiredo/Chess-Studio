# Evidência: Modo de estudo, tonalidade e faixa (#324)

Capturas do tabuleiro em Modo de estudo (Partida 1.e4 e5 2.Cf3, Revisão no meio-lance 2, Linha de estudo com 2.d4 jogado na Linha). Geradas com `--screenshot` e a opção de roteiro nova `--study [lances]`, que liga o Modo de estudo na posição mostrada e joga os Lances pelo mesmo gesto do usuário:

```
chess-studio --screenshot <dir> --panel --moves e2e4,e7e5,g1f3 --review 2 --study d2d4 --size 1280x720
chess-studio --screenshot <dir> --panel --theme light --moves e2e4,e7e5,g1f3 --review 2 --study d2d4 --size 800x600 --single estudo-claro-800x600 --tabs Board2d
```

| Arquivo | O que mostra |
| --- | --- |
| `estudo-3d-profissional.png` | 3D profissional, Tema Escuro: tonalidade sobre as casas e a moldura, faixa "Estudo" com "Voltar à Partida". |
| `estudo-mesa-imersiva.png` | Mesa imersiva, o mesmo. |
| `estudo-2d-tecnico.png` | 2D técnico: casas tingidas e uma moldura fina tingida. |
| `estudo-painel-tabuleiro-2d.png` | O painel Tabuleiro 2D (miniatura no dock) acompanha a tonalidade. |
| `estudo-claro-800x600.png` | Tema Claro em 800x600: a faixa cabe sem corte (a frase de apoio some e ficam o título e o botão). |

O que não está aqui: a faixa com o aviso "Lance novo na Partida" (precisa da Engine adversária jogando durante o estudo; coberta só por teste de unidade) e as interações (clique na faixa, Esc).
