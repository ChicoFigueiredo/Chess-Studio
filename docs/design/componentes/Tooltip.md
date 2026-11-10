# Tooltip

O tooltip mostra nome, descrição curta e atalho de uma Ação ao parar o mouse (500 ms) ou ao focar pelo teclado.

- Título em `body-strong`, descrição em `body`, atalho em `caption`/`ink-muted`.
- Ação desabilitada: o tooltip diz por quê ("Só no modo Assistido").
- **egui:** `Response::on_hover_ui` com um layout fixo de três linhas; o texto vem da Ação.
