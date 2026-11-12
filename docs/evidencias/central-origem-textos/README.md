# Evidência: filtros da Central, licença e Origem, textos do GLOSSARY (#266, #264, #263)

Capturas com dados **fictícios**, nenhum pedido de rede, em Escuro (`escuro/`) e Claro (`claro/`). Geradas com o modo `--screenshot` do app (renderização por software, `--quality economico`); todas foram abertas e conferidas.

```
chess-studio --screenshot <pasta> --single <nome> --size 1600x1000 --quality economico --theme dark|light \
  --backstage-page connections --connections-demo \
  [--connections-filter signup|manual|free|paid] [--connections-state-filter off|on|authorized|manual|soon] \
  [--connections-open lichess|fide --connections-scroll <px>]
chess-studio --screenshot <pasta> --size 1600x1000 --theme dark|light --engine-online-tour remote-active
```

`--connections-filter` e `--connections-state-filter` só existem para capturas.

| Arquivo | O que mostra |
|---|---|
| `filtros-com-cadastro.png` | Filtro de custo **Com cadastro** (US-CRED-01) ativo, com a contagem em cada botão, e a nova linha **Estado**. |
| `filtros-manual.png` | Filtro **Manual**: só os Provedores sem API (importação de arquivo). |
| `filtros-estado-desligados.png` | Filtro de estado **Desligados** (os outros: Ligados, Com autorização, Importação manual, Em breve). |
| `licenca-detalhes.png` | Detalhes do cartão do Lichess com a linha **Licença dos dados** (licença e, em cinza, as restrições). |
| `rating-fide.png` | Cartão da FIDE (rolado até o fim): "FIDE, via Lichess, consultado …" e a licença guardada ("Dados da FIDE reproduzidos pelo Lichess; os direitos são da FIDE…"). |
| `engine-remota.png` | Painel Análise com a Engine remota: selo, aviso, **Origem** (Provedor, instante e termos) e sem Threads, Hash, Carga nem o aviso da segunda Engine. |

Os textos trocados (#263: "conta" por "usuário BOT" e "Conexão de Engine", "chaveiro" por "cofre do sistema", erro de download sem código HTTP) são cobertos pelo teste `integration_texts_avoid_the_glossary_banned_words_and_http_codes`, que lê os `.ftl` das integrações.
