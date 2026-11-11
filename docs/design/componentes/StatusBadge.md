# StatusBadge (selo de estado)

O selo diz em que estado algo está, com **ícone e palavra**: a cor sozinha nunca basta. Serve ao cartão de Provedor ("Ligado · nome vinculado", "Com falha"), à marca "Padrão" de uma Conexão e a qualquer estado curto de uma lista.

- Altura `control-h-compact` (24), cantos de pílula (um dos chips permitidos), texto `caption` em semibold e ícone `icon-s` de 14.
- O texto é sempre `ink`. O tom só pinta o ícone: neutro `ink-muted`, sucesso `success`, informação `info`, atenção `warning`, falha `danger`. Contorno `border-strong`, sem fundo; vale sobre `surface-1` e `surface-2`, onde essas cores cumprem 4,5:1 em todos os Temas.
- O tom `Accent` (fundo `accent-subtle`, texto e ícone `accent-ink`) é só para a marca de identidade "Padrão".
- Não é clicável e não tem foco. Se precisar explicar, o chamador põe o tooltip (`on_hover_text`).
- A palavra vem do estado, não da cor: "Desligado", "Ligado", "Limitado · volta em 3 min", "Com falha", "Importação manual", "Em breve", "Requer aprovação".
- **egui:** `cs_status_badge(ui, StatusTone, icon, label)` em `ui_theme`.
