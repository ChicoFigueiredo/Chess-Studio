# Tela cheia (F11) e modo Foco (F10) — C3

Dois estados de interface **independentes**, só Apresentação (ADR-0001): nenhum emite Comando
de Partida, pausa o relógio ou muda o turno (teste `fullscreen_and_focus_never_touch_the_game_or_the_clock`).
Ambos são preferências gravadas no ato (`fullscreen`, `focus_mode`) e voltam ao reabrir o app.

## Tela cheia

- F11 ou "Tela cheia (F11)" na seção "Exibição" da janela principal alterna
  `BorderlessFullscreen` e `Windowed`. Ao entrar, tamanho e posição da janela são guardados e
  restaurados na volta.
- Diferenças por sistema:
  - **Windows**: F11 funciona; a tela cheia sem bordas mantém a barra de tarefas oculta pelo sistema.
  - **Linux (X11/Wayland)**: depende do gerenciador de janelas; alguns reservam F11 para si. No
    Wayland a posição da janela não é conhecida pelo aplicativo, então só o tamanho é restaurado.
  - **macOS**: o sistema usa Ctrl+Cmd+F para a tela cheia e pode interceptar F11 (Mostrar Mesa);
    use o item de menu. A tela cheia sem bordas do winit não cria um Space próprio.
- O item de menu existe justamente para os casos em que o sistema intercepta a tecla.

## Modo Foco

- F10 ou "Modo Foco (F10)" esconde o dock e a janela principal. Ficam o tabuleiro, a mesa, o
  relógio 3D e o botão discreto "Sair do Foco" (canto superior direito). No 2D técnico, que não
  tem relógio na mesa, um relógio compacto aparece no canto superior esquerdo.
- Janelas que a Partida exige (promoção, retomada em Pausa, Nova Partida em curso) continuam.
- Gancho para o HUD (C5): `focus::in_focus` / `ViewState::focus`. O Foco não esconde HUDs; a
  visibilidade deles segue só a Política de arbitragem (BDD-09).
- Os Auxílios visuais continuam como o usuário os configurou (o perfil "Jogo físico" já os desliga).

## Enquadramento

`chess_input::frame_points` (pura) calcula alvo e distância para o tabuleiro com peças e o
relógio 3D caberem na área útil; `framing::fit_active_view` a reaplica enquanto a Vista é
nomeada, ao mudar a largura do viewport (dock aberto/fechado/arrastado, Foco, janela).
Câmera livre (o usuário mexeu) não é reenquadrada; as teclas 1-4/0 voltam a enquadrar.
