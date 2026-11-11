# Evidência: autorizar o Lichess pela Central (#231)

Capturas geradas com o Cofre **falso** (cofre do sistema em memória e diretório temporário): nenhuma toca o cofre real nem o Lichess, e os nomes são fictícios. Em Escuro (`escuro/`) e Claro (`claro/`).

```
chess-studio --screenshot <pasta> --single <nome> --size 1600x1000 --backstage-page connections \
  --theme dark|light --connections-auth <cena> [--connections-scroll <px>]
chess-studio --screenshot <pasta> --single explorador-mestres --size 1920x1080 --panel \
  --theme dark|light --tabs Explorer --explorer-demo masters --moves e2e4,e7e5,g1f3
```

| Arquivo | Cena | O que mostra |
|---|---|---|
| `central-authorized.png` | `authorized` | Faixa "Segredos guardados no cofre do sistema", selo "Autorizado" e, nos Detalhes, a Conexão autorizada com "O que esta autorização permite". |
| `central-waiting.png` | `waiting` | Espera do navegador (fluxo real, navegador simulado): "Esperando você autorizar no navegador", prazo, "Abrir o navegador de novo" e "Cancelar"; "Autorizar…" fica parado. |
| `central-vault-create.png` | `vault-create` | Diálogo da senha-mestra ao criar o cofre: aviso de que não há recuperação, confirmação de entendimento, campos mascarados. |
| `central-vault-unlock.png` | `vault-unlock` | Cofre próprio bloqueado e o diálogo de desbloqueio. |
| `central-expired.png` | `expired` | "Autorização expirada" (o Lichess recusou) com "Autorizar de novo"; os dados locais ficam. |
| `central-locked.png` | `locked` | "Cofre bloqueado": a autorização existe, mas só volta com a senha-mestra. |
| `central-chave-pessoal.png` | `key-form --connections-scroll 120` | Fluxo avançado: campo mascarado da chave pessoal. |
| `explorador-mestres.png` | `--explorer-demo masters` | Painel Explorador com a fonte Mestres (dados fictícios; com a Conexão autorizada a tabela é a que o Lichess responde). |
| `explorador-filtros.png` | `--explorer-demo lichess-filters` | Fonte Lichess com os ritmos e as faixas de rating marcados. |

Outras cenas (`refused`, `timeout`, `wrong-key`, `engine`, `disconnect`, `vault-banner`, `vault-change`, `vault-working`) são desenhadas e conferidas pelos testes de `connections::view` (sem palavras proibidas, sem texto cortado).
