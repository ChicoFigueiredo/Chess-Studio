# ToggleButton

Botão de alternância liga ou desliga uma opção de efeito imediato na tela, como coordenadas, realce ou Apresentação.

- Ligado: fundo `accent-subtle`, contorno `accent`, texto e ícone em `accent-ink`. Desligado: igual ao botão secundário.
- Ligado e desabilitado: fundo `surface-4`, contorno `border-strong` e texto `ink-disabled`, sem `accent`.
- Use em grupos mutuamente exclusivos (Apresentação: 3D, Mesa, 2D) ou alternâncias isoladas na aba Exibir.
- O estado vem da Ação (`checked`), nunca do widget: a mesma Ação no Ribbon, na Paleta e no atalho mostra o mesmo estado.
- **egui:** `egui::Button::selectable` ou `ui.toggle_value`, com `Visuals.selection.bg_fill = accent-subtle`.
