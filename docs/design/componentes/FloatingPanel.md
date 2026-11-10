# FloatingPanel

O Painel flutuante fica numa janela dentro da janela principal, sobre o tabuleiro.

- Contorno `border-strong`, `radius-l` e `shadow-floating`, para se destacar da cena 3D.
- Lembra posição e tamanho no Perfil de interface. Uma posição fora da tela volta para dentro ao abrir.
- O Painel destacado (janela do SO) usa o mesmo conteúdo, sem a sombra.
- **egui:** `egui::Window` com `title_bar(false)` e o cabeçalho de PanelTabs.
