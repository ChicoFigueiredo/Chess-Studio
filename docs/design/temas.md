# Temas

O Chess Studio tem 22 Temas: Escuro e Claro, de fábrica, e mais 20 com temática de xadrez, 10 escuros e 10 claros. O usuário escolhe em Opções > Aparência > Tema; "Seguir o sistema" alterna entre Escuro e Claro.

## Como usar

- Escuro e Claro ficam em `tokens.json`. Os outros 20 ficam em `themes.json`: cada um tem uma `base` (`dark` ou `light`) e a lista `colors`, com as 22 cores que ele troca.
- Aplique primeiro a base e depois as cores do Tema. Nas prévias: `data-theme="<base>" data-cs-theme="<id>"` no mesmo elemento. No app: o `Visuals` da base com as cores do Tema por cima.
- Um Tema troca só superfícies, contornos, tintas, `accent`, estados e `scrim`. O tabuleiro (`board-light`, `board-dark`, `board-select`, `last-move`, `hover-ring`), a barra de avaliação e as sombras vêm sempre da base: o tabuleiro não muda com o Tema.
- Use sempre o nome do token, nunca o hex de um Tema.
- A página "Themes" mostra a interface em cada Tema, com os valores e o contraste medido.

## Os 22 Temas

| Tema | id | Base | Família | surface-1 | accent | Inspiração |
| --- | --- | --- | --- | --- | --- | --- |
| Escuro | `dark` | Escuro | Padrão | `#1c1916` | `#d99a4e` | O Tema de fábrica: neutros quentes e o âmbar da madeira do tabuleiro. |
| Claro | `light` | Claro | Padrão | `#fbf9f6` | `#a8641c` | O Tema de fábrica claro: papel quente e âmbar escuro. |
| Ébano | `ebano` | Escuro | Materiais | `#141517` | `#eaddc1` | As peças pretas de ébano: carvão quase neutro, com o marfim das peças brancas como cor de ação. |
| Jacarandá | `jacaranda` | Escuro | Materiais | `#231613` | `#e0c98a` | O jacarandá das peças Staunton: marrom-avermelhado profundo, com o amarelo-palha do buxo. |
| Feltro | `feltro` | Escuro | Materiais | `#111d16` | `#d5aa55` | O feltro verde sob as peças e sobre a mesa de jogo: verde-garrafa, com latão. |
| Engine | `engine` | Escuro | Jogo | `#121723` | `#89a7fa` | A noite de análise com a Engine: azul-meia-noite, com o azul elétrico do cálculo. |
| Siciliana | `siciliana` | Escuro | Aberturas | `#1c1917` | `#f79643` | Defesa Siciliana: pedra vulcânica do Etna e laranja-siciliano. |
| Escandinava | `escandinava` | Escuro | Aberturas | `#1f252d` | `#8ed1df` | Defesa Escandinava: noite polar em azul-ardósia, com azul-gelo. |
| Índia do Rei | `india-do-rei` | Escuro | Aberturas | `#0c1d1f` | `#f9ac46` | Defesa Índia do Rei: azul-pavão profundo, com açafrão. |
| Gambito da Dama | `gambito-da-dama` | Escuro | Aberturas | `#1e1620` | `#dda2e6` | Gambito da Dama: púrpura real em tom de ameixa, com lilás. |
| Blitz | `blitz` | Escuro | Jogo | `#0e0f0e` | `#bbe75f` | Partida relâmpago: o preto do relógio digital e o verde-limão do visor. |
| Pretas | `pretas` | Escuro | Alto contraste | `#030303` | `#fdc768` | O lado das pretas em alto contraste: preto absoluto, texto branco e âmbar, 7:1 em todo texto e contornos de 3:1. |
| Marfim | `marfim` | Claro | Materiais | `#fbf9f5` | `#362c24` | As peças brancas de marfim: branco quente, com o ébano das peças pretas como cor de ação. |
| Buxo | `buxo` | Claro | Materiais | `#fcf9f1` | `#6f3f20` | O buxo claro das peças brancas Staunton: amarelo-palha, com o castanho do jacarandá. |
| Mármore | `marmore` | Claro | Materiais | `#f8f9fb` | `#3d596c` | Tabuleiro de mármore: branco frio com veios em azul-ardósia. |
| Súmula | `sumula` | Claro | Jogo | `#faf9f8` | `#1f50b2` | A Súmula de papel preenchida à caneta: branco de papel e azul de esferográfica. |
| Torneio | `torneio` | Claro | Jogo | `#fbfaf3` | `#2d6a39` | O tabuleiro de vinil dos torneios: creme e verde. |
| Francesa | `francesa` | Claro | Aberturas | `#faf9fc` | `#6343a4` | Defesa Francesa: lavanda da Provença, branco-lilás com violeta. |
| Italiana | `italiana` | Claro | Aberturas | `#fdf8f6` | `#596423` | Abertura Italiana: reboco rosado e verde das oliveiras. |
| Catalã | `catala` | Claro | Aberturas | `#fcf9f4` | `#006772` | Abertura Catalã: areia clara e azul-petróleo do Mediterrâneo. |
| Lewis | `lewis` | Claro | Materiais | `#fdf9f3` | `#6a1d30` | As peças de Lewis, do século XII: marfim de morsa, com o bordô da tinta de garança. |
| Brancas | `brancas` | Claro | Alto contraste | `#ffffff` | `#693b03` | O lado das brancas em alto contraste: branco absoluto, texto preto e âmbar escuro, 7:1 em todo texto e contornos de 3:1. |

## Contrato de contraste

Todo Tema, de fábrica ou de xadrez, cumpre os mesmos pares (WCAG 2):

- **Texto, 4,5:1:** `ink` sobre `surface-0` a `surface-4` e `accent-subtle`; `ink-muted` sobre `surface-0` a `surface-3` e `accent-subtle`; `on-accent` sobre `accent`, `accent-hover` e `accent-pressed`; `accent-ink` sobre `accent-subtle` e `surface-1` a `surface-3`; `info`, `success` e `warning` sobre `surface-1` a `surface-3`; `danger` sobre `surface-1` a `surface-4`.
- **Contornos e marcas, 3:1:** `border-strong` e `accent` sobre `surface-0` a `surface-2`; `accent` sobre `accent-subtle`; `focus-ring` sobre todas as superfícies; `board-select` sobre as duas casas.
- **Alto contraste (Pretas e Brancas):** todo texto com 7:1 e também `border` com 3:1, para que Painéis e controles se separem por contorno, não só por tom.
- `ink-disabled` é isento, e por isso nunca é o único sinal.

## Regras

- Estados (`success`, `warning`, `danger`, `info`) vêm sempre com ícone ou palavra. Em Temas com `accent` verde, azul ou bordô (Torneio, Blitz, Engine, Escandinava, Súmula, Mármore, Lewis), a cor sozinha não distingue estado de identidade.
- Onde o `accent` é azul, `info` muda para azul-petróleo ou violeta; onde é verde, `success` vai para o verde-azulado.
- Para criar um Tema: escolha a base, defina as 22 cores e confira todos os pares do contrato antes de publicar. Um Tema que falha num par não entra.
- Não crie Tema que mude o tabuleiro. Aparência de peças e casas é preferência do Tabuleiro, não do Tema.
