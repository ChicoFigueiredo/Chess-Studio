# Decisões: rating FIDE, dados guardados e Partidas importadas por Origem (11/11/2026)

Issue [#242](https://github.com/ChicoFigueiredo/Chess-Studio/issues/242) (CX-19), parte do épico #211. Fecha as lacunas apontadas ao escrever o [guia](../integracoes/guia.md) (#240). Continua as [decisões de Provedores e Conexões](2026-11-11-conexoes.md) (CX-04, CX-09, CX-14) e a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md).

| # | Decisão | Escolha |
|---|---|---|
| FD-01 | De onde vem o rating | Do **Lichess**, que reproduz a FIDE. O app diz "FIDE, via Lichess, consultado ...". Nada vai para a FIDE. |
| FD-02 | Três ritmos | Standard, Rápido e Blitz são caixas separadas, em ordem fixa. Um ritmo sem rating mostra "—" e nunca herda o valor de outro. Nada é somado, comparado ou ordenado junto com ratings online. |
| FD-03 | Data | A resposta do Lichess **não traz o mês da lista da FIDE**. A data guardada e mostrada é a da **consulta**, e a tela avisa isso. Até 24 h mostra "há N min/h"; depois, a data (UTC) com "há N dias", porque o app não lê o fuso do sistema. |
| FD-04 | Guardar | Último valor por Perfil local **e por ID FIDE** (migração 17, tabela `fide_ratings`). Aparece sem rede e com o Lichess desligado. Trocar o ID vinculado não apaga o valor do anterior. |
| FD-05 | Quando consulta | Só com o Lichess ligado; desligado, o diálogo "Ligar Lichess?" pergunta antes e "Agora não" não envia nada. Não é bloqueada pela Trava de fair play: o rating não calcula nada. |
| FD-06 | Atualizar | Sempre vai ao Lichess, mesmo com o cache de 24 h ainda válido (`Request::refresh`; pedido condicional, um `304` renova a data da consulta). Diz se os ratings "continuam os mesmos". |
| FD-07 | Achar o ID | Busca por nome no cartão da FIDE (`Lichess::fide_search`), até 8 resultados com os três ritmos separados; **Vincular este ID** cria a Conexão e a torna a padrão. A página Perfil local só mostra o rating e leva à Central para vincular. |
| FD-08 | Limpar dados guardados | Uma ação na Central apaga o cache de **todos** os Provedores do Perfil local ativo. A confirmação lista o que some e o que fica: Partidas jogadas, Partidas e Exercícios importados, Conexões e autorizações, o último rating FIDE e os interruptores. O cache mora no `rede.db`, os dados do usuário no banco do app: limpar um nunca toca o outro. |
| FD-09 | Importações por Origem | Aba **Origens** na Biblioteca: "Tudo de um Provedor" e "Por importação" (nome e data). Marcar o Provedor cobre as importações dele, sem contar duas vezes. A confirmação traz a contagem e o que fica. |
| FD-10 | Partida com duas Origens | A mesma Partida que veio de dois lugares é uma Partida com duas Origens. Apagar uma Origem **não** apaga essa Partida: ela fica e a confirmação conta quantas ficam, "porque também vieram de outra origem". |
| FD-11 | Nunca as próprias | O apagamento só alcança `imported_games`. As Partidas jogadas no app vivem em outra tabela e nenhuma consulta de lote as nomeia. |
