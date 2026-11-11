# Rede offline por padrão; integrações por Provedor e Conexão, sem scraping

O Chess Studio passa a falar com serviços externos de xadrez (Lichess, Chess.com, FIDE e outros), mas continua **offline por padrão**: tudo o que existia antes funciona sem internet, e nenhuma chamada de rede acontece para um **Provedor** que o usuário não ligou. Cada Provedor nasce desligado; liga-se na Central de Conexões ou quando um recurso precisa dele pela primeira vez, com o aviso do que será enviado. O vínculo de um Perfil local com um Provedor é uma **Conexão** (autorizada, por nome público ou só de importação). Todo dado que vem de fora guarda a sua Origem: Provedor, instante da consulta e licença.

Só entram integrações por API ou exportação documentada. Onde não há API oficial (Queen Alice, ChessBase, parte do Chess.com), o app oferece identidade pública e importação manual de PGN e **nunca pede senha nem automatiza o site**. Segredos (tokens, chaves) ficam no cofre do sistema operacional; sem cofre, num cofre próprio cifrado com senha-mestra pedida só quando um segredo é necessário. Nunca em texto puro, em log ou em exportação.

Engines remotas são escolhidas explicitamente, como qualquer Engine, com consentimento por Provedor: não há troca silenciosa da Engine local por uma remota, porque a posição analisada sai do computador. O modo padrão tem custo zero; Provedor pago só com consentimento expresso.

Alternativas rejeitadas: rede ligada de fábrica para consultas públicas (o app falaria com servidores sem o usuário pedir, contra a proposta de mesa de treino offline); um interruptor global Online/Offline (esconde para onde os dados vão); consentimento por recurso (um recurso pode usar vários Provedores); cadeia automática de Engines local, cache e remota (posições sairiam sem o usuário perceber); segredo só em memória quando não há cofre do sistema (obrigaria a reautorizar a cada abertura).

## Consequências

- A frase "o app usa a rede apenas no download da Engine" (ADR-0003) deixa de valer; a regra passa a ser esta ADR. O download fixado da Engine não muda.
- A camada de rede é única, por Provedor: limites de taxa, `Retry-After`, cache com Origem e cancelamento; a interface nunca guarda segredo.
- Uma falha de rede, um 429 ou uma autorização revogada nunca corrompe uma Partida nem trava o uso offline.
- Dependências novas (cofre do SO, cifra do cofre próprio) seguem a ADR-0004: nada GPL/AGPL/LGPL.
- A especificação de origem está em `docs/integracoes/`; as escolhas, em `docs/decisoes/2026-11-11-conexoes.md`.
