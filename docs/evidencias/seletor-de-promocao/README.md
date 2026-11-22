# Seletor de promoção (#336)

Janelinha centralizada com as quatro peças desenhadas (no lado de quem promove), a **Dama já selecionada**, o nome e a letra embaixo de cada uma.

- **Enter** confirma a selecionada; **← →** mudam a seleção (em círculo); **D, T, B, C** escolhem direto; o **clique** continua valendo.
- Esc segue com o intérprete de gestos (retrata o Lance, DEC-40); "Sempre promover a dama" e os avisos competitivos ficam. Enter nas Políticas competitivas é escolha explícita, não dama automática.
- As teclas não agem com campo de texto em foco.

Geradas com `chess-studio --screenshot <dir> --moves h2h4,g7g5,h4g5,h7h6,g5h6,a7a6,h6h7,a6a5,h7g8 --size 1280x720` (brancas; pretas com `a2a3,b7b5,a3a4,b5b4,a4a5,b4b3,h2h3,b3c2,h3h4,c2d1`; Tema claro com `--theme light`).

- `2d-tecnico.png`, `3d-profissional-padrao.png`: brancas, Tema escuro.
- `2d-tecnico-pretas.png`: pretas (peça escura com contorno claro).
- `2d-tecnico-claro.png`: Tema claro.

**UNVERIFIED:** as teclas com a janela real (o teste cobre a lógica de seleção, não o egui lendo o teclado); Tema de alto contraste; o seletor dentro do Tabuleiro 2D do painel.
