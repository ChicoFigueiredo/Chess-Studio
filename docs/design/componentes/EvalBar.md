# EvalBar

A barra de avaliação é vertical, colada à esquerda do tabuleiro: a parte clara (`eval-white`) mostra a vantagem das brancas, a escura (`eval-black`) a das pretas. O valor fica embaixo, em `eval`.

- Só aparece com a Análise ligada e a Engine consentida (ADR-0003). Ela nunca decide nada da Partida.
- Mate aparece como "M3", com a barra cheia. A curva é logística, com saturação em ±8.
- Fica virada junto com o tabuleiro.
- **egui:** `egui::Painter` num `SidePanel` estreito ou uma camada sobre o viewport 3D.
