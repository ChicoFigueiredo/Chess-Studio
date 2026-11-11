# Switch

O Switch liga uma preferência que vale imediatamente, sem confirmar, como coordenadas nas bordas e realce ao passar o mouse.

- Ligado: trilho `accent`, botão `on-accent`. Desligado: trilho com contorno `border-strong` e botão `ink-muted`.
- Desabilitado e ligado: trilho `ink-disabled` com o botão à direita; a posição do botão mostra o estado, não a cor.
- O rótulo descreve o estado ligado ("Mostrar coordenadas"), nunca uma negação.
- **egui:** widget próprio (`toggle_switch`, como na demo do egui) pintado com os tokens.
