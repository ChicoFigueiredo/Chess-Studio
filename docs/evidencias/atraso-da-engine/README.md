# Atraso de resposta da Engine (#335)

Opções > Engines ganha o controle **Atraso de resposta da Engine** (0 a 10 s, passo de 1 s; 0 mostra "sem atraso"). A Engine segura o Lance até esse tempo, só para parecer que pensa; o status segue "pensando". Cada Lance varia de 70% a 130% do valor (determinístico, sem sorteio) e, com relógio, o atraso nunca passa de um quarto do tempo restante da Engine.

Geradas com `chess-studio --screenshot <dir> --options --options-category engines --size 1280x720`.

- `2d-tecnico.png`: o diálogo sobre a Apresentação 2D técnico.
- `3d-profissional-padrao.png`: o mesmo sobre o 3D profissional.

**UNVERIFIED:** o atraso com uma Engine de verdade na janela (testado só com a Engine falsa e relógio injetado) e a sensação da variação de ±30%.
