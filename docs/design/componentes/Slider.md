# Slider

O slider ajusta um valor contínuo com efeito visível: Escala da interface, volume, tempo da Engine.

- O valor atual fica no rótulo ("Escala da interface · 125%"). As setas do teclado mudam um passo.
- Trilho em `border-strong` (3:1 sobre o Painel), parte preenchida em `accent`. Área de clique de 24px de altura; o foco põe o anel `focus-ring` no botão.
- **egui:** `egui::Slider` com `Visuals.selection.bg_fill = accent`, `trailing_fill = true`.
