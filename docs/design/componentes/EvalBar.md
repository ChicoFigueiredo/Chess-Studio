# EvalBar

A barra de avaliação é vertical, colada à esquerda do tabuleiro: a parte clara (`eval-white`) mostra a vantagem das brancas, a escura (`eval-black`) a das pretas. O valor fica embaixo, em `eval`.

- Só aparece com a Análise ligada e a Engine consentida (ADR-0003). Ela nunca decide nada da Partida.
- Mate aparece como "M3", com a barra cheia. A curva é logística, com saturação em ±8.
- Fica virada junto com o tabuleiro.
- No Tabuleiro 2D ao lado, o último Lance usa `last-move` e a casa selecionada um contorno de 3px em `board-select`. Barra e tabuleiro não mudam com o Tema.
- **egui:** `egui::Painter` num `SidePanel` estreito ou uma camada sobre o viewport 3D.
