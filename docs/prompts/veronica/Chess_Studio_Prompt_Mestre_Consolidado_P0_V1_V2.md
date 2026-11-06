# Chess Studio — Prompt mestre consolidado para desenvolvimento

> **Versão:** 1.0 — consolidação da conversa de planejamento (5–6 de novembro de 2026)  
> **Estado:** especificação de produto e plano de implementação; **não** é evidência de software implementado.  
> **Público:** equipe de produto, UX, engenharia Rust, gráficos 3D, testes e homologação.  
> **Objetivo deste arquivo:** ser o documento único de transferência do escopo discutido, com decisões, histórias, requisitos, critérios de aceite, testes, limites de versão e pendências.

---

## 0. Instrução principal para a equipe e para agentes de desenvolvimento

Você é uma equipe de engenharia sênior responsável por projetar, implementar, testar e homologar o **Chess Studio**, um aplicativo **desktop nativo, offline-first, multiplataforma**, para treinar xadrez com experiência próxima à de uma mesa física, mantendo as vantagens de um ambiente digital configurável.

**Execute esta especificação sem substituir requisitos por suposições.** Leia primeiro as decisões de produto, o glossário e as prioridades. Planeje entregas verticais que possam ser executadas e demonstradas; escreva testes antes das implementações críticas; produza evidência verificável para cada critério de aceite. Não declare algo concluído por existir uma tela ou por compilar. Não implante nem distribua código de terceiros em desacordo com sua licença.

### Regras de atuação

1. Preserve **todas as decisões aprovadas** e indique claramente qualquer proposta nova como `PROPOSTA`.
2. Em conflito entre requisitos antigos e decisões mais recentes, aplique a decisão mais recente, **sem apagar o histórico**; documente a resolução.
3. Separe `DECIDIDO` (aprovado), `BASELINE` (alvo previamente estabelecido), `PROPOSTA` (implementação sugerida) e `PENDENTE` (não decidido). Uma proposta não é aprovação automática.
4. **P0, V1 e V2 são marcos distintos.** Não antecipe funcionalidades da V2 nem use recursos de V1 como condição oculta para declarar o P0 pronto.
5. O usuário deve poder usar o Chess Studio como **mesa física virtual** sem setas, bolinhas, fantasmas, sugestões, painel lateral ou outras facilidades que não existiriam na mesa real.
6. Não misture **legalidade de uma jogada**, **interface de movimentação** e **política de arbitragem**. São subsistemas independentes e testados separadamente.
7. Não permita que animações, arrasto, câmera, perda de foco ou painéis alterem indevidamente a posição canônica.
8. Em modo competitivo, não ofereça assistência incompatível com a modalidade nem permita apagar infrações ou desfazer lances efetivos como se fosse estudo.
9. Trabalhe **offline durante o jogo e o estudo**. Rede pode ser usada apenas para ações explícitas, como baixar/atualizar engines e aplicações.
10. Registre decisões técnicas irreversíveis em ADRs, mantenha linguagem de domínio consistente em `GLOSSARY.md` e gere uma matriz de rastreabilidade requisito → código → teste → evidência.
11. Se um requisito não puder ser implementado fielmente no escopo, apresente risco, alternativa, impacto e critério de homologação; não simule sucesso.
12. Não invente repositório, código, benchmark ou resultado de teste. Esta especificação foi elaborada a partir da **conversa**, sem auditoria de um código Chess Studio já existente.

---

## 1. Visão do produto e objetivo pedagógico

O Chess Studio **não é apenas um jogo de xadrez em 3D** nem apenas mais um cliente de engine. É uma **mesa pessoal de treinamento** para que o enxadrista aprenda a jogar presencialmente, mesmo quando não tem outra pessoa, técnico ou treinador ao lado.

Problemas reais que deve resolver:

- Em programas online/2D, indicações de casas e movimentos legais podem criar dependência visual; na mesa física essas ajudas não existem.
- O usuário quer aprender a **manipular peças e memorizar posições**, reconhecer lances legais e desenvolver confiança para partidas presenciais.
- Precisa treinar **situações de regra**, sobretudo finais, afogamento, xeque e situações de erro.
- Quer jogar contra o computador **offline**, com diferentes níveis de rigor, incluindo simulação regulamentar e derrota por primeira infração como desafio personalizado.
- Quer **salvar, encontrar, reproduzir e estudar partidas** mais tarde, com dados locais e exportação interoperável.
- Deseja interface visualmente agradável, GPU acelerada, com **painéis, tela cheia e HUD sobreposto** controláveis pelo próprio usuário.

**Métrica de sucesso do produto:** o usuário evolui até conseguir jogar em tabuleiro físico **sem depender dos auxílios do software**. A interface deve permitir retirar gradualmente esses auxílios.

### Princípios não negociáveis

- **Fidelidade física opcional:** mesa discreta, peças Staunton, câmera natural, manipulação precisa, controles sem adornos por padrão no perfil físico.
- **Ajuda inteiramente configurável:** tecnologia disponível, nunca obrigatória.
- **Estado canônico único:** três apresentações visuais não criam três jogos diferentes.
- **Offline-first e privacidade local:** SQLite embarcado, sem login ou servidor para jogar.
- **Modularidade:** engine de xadrez, renderização, UI, arbitragem e banco evoluem sem acoplamento desnecessário.
- **Confiabilidade:** regras corretas, salvamento recuperável, sem lances fantasmas, testes automatizados e benchmarks reprodutíveis.

---

## 2. Decisões consolidadas, prioridade e histórico de escopo

| ID | Decisão | Situação | Entrega |
|---|---|---|---|
| DEC-01 | Aplicativo desktop nativo para Windows, Linux e macOS; sem exigir navegador, conta ou servidor para partidas. | DECIDIDO | P0 |
| DEC-02 | Rust como linguagem principal; arquitetura candidata Bevy + wgpu + egui; SQLite. | DECIDIDO como direção; versões exatas a validar | P0 |
| DEC-03 | Experiência 3D profissional, variante **mesa imersiva discreta**, e 2D técnico; mesmo estado de partida. | DECIDIDO | P0 |
| DEC-04 | Mesa física discreta **sem sala 3D completa**; foco em madeira, peças, iluminação, sombras e ergonomia. | DECIDIDO | P0 |
| DEC-05 | Regras completas de xadrez, adversário local por engine UCI, movimentação e salvamento. | DECIDIDO | P0 |
| DEC-06 | SQLite embarcado desde P0, incluindo eventos de jogada, relógio, infrações e preferências. | DECIDIDO | P0 |
| DEC-07 | Exportação PGN e uso de FEN no P0; biblioteca/estudo ampliados na V1. | DECIDIDO | P0/V1 |
| DEC-08 | Três políticas de arbitragem: Assistido, Simulação FIDE e Eliminação Imediata personalizada. | DECIDIDO | P0 |
| DEC-09 | Auxílios visuais independentes da política de arbitragem e da forma de mover peças. | DECIDIDO | P0 |
| DEC-10 | Suportar dois toques (origem → destino) e arrastar/soltar, permitindo também 'ambos'. | DECIDIDO | P0 |
| DEC-11 | No modo FIDE, diferenciar toque intencional, peça solta e conclusão de jogada pelo relógio; sem punir clique acidental. | DECIDIDO | P0 |
| DEC-12 | Motores como processos externos UCI; **não distribuir binários de terceiros junto ao aplicativo** antes de decidir licenciamento. | DECIDIDO | P0 |
| DEC-13 | Gerenciador completo de instalação/atualização/configuração de múltiplos engines. | DECIDIDO | V1 |
| DEC-14 | Painéis modulares, F11/tela cheia, modo Foco, layouts e HUD opcionais. | DECIDIDO | V1 |
| DEC-15 | Treinos de regras, finais e memória; mensurar erros e permitir revisão/repetição. | DECIDIDO | P0 (mínimo) / V1 (ampliação) |
| DEC-16 | Súmula manual, conferência da súmula e impressão A4; **não** fazer no P0/V1. | DECIDIDO | V2 |
| DEC-17 | Jogos online, conta, servidor, multiplayer remoto, sala 3D completa e IA generativa não são necessários para o primeiro escopo. | DECIDIDO como exclusão do P0 | Fora do P0 |
| DEC-18 | A primeira rodada de cinco decisões do método Grill with Docs foi integralmente aprovada. | DECIDIDO | Projeto |
| DEC-19 | UX deve suportar 'jogo físico sem dicas' e revisão posterior com explicação. | DECIDIDO | P0 |
| DEC-20 | P0 é protótipo **funcional e homologável**, não uma renderização estática. | DECIDIDO | P0 |

**Resolução explícita da evolução de escopo:** a formulação inicial do P0, que deixava engine, PGN completo e treino avançado para depois, foi substituída nas decisões posteriores **aprovadas**. Agora o P0 inclui **adversário por UCI, exportação PGN, SQLite e treinamento mínimo**. Painéis ricos, HUD avançado e gerenciamento completo de engines continuam na V1. A classificação do 3D pode ser tratada como **duas apresentações/presets do mesmo renderizador 3D** (profissional e mesa imersiva) mais 2D técnico; não é necessário criar dois motores gráficos independentes.

**Preferência de experiência:** a mesa imersiva discreta é a direção estética escolhida pelo usuário. O aplicativo deve permitir configurar qual visual é aberto por padrão; quando não houver preferência armazenada, usar preset 3D profissional com acesso imediato à mesa imersiva, conforme a especificação inicial. Não presumir que o usuário quer uma sala de jogo cenográfica.

---

## 3. Histórias de usuário

| ID | História / benefício | Marco |
|---|---|---|
| US-01 | Como enxadrista, quero abrir um tabuleiro sobre mesa realista para me sentir praticando fisicamente. | P0 |
| US-02 | Como enxadrista, quero alternar entre 3D, mesa imersiva e 2D técnico sem perder a posição. | P0 |
| US-03 | Como enxadrista, quero mover peças por dois toques ou por arrasto e escolher meu método preferido. | P0 |
| US-04 | Como enxadrista, quero desligar todas as indicações de destino para não depender delas em torneios presenciais. | P0 |
| US-05 | Como iniciante, quero ativar dicas e explicações para aprender movimentos legais. | P0 |
| US-06 | Como competidor, quero simular peça tocada e relógio para treinar a disciplina de uma partida presencial. | P0 |
| US-07 | Como competidor, quero escolher penalidade FIDE ou eliminação imediata para variar o rigor de treino. | P0 |
| US-08 | Como estudante, quero praticar finais, afogamento e erros de regras, com revisão posterior. | P0 |
| US-09 | Como enxadrista sozinho, quero jogar contra engine UCI instalado na máquina sem internet. | P0 |
| US-10 | Como estudante, quero salvar partidas e retomar depois, sem perder jogadas ou infrações. | P0 |
| US-11 | Como enxadrista, quero exportar PGN e carregar posições FEN para compartilhar e revisar. | P0 |
| US-12 | Como estudante, quero comparar erros, memorizar posições e repetir exercícios. | P0/V1 |
| US-13 | Como usuário, quero configurar qualidade gráfica para minha GPU sem perder precisão de interação. | P0 |
| US-14 | Como usuário, quero configurar tamanho/visibilidade dos painéis e restaurar o layout. | V1 |
| US-15 | Como usuário, quero F11 e modo Foco para remover distrações. | V1 |
| US-16 | Como estudante, quero sobreposições estilo HUD/realidade aumentada que eu possa ligar e desligar. | V1 |
| US-17 | Como estudante, quero consultar análise do engine sem interferir em partidas competitivas. | V1 |
| US-18 | Como usuário, quero instalar/atualizar múltiplos engines com verificação de segurança e compatibilidade. | V1 |
| US-19 | Como usuário, quero biblioteca pesquisável de partidas e posições favoritas. | V1 |
| US-20 | Como competidor, quero praticar o preenchimento manual de súmula e conferi-la depois. | V2 |

---

## 4. Escopo por versão e definição de pronto

### P0 — Protótipo jogável, offline e testável

**Obrigatório:**

- Executável local Windows/macOS/Linux; inicializa sem internet, conta, API ou servidor.
- Tabuleiro xadrez padrão, três apresentações: 3D profissional, preset de mesa imersiva discreta e 2D técnico.
- Modelos Staunton, materiais coerentes, iluminação elegante, mesa discreta e câmera ajustável.
- Movimento por clique origem/destino e drag-and-drop (seleção por peça ou casa), sem alterar ilegalmente a posição.
- Configurações independentes de fantasmas, destinos legais, destaques de seleção, último lance e alertas.
- Regras de xadrez e estado de partida completos; políticas Assistido, FIDE clássico **delimitado** e Eliminação Imediata.
- Relógio virtual para políticas competitivas, toque intencional, soltura, confirmação e infração.
- Adversário offline com **um engine externo UCI funcional** instalado/configurado no equipamento. Se não houver engine, informar claramente como localizar/configurar; não prometer adversário sem ele.
- SQLite: partidas, eventos necessários à reconstrução, relógio, infrações, exercícios mínimos e preferências.
- FEN para posições, notação algébrica/histórico, **exportação PGN**, desfazer/refazer **somente nas modalidades permitidas**.
- Modo de treino mínimo de regras/finais: ao menos casos de afogamento, mate, xeque e movimento ilegal, com feedback conforme a configuração, replay e registros de erros.
- Testes, benchmarks e evidência nos três sistemas.

### V1 — Estação de estudo e interface configurável

- Painéis independentes de notação, anotações de estudo, relógio, material capturado, análise, biblioteca, treinamento e engines.
- Layout de painéis persistente, redimensionamento/reposicionamento em áreas predefinidas, presets de interface.
- `F11` quando suportado pelo sistema, alternativa via menu e modo Foco independente da tela cheia.
- HUD/realidade aumentada **virtual** opcional: setas, casas atacadas, sugestões, destaque de ameaça, avaliação e transparência.
- Biblioteca pesquisável e navegação rica pela partida; importação e variações, após decisão de formato e UX.
- Gerenciador de múltiplos engines: detectar, instalar, atualizar, configurar, remover de maneira segura, verificar hash/assinatura se disponíveis e licenças.
- Análise avançada e métricas pedagógicas expandidas, perfis visuais e de treinamento.
- Perfis de qualidade GPU avançados e recursos Ultra opcionais, se não comprometerem a compatibilidade.

### V2 — Backlog explícito, excluído de P0 e V1

- Súmula manual como numa competição física, treino de escrita/notação sem mostrar a resposta, comparação contra lances registrados e identificação de erros.
- Impressão da súmula em A4 e fluxo de conferência; exportação PGN já existe desde P0.
- Outros recursos futuros **não aprovados para implementação agora** devem entrar como propostas separadas.

### Excluídos do P0

Multiplayer online, backend, contas, SSO, sincronização em nuvem, ranking público, IA conversacional, sala 3D completa, cenografia complexa, explorador online de aberturas, gerenciador rico de engines, editor completo de árvores de variações, HUD sofisticado e súmula manual.

---

## 5. Glossário canônico de domínio (base de `GLOSSARY.md`)

- **Partida:** sequência de lances e eventos regulamentares entre lados Branco e Preto, com resultado e contexto de treinamento/competição.
- **Posição canônica:** estado oficial reconhecido pelo núcleo de regras, incluindo tabuleiro, lado a jogar, direitos de roque, en passant e contadores pertinentes.
- **Posição visual provisória:** disposição renderizada durante um arrasto ou tentativa não concluída; **não** altera a posição canônica.
- **Seleção:** gesto de foco de uma peça/casa na interface; por si só não é necessariamente toque intencional FIDE.
- **Toque intencional:** ação explícita de manipulação que pode vincular o jogador à peça no modo regulamentar.
- **Jogada feita:** movimento executado no tabuleiro segundo a semântica definida pela modalidade; não equivale sempre a jogada concluída no relógio.
- **Jogada concluída:** etapa regulamentar que encerra a oportunidade do jogador, normalmente associada ao acionamento do relógio, ressalvadas situações previstas nas leis.
- **Tentativa ilegal:** ação de movimentação não permitida pelo estado e pelas regras; pode existir provisoriamente para arbitragem, sem entrar no histórico legal.
- **Infração:** ocorrência registrada pela política de arbitragem; contém causa, instante, jogador e consequência.
- **Política de arbitragem:** Assistido, Simulação FIDE ou Eliminação Imediata; independente da política de apresentação.
- **Auxílio visual:** marcação, fantasma, destaque, seta ou informação adicionada ao tabuleiro.
- **HUD:** sobreposição de treinamento/análise ligada às casas/peças, não uma realidade aumentada física com câmera externa.
- **Preset visual:** conjunto configurável de câmera, materiais, qualidade, mesa e painéis.
- **Engine:** processo externo de xadrez que recebe estado/comandos e retorna movimentos ou análise, preferencialmente UCI.
- **FEN:** representação textual de uma posição; **não substitui** histórico completo da partida.
- **PGN:** formato interoperável de sequência de jogadas e metadados de partida.
- **Súmula manual:** registro escrito pelo jogador como exercício de competição; recurso V2, diferente da notação automática.

---

## 6. Arquitetura recomendada e fronteiras de responsabilidade

### Stack base

| Função | Tecnologia candidata | Decisão / cuidado |
|---|---|---|
| Linguagem e organização | Rust, `cargo`, workspace | Principal stack aprovada; modularizar por responsabilidades. |
| Renderização nativa 3D | Bevy (versão candidata 0.20) | Padrão ECS, câmera, luzes, animação, picking. Congelar versões por lockfile. |
| Backend GPU | `wgpu` | Priorizar DirectX 12 em Windows, Vulkan em Linux e Metal em macOS, conforme suporte real. |
| UI nativa | `egui` e ponte compatível `bevy_egui` | Painéis, menus, relógio, HUD e preferências. Verificar versão cruzada antes de travar dependências. |
| Docking V1 | `egui_tiles` ou equivalente | Usar somente se integração e UX passarem no spike técnico. |
| Regras | Biblioteca Rust testada (`cozy-chess` candidata) | Fazer prova de legalidade/perft antes de decidir; o núcleo é autoridade. |
| Políticas competitivas | Módulo próprio de arbitragem | Distinguir regras FIDE de modo customizado. Implementar por estado/eventos. |
| Engines | UCI em processos filhos | Stockfish CPU, Lc0 GPU onde suportado; crash não pode travar jogo/UI. |
| Banco local | SQLite + `rusqlite` candidato | Transações, migrações, backup e recuperação. |
| Arquivos | FEN/PGN/JSON de preferências se útil | SQLite é fonte persistente; exportações são interoperabilidade, não segunda verdade. |
| Modelos/ativos | Blender + glTF 2.0, PBR | Peças Staunton, tabuleiro, mesa, texturas licenciadas. |
| Empacotamento | `cargo-dist` ou equivalente | Validar instaladores, assinatura conforme plataforma e dependências de sistema. |
| Teste/CI | Cargo test, integração e GitHub Actions | Linux/Windows/macOS, artefatos e evidências auditáveis. |

**Importante:** versões acima são **candidatas discutidas**, não autorização para ignorar atualizações de compatibilidade. A equipe deve validar o conjunto exato no Gate A e produzir ADR se a alteração for material.

### Separação lógica (não confundir módulos com executáveis)

```text
Entrada (mouse/teclado)   Interface/painéis   Câmera/HUD   Renderização 2D/3D
          \                   |                 |              /
           +------------------+-----------------+-------------+
                              |
                  Controlador da sessão
                /             |              \
        Núcleo de regras   Arbitragem     Serviço de treinamento
                \             |              /
                 +--------Eventos----------+
                              |
                Persistência SQLite / PGN / FEN
                              |
                   Gerenciador externo UCI
                    (processos isolados)
```

A integração não obriga SQLite a intermediar todas as chamadas do engine. Comunicação UCI deve ocorrer em worker/processo com timeouts e filas, preservando a responsividade; persistência de eventos deve ser transacional. O gráfico não decide legalidade. Na análise por Lc0, limitar uso de GPU ou pausar/ajustar análise quando afetar FPS.

### Contratos e invariantes

- `GameState` canônico (posição, histórico, relógios, modo, resultado) possui uma única autoridade de escrita.
- `MoveIntent` vem da UI; `MoveValidation` consulta núcleo; `ArbitrationDecision` determina aceitação, obrigação de correção, penalidade, fim ou necessidade de confirmação.
- `RenderState` pode exibir peça arrastada, prévia e animações sem mutar `GameState`.
- Eventos persistidos permitem reconstruir a sessão; identificadores estáveis para partida, movimento, incidente e exercício.
- UI comunica ações sem conhecer detalhes de UCI/SQLite; banco não contém referências diretas a entidades gráficas.
- Desfazer/refazer só em estudo/treino que o permita; em torneio, ações são bloqueadas e infrações permanecem no registro.
- Ativos 3D e renderização não bloqueiam a thread de entrada; análise de engine roda isolada.

---

## 7. Requisitos de jogo e regras (P0)

### 7.1 Núcleo regulamentar

- Movimento de todas as peças; xeque, xeque-mate e afogamento.
- Roque pequeno e grande, inclusive restrições sobre xeque, casas atacadas e direitos perdidos.
- En passant com janela legal e atualização de direitos.
- Promoção para dama, torre, bispo ou cavalo; comportamento quando o usuário não escolhe promoção.
- Empates: afogamento, posição morta, repetição e regras de 50/75 movimentos, com distinção entre reclamação e encerramento automático, quando cabível.
- Histórico de posição inclui metadados suficientes para direitos de roque, en passant, repetições e relógio.
- FEN válido para importar/montar posição, inclusive treinamento; PGN exportado de jogadas legais.
- Um motor UCI **não substitui** o núcleo de legalidade nem a arbitragem. Comandos incorretos do engine são rejeitados/logados.

### 7.2 Três políticas independentes

**A. Assistido:** o movimento ilegal não altera a partida. A peça retorna à origem; destinos legais/explicações aparecem somente conforme preferências. Não há penalidade competitiva.

**B. Simulação FIDE (P0: clássico como perfil regulamentar principal):** simular peça tocada intencionalmente, soltura e acionamento do relógio. O relógio e as regras de conclusão devem seguir a versão FIDE definida, inclusive exceções quando a partida termina por mate/empate antes do acionamento. Depois de jogada ilegal **concluída**, restaurar a última posição regular; na **primeira infração concluída do mesmo jogador**, conceder **2 minutos ao adversário**; na **segunda**, declarar derrota, salvo empate se o adversário não puder dar mate por qualquer sequência legal. Não ensinar que todo toque acidental ou tentativa cancelada é infração concluída. A obrigação da peça tocada e regras especiais de captura, roque e promoção devem seguir a referência oficial.

**C. Eliminação Imediata:** regra **personalizada de treinamento**, nunca rotulada como FIDE: primeira infração concluída segundo a etapa de confirmação escolhida encerra com derrota (ou empate nas situações em que a configuração regulamentar expressamente exigir; documentar a política). Não confundir com a primeira irregularidade antes de acionar o relógio.

**Rápido e blitz:** têm nuances sobre penalidade, presença de árbitro, reclamação e correção; em P0 não anunciar equivalência completa a todos os formatos sem implementação/teste. Tratar como perfil expandido ou requisito de homologação adicional quando houver escopo aprovado. Referência oficial: https://handbook.fide.com/chapter/e012023 , especialmente artigos 4, 5, 6 e 7 e apêndices A e B. Revisar a edição vigente antes de lançar.

### 7.3 Estados de interação para fidelidade física

```text
IDLE -> SELECTED -> HELD / DRAGGING -> RELEASED_PENDING
  |         |             |                 |
  +--ESC----+------CANCEL+                 +--(confirmar relógio)--> ARBITRATION
                                                 | legal      | ilegal
                                                 v            v
                                              COMMIT      RESTORE/PENALTY
```

- `SELECTED` não é automaticamente `TOUCHED_INTENTIONALLY`.
- Um clique para avaliar uma casa pode permanecer inocente no modo Assistido; o modo FIDE exige gesto explícito/documentado para tocar intencionalmente.
- `RELEASED_PENDING` é visualmente provisório até regra de conclusão aplicável; o tabuleiro lógico não pode ficar com estado inválido confirmado.
- Esc, perda de foco, interação com painel, início da órbita e mudança de janela cancelam arrastos **não concluídos**, conforme modalidade, sem gravar lance fantasma.
- Se houver obrigação de peça tocada, cancelar gesto não significa necessariamente apagar a obrigação; decidir segundo regra da modalidade e evidenciar no teste.
- Peça promovida, captura, roque e movimento com relógio têm transições especiais testadas.

---

## 8. UX visual, mesa, câmera e controles (P0)

### Três apresentações com mesmo estado

1. **3D profissional:** tabuleiro madeira, peças Staunton realistas, perspectiva natural, iluminação elegante; referência inicial de apresentação.
2. **Mesa imersiva discreta:** tabuleiro apoiado numa mesa, sombras suaves, fundo neutro/discreto ou desfocado; sem sala 3D, móveis adicionais ou cenografia pesada; estética preferida para o objetivo físico.
3. **2D técnico:** tabuleiro plano, coordenadas claras, foco em precisão e histórico; adequado a estudo.

**Transição:** nenhuma mudança em turno, direitos de roque/en passant, histórico, relógios, eventos e resultado quando alternar visuais. Preferências de câmera específicas de cada visual podem ser preservadas separadamente.

### Câmera e seleção

- Botão central pressionado + movimento: órbita da câmera ao redor do tabuleiro; suporte a 360 graus, sem arrastar peças.
- Pan (mover enquadramento), zoom por roda/gesto, vista superior, oblíqua, invertida e reset; controles configuráveis quando possível.
- Limites de zoom e ângulo evitam atravessar a mesa/perder tabuleiro; inversão da vista não inverte casas lógicas.
- Raycasting/picking de peça e casa com IDs estáveis; todas as 64 casas selecionáveis de qualquer preset relevante.
- Destino por toque origem → destino; drag-and-drop; ambos. Cancelar com Esc, perda de foco ou mouse fora não gera movimentos acidentais.
- Animação de elevação suave da peça durante o arrasto e encaixe natural ao soltar, sem atrasar o controle.

### Preferências configuráveis individualmente

| Recurso | Opções essenciais | Observação |
|---|---|---|
| Destinos legais (pontos/casas) | ligado/desligado | Deve desaparecer completamente em modo físico. |
| Peças fantasmas nos destinos | ligado/desligado | Independente dos pontos. |
| Peça selecionada | destaque ligado/desligado | Feedback mínimo de foco acessível. |
| Último lance | ligado/desligado | Nunca obrigatório. |
| Indicação de xeque | ligado/desligado | Regras não mudam ao desligar. |
| Explicação de erro | imediata / após tentativa / somente revisão | Em modo competitivo, respeitar política. |
| Método de movimento | dois toques / drag / ambos | Persistir por perfil. |
| Peça tocada | ligado/desligado quando a modalidade permite | Não desligar arbitragem mandatória do preset regulamentar. |
| Política de arbitragem | Assistido / FIDE / Eliminação Imediata | Separada da apresentação. |
| Visualização | 3D profissional / mesa / 2D | Preferências persistentes. |
| Qualidade GPU | Econômico / Equilibrado / Ultra opcional | Medir desempenho real. |

**Exemplo obrigatório:** na posição inicial, ao selecionar peão branco em e2, as casas e3/e4 só aparecem se a indicação pertinente estiver ligada e se estiverem livres. Se o usuário tentar e2 → e5, o lance é ilegal; não corrigir para e4 silenciosamente, e aplicar a política de arbitragem correspondente.

---

## 9. Painéis, tela cheia e HUD (V1)

### Painéis independentes

| Painel | Função |
|---|---|
| Notação | Notação algébrica automática, lista de lances, navegação. |
| Anotações de estudo | Comentários e variações fora de partida competitiva. |
| Relógio | Tempos, incrementos, turno ativo, penalidades. |
| Análise | Avaliação, linha principal, profundidade do engine. |
| Peças capturadas | Material capturado e saldo. |
| Biblioteca | Pesquisa por partidas, posições e exercícios no SQLite. |
| Treinamento | Exercício, dificuldade, erros, histórico e explicação. |
| Engines | Seleção, instalação, atualização, estado e preferências. |

- Exibição/ocultação individual, redimensionamento, abas e encaixe em regiões predefinidas; layout salvo por perfil.
- Em F11, a janela ocupa a tela inteira quando suportado. Fornecer alternativa de menu em plataformas que interceptem F11 (incluindo algumas configurações macOS).
- **Tela cheia** e **modo Foco** são estados diferentes: F11 gerencia apresentação; Foco esconde painéis e barras, preservando apenas tabuleiro e controles mínimos configurados.
- HUD permanece opcional no modo Foco; mostrar setas de jogadas, casas atacadas, ameaças, dicas, linhas do engine, transparência e cores discretas.
- Os overlays seguem geometria das 64 casas em qualquer orientação, sem mudar a lógica de jogadas.
- Em modos competitivos, **bloquear** sugestões, linha do engine e dicas vedadas, mesmo que o usuário tenha um preset de HUD ativo.
- Nunca usar `realidade aumentada` como sinônimo de câmera física: aqui significa **sobreposição visual virtual em 3D**.

### Perfis iniciais de interface

- **Jogo Físico:** mesa 3D e zero dicas, painéis ocultos, relógio opcional/obrigatório conforme política.
- **Treinamento Assistido:** auxílios ligados, explicações e feedback configuráveis.
- **Estudo e Análise:** notação, biblioteca, variações, avaliação do engine.
- **Personalizado:** escolhas individuais permitidas pela modalidade.

---

## 10. Persistência SQLite, formatos e recuperação

### Responsabilidade dos dados

Banco local guarda partidas, jogadores/lados, posições relevantes, lances legais, relógios, incidentes, preferências, exercícios, progresso e metadados dos engines. Formatos PGN/FEN são interfaces de importação/exportação e reconstrução, não fonte de verdade concorrente.

### Modelo mínimo lógico (não impor SQL antes do spike)

- `profiles`: perfil local e preferências do usuário.
- `games`: ID estável, modo, controle de tempo, engine e resultado, timestamps.
- `moves`: sequências e notação, posição antes/depois ou referência para reconstrução, lado, tempos registrados, ordenação.
- `game_events`: toque intencional, tentativa ilegal, relógio acionado, penalidade, restauração, fim e motivo, com tipo e payload versionados.
- `positions`: FEN, referência à partida/exercício, marcadores.
- `exercises`: tipo (afogamento, mate, xeque, legalidade, memória), estado inicial, solução e versão.
- `attempts`: tentativa, acerto/erro, dicas usadas, duração e revisão.
- `ui_preferences`: câmera, perfil visual, auxílios, método de entrada, painéis da V1.
- `engine_installations`: nome, versão, caminho, protocolo, checksum quando conhecido, status (V1 com instalador completo).

**Implementação recomendada:** migrações versionadas, transações para alterações críticas, chaves estrangeiras, índices de busca, WAL quando apropriado, autosave e procedimento de backup/restauração. Não salvar binário de engine no SQLite.

### Casos mínimos

- Salvar → fechar → abrir → retomar com mesma posição, lado, histórico, relógios, preferência visual e infrações.
- Retomar uma partida interrompida e reconstruir jogo com direito de roque/en passant correto.
- Abrir FEN arbitrário válido em exercício; rejeitar FEN inválido sem corromper banco.
- Exportar PGN com metadados e notação corretos; importar/validar exportado em ferramenta de referência (sem exigir importador rico dentro do P0).
- Queda durante escrita não deve gerar partida parcialmente corrompida ou perda de consistência; transação e recuperação.
- Migração de schema mantém dados antigos; copiar arquivo do banco para backup sem deixar escrita concorrente inconsistente.

---

## 11. Engines UCI, instalação e recursos de CPU/GPU

### Estratégia aprovada

- O Chess Studio se comunica com motores **externos** via protocolo **UCI**.
- Stockfish (CPU) é referência inicial; Lc0 pode usar GPU conforme backend/driver e configuração. Não assumir que todo engine neural funciona em toda GPU.
- **Não embutir nem distribuir por padrão executáveis de terceiros no P0** enquanto licenciamento/distribuição não forem decididos.
- Deve existir caminho claro para localizar/configurar um engine já instalado; se não houver, modo local de dois jogadores/estudo continua disponível, e mensagem explica o bloqueio para jogar contra a máquina.
- Segurança: executáveis externos são software de terceiros; não executar downloads sem consentimento, preferir fontes oficiais e validar integridade quando distribuída no futuro.
- UCI inclui inicialização, `uci`, `isready`, opções, posição, `go`, `stop`, `bestmove`, timeouts, erros e encerramento do processo; log limitado de diagnósticos.
- Engine **não pode** comandar jogada ilegal; rejeitar e mostrar erro recuperável.
- UI e banco não travam durante busca; engine com falha, timeout ou fechamento não derruba o Chess Studio.
- Para engines que usam GPU, prover limitação de recursos/prioridade ou orientação ao usuário; benchmark mede impacto simultâneo em renderização.

### Fronteira P0 / V1

**P0:** integração **de pelo menos um engine UCI externo**, seleção inicial de executável, verificação de compatibilidade e jogo offline; UX mínima de configuração. **PENDENTE Q6:** modo de primeira execução sem engine (ver seção 17).

**V1:** catálogo/gerenciador de diversos engines, detecção, downloads com confirmação, instalação, atualização, configuração de threads, hash, parâmetros, níveis, diferentes executáveis CPU/GPU e remoção segura. Definir política de licença e atualização antes de distribuir binários.

---

## 12. Treinamento, exercícios e métricas

### Treino mínimo P0

1. **Jogo livre offline contra engine**, com três políticas de arbitragem.
2. **Prática sem dicas**: desligar destinos legais, fantasmas e indicações de xeque/último lance.
3. **Exercícios de regra**: ao menos posição de afogamento, xeque-mate, xeque, movimento ilegal de peão, roque e promoção com feedback configurável.
4. **Memória espacial básica**: reproduzir uma posição/pequena sequência depois de uma apresentação, ou registrar proposta de implementação equivalente com teste.
5. **Revisão da tentativa**: registrar erro, posição, causa e política, permitir repetir após fim do exercício; não adulterar resultado competitivo original.

### Indicadores aprovados

- Quantidade e tipos de erros de regra/infrações por sessão.
- Percentual de exercícios concluídos sem dicas e tipos de auxílio usados.
- Acerto de reprodução de posições e sequências de memória.
- Desempenho em exercícios de finais (incluindo erros por afogamento).
- Evolução por período/perfil, sem afirmar causalidade pedagógica sem teste de uso.

### Regras de feedback

- Assistido pode dar explicação imediata ou diferida conforme opção.
- Treino físico pode não exibir nenhum auxílio durante a jogada; ao final oferecer uma revisão clara, especialmente afogamento vs mate.
- Competição: análise e feedback de engine desabilitados enquanto partida estiver ativa; somente depois do resultado, a revisão pode apresentar dicas e erros.
- Não confundir `jogada ruim` com `jogada ilegal`: avaliação estratégica de engine não muda legalidade.

---

## 13. Metas não funcionais, portabilidade e visual

### Desempenho — metas iniciais, não resultados comprovados

| Hardware de referência | Qualidade/resolução | Meta inicial |
|---|---|---|
| GPU dedicada | Equilibrado, 1080p | 60 FPS sustentados |
| GPU integrada | Econômico, 720p | 30 FPS sustentados |

O benchmark de cada modo tem duração **mínima de cinco minutos** e registra FPS, distribuição de frame time, RAM, VRAM quando disponível, CPU, GPU, sistema operacional, driver, modelo de GPU, resolução, preset e engine ativo/ocioso. Meta formal aprovada ou renegociada se equipamento-alvo não a sustentar. Medir especialmente latência entre ação de mouse e resposta visível, sem valores arbitrários tratados como contratados.

### Requisitos de qualidade

- PBR/material madeira correto; peças com leitura de forma, cor e silhueta; texturas e ativos legalmente utilizáveis.
- Sem sala 3D completa nem efeitos Ultra obrigatórios.
- Antialiasing, sombras e escalonamento adaptáveis à GPU.
- Janela redimensionável e DPI/escala de UI legíveis; texto e notação acessíveis por contraste e tamanho.
- Teclado: Esc cancela gesto apropriado; foco previsível; alternativa para operações básicas sem mouse quando tecnicamente possível.
- Compatibilidade testada Windows, Linux (incluindo diferenças X11/Wayland quando aplicáveis) e macOS; documentar diferenças de mouse e tecla F11.
- Falhas de banco, engine, GPU e arquivos geram mensagem útil, sem perda silenciosa de partidas.
- Totalmente jogável após configuração local com rede desligada.

---

## 14. Matriz única de critérios de aceite obrigatórios

**Status inicial de todos:** `UNVERIFIED` até teste executado. A tabela consolida **os IDs nomeados na conversa** e adiciona IDs específicos onde faltava identificador. Marcação `[P0]`, `[V1]` ou `[V2]` indica escopo.

### 14.1 Regras e estado

| ID | Marco | Critério testável |
|---|---|---|
| CORE-01 | P0 | Partida completa de dois lados executa todos os movimentos e encerra em mate/empate com resultado correto. |
| CORE-02 | P0 | Posição inicial obtém `perft` 20, 400, 8.902 e 197.281 para profundidades 1–4. |
| CORE-03 | P0 | Roques ilegais (xeque, casas atacadas, peça movida) são recusados; direitos persistem corretamente. |
| CORE-04 | P0 | En passant legal e ilegal coberto por testes e persistência. |
| CORE-05 | P0 | Promoção permite D/T/B/C, inclui promoção com captura e escolha pendente. |
| CORE-06 | P0 | Afogamento é empate: lado da vez sem lances legais, rei fora de xeque; não tratar como mate. |
| CORE-07 | P0 | Repetição, 50/75 movimentos e posição morta aplicam reclamação/encerramento de modo regulamentar. |
| CORE-08 | P0 | Engine UCI não pode alterar posição canônica com lance ilegal. |
| CORE-09 | P0 | Estado de turno, contadores e direitos sobrevive a reconstrução a partir da persistência. |
| CORE-10 | P0 | Desfazer/refazer permitido em estudo e bloqueado para alterar resultado de torneio. |

### 14.2 Visualizações, câmera e interação

| ID | Marco | Critério testável |
|---|---|---|
| VIS-01 | P0 | 3D profissional possui materiais, peças e iluminação coerentes com a referência de qualidade. |
| VIS-02 | P0 | Mesa imersiva discreta sem sala 3D completa, com desempenho medido. |
| VIS-03 | P0 | 2D técnico apresenta tabuleiro correto e coordenadas legíveis. |
| VIS-05 | P0 | Alternar 3D/mesa/2D preserva posição, turno, direitos, histórico, relógios e resultado. |
| CAM-01 | P0 | Arrastar botão central orbita câmera 360° e não move peças. |
| CAM-02 | P0 | Pan, zoom, reset, vista superior/oblíqua/invertida funcionam conforme mapeamento de controles. |
| CAM-03 | P0 | Rotação/inversão não modifica interpretação lógica de a1...h8. |
| CAM-04 | P0 | Todas as 64 casas são selecionáveis nas vistas superior, oblíqua, padrão e invertida. |
| INT-01 | P0 | Dois toques e drag-and-drop chegam à mesma jogada canônica quando válida. |
| INT-02 | P0 | Destino ilegal não vira jogada válida nem é convertido silenciosamente em outra. |
| INT-03 | P0 | Esc, perda de foco, painel ou mudança para órbita não geram lances fantasmas. |
| INT-04 | P0 | Preview de peça durante arrasto não altera estado canônico. |
| INT-05 | P0 | Animação de soltura/encaixe respeita destino sem deslocar peça para casa errada. |

### 14.3 Arbitragem — critérios da conversa `AC-01` a `AC-09`

| ID | Marco | Critério testável |
|---|---|---|
| AC-01 | P0 | Seleção de Assistido, FIDE e Eliminação Imediata antes da partida. |
| AC-02 | P0 | Política escolhida funciona identicamente em 2D e 3D. |
| AC-03 | P0 | Assistido rejeita lance ilegal sem mudar posição e explica somente se configurado. |
| AC-04 | P0 | Competição separa movimentar, soltar e concluir pelo relógio, com exceções regulamentares. |
| AC-05 | P0 | FIDE clássico: primeira infração concluída restaura posição e concede +2 min ao oponente; segunda perde ou empata na exceção de impossibilidade de mate. |
| AC-06 | P0 | Eliminação Imediata encerra na primeira infração concluída, rotulada como regra personalizada. |
| AC-07 | P0 | SQLite guarda modo, infrações, penalidades, resultado, motivos e histórico. |
| AC-08 | P0 | Tentativas ilegais não entram no PGN como lances legais, mas persistem no log de incidentes. |
| AC-09 | P0 | Testes cobrem xeque, roque ilegal, movimentos impossíveis, promoção, relógio e penalidades. |
| RULE-01 | P0 | Política e critérios são rastreáveis e possuem versão de regulamento/explicação no produto. |
| RULE-02 | P0 | Clique acidental não se transforma automaticamente em peça tocada intencionalmente. |
| RULE-03 | P0 | Obrigação da peça tocada, quando aplicável, persiste apesar de cancelamento de gesto de UI. |
| RULE-04 | P0 | Pressionar relógio sem jogar e situações equivalentes são tratadas conforme regras regulamentares aplicáveis. |
| RULE-05 | P0/V1 | Regras rápido/blitz só são anunciadas como homologadas após testes específicos de seus apêndices e condições de arbitragem. |

### 14.4 UX de ajuda — critérios da conversa `UX-01` a `UX-10`

| ID | Marco | Critério testável |
|---|---|---|
| UX-01 | P0 | Desligar todos os auxílios remove pontos, fantasmas, setas e destinos legais. |
| UX-02 | P0 | Dois toques e arrastar/soltar produzirem mesma jogada válida. |
| UX-03 | P0 | Configurações visuais não alteram legalidade nem penalidades. |
| UX-04 | P0 | e2–e5 inicial é identificado como ilegal e não entra no histórico legal. |
| UX-05 | P0 | FIDE distingue seleção, toque intencional, soltura e relógio. |
| UX-06 | P0 | Picking correto das 64 casas após qualquer rotação da câmera. |
| UX-07 | P0 | Trocar 2D/3D conserva posição, histórico e preferências aplicáveis. |
| UX-08 | P0 | SQLite recupera preferências depois de reiniciar. |
| UX-09 | P0 | Cancelamento/perda de foco não move peça acidentalmente. |
| UX-10 | P0 | Animações suaves preservam precisão e metas documentadas de performance. |

### 14.5 UI modular — critérios da conversa `UI-01` a `UI-12`

| ID | Marco | Critério testável |
|---|---|---|
| UI-01 | V1 | Cada painel é exibido/ocultado sem mudar estado da partida. |
| UI-02 | V1 | Layout, tamanhos e visibilidade dos painéis persistem no SQLite. |
| UI-03 | V1 | F11 alterna tela cheia quando disponível; menu alternativo funciona. |
| UI-04 | V1 | Modo Foco não interrompe relógio nem modifica partida. |
| UI-05 | V1 | Todas as dicas podem sumir, restando somente mesa, tabuleiro e peças. |
| UI-06 | V1 | HUD segue as 64 casas após câmera orbitar/inverter/zoom. |
| UI-07 | V1 | Notação automática corresponde aos lances legais registrados. |
| UI-08 | V1 | Painéis de engine/dicas proibidas ficam inacessíveis em competição. |
| UI-09 | V1 | Perfis de interface persistem e podem ser restaurados. |
| UI-10 | V1 | Reorganização responde a tamanhos de janela sem esconder controles essenciais. |
| UI-11 | V1 | 2D, 3D e mesa mantêm preferências aplicáveis. |
| UI-12 | V1 | Benchmarks com HUD/painéis validam metas ou renegociação formal. |

### 14.6 Persistência, motores, treino e plataforma

| ID | Marco | Critério testável |
|---|---|---|
| DB-01 | P0 | Salvar/reabrir partida restaura posição, histórico, modo, relógio e incidentes. |
| DB-02 | P0 | FEN válido carrega posição; FEN inválido gera erro sem corromper banco. |
| DB-03 | P0 | Exportação PGN pode ser lida/validada por ferramenta independente. |
| DB-04 | P0 | Eventos de tentativa ilegal e penalidades não são confundidos com jogadas legais. |
| DB-05 | P0 | Interrupção durante escrita não gera estado parcialmente persistido; existe recuperação demonstrada. |
| DB-06 | P0 | Migrações versionadas preservam dados e preferências. |
| UCI-01 | P0 | Engine externo configurado responde ao handshake UCI e joga offline. |
| UCI-02 | P0 | Timeout/crash não bloqueia UI nem danifica partida. |
| UCI-03 | P0 | Engine ilegal é rejeitado e causa erro recuperável. |
| UCI-04 | P0 | Sem engine externo, aplicativo informa o limite e permite estudo/dois jogadores locais. |
| UCI-05 | V1 | Múltiplos engines são detectados/instalados/atualizados com confirmação, fonte e integridade verificável. |
| UCI-06 | V1 | Análise Lc0/engine GPU não causa perda de responsividade sem aviso/controle de carga. |
| TRN-01 | P0 | Concluir treino de afogamento explica por que não é xeque-mate. |
| TRN-02 | P0 | Repetição do exercício após erro usa posição original correta. |
| TRN-03 | P0 | Exercício sem dicas mantém registro do auxílio usado/desligado. |
| TRN-04 | P0 | Ao menos um exercício de memória registra resultado da reprodução. |
| TRN-05 | P0 | Histórico do treino quantifica erros e acertos sem alterar resultados competitivos. |
| PLAT-01 | P0 | Compila e executa com instalável/documentação nos três sistemas. |
| PLAT-02 | P0 | Jogo continua sem login, internet ou serviço externo. |
| PERF-01 | P0 | Benchmark dedicado 1080p Equilibrado mira 60 FPS conforme máquina homologada. |
| PERF-02 | P0 | Benchmark integrado 720p Econômico mira 30 FPS conforme máquina homologada. |
| PERF-03 | P0 | Teste 5 min registra FPS/frame time/RAM/VRAM/CPU/GPU/OS/drivers. |
| PERF-04 | P0 | Medir desempenho com e sem engine ativo, documentando interferência. |
| V2-SUM-01 | V2 | Súmula manual suporta anotação sem mostrar resposta pronta. |
| V2-SUM-02 | V2 | Conferência compara súmula manual com lances reais. |
| V2-SUM-03 | V2 | Impressão A4 e revisão de erros, sem bloquear P0/V1. |

**Regra de avaliação da matriz:** cada item deve terminar com `PASS`, `FAIL`, `UNVERIFIED` ou `N/A` justificado, vinculado a teste, plataforma, commit, evidência e responsável. Duplicidades de teste entre IDs são permitidas, mas nenhum ID pode ficar sem classificação.

---

## 15. Cenários BDD obrigatórios

### BDD-01 — Peão e dica desligada

```gherkin
Dado que uma partida inicia na posição padrão
E destinos legais e peças fantasmas estão desligados
Quando seleciono o peão em e2
Então não aparece marcação de e3 ou e4
E posso tentar mover a peça sem qualquer indicação visual
Quando tento concluí-la em e5
Então a tentativa não se converte em lance legal
E a consequência é determinada exclusivamente pela política de arbitragem
```

### BDD-02 — Troca de visual

```gherkin
Dado que existe uma partida com jogadas, direitos de roque, relógio e incidentes
Quando alterno 3D profissional -> mesa imersiva -> 2D -> 3D
Então os dados canônicos e o turno permanecem idênticos
E os incidentes continuam no registro
E a configuração de câmera é restaurada segundo o preset
```

### BDD-03 — Primeira infração FIDE clássico concluída

```gherkin
Dado um jogo clássico em simulação FIDE com zero infrações do jogador
Quando ele faz tentativa ilegal e a conclui na etapa regulamentar do relógio
Então a posição regular é restaurada
E o adversário recebe dois minutos
E uma infração concluída é registrada
E o jogador continua sujeito à obrigação de peça tocada quando aplicável
```

### BDD-04 — Segunda infração FIDE e exceção

```gherkin
Dado um jogador com uma infração ilegal concluída anterior
Quando ele conclui uma segunda jogada ilegal
Então a partida termina em derrota por infração
Exceto se o adversário for incapaz de dar mate por qualquer sequência legal
Nesse caso o resultado é empate, registrado com motivo
```

### BDD-05 — Treino de afogamento

```gherkin
Dado um exercício em que o jogador pode provocar afogamento
E a assistência visual está desativada
Quando o lance legal deixa o adversário sem lances legais e fora de xeque
Então o resultado é empate por afogamento
E a revisão explica a diferença para xeque-mate
E permite repetir o exercício sem reescrever a tentativa original
```

### BDD-06 — Jogo offline e engine indisponível

```gherkin
Dado que a internet está desligada
Quando inicio o Chess Studio com engine UCI previamente configurado
Então consigo jogar normalmente contra o computador
Quando simulo a falha do processo UCI
Então o aplicativo permanece responsivo
E preserva a partida
E apresenta recuperação ou escolha de engine alternativa
```

### BDD-07 — Persistência com eventos

```gherkin
Dado uma partida com lances legais, tempo de relógio e tentativa ilegal
Quando salvo, fecho e reabro o programa
Então recupero a posição canônica e o histórico legal
E a ocorrência ilegal permanece registrada separadamente
Quando exporto PGN
Então apenas jogadas legais e metadados pertinentes entram na sequência
```

### BDD-08 — Cancelamento de arrasto

```gherkin
Dado que comecei a arrastar uma peça em 3D
Quando pressiono Esc ou a janela perde foco antes de concluir a jogada
Então nenhuma jogada fantasma é criada
E a posição canônica permanece correta
E eventual obrigação regulamentar de peça tocada é preservada quando aplicável
```

### BDD-09 — Painel F11 e modo Foco (V1)

```gherkin
Dado que a partida está em andamento
Quando ativo F11 ou o controle equivalente de tela cheia
Então não altero turno, relógio ou posição
Quando ativo o modo Foco
Então painéis ficam ocultos conforme preferências
E apenas os HUDs permitidos pela política atual podem permanecer
```

---

## 16. Plano de implementação por gates e entregáveis

### Gate A — Prova de viabilidade técnica (bloqueante)

- Criar workspace Rust, lockfile, estrutura de módulos e CI.
- Compilar Bevy/wgpu/egui em **Windows/macOS/Linux** de fato, não apenas cross-compilar.
- Renderizar tabuleiro, 32 peças iniciais, luzes, mesa discreta e preset 2D.
- Validar picking de todas as casas, ID estável, mouse botão central, pan/zoom/rotação e cancelamentos.
- Medir primeira linha de base gráfica (GPU dedicada e integrada quando disponíveis).
- Provar a integração UCI com um engine externo e tolerância a falha, nem que inicialmente por CLI.

**Aprovação:** sem bloqueadores de versão, GPU, input ou compatibilidade conhecidos sem mitigação; relatório com vídeos/capturas e comandos de build/teste.

### Gate B — Núcleo jogável e histórico

- Implementar engine de regras testado, estado canônico, relógio, arbitragem e eventos.
- Testar perft e casos de xeque, en passant, roque, promoção, mate, afogamento e repetições.
- Integrar UI 3D/2D ao controlador e métodos de clique/arrasto.
- Integrar UCI para jogo local, configuração simples e recuperação de falha.
- SQLite com migração, autosave, PGN e FEN, importação de posição e reabertura.
- Implementar treino mínimo com afogamento, regras e memória.

**Aprovação:** a pessoa joga sozinha offline, salva e estuda; arbitragem comprovada por BDD e testes.

### Gate C — Experiência visual e V1

- Melhorar materiais, iluminação, qualidade de peças e câmera; zero objetos supérfluos na mesa.
- Adicionar painéis modulares, F11, modo Foco, presets, biblioteca avançada e HUDs.
- Gerenciador de múltiplos engines com segurança e licença.
- Melhorar exercícios, métricas pedagógicas e recursos de análise após partida.

**Aprovação:** UX com e sem auxílios, acessibilidade, bloqueios de dicas em torneio, persistência de layout, testes em plataformas.

### Gate D — Homologação e liberação

- Rodar suíte completa de regras, BDD, persistência e UCI em Windows/Linux/macOS.
- Benchmarks reproduzíveis de cinco minutos com todos os perfis homologados; registrar valores reais, não prometer metas sem máquina de referência.
- Revisar licenças de componentes/assets/engines; conferir documentação, instalador, procedimento de atualização e recuperação.
- Auditoria final da matriz de rastreabilidade, relatório de riscos e smoke test offline.
- Somente então propor release com versão, notas e evidência de homologação.

### Plano de tickets verticais (sequência sugerida)

1. `CHESS-001` — Bootstrap Rust/Bevy/wgpu/egui, CI e compilação multiplataforma.
2. `CHESS-002` — Tabuleiro 3D, modelos, câmera e picking de 64 casas.
3. `CHESS-003` — Núcleo de regras, perft, serialização de posição.
4. `CHESS-004` — Input toque/drag e estado provisório sem lances fantasmas.
5. `CHESS-005` — Arbitragem Assistido/FIDE/Eliminação + relógio + BDD.
6. `CHESS-006` — UCI externo e primeiro jogo offline vs engine.
7. `CHESS-007` — SQLite, eventos, histórico, FEN e exportação PGN.
8. `CHESS-008` — Três visuais sincronizados e presets de ajuda.
9. `CHESS-009` — Exercícios de regras, finais/afogamento e memória.
10. `CHESS-010` — Benchmarks, testes nas três plataformas e homologação P0.
11. `CHESS-011` — V1 painéis, F11 e modo Foco.
12. `CHESS-012` — V1 HUD/engine analysis/biblioteca.
13. `CHESS-013` — V1 gerenciador de engines, distribuição segura e licença.
14. `CHESS-014` — V1 homologação completa.
15. `CHESS-015` — V2 súmula manual e sua conferência (somente backlog).

Não executar tudo em paralelo indiscriminadamente. Definir dependências, um dono por contrato, testes de integração cedo e stop conditions para falhas reincidentes.

---

## 17. Pendências e decisões que NÃO devem ser inventadas

### PENDENTE Q6 — Primeiro uso sem engine

A primeira rodada de cinco perguntas da entrevista `grill-with-docs` foi **aprovada integralmente**. A **sexta pergunta** foi proposta, mas **não foi respondida** na conversa até este arquivo:

> Como garantir o adversário offline no primeiro uso sem distribuir executável de terceiro junto ao aplicativo?

**Proposta para aprovação:** assistente mínimo que localiza/configura o Stockfish ou outro UCI já instalado; se ausente, usuário recebe orientação para instalação explícita. Um fluxo de download integrado e atualização fica para V1. O P0 pode manter modo de dois jogadores locais/estudo enquanto o engine não estiver presente, sem prometer jogo contra máquina nesse estado.

### Outras decisões em aberto

| ID | Questão | Direção sugerida, ainda não aprovada |
|---|---|---|
| PEND-01 | Produto aberto, fechado ou inicialmente privado? | Formalizar licenciamento e distribuição antes de incluir terceiros. |
| PEND-02 | Versões finais exatas da stack? | Fixar versões após Gate A, respeitando compatibilidade Bevy/egui e drivers. |
| PEND-03 | Máquina mínima para perfis de 60/30 FPS? | Definir placa de referência, CPU, RAM e SO; publicar hardware/testes. |
| PEND-04 | Gestos para 'toque intencional' em mouse e trackpad? | Projetar protótipo e validar ergonomia física; não punir seleção acidental. |
| PEND-05 | Qual política detalhada de incidente em rápido e blitz? | Implementar apenas após mapear apêndices e cenário de arbitragem. |
| PEND-06 | Metadados/estrutura detalhada do banco e importação PGN completa? | Adotar modelo evolutivo e testes de migração; importação rica V1. |
| PEND-07 | Qual conjunto mínimo e sequência de exercícios de memória? | Aferir em teste com usuário, preservar objetivo de jogar fisicamente. |
| PEND-08 | Interfaces específicas para instalação/atualização de engines e fontes confiáveis? | Elaborar design de segurança antes da V1. |
| PEND-09 | Ativação permanente da skill `grill-with-docs` no ambiente da equipe? | Setup por repositório; não confundir pacote local com instalação efetiva. |

**Instrução:** registrar cada decisão aprovada em ADR quando for difícil de reverter e consequência de trade-off; perguntas ainda abertas podem ser tickets de decisão, nunca aceitação presumida.

---

## 18. Método Matt Pocock / Grill with Docs e governança

O usuário pediu revisão de arquitetura e PRD usando a metodologia da skill **`grill-with-docs` de Matt Pocock**. A versão acessada no repositório oficial funciona combinando uma entrevista iterativa por rodadas e modelagem do domínio.

**Aplicação aprovada nesta conversa — Rodada 1:**

1. **P0 delimitado** com engine UCI, modos 2D/3D, três arbitragem, auxílios configuráveis e SQLite; painéis ricos/HUD/gestão completa de engines na V1.
2. **Peça tocada** distinta de seleção, soltura e relógio; cliques acidentais não podem produzir punição arbitrária.
3. **Engines externos UCI**, sem distribuir terceiros junto ao app até decisão de licenciamento.
4. **SQLite + registro de eventos + PGN exportável no P0**; súmula manual V2.
5. **Métricas pedagógicas** de regras, jogo sem assistência, memória e finais.

**Forma de revisão:** elaborar árvore de decisões; perguntar em rodadas apenas o que depende de decisões já estabilizadas; dar recomendação que possa ser aprovada; depois atualizar glossário e ADR; não inundar o usuário com detalhes que a equipe pode pesquisar sozinha. Não inventar aprovação para Q6 ou demais pendências.

**Fontes:** [Repositório oficial das skills](https://github.com/mattpocock/skills), [Grill with Docs](https://github.com/mattpocock/skills/blob/main/skills/engineering/grill-with-docs/SKILL.md), [Grilling](https://github.com/mattpocock/skills/blob/main/skills/productivity/grilling/SKILL.md), [Domain Modeling](https://github.com/mattpocock/skills/blob/main/skills/engineering/domain-modeling/SKILL.md).

### ADRs candidatos para a equipe

- `ADR-0001`: desktop nativo offline Rust, sem servidor necessário — trade-off contra Electron/web.
- `ADR-0002`: separação entre estado canônico de regras, representação visual provisória e arbitragem — evita lances fantasmas.
- `ADR-0003`: SQLite/eventos como persistência primária, FEN/PGN como formatos de interoperabilidade.
- `ADR-0004`: engines externos por UCI, licenciamento separado e tolerância a falhas.

Nenhum ADR está automaticamente aceito só por constar aqui; escrever contexto, decisão e motivo em arquivo próprio quando a equipe confirmar a implementação.

---

## 19. Evidências mínimas e formato de relatório da equipe

Ao entregar cada gate, devolver:

```text
CHESS_STUDIO_GATE: A | B | C | D
STATUS: PASS | FAIL | BLOCKED
REPO / BRANCH / COMMIT:
VERSOES: Rust / Bevy / egui / wgpu / SQLite / Engines
SISTEMAS_TESTADOS: Windows / Linux / macOS (SO, CPU, GPU, driver)
ITENS_IMPLEMENTADOS: IDs da matriz
ITENS_PENDENTES: IDs da matriz + justificativa
TESTES_EXECUTADOS: comando, total, aprovados, falhas, relatório
PERFT: profundidades 1–4 e resultados
FIDE: versão normativa e cenários executados
PERSISTENCIA: migrações, retomada, PGN/FEN, falha simulada
ENGINES: UCI, timeout/crash, uso CPU/GPU
PERFORMANCE: resolução, preset, duração, FPS/frame time/RAM/VRAM/CPU/GPU
VISUAL: capturas/videos em 2D, 3D e mesa; picking em 64 casas
LICENCAS: projeto, bibliotecas, modelos, engines (distribuição)
DECISOES_PENDENTES: questão, proposta, impacto
RISCO_RESIDUAL: severidade, mitigação, dono
RESULTADO: GO | NO-GO | REPLANEJAR
```

**Critério de conclusão:** todos os critérios bloqueantes do marco têm evidência `PASS`; nenhum defeito crítico/alto aberto; três plataformas homologadas; benchmarks validados ou renegociados formalmente; documentação e licença prontas. `UNVERIFIED` não equivale a `PASS`. Não autorizar V1 nem declarar P0 pronto com base somente em captura de tela ou apresentação 3D.

---

## 20. Referências normativas, técnicas e rastreabilidade das fontes

- **FIDE, Laws of Chess (vigentes a partir de 1º de janeiro de 2023, edição consultada em novembro de 2026):** https://handbook.fide.com/chapter/e012023 — artigos 4.3 (peça tocada), 4.7 (peça solta), 5.2.1 (afogamento), 6.2 (relógio), 7.5.1/7.5.5 (infração concluída/penalidades), apêndices A/B (rápido/blitz). Verificar se há revisão posterior antes do release.
- **UCI / motores:** consultar documentações oficiais de Stockfish e Lc0 para suporte, opções, hardware, licenças e protocolo. Não inferir que Lc0 usa a GPU em todas as plataformas.
- **Rust/Bevy/wgpu/egui:** validar compatibilidade de versões e backends no Gate A. Números de versão citados na reunião são hipótese de implementação, não prova de build.
- **Histórico da conversa:** escopo visual inicial, sua evolução para treinador offline, preferências de UX, simulação física/FIDE, SQLite, motores UCI, painéis/HUD, metas gráficas, súmula V2 e rodada de cinco decisões aprovada. Esta é a base vinculante do produto; material externo serve a precisão normativa e técnica, não para substituir decisões do usuário.

---

## 21. Comando final à equipe

1. Entregue primeiro um **plano de execução** curto, com módulo responsável, contratos, tickets verticais, riscos e dependências.
2. Apresente o **Gate A demonstrável**, antes de investir em HUD avançado ou integração de servidor.
3. Implemente o P0 com **TDD**, FIDE regulamentar delimitado, experiências com e sem ajuda, banco, UCI e treino básico.
4. Conduza **homologação real**, com logs, vídeos, medidas, evidências e classificação de todos os IDs.
5. Submeta pendências de arquitetura/produto à próxima rodada `grill-with-docs`; **não transforme recomendações em decisões sem aprovação**.
6. Conclua V1 depois de P0 homologado e mantenha a **súmula manual exclusivamente no backlog V2**.

**Resultado esperado:** um treinador de xadrez offline e multiplataforma, visualmente convincente, fiel ao aprendizado no tabuleiro físico, configurável desde zero auxílio até estudo avançado, com estado correto, engines independentes, histórico recuperável e critérios de aceite auditáveis.

