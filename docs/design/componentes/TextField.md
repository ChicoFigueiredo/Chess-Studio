# TextField

O campo de texto recebe nomes (Perfil, Partida), FEN e busca.

- O foco usa o anel `focus-ring` e o contorno `accent`. Erro: contorno `danger` de 2px e mensagem logo abaixo, com ícone, que diz o que fazer.
- Placeholder em `ink-muted` com um exemplo real, nunca como rótulo.
- **egui:** `egui::TextEdit::singleline` com `Visuals.extreme_bg_color = surface-2`.
