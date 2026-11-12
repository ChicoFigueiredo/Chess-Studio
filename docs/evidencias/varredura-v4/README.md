# Evidência: varredura visual do V4 (#274)

Varredura pelo modo `--screenshot` das telas do V4 (Central de Conexões e seus estados, consentimento, autorização, Explorador, importação online, Origens, Treino > Pacotes e download, Exercícios, fim de Partida, Partida online no Lichess e no FICS, Engine online e remota, Perfil local e FIDE). Todos os dados são fictícios; nenhum pedido de rede, nenhum diálogo do sistema.

Matriz gerada (fora do repositório, para não commitar centenas de imagens): 35 telas x 4 Temas (Escuro `dark`, Claro `light`, Tema de cor `feltro`, Alto contraste `pretas`) x 3 tamanhos (1280x720, 1600x900, 1920x1080) = 420 capturas "antes". Cada captura foi aberta e conferida; as telas com defeito estão listadas abaixo. As capturas desta pasta são as de antes e depois dos defeitos corrigidos.

```
chess-studio --screenshot <pasta> --single <nome> --size 1280x720 --theme dark|light|feltro|pretas <cena>
  central      --backstage-page connections --connections-demo --connections-open lichess,fide,fics,playstrategy
  perfil-fide  --backstage-page connections --connections-demo --connections-open fide --connections-fide-search
  perfil-local --backstage-page local-profile --connections-demo
  limpar       --backstage-page connections --connections-demo --connections-clear-confirm --connections-scroll 900
  consent      --panel --consent-demo
  auth-<cena>  --backstage-page connections --connections-auth authorized|waiting|expired|locked|refused
  expl-<cena>  --panel --explorer-demo library|masters|signin|off|cloud|import|import-done|import-playstrategy[-done] --moves e2e4,e7e5,g1f3
  lib-<cena>   --panel --tabs library --library imports|imports-confirm|exercises
  tr-<cena>    --panel --dock-demo training-float --training list|download|download-running|download-done|download-error|download-consent|download-list
  fim          --moves f2f3,e7e5,g2g4,d8h4
  on-<cena>    --panel --online-demo dialog|playing|ended|fics-guest|fics-lobby|fics-playing
```

## Defeitos corrigidos

| # | Defeito | Onde | Correção | Antes | Depois |
|---|---|---|---|---|---|
| 1 | Lista de Exercícios (Treino) com área de rolagem fixa de 260 px: sobra o resto do Painel vazio e a lista é cortada no meio de uma linha | `training_ui.rs` (`list_view`) | A área ocupa a altura disponível (`max_height(ui.available_height())`, sem encolher) | [antes](antes/exercicios-lista-escuro-1600x900.png) | [depois](depois/exercicios-lista-escuro-1600x900.png) |
| 2 | Botões egui padrão no diálogo de fim de Partida (Revisar, Nova Partida, Exportar PGN, Fechar); o "Salvar como…" já era do design system e destoava | `end_game.rs` | `cs_button`: Revisar primário, Nova Partida e Exportar PGN secundários, Fechar discreto | [antes](antes/fim-de-partida-escuro-1600x900.png) | [depois](depois/fim-de-partida-escuro-1600x900.png) |
| 3 | Nomes de abertura com trecho em inglês: "Variante Dragon" (51 linhas), "Variante Positional", "Linha Retreat", "Variante Kingside", "Dragon invertida", "Botvinnik System Reversed", "Kingside Fianchetto" (117 linhas), "Maróczy Bind", "Hedgehog", "Fool's Mate" e outros | `chess-openings/src/pt_br.rs` | Novos termos nas tabelas e regras de "X Dragon" e "X Reversed"; testes `common_english_leftovers_are_translated` e `the_book_has_no_generic_english_words_left` | sem imagem (texto; ver testes) | sem imagem |
| 4 | Central, cartão "Dados guardados": o botão "Limpar dados guardados…" cobre o fim do texto em 1280x720 | `connections/stored_data.rs` | Botão reservado à direita e texto quebra no espaço que sobra | [antes](antes/central-dados-guardados-escuro-1280x720.png) | [depois](depois/central-dados-guardados-escuro-1280x720.png) |
| 5 | Diálogo "Ligar Lichess?" maior que a janela de 720 px: título e botões cortados, sem rolagem | `provider_consent.rs` | O texto rola numa área limitada pela altura da janela; título e botões ficam sempre visíveis | [antes](antes/consentimento-escuro-1280x720.png) | [depois](depois/consentimento-escuro-1280x720.png) |
| 6 | "Requer autorizar sua conexão do Lichess" (frase torta) no Explorador e na Central | `explorer.ftl`, `authorization.ftl`, `connections/model.rs` (comentário) | "Autorize sua conexão do Lichess" / "Para usar o Explorador de aberturas, autorize sua conexão do Lichess." | [antes](antes/explorador-autorizar-altocontraste-1280x720.png) | [depois](depois/explorador-autorizar-altocontraste-1280x720.png) |
| 7 | Origens: os grupos "Outro ou não sei" e "Origem não informada" diziam "Marca todas as importações deste Provedor" | `library_ui`/`library_bulk.rs`, `profiledata.ftl` | Texto próprio: "sem Provedor identificado" | [antes](antes/origens-escuro-1280x720.png) | [depois](depois/origens-escuro-1280x720.png) |
| 8 | Biblioteca: campos de busca e de nome do favorito em egui padrão, com a dica cortada ("Nome (padrão: lanc…") e o botão "Marcar posição" padrão | `library_ui.rs`, `library.ftl` | `cs_text_field` e `cs_button`; dicas curtas ("Jogador, evento ou abertura…", "Nome (opcional)") | [antes](antes/biblioteca-campos-claro-1280x720.png) | [depois](depois/biblioteca-campos-claro-1280x720.png) |
| 9 | Treino > Pacotes: campos "Dificuldade mín./máx." com a dica cortada ("Dificuldade …") e desalinhados | `training_ui/packs.rs` | Largura dos campos de 140/110 para 170/160 px e altura fixa (`field_slot`), sem "escada" quando a linha quebra | [antes](antes/pacotes-campos-claro-1280x720.png) | [depois](depois/pacotes-campos-claro-1280x720.png) |
| 10 | Linha de Exercício com o texto quebrando ao lado do botão ("Resolver Lichess 001Kq — acertos: 0 \| resoluções: 0 — dificuldade 1250 —") | `training_ui.rs` (`exercise_row`) | Título em negrito ao lado do botão; números e temas numa linha pequena embaixo | [antes](antes/exercicios-lista-escuro-1600x900.png) | [depois](depois/exercicios-lista-escuro-1600x900.png) |
| 11 | Consentimento de download: "A baixada de problemas precisa de Lichess" (e "precisa de Lichess" em todos os consentimentos) | `training.ftl`, `connections.ftl` | "O download de problemas precisa do Lichess, que está desligado." | [antes](antes/download-consentimento-feltro-1280x720.png) | [depois](depois/download-consentimento-feltro-1280x720.png) |
| 12 | Jargão de desenvolvedor: "Sem timeseal" (cartão do FICS), "Em cache (Lichess)", "do cache", "Limpar cache", "itens do cache" | `fics.ftl`, `explorer.ftl`, `connections.ftl`, `gallery.ftl`, `chess-net/src/catalog.rs` | "Sem compensação de atraso", "Avaliação pronta (Lichess)", "avaliação guardada em…", "Limpar dados guardados" | [antes](antes/central-fics-timeseal-feltro-1920x1080.png) | [depois](depois/central-fics-timeseal-feltro-1920x1080.png) |

## Defeitos em `online/` e `engine_online/` (NÃO corrigidos aqui; para quem trabalha em #272 e #271)

Capturas em [`relatorio-outros/`](relatorio-outros/).

`online/` (#272):
- **Janela de 1280x720**: o Painel "Partida online" fica com ~100 px de altura entre os outros Painéis do dock; o conteúdo é cortado (`online-partida-escuro-1280x720.png`: a linha "Controle de tempo", o rodapé "Relógio do Lichess" e, na cena `ended`, os botões "Analisar" e "Ver na Biblioteca" aparecem pela metade). Sugestão: dar altura mínima ao Painel ou rolagem.
- **Diálogo "Partida online" (Lichess e FICS) em 1280x720**: mais alto que a janela; o título e as abas de cima somem atrás do Ribbon, sem rolagem (`online-dialogo-altocontraste-1280x720.png`, `online-fics-feltro-1280x720.png`). Mesma correção usada em `provider_consent.rs`: altura máxima da tela e `vscroll(true)`.
- Texto repetido no FICS: "Visitante: Partidas sem rating." no topo e "Visitante só joga sem rating." embaixo.
- `online/fics_view.rs` ainda cita "timeseal" num comentário (sem efeito na tela; o texto da tela foi trocado em `fics.ftl`).

`engine_online/` (#271) (conferido nas capturas de `docs/evidencias/engine-remota-online/`, 1920x1080; as de 1280x720 não puderam ser regravadas, ver "Limites"):
- Diálogo "Engine online no Lichess": a base passa da área visível, os botões "Parar" e "Parar já" aparecem cortados no rodapé, e o topo encosta no Ribbon. Mesma correção: altura máxima e rolagem.
- Dialogo "Usar a Engine remota?": "o FEN" é jargão (a frase explica, mas "a Posição em notação FEN" seria menos técnica) e "Threads", "Hash (MB)" no Painel de Análise são termos de Engine sem explicação.
- Ritmos aceitos mostra "Bullet" enquanto o resto do app (FICS) usa "Relâmpago".

## Limites desta varredura

- A GPU da máquina ficou sem memória livre durante a regravação (um processo `ollama` de outro uso ocupava 8 GB e a placa ficou em 15,7 de 16 GB): as capturas "depois" desta pasta foram feitas com o renderizador por software do Vulkan (`lvp`), o que muda levemente o 3D (sombras, antisserrilhado) mas não a interface egui. As "antes" são da GPU. A matriz completa "depois" (420) não pôde ser regravada por isso; foram regravadas as telas dos defeitos corrigidos (cada uma conferida aberta). Itens abaixo marcados UNVERIFIED precisam de uma nova passada com a GPU livre.
- UNVERIFIED: efeito dos itens 6 a 8, 11 e 12 nos demais Temas e tamanhos (o texto é o mesmo em todos; a verificação feita foi nas telas listadas).
- Não foi possível regravar as cenas de Engine online em 1280x720 (`--engine-online-tour`, mesma falta de GPU).
