# Homologação da interface nova (design system, épico #159; troca única #167)

Lista de verificação manual da interface que substituiu o painel esquerdo. Todos os itens estão **UNVERIFIED**: dependem do usuário abrir o app em hardware físico (Linux, Windows e macOS) e aprovar o visual. Os testes automatizados citados cobrem a lógica; as capturas em `docs/evidencias/ds-8/` mostram o estado final, mas não substituem a aprovação.

Legenda: **UNVERIFIED** = pendente do usuário (não é PASS). Marque PASS ou FAIL aqui, com a data, ao verificar.

Referências: `docs/decisoes/2026-11-09-design-system.md` (DS-04, DS-15), `docs/design/README.md`, `GLOSSARY.md`.

| ID | O que conferir | Como conferir | Evidência / teste | Status |
|---|---|---|---|---|
| **UI-NOVA-01** | A janela abre só com a interface nova: Ribbon no topo, dock à direita, barra de status no pé. Não existe janela "Chess Studio" com painel esquerdo, nem `--nova-ui`. | `cargo run -p chess-studio` | `docs/evidencias/ds-8/perfil-estudo-escuro.png` | UNVERIFIED |
| **UI-NOVA-02** | Ribbon, modo clássico: abas Arquivo, Início, Análise, Biblioteca, Treino, Exibir; grupos e botões grandes; Desfazer, Refazer, Abandonar e Nova Partida respondem. | Clicar em cada aba e em cada botão | `ribbon/mod.rs::a_ribbon_click_requests_the_same_action_as_the_shortcut` | UNVERIFIED |
| **UI-NOVA-03** | Ribbon, modo simples (uma linha, "Mais" para o excesso) e modo recolhido (só as abas; clicar numa aba mostra o corpo). | Menu do Ribbon (canto direito) | `ribbon/mod.rs::the_collapse_action_toggles_the_ribbon` | UNVERIFIED |
| **UI-NOVA-04** | Barra de Acesso Rápido: fixar e desafixar Ações, ordem preservada, volta após reiniciar. | Fixar Desfazer e Nova Partida; reiniciar | `ribbon/mod.rs::pinning_to_the_quick_access_bar_is_idempotent` | UNVERIFIED |
| **UI-NOVA-05** | KeyTips: Alt mostra as letras das abas, a letra da aba mostra as das Ações, executar não dispara o atalho simples. | Alt, depois a letra de uma aba e de uma Ação | `ribbon/mod.rs::alt_then_letters_perform_an_action_without_firing_the_plain_shortcut` | UNVERIFIED |
| **UI-NOVA-06** | Bastidores (aba Arquivo): páginas de Perfil de interface e Perfil local, voltar fecha. Só um título "Perfil de interface" por página. | Arquivo | `ribbon/backstage.rs` | UNVERIFIED |
| **UI-NOVA-07** | Paleta de Ações (Ctrl+K): busca sem acentos, setas e Enter, Esc fecha, Ação desabilitada mostra o motivo, o véu (scrim) escurece o fundo (3D e dock). | Ctrl+K, digitar "nova" | `palette.rs` (testes); `docs/evidencias/ds-8/paleta-escuro.png` | UNVERIFIED |
| **UI-NOVA-08** | Opções (Ação "Opções", no Ribbon ou na Paleta): categorias Aparência, Tabuleiro, Auxílios, Engines etc.; cada linha muda o mesmo recurso do atalho; "Perfil de interface" aparece uma vez só; Restaurar padrões pede confirmação. | Abrir Opções e percorrer as categorias | `options/tests.rs` | UNVERIFIED |
| **UI-NOVA-09** | Nenhum controle do painel antigo ficou sem caminho: cada item do inventário tem Ação (Ribbon e Paleta) ou Opção. | Conferir o inventário contra a interface | `options/tests.rs::every_inventory_action_is_reachable_in_the_ribbon_and_the_palette`, `every_inventory_item_has_a_destination` | UNVERIFIED |
| **UI-NOVA-10** | Barra de status reserva a faixa de 24 px: o 3D, o dock e as janelas flutuantes terminam acima dela; mostra Perfil, Engine com consentimento (ponto e palavra), vez e lance, casa sob o cursor, Vista, avisos (xeque, pouco tempo), FPS opcional (botão direito na Escala) e Escala. | Mover o mouse sobre o tabuleiro; jogar até o xeque | `status_bar.rs` (testes); `ribbon/mod.rs::the_usable_rect_ends_above_the_status_bar` | UNVERIFIED |
| **UI-NOVA-11** | Dock: abas de Painel com cabeçalho novo, arrastar uma aba para outro grupo, destacar em janela flutuante (e devolver), guias de encaixe ao arrastar, "Restaurar layout padrão". | Arrastar abas; botões de destacar e fechar | `dock.rs` (testes); `panel_chrome.rs` | UNVERIFIED |
| **UI-NOVA-12** | Barra de avaliação colada ao tabuleiro, acompanha a câmera (3D e 2D técnico), some sem Engine consentida ou com a análise bloqueada. | Ligar a análise com uma Engine consentida | `eval_bar.rs` (testes); `docs/evidencias/ds-6/` | UNVERIFIED |
| **UI-NOVA-13** | Os 5 Perfis de interface (Jogo físico, Treinamento assistido, Estudo e análise, Competição, Mínimo) aplicam Apresentação, painéis, Auxílios, Foco e Ribbon; Personalizado aparece ao mexer em algo. | Trocar de perfil pela barra de status e pelas Opções | `profiles.rs` (testes); `docs/evidencias/ds-8/perfil-*-escuro.png` e `perfil-*-claro.png` | UNVERIFIED |
| **UI-NOVA-14** | Tema (Sistema, Escuro, Claro), Densidade (confortável, compacta) e Escala (Ações de zoom da interface) valem em todas as telas: Ribbon, dock, Opções, Paleta, barra de status. Legível e sem sobreposição nos dois Temas. | Opções > Aparência | `options/tests.rs::a_dialog_change_updates_the_same_resource_the_old_control_did`; `docs/evidencias/ds-8/perfil-estudo-claro.png` | UNVERIFIED |
| **UI-NOVA-15** | Modo Foco (F10) esconde Ribbon, dock e barra de status e devolve a faixa ao tabuleiro; ficam o tabuleiro, o relógio e "Sair do Foco". Janelas que a Partida exige (promoção, retomar Engine) continuam. | F10, jogar, F10 | `focus.rs::f11_and_f10_toggle_independent_states` | UNVERIFIED |
| **UI-NOVA-16** | Tela cheia (F11) funciona sozinha e junto com o Foco; a janela volta ao tamanho e à posição anteriores. | F11 e F10 em todas as combinações | `focus.rs` (testes) | UNVERIFIED |
| **UI-NOVA-17** | Partida contra Engine sem o painel esquerdo: a janela "Engine adversária" aparece só quando há o que mostrar (Engine pensando, falha com "Tentar de novo" e "Trocar Engine", "Conectar Engine e retomar" numa Partida reaberta, aviso de execução). | Reabrir uma Partida contra Engine; derrubar a Engine no meio | `engine.rs`, `engine_consent.rs` (testes) | UNVERIFIED |
| **UI-NOVA-18** | Janela pequena (800x600 e 1024x576): dock vira sobreposição, a barra de essenciais fica acima da barra de status, nada se sobrepõe. | Redimensionar a janela | `responsive.rs` (testes) | UNVERIFIED |
| **UI-NOVA-19** | 60 FPS com a interface completa no cenário de análise e no de Partida. | `scripts/benchmark_matriz.sh` | `docs/benchmark.md` | UNVERIFIED |

## O que falta

Todos os itens acima são do usuário: abrir o app nos três sistemas, percorrer a lista, marcar PASS ou FAIL e aprovar o visual dos 5 Perfis de interface nos dois Temas.
