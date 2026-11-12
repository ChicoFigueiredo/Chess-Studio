# Evidência: prévia da importação de PGN, fonte detectada e confirmação de direitos (#270)

Capturas com o binário de desenvolvimento, dados fictícios e nenhum arquivo lido (as cenas montam a prévia pronta), em `escuro/` e `claro/`. Todas foram abertas e conferidas.

```
chess-studio --screenshot <pasta> --single <cena> --size 1600x900 --panel --theme dark|light --tabs Library --explorer-demo <cena>
```

| Cena | O que mostra |
|---|---|
| `import-file` | Prévia de um arquivo do Lichess: "Cerca de 2140 Partidas", cabeçalhos das primeiras, "As tags indicam Lichess" e a fonte já sugerida. |
| `import-file-twic` | Fonte TWIC (uso restrito): aviso e caixa "Confirmo que posso usar estas Partidas"; Importar desligado até marcar. |
| `import-file-twic-done` | Depois de confirmar e importar: o resumo traz "1842 Partidas com licença restrita (TWIC)". |
