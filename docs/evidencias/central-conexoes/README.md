# Evidência: Central de Conexões (#214) e seu acabamento (#225)

Capturas geradas com `chess-studio --screenshot <dir> --single central --size 1920x1080 --backstage-page connections --connections-demo [--theme <id>] [--density compact] [--connections-open lichess,fide] [--connections-scroll <pt>]`. `--connections-demo` liga Lichess e Chess.com e cria Conexões de exemplo só em memória, com nomes fictícios (`ana_exemplo`, `bruno_exemplo`, `carla_exemplo`, ID `9999991`) e um teste feito "há 3 min". `--connections-open` abre os Detalhes dos cartões; `--connections-scroll` rola a página.

## Acabamento (#225)

- `escuro-resumido/` (Escuro): a página no topo, com o cartão do Lichess **resumido** (nome, estado "Ligado · nome vinculado", "O que você pode fazer agora", Switch e a ação principal "Testar"), os cartões menores de "Em breve" e "Requer aprovação" e o início de "Dados e consultas".
- `escuro-detalhes/` (Escuro): o mesmo cartão com **Detalhes** abertos: o que é enviado, como se identifica, custo por serviço (sem cadastro e com cadastro, sem contradição), o que vem depois, última consulta, instruções e termos e as **Conexões vinculadas** (apelido, selo "Padrão", nome no Provedor, resultado do teste "há 3 min", Tornar padrão e o menu "⋯" com Renomear apelido e Remover).
- `claro-detalhes/` e `claro-resumido/` (Claro): as mesmas telas no Tema Claro; a segunda mostra Chess.com resumido e as faixas "Em breve" de Dados e de Engines.
- `alto-contraste-pretas/` (Pretas, 7:1): Chess.com com Detalhes abertos.
- `alto-contraste-brancas-compacta/` (Brancas, Densidade Compacta): Importação de arquivos e FIDE aberto. As letras sobre o título (A, I, N…) são KeyTips que o modo de captura deixa aparecer neste Tema; não fazem parte da Central.
- `uma-coluna/` (Escuro, Compacta, janela de 1100 pt): uma coluna abaixo de 980 pt, com o cartão aberto e sem texto cortado.
- `importacao-e-ratings/` (Escuro): "Importação de arquivos" e "Ratings", com Provedores só de arquivo ("Importação manual", ação principal "Abrir instruções" ou "Vincular nome") e FIDE vinculando um ID.
- `dialogo-ligar/` (Escuro, `--panel --consent-demo`): o diálogo "Ligar Lichess?" com o texto de usuário do que será enviado e a lista curta de endereços.
- `galeria/` (`--gallery --gallery-scroll 1200`): os componentes novos (selos de estado, cartão completo e discreto, "Detalhes", menu de ações) nos dois Temas.

## Estado da rede na barra de status (#214)

- `barra-offline/` (Escuro) e `barra-ligada/` (Claro, 2 Provedores ligados): indicador de rede na barra de status ("Rede: offline" e "Rede: 2 Provedores ligados"), com ícone e palavra. Geradas com `--single 3d-profissional-padrao --panel [--connections-demo]`.
