# QuickAccessBar

A Barra de Acesso Rápido mostra, acima do Ribbon, as Ações fixas que o usuário escolheu (Desfazer, Nova Partida, Virar tabuleiro…) e o botão da Paleta de Ações à direita.

- Botões sutis de 24px, só com ícone e tooltip. A seta abre "Personalizar" (marcar e desmarcar Ações).
- É guardada no Perfil de interface.
- **egui:** a mesma faixa da barra de título, `ui.horizontal` com `cs_button(Variant::Subtle)`.
