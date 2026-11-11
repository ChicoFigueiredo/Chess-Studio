# Evidência: acabamento das integrações (#239)

Capturas geradas com o binário de desenvolvimento, dados fictícios, nenhum pedido de rede. Todas foram abertas e conferidas.

```
chess-studio --screenshot <pasta> --single central --size 1920x1080 --backstage-page connections --connections-demo \
  --theme dark|light|pretas [--connections-open lichess] [--connections-state limited|failing]
chess-studio --screenshot <pasta> --single explorador --size 1920x1080 --panel --tabs Explorer --theme dark|light \
  --explorer-demo masters|reference-opening|reference-failed|limited|library --moves e2e4,e7e5,g1f3
```

`--connections-state` e `--explorer-demo reference-*` só existem para capturas.

| Pasta | O que mostra |
|---|---|
| `central-escuro-detalhes`, `central-claro-detalhes`, `central-pretas-alto-contraste` | Um cartão completo ocupa a largura toda da seção; "Custo" e "O que vem depois" viram listas curtas separadas por pontos; "Testado às HH:MM" na hora local. |
| `central-escuro-limitado` | Estado Limitado: selo "Limitado · volta em 4 min" e a linha "Em pausa a pedido do Provedor". |
| `central-claro-falha` | Estado Com falha: selo e linha "Falhou: o Provedor não respondeu no prazo". |
| `explorador-escuro-referencias`, `explorador-claro-referencias` | Nome da abertura em português ("Abertura do Cavalo do Rei"), rodapé "consultado às HH:MM". |
| `explorador-escuro-abrindo`, `explorador-claro-falha` | Clique numa Partida de referência: "Abrindo a Partida…" e a falha em palavras de usuário. |
| `explorador-escuro-pausa` | "Pausa até HH:MM" na hora local. |
| `explorador-escuro-biblioteca` | Fonte Biblioteca com o nome do livro local. |
| `kt-repro-brancas-compacta`, `kt-com-flag-keytips` | Tema Brancas, Compacta, com e sem `--keytips`: nenhuma letra de KeyTip sobre a Central. |

Não capturados: o menu "⋯" aberto e o foco por teclado na Central. Os ganchos de captura que tentei não surtiram efeito no modo de captura (o popup do egui e o anel de foco dependem de entrada real); ficam sem evidência de imagem.
