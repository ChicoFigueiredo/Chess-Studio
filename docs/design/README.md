# Design system do Chess Studio

Fonte de verdade para implementar a nova interface em egui: `tokens.json` (cores por Tema, tipografia, espaço, raio, sombra, tamanhos), as diretrizes por componente em `componentes/` e as decisões em `docs/decisoes/2026-11-09-design-system.md` e na ADR-0005. Prévia visual: o artefato "Chess Studio" (design system) do usuário. `referencia-web.css` e `tela-estudo-e-analise.html` são a referência web das prévias, só para consulta: os valores que importam estão em `tokens.json`.

Design system da interface do Chess Studio, um app desktop offline de xadrez em Rust (Bevy + egui). Esta página é a referência visual; a implementação **emula estes tokens e componentes em egui**, sem HTML. As decisões estão em `docs/decisoes/2026-11-09-design-system.md` e na ADR-0005 do repositório.

## Princípios

1. **O tabuleiro é a tarefa.** Ele é sempre o maior elemento da janela. Só a barra de avaliação e as coordenadas ficam coladas a ele; todo o resto está num Painel, no Ribbon ou na barra de status.
2. **Ação primeiro, preferência depois.** O que se faz agora vai no Ribbon. O que se ajusta uma vez vai em Opções. A tela de jogo nunca mostra formulários de configuração.
3. **Uma Ação, muitos lugares.** Ribbon, Barra de Acesso Rápido, KeyTips, Paleta de Ações e atalhos mostram a mesma Ação, com o mesmo rótulo, ícone, atalho e estado.
4. **Familiar como o Office.** Abas, grupos, Arquivo como bastidores, Alt para KeyTips, Ctrl+F1 para recolher. Quem conhece o Office não precisa aprender a navegação.
5. **Madeira, não vitrine.** A cor de identidade é o âmbar da madeira do tabuleiro. Neutros são levemente quentes. Sem gradientes, sem brilho, sem sombra decorativa.

## Conteúdo e voz

- Interface em pt-BR, termos do GLOSSARY: Partida, Lance, Painel, Perfil de interface, Ação. Nunca use "card", "gadget", "comando" ou "workspace".
- Botões e itens do Ribbon são verbos no infinitivo ou o nome da Ação: "Nova Partida", "Virar tabuleiro", "Flutuar", "Destacar em janela".
- Use maiúscula só no início e em termos do glossário ("Perfil de interface", "Súmula").
- Uma mensagem de erro diz o que aconteceu e o que fazer: "Já existe um perfil com esse nome. Escolha outro." Sem desculpas, sem "Ops".
- Uma Ação desabilitada explica o motivo no tooltip: "Só no modo Assistido."
- Números de xadrez usam vírgula decimal e sinal real: "+0,4", "−3,1", "M3".
- Sem emoji na interface.

## Cor

- Fundo da janela e da cena: `surface-0`. Painéis e diálogos: `surface-1`. Ribbon, cabeçalhos, campos e barra de status: `surface-2`. Hover: `surface-3`. Pressionado e aba inativa com foco perdido: `surface-4`.
- Texto em `ink`; secundário (rótulos de grupo, atalhos, metadados) em `ink-muted`; desabilitado em `ink-disabled`, nunca como único sinal.
- `accent` (âmbar) só para: botão primário, aba ativa do Ribbon (sublinhado), alternância e Switch ligados, slider, Painel ativo (traço) e relógio em andamento. Sobre `accent`, texto em `on-accent`.
- Seleção e estado ligado em superfícies usam `accent-subtle` com texto em `accent-ink`.
- `info` é para informação e Engine pensando, nunca para identidade. A peça e a casa selecionadas no Tabuleiro 2D usam um contorno de 3px em `board-select`.
- Estados usam `success`, `warning` e `danger` **sempre com ícone ou palavra**, nunca só a cor.
- `board-light`, `board-dark`, `board-select`, `eval-black`, `last-move` e `hover-ring` são iguais em todos os Temas: o tabuleiro não muda com o Tema. `eval-white` só muda entre as bases Escuro e Claro.
- Escuro (primeiro) e Claro são os Temas de fábrica e seguem o sistema operacional por padrão. Há mais 20 Temas de xadrez, 10 escuros e 10 claros, em `themes.json`; veja a seção Temas e a página "Themes".
- Todo Tema cumpre o mesmo contrato: texto com 4,5:1 sobre as superfícies que a nota do token cita; contornos de controle, anel de foco, trilho do slider e marcas com 3:1. Pretas e Brancas são os Temas de alto contraste, com 7:1 em todo texto.
- Nunca escreva um hex na interface: use o token. Um Tema novo só entra se passar em todos os pares.

## Tipografia

- Interface em **Inter** (OFL), embutida no app. Log UCI e FEN em **JetBrains Mono** (OFL).
- Use só a escala: `caption` 12/16, `ribbon-label` 12/14 (só nos botões grandes do Ribbon), `body` 14/20, `body-strong` 14/20 semibold, `body-large` 16/22 (só no campo da Paleta de Ações), `subtitle` 16/22 semibold, `title` 20/28, `display` 28/36 (só nos bastidores e no fim de Partida).
- Nenhum texto é menor que 12px, nem em miniaturas.
- `display` usa espaçamento de −0,02em e `title`, −0,01em; os demais, 0.
- Xadrez: `clock` 32/36 com números tabulares, `notation` 14/24, `eval` 12/16 negrito.
- Números que se alinham (relógio, notação, avaliação, atalhos, barra de status) usam números tabulares (`tnum`). Nas prévias isso é `font-variant-numeric: tabular-nums`; nunca declare `font-feature-settings` num ancestral, porque ele anula essa propriedade.

## Espaço, raio, sombra e tamanho

- Grade de 4px: `space-1` (4) entre controles de um grupo, `space-2` (8) de padding de botão, `space-4` (16) de padding de Painel (Confortável) ou `space-3` (12) na Compacta, `space-6` (24) de margem de diálogo.
- Raios pequenos: `radius-m` (4) em controles e abas, `radius-l` (6) em menus e Painéis flutuantes, `radius-xl` (8) em diálogos. Nada de pílulas, exceto o Switch e os chips.
- Sombra só em camadas que flutuam: `shadow-flyout` (menus, tooltips), `shadow-floating` (Painel flutuante), `shadow-dialog` (diálogos e Paleta). Painéis encaixados não têm sombra; usam `border`.
- Alturas: `control-h` 32 (Confortável) ou `control-h-compact` 24 (Compacta), `ribbon-classic-h` 96, `ribbon-simple-h` 40, `panel-header-h` 32, `statusbar-h` 24. A Escala da interface (80% a 200%) multiplica todas.
- Nada clicável é menor que 24px (`control-h-compact`): por isso o Ribbon clássico tem três linhas de 24px, e o slider e a caixa de seleção têm área de clique de 24px.

## Estados e foco

Todo controle interativo tem cinco estados: repouso, hover, pressionado, desabilitado e foco. Um controle ligado e desabilitado continua mostrando que está ligado: o visto, o botão do Switch e o preenchimento ficam, em `ink-disabled`, sem `accent`. O foco do teclado é um anel sólido de 2px em `focus-ring` (cor de `ink`), afastado 1px, visível sobre qualquer superfície. Movimento: só transições de 100–150 ms em hover e abertura de menus; nada anima com "reduzir movimento" ligado no sistema.

## Iconografia

- **Phosphor**, peso regular, MIT (`egui-phosphor` 0.14 no app; nesta página, os SVG de `@phosphor-icons/core` 2.1.1). Tamanho `icon-s` (16) em botões, abas, Ribbon simples e menus; `icon-l` (32) nos botões grandes do Ribbon clássico.
- Ícone herda a cor do texto ao lado. Botão só com ícone tem tooltip com nome e atalho.
- Não existe ícone de xadrez próprio: use `horse` ou `crown` só como marca de lado no relógio; as peças são do tabuleiro, nunca da interface.

## Estrutura da janela

De cima para baixo:

1. barra de título com a Barra de Acesso Rápido e a Paleta (Ctrl+K);
2. Ribbon (clássico, simples ou recolhido);
3. área de trabalho: tabuleiro com a barra de avaliação e o dock à direita, com Relógio e Notação no primeiro grupo e as ferramentas da tarefa em abas;
4. barra de status.

Todo Painel pode estar encaixado, flutuante ou destacado (janela do SO). O Perfil de interface guarda isso. Veja as telas "ScreenStudy" e "InterfaceProfiles".

## Implementação em egui

- Um módulo `ui_theme` lê estes tokens (cores por Tema, espaçamentos, raios, alturas) e preenche `egui::Style`/`Visuals`: `widgets.{inactive,hovered,active,noninteractive}`, `selection`, `window_*`, `menu_*`, `extreme_bg_color` e `spacing.{item_spacing,button_padding,interact_size}`.
- Densidade e Escala mudam `Spacing` e `pixels_per_point`; o Tema troca o `Visuals` inteiro: parte da base (Escuro ou Claro) e aplica as cores do Tema escolhido em `themes.json`.
- Números tabulares: confira se o texto do egui aplica `tnum`; se não aplicar, embuta uma Inter com `tnum` fixado para o relógio, a notação e a avaliação.
- As diretrizes de cada componente moram em `componentes/`. Os das integrações são `StatusBadge`, `OverflowMenu`, `Card`, `Segmented`, `ExplorerPanel`, `SecretField` (senha-mestra e chave pessoal: mascarado, sem área de transferência, sem rastro) e `ThemeCard` (galeria de Temas). Widget `cs_*` novo leva a diretriz no mesmo PR.
- Componentes são widgets do projeto (`cs_button`, `toggle_switch`, `ribbon_group`, `panel_tabs`…), nunca `egui::Button` com estilo ad hoc.
- As fontes Inter e JetBrains Mono entram por `FontDefinitions`, com a licença registrada (ADR-0004).
