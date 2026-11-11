# Evidência: acabamento da etapa 5 (#255)

Capturas com dados fictícios, nenhum pedido de rede e nenhum diálogo do sistema aberto. Em Escuro (`escuro/`) e Claro (`claro/`). Todas foram abertas e conferidas.

```
chess-studio --screenshot <pasta> --single consentimento --size 1600x1000 --panel --consent-demo --theme dark|light
chess-studio --screenshot <pasta> --single central-lichess --size 1600x1000 --backstage-page connections --connections-demo --connections-open lichess --theme dark|light
chess-studio --screenshot <pasta> --single fim-de-partida --size 1600x1000 --moves f2f3,e7e5,g2g4,d8h4 --theme dark|light
```

| Arquivo | O que mostra |
|---|---|
| `consentimento.png` | "Ligar Lichess?": uma frase por uso real (Explorador, avaliação em cache, finais, consultas, **download de problemas**, Partidas online) e a nota final. |
| `central-lichess.png` | Cartão do Lichess com Detalhes: o mesmo texto em "O que é enviado", Baixar e Jogar já aparecem como disponíveis, só "Engine online" fica em "O que vem depois". |
| `fim-de-partida.png` | Diálogo de fim de Partida com **Exportar PGN** (Documentos) e o novo **Salvar como…**. |

A janela de escolha do sistema não aparece nas capturas (modo de captura não abre diálogo real); o fluxo é coberto por testes com seletor falso e pelo item CX-HOM-43.
