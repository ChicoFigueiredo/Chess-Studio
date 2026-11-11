# Evidência: Painel Explorador, avaliação em cache e importação online (#226)

Capturas geradas com `chess-studio --screenshot <pasta> --single <nome> --size 1920x1080 --panel --theme dark|light --explorer-demo <cena> [--tabs <Painel>] [--moves ...]`. Os dados das cenas são fictícios (jogadores inventados) e nenhum pedido sai: `--explorer-demo` só existe com `--screenshot`.

## Os quatro pedidos do ticket, em Escuro (`escuro/`) e em Claro (`claro/`)

| Arquivo | Cena | O que mostra |
|---|---|---|
| `aberturas-biblioteca.png` | `library --tabs Explorer --moves e2e4,e7e5,g1f3` | Aba Aberturas com a fonte padrão **Minha Biblioteca** (offline): abertura e ECO do livro, barra-resumo, legenda, tabela Lance, Partidas, barra de resultado e a Origem no rodapé ("sua Biblioteca, sem internet"). |
| `finais-torre-e-peao.png` | `endgame --tabs Explorer` | Aba Finais num final de torre e peão contra torre (5 peças): veredito em palavras ("Vitória das Brancas"), mate em N Lances e DTZ, Lances do melhor ao pior com a nuance da regra dos 50 Lances ("Empate (50 Lances)") e a Origem. |
| `analise-em-cache.png` | `cloud --tabs Analysis --moves e2e4,e7e5,g1f3,b8c6` | Painel Análise com a linha separada **Em cache (Lichess)**: valor do ponto de vista das Brancas, profundidade, linha principal em SAN e a Origem. A Engine local continua à parte ("Análise desligada"). |
| `importacao-em-andamento.png` | `import --tabs Library` | Biblioteca > Importar > Lichess (online): andamento com Recebidas, Novas e Repetidas, Cancelar, e a lista já mostrando Partidas importadas com a Origem. |

## Estados do Painel (`escuro/estados/`, recorte do dock)

`aberturas-mestres` (como a tabela ficará com a Conexão autorizada: rating médio e Partidas de referência), `requer-autorizar-lichess` (o estado real de hoje para Mestres, Lichess e Um jogador, com atalho para a Central), `lichess-desligado` e `finais-lichess-desligado` ("Ligar Lichess" abre o diálogo de consentimento), `pausa-do-servico`, `sem-internet-com-cache` (cache com a idade), `mais-de-7-pecas` (sem pedido), `durante-a-partida` (Trava de fair play, `--policy fide`), `analise-lichess-desligado` e `importacao-concluida` (resumo e botão Atualizar).

## Como reproduzir

```
chess-studio --screenshot /tmp/ev --single aberturas-biblioteca --size 1920x1080 --panel --theme dark \
  --tabs Explorer --explorer-demo library --moves e2e4,e7e5,g1f3
```

Cenas: `library`, `masters`, `signin`, `off`, `limited`, `stale`, `nodata`, `locked`, `endgame`, `endgame-off`, `toomany`, `cloud`, `cloud-off`, `import`, `import-done`.
