# Slider

O slider ajusta um valor contínuo com efeito visível: Escala da interface, volume, tempo da Engine.

- O valor atual fica no rótulo ("Escala da interface · 125%"). As setas do teclado mudam um passo.
- **egui:** `egui::Slider` com `Visuals.selection.bg_fill = accent`, `trailing_fill = true`.
- O trilho usa `control-track` (3:1 sobre as superfícies), não `surface-4`; a parte preenchida usa `accent`.
