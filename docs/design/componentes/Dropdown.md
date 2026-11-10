# Dropdown

O Dropdown escolhe um valor numa lista longa ou que muda pouco: Qualidade, Tema, Escala, Engine.

- Fechado: campo `surface-2` com seta. Aberto: menu `surface-1`, `shadow-flyout`, `radius-l`; o item escolhido fica em `accent-subtle` com visto.
- O último item pode levar a Opções. Os atalhos aparecem à direita, em `ink-muted`.
- **egui:** `egui::ComboBox` com `Visuals.window_*` e `menu_corner_radius` dos tokens.
