# Evidência: PlayStrategy na Central e na importação online (#253)

Capturas com o binário de desenvolvimento, dados fictícios, nenhum pedido de rede. Todas foram abertas e conferidas, em `escuro/` e `claro/`.

```
chess-studio --screenshot <pasta> --single central --size 1920x1080 --backstage-page connections --connections-demo \
  --theme dark|light --connections-open playstrategy
chess-studio --screenshot <pasta> --single importacao --size 1920x1080 --panel --theme dark|light --tabs Library \
  --explorer-demo import-playstrategy|import-playstrategy-done
```

| Pasta | O que mostra |
|---|---|
| `central` | Cartão do PlayStrategy completo (Switch, Testar, Detalhes): "O que é enviado" diz que nesta versão não há Partida online; "O que vem depois" lista Jogar online e Engine online; sem "Requisito" de autorização. |
| `importacao-andamento` | Biblioteca > Importar > PlayStrategy (online): aviso de só xadrez padrão e de que não há Partida online, andamento (Recebidas, Novas, Repetidas) e Cancelar. |
| `importacao-concluida` | Resumo com a linha "41 Partidas de variantes do PlayStrategy foram puladas". |

`--explorer-demo import-playstrategy*` só existe com `--screenshot`.
