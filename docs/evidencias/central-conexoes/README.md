# Evidência: Central de Conexões e estado da rede (#214)

- `escuro/` e `claro/`: Arquivo > Conexões em Escuro e em Claro (`--backstage-page connections --connections-demo --theme dark|light`). Mostra filtros por custo, busca, cartões por categoria (gratuitos primeiro), Switch "Ligado" com o texto do que é enviado, ações por método (Autorizar e Inserir chave desabilitados com a etapa), Conexões vinculadas com apelido e padrão, "Testar" e a linha do Explorador (gratuito com cadastro, requer Conexão autorizada do Lichess na etapa 3).
- `barra-offline/` (Escuro) e `barra-ligada/` (Claro, 2 Provedores ligados): indicador de rede na barra de status ("Rede: offline" e "Rede: 2 Provedores ligados"), com ícone e palavra.

- `dialogo-ligar/` (Escuro): o diálogo "Ligar Lichess?" aberto pelo Testar (`--consent-demo`), com o que será enviado.

Geradas com `chess-studio --screenshot <dir> --single 3d-profissional-padrao --size 1920x1080 [--panel] [--connections-demo]`. O modo `--connections-demo` liga Lichess e Chess.com e cria Conexões de exemplo só em memória.
