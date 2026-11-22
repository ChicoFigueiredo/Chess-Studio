# StudyBanner (faixa do Modo de estudo)

A faixa avisa que o tabuleiro principal está no **Modo de estudo** (decisões 5, 6, 9, 11 e 12 da spec #320): os Lances jogados ali vão para a Linha de estudo e não alteram a Partida. Junto com ela, as casas e a moldura do tabuleiro ganham a tonalidade `study-tint`.

- **Onde:** centrada no topo da área do tabuleiro (a que sobra do dock), a 8 pontos da borda, com no máximo 560 pontos de largura e nunca mais larga que a área. Camada egui em primeiro plano, sobre o viewport 3D ou 2D; não há câmera nova.
- **Fundo e contorno:** `surface-2` com contorno de 1px em `border-strong` e raio `radius-l`.
- **Conteúdo, da esquerda para a direita:** ícone `flask` e a palavra "Estudo" em `accent-ink` (semibold); a frase "Os Lances daqui não alteram a Partida." em `ink-muted`, só quando a faixa tem 520 pontos ou mais (em 800x600 ela some e o resto cabe); e, à direita, o botão primário "Voltar à Partida" com o ícone `arrow-u-up-left`.
- **"Lance novo na Partida":** quando a Partida avançou durante o estudo (a Engine adversária jogou), o aviso entra no lugar da frase de apoio, em `warning`, **com ícone e palavra**. O estudo continua; só o botão, Esc ou End voltam.
- **Voltar:** um clique no botão, **Esc** (quando não há peça selecionada, arrastada ou seletor de promoção aberto; com gesto em andamento o primeiro Esc só o cancela) e **End** (tratado pela Notação) chamam `Session::leave_study`. A faixa e a tonalidade somem. ← no primeiro Lance da Linha não sai do estudo.
- **Contraste:** `accent-ink`, `ink-muted` e `warning` sobre `surface-2` já cumprem 4,5:1 em todos os Temas (contrato de `temas.md`); o botão primário segue `Button.md`.
- **Tonalidade `study-tint`:** véu translúcido sobre as 64 casas e a moldura, **abaixo** das peças, dos Auxílios e do HUD; as peças não mudam de cor. Existe nos 22 Temas (o `info` do Tema a 45% nos escuros e a 40% nos claros) e vale nas três Apresentações (3D profissional, Mesa imersiva e 2D técnico, que ganha uma moldura fina tingida) e no painel Tabuleiro 2D (casas e contorno de 4px por dentro).
- **Sem efeito no jogo:** a tinta e a faixa não entram no picking nem no Método de movimento; a faixa é a única parte clicável.
- **egui/Bevy:** a tinta são malhas e sprites translúcidos (como os Auxílios) e o painel desenha com o `painter`. Nenhuma câmera nova e nenhum `Msaa` ou `Hdr` próprio (regra de câmeras, #82 e #142).
