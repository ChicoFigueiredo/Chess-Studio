# OptionsDialog

O diálogo Opções reúne as preferências persistentes por categoria: Aparência, Tabuleiro, Auxílios, Engines, Controles e Perfil local. É o destino de tudo o que hoje fica no painel esquerdo e não é Ação.

- Cada linha tem rótulo, explicação curta em `ink-muted` e controle à direita.
- As mudanças valem na hora. "Restaurar padrões" pede confirmação.
- **egui:** `egui::Modal` com `SidePanel` de categorias e `ScrollArea`.
