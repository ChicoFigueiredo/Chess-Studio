# Checkbox

A caixa de seleção serve para preferências em listas (Opções, Auxílios) que só valem depois de confirmar o diálogo ou que formam um conjunto.

- Ligada: caixa `accent` com visto em `on-accent`. Parcial: traço, para um grupo misto (ex.: "Todos os Auxílios").
- Contorno `border-strong` (3:1). O rótulo à direita é clicável; a área de clique tem 24px de altura.
- Desabilitada e ligada: caixa em `ink-disabled` com o visto em `surface-1`, para o estado continuar visível.
- Para ligar algo que vale na hora, prefira o Switch.
- **egui:** `ui.checkbox`, com `Visuals.widgets.inactive.bg_stroke = border-strong` e `selection.bg_fill = accent`.
