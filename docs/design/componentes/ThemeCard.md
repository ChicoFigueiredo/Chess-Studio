# ThemeCard (cartão de Tema)

O cartão de Tema é a escolha da galeria de Opções > Aparência: mostra uma amostra do Tema e o nome, para a pessoa ver antes de escolher. A galeria agrupa os 22 Temas por família (veja `temas.md`).

- Tamanho fixo de 132 x 76, acima do alvo mínimo de 24. Fundo `surface-2`, `surface-3` no hover, `surface-4` pressionado, cantos de raio 6 (`radius-l`).
- **Amostra** no alto: um retângulo com o `surface-1` do Tema, uma faixa de 30 x 6 no `accent` do Tema e dois traços no `ink` do Tema (o segundo mais fraco). É a única parte que usa as cores do Tema mostrado; o resto do cartão usa o Tema ativo.
- **Nome** em `body`, no pé do cartão.
- **Marca**, sempre com forma além da cor:
  - escolhido: contorno `accent` de 2px e uma bolinha `accent` com visto (`on-accent`) no canto da amostra;
  - em uso por "Seguir o sistema": contorno `accent` fino, sem visto;
  - nenhum dos dois: contorno `border-strong`.
- O tooltip leva o nome e a inspiração do Tema.
- Foco do teclado com o anel `focus-ring`. O leitor de tela lê o nome e se está escolhido.
- A escolha vale na hora e pertence ao Perfil local; o tabuleiro nunca muda com o Tema.
- **egui:** `cs_theme_card(ui, &ThemeSwatch, ThemeCardMark)` em `ui_theme`; as amostras vêm de `themes_by_family()`.
