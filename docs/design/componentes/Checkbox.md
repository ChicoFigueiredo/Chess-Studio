# Checkbox

A caixa de seleção serve para preferências em listas (Opções, Auxílios) que só valem depois de confirmar o diálogo ou que formam um conjunto.

- Ligada: caixa `accent` com visto em `on-accent`. Parcial: traço, para um grupo misto (ex.: "Todos os Auxílios").
- Contorno `border-strong` (3:1). O rótulo à direita é clicável.
- Para ligar algo que vale na hora, prefira o Switch.
- **egui:** `ui.checkbox`, com `Visuals.widgets.inactive.bg_stroke = border-strong` e `selection.bg_fill = accent`.
