# Spike de docking (C1, issue #43)

Pergunta: dá para ter painéis encaixáveis (abas, divisões, redimensionamento, layout persistido) sobre `bevy_egui` 0.43 / `egui` 0.36 sem perder o picking e a órbita do tabuleiro 3D e sem violar a ADR-0004 (licenças)?

## Alternativas

| | `egui_tiles` 0.17.1 | `egui_dock` 0.21.1 | Docking próprio |
|---|---|---|---|
| egui | 0.36 (compatível) | 0.36 (compatível) | - |
| Licença | MIT OR Apache-2.0 | MIT | nossa |
| `cargo deny check licenses` | passa | não testado | - |
| Modelo | árvore de tiles (abas, linear, grade), `Behavior` com `pane_ui` | `DockState` com superfícies/janelas destacáveis | muito trabalho |
| Serde | feature `serde` na árvore inteira (tamanhos, visibilidade, abas ativas) | feature `serde` | manual |
| Visibilidade por painel | `Tree::set_visible` nativo | remoção/reinserção de aba | manual |

## Escolha: `egui_tiles` 0.17.1

- Mesma licença do projeto (MIT OR Apache-2.0), mantido pela Rerun, release acompanha o egui (0.18 já existe para egui 0.37; ficamos na 0.17 por casar com o egui 0.36 do `bevy_egui` 0.43).
- `Tree<PanelKind>` serializa inteira com serde: posições, proporções dos divisores, aba ativa e visibilidade. O `DockLayout` (árvore + largura da região) vai como JSON na preferência `dock_layout` (`set_pref`, perfil padrão).
- Mostrar/ocultar é `Tree::set_visible`, sem remover o painel da árvore: reexibir devolve o painel onde estava.
- `egui_dock` também serve; a vantagem dele (janelas destacáveis) é V2 e a árvore do `egui_tiles` é mais simples de validar e testar.

## Integração com bevy_egui e o tabuleiro 3D

Achados que mudaram o desenho:

1. O `bevy_egui` roda os sistemas dentro de `Context::run_ui`, mas descarta a raiz `Ui` e entrega só o `Context`. `egui::Panel::show` exige um `Ui`; o exemplo `side_panel` do `bevy_egui` cria um `Ui` na camada de fundo para contornar.
2. Para o egui, a camada de fundo cobre a janela toda. `is_pointer_over_egui` só trata o fundo como "livre" fora da área disponível da raiz (que o app não usa). Resultado: qualquer coisa desenhada na camada de fundo ocupa o ponteiro e o tabuleiro ficaria sem mouse na área inteira (testado: falhava).
3. Solução adotada: o dock é uma `egui::Area` em `Order::Middle` colada na borda direita, com alça de redimensionamento na borda esquerda, e **o centro da janela não é um painel do dock**: é o que sobra, sem nenhum widget. Assim `EguiWantsInput` só reivindica o ponteiro sobre o dock e as janelas; o picking (`picking::pick`), a órbita e o zoom continuam como antes no centro. Além disso, `Dock::pointer_in_board` exige o cursor na área livre (defesa em profundidade).
4. Teste automatizado (`dock::tests::the_centre_is_transparent_to_the_pointer_and_the_panel_is_not`): reproduz o ciclo do `bevy_egui` (`run_ui` + `is_pointer_over_egui` depois) e confirma centro livre e dock reivindicado.

Resultado: **viável**. `egui_tiles` funciona com `bevy_egui` 0.43; o painel central transparente é conseguido sem o `CentralPanel`.

## Riscos e pendências

- As câmeras ainda renderizam na janela inteira: o dock cobre a faixa direita do tabuleiro em janelas estreitas. Recortar o `Viewport` das câmeras 3D/2D à área livre (`Dock::board_rect`) fica para o C2 (exige ajustar `cursor_to_world` do 2D e a projeção do picking).
- A `Area` do dock está em `Order::Middle`: clicar nela a coloca acima de janelas flutuantes sobrepostas. Aceitável enquanto as janelas migram para o dock.
- Só existe uma região de dock (direita). Regiões esquerda/inferior e arrastar abas para fora da região ficam para o C2.
- Atualização do `egui_tiles` acompanha o egui: ao subir o `bevy_egui`, subir junto (a árvore serializada tem fallback para o layout padrão se não puder ser lida).
- `Area` sem `Panel` não reserva espaço para outras janelas ancoradas: a janela de Auxílios e o relógio do 2D usam `Dock::right_offset` para se afastar do dock.
