# Roteiro de homologação do RC-1

Roteiro para você testar o RC-1 (`v0.1.0`, release candidate 1) e marcar PASS, FAIL ou N/A. A mesma lista vive no artefato **Partida do Chess Studio**, seção Homologação do RC-1, onde cada item tem passos, critérios de aceite marcáveis, campos de escolha e de texto, e o botão para baixar o JSON. Este arquivo é a versão para ler e versionar; o resultado que vale é o que você marcar no artefato.

## Antes de começar

- **O pacote.** O pre-release [`v0.1.0-rc1`](https://github.com/ChicoFigueiredo/Chess-Studio/releases/tag/v0.1.0-rc1) foi gerado pela CI a partir do commit `c8aa3d5`, que tem o mesmo código da `main` em `0820e53`. O build local do RC-1 sai do commit `0820e53` e fica em `dist/`. Os dois valem, mas use só um em cada passada e anote qual no item RC1-AMB.
- **Perfil local novo.** Crie um (Arquivo > Perfil local) para o roteiro, para não misturar com o uso diário. Para testar a primeira abertura, use `--db /tmp/rc1.db`.
- **Contas.** Para os itens do grupo 6, use **contas de teste**, nunca a principal: uma do Lichess sem Partidas importantes, uma segunda para desafiar e, só para a Engine online, uma **nova** (a conversão em usuário BOT não tem volta). O app nunca pede a sua senha; a autorização é feita no navegador.
- **Monitor de rede.** `ss -tnp | grep chess-studio` no Linux; `Get-NetTCPConnection` no Windows; `lsof -i` no macOS.
- **Como marcar.** PASS só se todos os critérios de aceite do item foram atendidos. FAIL abre uma issue (o relatório seguinte lista). N/A vale quando o item não se aplica ao seu sistema ou à sua conta, com a razão na observação. Nada aqui vira PASS por teste automatizado: a suíte já cobre a lógica, e este roteiro cobre o que ela não alcança.
- **Os detalhes passo a passo** das integrações estão em [integracoes.md](integracoes.md) (CX-HOM-01 a 83); a ordem por risco, em [verificar-com-servico-real.md](../usabilidade/verificar-com-servico-real.md).

## Resumo

| Grupo | Itens | Risco mais alto |
|---|---|---|
| 0 · Preparar o teste | RC1-01 a RC1-03 (3) | Base |
| 1 · Instalar e rodar sem rede | RC1-04 a RC1-07 (4) | Base |
| 2 · Jogar | RC1-08 a RC1-15 (8) | Integridade |
| 3 · Interface | RC1-16 a RC1-22 (7) | Integridade |
| 4 · Treino, Súmula e Biblioteca | RC1-23 a RC1-26 (4) | Conferência |
| 5 · Provedores sem conta | RC1-27 a RC1-32 (6) | Integridade |
| 6 · Provedores com conta de teste | RC1-33 a RC1-44 (12) | Irreversível |
| 7 · Fechamento | RC1-45 a RC1-46 (2) | Base |

Total: 46 itens.

## Grupo 0 · Preparar o teste

Ambiente, pacote e primeira abertura. Faça antes de tudo.

### RC1-01. Ambiente e pacote testado

**Critérios:** RC1-AMB · **Risco:** Base

Passos:

1. Anote o sistema, a placa de vídeo e qual pacote você vai testar: o do GitHub (v0.1.0-rc1) ou o build local desta máquina.
2. Baixe o pacote e o SHA256SUMS na mesma pasta e confira: `sha256sum -c SHA256SUMS --ignore-missing` (Windows: `Get-FileHash`; macOS: `shasum -a 256`).
3. Para o build local: `dist/` no repositório, mesmo comando.

Critérios de aceite:

- [ ] O hash do pacote bate com o SHA256SUMS.
- [ ] Você sabe qual pacote está testando e vai usar só ele no resto do roteiro.

Registre:

- Sistema: Linux / Windows / macOS
- Pacote: Build local (esta máquina) / AppImage / .deb / tar.gz / Windows setup.exe / Windows zip / macOS dmg
- Vídeo: Placa dedicada / Integrada / Máquina virtual / Por software
- Modelo da máquina e versão do sistema: ______

Detalhe: [docs/instalacao.md#conferir-o-download](../instalacao.md#conferir-o-download)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-02. Primeira abertura com banco novo

**Critérios:** CX-HOM-01 · DB-01 · **Risco:** Base · **Precisa de:** Monitor de rede (ss -tnp)

Passos:

1. Abra com um banco vazio: `chess-studio --db /tmp/rc1.db` (build local: `cargo run --release -p chess-studio -- --db /tmp/rc1.db`).
2. Com o app aberto, rode `ss -tnp | grep chess-studio` e olhe a barra de status.
3. Feche e abra de novo com o mesmo `--db`.

Critérios de aceite:

- [ ] A janela abre sem erro, com o Ribbon no alto, o dock à direita e a barra de status no pé.
- [ ] A barra de status diz **Rede: offline** e o `ss` não mostra conexão do app.
- [ ] Nenhuma Engine foi baixada nem executada sozinha.
- [ ] O segundo `--db` reabre o mesmo Perfil local, sem perder nada.

Registre:

- Segundos até a janela aparecer: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-01-offline-por-padrão-e-estado-da-rede](integracoes.md#cx-hom-01-offline-por-padrão-e-estado-da-rede)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-03. Dados de demonstração

**Critérios:** CX-HOM-02 · US-29 · **Risco:** Conferência

Passos:

1. Abra com `--demo-data`.
2. Percorra Arquivo > Conexões, a aba Biblioteca e o Painel Explorador.
3. Feche e confira que o seu banco de verdade não mudou.

Critérios de aceite:

- [ ] O Perfil Demonstração aparece com Partidas de várias Origens e três Conexões fictícias, nenhuma autorizada.
- [ ] Nenhum Provedor vem ligado e a barra de status segue **Rede: offline**.
- [ ] Ao fechar, nada da demonstração sobra no seu banco.

Detalhe: [docs/integracoes/estado-atual.md#104-dados-de-demonstração-e-roteiro-de-usabilidade-existe](../integracoes/estado-atual.md#104-dados-de-demonstração-e-roteiro-de-usabilidade-existe)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 1 · Instalar e rodar sem rede

PLAT-01 e PLAT-02, nos sistemas que você tem.

### RC1-04. Linux: instalar e abrir

**Critérios:** PLAT-01 · **Risco:** Base · **Precisa de:** Linux · **Ticket:** #57

Passos:

1. AppImage: `chmod +x chess-studio-0.1.0-linux-x86_64.AppImage && ./chess-studio-0.1.0-linux-x86_64.AppImage`.
2. .deb: `sudo apt install ./chess-studio_0.1.0_amd64.deb`, abra pelo menu de aplicativos.
3. tar.gz: extraia e rode `./chess-studio` sem separá-lo da pasta `assets`.
4. Desinstale o .deb (`sudo apt remove chess-studio`) e confira a pasta de dados.

Critérios de aceite:

- [ ] O app abre pelo menos por um dos três formatos, com ícone e entrada no menu no caso do .deb.
- [ ] O 3D renderiza (tabuleiro visível, sem tela preta).
- [ ] Desinstalar não apaga as suas Partidas.

Registre:

- Formatos que funcionaram: Todos / AppImage e .deb / Só AppImage / Só .deb / Só tar.gz / Nenhum
- Erro ou aviso que apareceu: ______

Detalhe: [docs/instalacao.md#linux](../instalacao.md#linux)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-05. Windows: instalar e abrir

**Critérios:** PLAT-01 · **Risco:** Base · **Precisa de:** Windows 10 ou 11 · **Ticket:** #24

Passos:

1. Execute `chess-studio-0.1.0-windows-x86_64-setup.exe`. No SmartScreen: Mais informações > Executar assim mesmo.
2. Abra pelo Menu Iniciar. Teste também o zip portátil, se quiser.
3. Desinstale por Configurações > Aplicativos e veja se as Partidas continuam em `%APPDATA%`.

Critérios de aceite:

- [ ] O instalador não pede administrador e cria o atalho no Menu Iniciar.
- [ ] O app abre e o 3D renderiza (DirectX 12 ou Vulkan).
- [ ] Não aparece erro de `VCRUNTIME140.dll`.
- [ ] Desinstalar não apaga as suas Partidas.

Registre:

- Renderização: 3D normal / 3D preto / Só 2D / Não abriu
- Erro ou aviso que apareceu: ______

Detalhe: [docs/instalacao.md#windows](../instalacao.md#windows)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-06. macOS: instalar e abrir

**Critérios:** PLAT-01 · **Risco:** Base · **Precisa de:** Mac com chip Apple · **Ticket:** #24

Passos:

1. Abra o `.dmg` e arraste o Chess Studio para Aplicativos.
2. Na primeira abertura, libere pelo Gatekeeper: Ajustes do Sistema > Privacidade e Segurança > Abrir Mesmo Assim (macOS 15) ou Control + clique > Abrir (12 a 14).
3. Teste trackpad: dois dedos orbita, pinça dá zoom.

Critérios de aceite:

- [ ] O app abre depois de liberado uma vez, e depois abre com dois cliques.
- [ ] O 3D renderiza (Metal).
- [ ] Os gestos de trackpad da câmera funcionam.

Registre:

- Versão do macOS: ______
- Erro ou aviso que apareceu: ______

Detalhe: [docs/instalacao.md#macos](../instalacao.md#macos)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-07. Jogar sem rede (smoke offline)

**Critérios:** PLAT-02 · **Risco:** Integridade · **Ticket:** #57

Passos:

1. Desligue a rede do computador (Wi-Fi e cabo).
2. Abra o app e jogue uma Partida de dois jogadores até o fim.
3. Resolva um Exercício. Se tiver uma Engine local, jogue contra ela.

Critérios de aceite:

- [ ] Nada trava, nem mostra erro de rede.
- [ ] A barra de status diz **Rede: offline**.
- [ ] A Partida fica na Biblioteca depois de terminada.

Registre:

- Engine local testada: Sim, funcionou / Sim, falhou / Não tenho Engine

Detalhe: [docs/instalacao.md#conferir-que-funciona-offline-plat-02](../instalacao.md#conferir-que-funciona-offline-plat-02)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 2 · Jogar

Núcleo de regras, Políticas, relógio, Engine local.

### RC1-08. Mover peças: dois toques, arrasto e cancelar

**Critérios:** INT-01..05 · UX-02/04/09/10 · **Risco:** Conferência

Passos:

1. Jogue e2-e4 com dois cliques e depois e7-e5 arrastando.
2. Arraste uma peça e aperte Esc no meio; solte uma peça fora do tabuleiro.
3. Tente e2 para e5 (movimento impossível) e depois solte a peça sobre uma casa ilegal.

Critérios de aceite:

- [ ] Os dois gestos geram o mesmo Lance.
- [ ] Cancelar o arrasto não cria Lance e a peça volta.
- [ ] e2 para e5 é recusado e nunca vira e4.
- [ ] Arrasto e encaixe têm boa sensação, sem tranco.

Registre:

- Sensação do arrasto: Ótima / Boa / Regular / Ruim

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-09. Promoção, mate e fim de Partida

**Critérios:** CORE-01 · CORE-05 · #141 · **Risco:** Conferência

Passos:

1. Carregue uma posição perto da promoção (Arquivo > FEN) ou jogue uma Partida até o mate.
2. Promova um peão e escolha a peça no seletor.
3. Dê o xeque-mate e veja o diálogo de fim.

Critérios de aceite:

- [ ] O seletor de promoção oferece as quatro peças.
- [ ] O diálogo de fim mostra Revisar, Nova Partida e Exportar PGN, e o rei tomba.
- [ ] O resultado gravado na Biblioteca é o certo.

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-10. Três Apresentações, Vistas e as 64 casas

**Critérios:** VIS-01/02/03 · CAM-01..04 · UX-06 · **Risco:** Conferência · **Ticket:** #24

Passos:

1. Aperte Tab para alternar 3D profissional, Mesa imersiva e 2D técnico.
2. Use 1, 2, 3, 4 e 0 para as Vistas; oriente com o botão do meio e dê zoom com a roda.
3. Clique nas 4 casas de canto e no centro em cada Vista.

Critérios de aceite:

- [ ] Trocar a Apresentação não muda a Partida.
- [ ] Orbitar não move peça e 0 volta ao padrão.
- [ ] Todas as casas respondem em todas as Vistas.

Registre:

- Apresentação de que mais gostou: 3D profissional / Mesa imersiva / 2D técnico
- Aprovação visual (VIS-01/02): Aprovo / Aprovo com ajustes / Não aprovo

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-11. Simulação FIDE: peça tocada, Ajuste e Infrações

**Critérios:** RULE-02/03 · AC-04/05 · BDD-03/04/08 · **Risco:** Integridade

Passos:

1. Nova Partida na Política Simulação FIDE. Toque num cavalo e solte sem mover; tente mover outra peça.
2. Alt + clique numa peça faz um Ajuste, que não obriga.
3. Faça um Lance ilegal e veja a Infração; aperte o relógio e deixe a bandeira cair.

Critérios de aceite:

- [ ] A peça tocada obriga o Lance e outra peça é recusada.
- [ ] O Ajuste não obriga; Esc e trocar de janela no meio do gesto não apagam a obrigação.
- [ ] A Infração é registrada e a bandeira dá o resultado correto.

Registre:

- As mensagens ficaram claras?: Sim / Em parte / Não

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-12. Eliminação Imediata, Assistido e Auxílios

**Critérios:** UX-01 · BDD-01 · Perfis de auxílios · **Risco:** Conferência

Passos:

1. Jogue na Política Eliminação Imediata e faça um Lance ilegal de propósito.
2. No perfil Jogo físico, procure qualquer Auxílio na tela.
3. Troque para Treinamento e Personalizado e veja a explicação de erro.

Critérios de aceite:

- [ ] Eliminação Imediata termina a Partida no primeiro Lance ilegal.
- [ ] Em Jogo físico não aparece nenhum Auxílio (HUD, setas, dica, avaliação).
- [ ] Treinamento e Personalizado ligam só o que foi escolhido.

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-13. Fechar e reabrir retoma a Partida

**Critérios:** DB-01 · DB-02 · DB-03 · UX-08 · **Risco:** Integridade

Passos:

1. Jogue 6 Lances. Feche a janela e abra de novo.
2. Jogue mais 2 Lances e mate o processo (`kill -9`; no Windows, Gerenciador de Tarefas) sem fechar.
3. Abra de novo. Depois carregue um FEN, exporte o PGN e importe-o de volta.

Critérios de aceite:

- [ ] A Partida volta no mesmo ponto, com relógio e Política.
- [ ] Depois de matar o processo, no máximo o Lance em curso se perde, nunca a Partida.
- [ ] O PGN exportado reimporta com os mesmos Lances.

Registre:

- Quantos Lances se perderam ao matar o processo: Nenhum / 1 / Mais de 1

Detalhe: [docs/atualizacao-e-recuperacao.md](../atualizacao-e-recuperacao.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-14. Engine local: assistente, Partida e consentimento

**Critérios:** UCI-01/04 · BDD-06 · ADR-0003 · **Risco:** Integridade · **Precisa de:** Stockfish ou outra Engine UCI

Passos:

1. Abra o gerenciador de Engines e registre o Stockfish (instalado ou baixado pelo assistente).
2. Antes de aceitar, leia o aviso de execução e a licença.
3. Jogue contra a Engine algumas jogadas. Feche e reabra a Partida.

Critérios de aceite:

- [ ] O app nunca executa a Engine sem a sua confirmação.
- [ ] O download (se usado) mostra a licença e confere o SHA-256.
- [ ] Ao reabrir, a Engine só conecta depois de **Conectar Engine e retomar**.
- [ ] Os Lances da Engine são sempre legais.
- [ ] Elo abaixo do piso da Engine (Stockfish: 1320): peça 400, e depois 1000, em Nova Partida; a tela diz "Elo N aproximado" e a Engine joga visivelmente pior que a 1500.

Registre:

- Engine e versão usadas: ______
- Engine em 400 comparada à de 1500: Bem mais fraca / Um pouco mais fraca / Igual / Não testei
- Quanto você acha que ela joga no 400 (Elo estimado): ______

Detalhe: [docs/decisoes/seguranca-engines.md](../decisoes/seguranca-engines.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-15. Análise: barra de avaliação e Stop

**Critérios:** US-17 · UI-08 · #296 · #325 · **Risco:** Integridade · **Precisa de:** Engine local · **Ticket:** #296, #325

Passos:

1. Abra a Análise com uma Engine registrada e deixe calcular uns 5 segundos.
2. Pare a análise e confira o uso de CPU (`top`, Monitor de Atividade ou Gerenciador de Tarefas).
3. Repita ligar e parar umas 20 vezes, rápido.
4. **Modo de estudo (#325).** Com a Análise ligada e o Explorador e o painel Aberturas abertos, numa Partida Assistida com uns 6 meios-lances jogados, clique num Lance da Notação (Revisão) e mova uma peça no tabuleiro (Lance legal) para iniciar a Linha de estudo; faça mais dois Lances. Acompanhe a Engine, a barra de avaliação, as Aberturas e o Explorador (Biblioteca e, se tiver Provedor ligado, uma fonte online).
5. Volte um Lance na linha (botão ou seta, quando houver) e confira os painéis; depois saia do estudo (faixa "Estudo" ou Esc) e confira de novo.
6. Com o Modo de estudo ligado, inicie uma Partida online contra pessoa (ou uma Simulação FIDE): o estudo deve se encerrar e nenhum painel deve calcular com a posição dele.

Critérios de aceite:

- [ ] A barra de avaliação acompanha o tabuleiro.
- [ ] Depois de Parar, o processo da Engine cai para perto de zero de CPU.
- [ ] Em nenhuma das 20 vezes a Engine seguiu calculando depois do Stop (é o defeito possível da #296).
- [ ] No estudo, a Engine avalia a posição da Linha de estudo (não a da Partida) e a barra de avaliação acompanha cada Lance da linha.
- [ ] No estudo, as Aberturas (nome e estatísticas) e o Explorador seguem a posição da linha, e o Explorador mantém a Origem, o Período e o aviso de amostra.
- [ ] Clicar num Lance do Explorador, das Aberturas ou da linha da Engine, no estudo, acrescenta o Lance à Linha de estudo (não abre variante) e a Partida não muda.
- [ ] Ao sair do estudo, Engine, barra, Aberturas e Explorador voltam à posição da Partida.
- [ ] Com uma Partida competitiva ou online iniciada, o estudo se encerra e nada calcula com a posição dele (Trava de fair play).

Registre:

- Stop perdido: Nunca / Aconteceu algumas vezes / Aconteceu sempre
- Se aconteceu: o que você estava fazendo: ______

Detalhe: [docs/integracoes/estado-atual.md#74-trava-de-fair-play-existe](../integracoes/estado-atual.md#74-trava-de-fair-play-existe)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 3 · Interface

Ribbon, Painéis, Foco, Escala, Temas e persistência (#316).

### RC1-16. Ribbon, Paleta de Ações e KeyTips

**Critérios:** UI-NOVA-02..09 · UI-NOVA-20 · **Risco:** Conferência

Passos:

1. Percorra as abas Arquivo, Início, Análise, Biblioteca, Treino e Exibir.
2. Ctrl+K e digite "nova"; Alt e as letras; Ctrl+F1; dois cliques numa aba.
3. Abra Opções pela Paleta e percorra as categorias.
4. Com uma Partida de uns 6 Lances, use ← → Home End e os botões do cabeçalho da Notação; depois clique numa nota rápida e digite com as setas.
5. Modo de estudo na Notação (#323): revise um Lance do meio e mova uma peça no tabuleiro. Confira a lista "Linha de estudo" abaixo da lista da Partida. Use ← → Home e os botões, clique num Lance da linha, depois End (ou o botão de posição atual). Repita clicando num Lance da lista da Partida. Teste também com o painel Notação estreito, no Tema claro e no escuro.

Critérios de aceite:

- [ ] Cada aba abre e cada botão responde.
- [ ] A Paleta acha Ações sem acento, mostra o motivo das desabilitadas e fecha com Esc.
- [ ] Dois cliques numa aba recolhem e expandem o Ribbon.
- [ ] Opções cobre o que o painel antigo fazia, sem item sem caminho.
- [ ] Na Notação, os quatro botões (início, anterior, próximo, posição atual) e as teclas ← → Home End navegam na Revisão; End volta ao vivo; digitando numa nota rápida, as setas não navegam (#321).
- [ ] Ao mover uma peça numa posição revista, aparece a lista "Linha de estudo" abaixo da lista da Partida (empilhada), com os Lances numerados a partir do Lance de partida e o Lance atual da linha destacado; a lista da Partida fica com o Lance de partida destacado (#323).
- [ ] Clicar num Lance da linha vai até ele; ← → Home e os botões andam pela linha sem descartá-la; End e o botão de posição atual encerram o estudo e a lista some; clicar num Lance da lista da Partida também encerra o estudo e mostra esse Lance (#323).
- [ ] A lista cabe no painel estreito, sem corte nem rolagem horizontal, nos dois Temas (#323).

Registre:

- Achei tudo o que procurei: Sim / Quase tudo / Não

Detalhe: [docs/homologacao/interface-nova.md](interface-nova.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-17. Dock, Painéis e janelas flutuantes

**Critérios:** UI-01/02 · UI-NOVA-11 · #183..#196 · **Risco:** Conferência

Passos:

1. Em Exibir, ligue e desligue Painéis; use Ocultar todos.
2. Arraste um Painel para fora (vira janela) e de volta ao dock.
3. Com 2 monitores (opcional), destaque um grupo para o segundo.

Critérios de aceite:

- [ ] Os Painéis entram e saem sem sobrepor o tabuleiro.
- [ ] A janela destacada mostra o Painel (não fica vazia) e volta ao dock.
- [ ] O layout continua usável em 1280x720.

Registre:

- Dois monitores: Testei, ok / Testei, falhou / Não testei

Detalhe: [docs/homologacao/interface-nova.md](interface-nova.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-18. Foco (F10) e Tela cheia (F11)

**Critérios:** UI-03/04/05 · UI-NOVA-15/16 · **Risco:** Conferência · **Ticket:** #55

Passos:

1. Aperte F10 e depois F10 de novo; aperte F11 e F11; combine os dois.
2. Em F11, mova o mouse para as bordas e redimensione ao sair.

Critérios de aceite:

- [ ] F10 esconde Ribbon, dock e barra de status e devolve a faixa ao tabuleiro; F10 de novo restaura.
- [ ] F11 entra e sai da tela cheia, sozinho e junto com o Foco, e a janela volta ao tamanho anterior.
- [ ] Nenhum atalho é interceptado pelo sistema.

Registre:

- F11 funcionou no seu sistema: Sim / Sim, com problema / Não

Detalhe: [docs/homologacao/interface-nova.md](interface-nova.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-19. Escala da interface (30% a 200%)

**Critérios:** UI-NOVA-21 · #315 · **Risco:** Conferência · **Ticket:** #315

Passos:

1. Em Opções, ajuste a Escala arrastando; ela só vale ao soltar.
2. Digite 150 na caixa; escolha 80% na lista; aperte Ctrl+0.
3. Feche e reabra o app.

Critérios de aceite:

- [ ] O valor só é aplicado ao soltar, de 5 em 5.
- [ ] Caixa de digitar, lista e Ctrl+0 funcionam.
- [ ] A Escala volta igual ao reabrir.

Registre:

- Escala que você prefere (%): ______

Detalhe: [docs/homologacao/interface-nova.md](interface-nova.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-20. Persistência ao reabrir (#316)

**Critérios:** UI-NOVA-22 · DB-01 · **Risco:** Integridade · **Ticket:** #316

Passos:

1. Mude: Tema para Claro, Densidade para Compacta, um Perfil de interface (ex.: Estudo e análise), um Painel aberto e fechado, o tamanho e a posição da janela, a Escala.
2. Feche o app normalmente (não mate o processo) e abra de novo. Anote o que voltou.
3. Se algo não voltou, repita só com esse ajuste isolado e anote os passos.

Critérios de aceite:

- [ ] Cada ajuste volta como estava.
- [ ] Se algum não voltar, está registrado qual e com quais passos (isso fecha as perguntas da #316).

Registre:

- Tema (Claro, Escuro, Sistema): Voltou / Não voltou / Não testei
- Densidade: Voltou / Não voltou / Não testei
- Perfil de interface: Voltou / Não voltou / Não testei
- Painéis abertos e fechados: Voltou / Não voltou / Não testei
- Tamanho e posição da janela: Voltou / Não voltou / Não testei
- Escala: Voltou / Não voltou / Não testei
- O que de Conexões não voltou, se algo: ______

Detalhe: [https://github.com/ChicoFigueiredo/Chess-Studio/issues/316](https://github.com/ChicoFigueiredo/Chess-Studio/issues/316)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-21. Temas, contraste e janela pequena

**Critérios:** UI-09/10/11 · UI-NOVA-18 · #203/#206 · **Risco:** Conferência · **Ticket:** #55

Passos:

1. Em Opções, abra a galeria e troque por 4 ou 5 Temas (claros e escuros).
2. Reduza a janela para 800x600 e para 1024x576.
3. Olhe a barra de status, o Explorador e a Central de Conexões nos Temas mais claros.

Critérios de aceite:

- [ ] Texto legível em todos os Temas e o fundo da cena segue o Tema.
- [ ] Em janela pequena, nada fica cortado nem inalcançável.
- [ ] O Ribbon passa ao modo simples sem perder Ações.

Registre:

- Tema que ficou ruim, se algum: ______

Detalhe: [docs/homologacao/interface-nova.md](interface-nova.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-22. Desempenho: benchmark de 5 minutos

**Critérios:** PERF-01..04 · UI-12 · UI-NOVA-19 · **Risco:** Conferência · **Precisa de:** Máquina ociosa · **Ticket:** #40

Passos:

1. Feche o resto do computador. Rode `chess-studio --benchmark` (build local: `--release -- --benchmark`).
2. Se tiver GPU integrada, repita nela em 720p Econômico.
3. Anote o FPS médio e o mínimo.

Critérios de aceite:

- [ ] O benchmark termina e grava o JSON e o CSV.
- [ ] O app se mantém fluido no cenário de análise e no de Partida.
- [ ] Os valores estão anotados (sem meta prometida; é a linha de base).

Registre:

- FPS médio: ______
- FPS mínimo: ______
- Hardware medido: ______

Detalhe: [docs/benchmark.md](../benchmark.md)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 4 · Treino, Súmula e Biblioteca

Exercícios, memória, Súmula manual e histórico.

### RC1-23. Treino: Exercícios e conteúdo

**Critérios:** TRN-01..05 · DEC-46 · **Risco:** Conferência · **Ticket:** #42

Passos:

1. Abra Treino e resolva um Exercício de afogamento e um de final.
2. Veja as métricas e os Auxílios do treino. Leia ao menos 5 dos 20 Exercícios de fábrica.
3. Confira a Notação: português ou inglês?

Critérios de aceite:

- [ ] O Exercício aceita só a solução e explica o erro.
- [ ] O conteúdo faz sentido e está correto nos que você leu.
- [ ] A Notação segue a que você escolheu.

Registre:

- Notação preferida: Português / Inglês
- Conteúdo dos Exercícios: Aprovo / Aprovo com correções / Reprovo
- Exercícios com erro (número e o que está errado): ______

Detalhe: [docs/homologacao/matriz-p0.md](matriz-p0.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-24. Treino: memória de sequência, de posição e Evolução

**Critérios:** TRN-04 · DEC-45 · US-25..27 · **Risco:** Conferência

Passos:

1. Faça a memória de sequência e a de posição.
2. Abra a Evolução e os Perfis de treino.

Critérios de aceite:

- [ ] O resultado de cada tentativa fica gravado.
- [ ] A Evolução mostra o histórico sem erro.
- [ ] Os Perfis de treino mudam o que o treino oferece.

Detalhe: [docs/homologacao/matriz-v1.md](matriz-v1.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-25. Súmula manual, conferência e impressão A4

**Critérios:** V2-SUM-01/02/03 · **Risco:** Conferência · **Precisa de:** Impressora ou PDF

Passos:

1. Ative a Súmula e jogue 10 Lances anotando só na Súmula, sem ver a resposta.
2. Faça a conferência contra os Lances reais.
3. Gere o A4 em PDF e veja o histórico de erros.

Critérios de aceite:

- [ ] A Súmula não mostra a resposta enquanto você anota.
- [ ] A conferência marca cada divergência.
- [ ] O A4 sai legível, em uma página, e o histórico de erros acumula.

Registre:

- Decisões que dependem de você (V2): ______

Detalhe: [docs/homologacao/matriz-v2.md](matriz-v2.md)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-26. Biblioteca, favoritas e Revisão

**Critérios:** US-19 · DB-02 · **Risco:** Conferência

Passos:

1. Abra a Biblioteca, busque por jogador e por Posição, marque uma favorita.
2. Abra uma Partida e use a Revisão lance a lance.
3. Crie um segundo Perfil local e confira que a Biblioteca dele é separada.

Critérios de aceite:

- [ ] A busca acha o que existe.
- [ ] A Revisão avança e volta Lance a Lance.
- [ ] Os Perfis locais são isolados, e Desfazer após o fim da Partida não corrompe nada.

Detalhe: [docs/homologacao/matriz-v1.md](matriz-v1.md)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 5 · Provedores sem conta

Rede ligada só com o seu consentimento, sem autorizar nada.

### RC1-27. Offline por padrão e Central de Conexões

**Critérios:** CX-HOM-01/02/03/72 · **Risco:** Integridade · **Precisa de:** Monitor de rede

Passos:

1. Abra Arquivo > Conexões com tudo desligado e rode `ss -tnp` durante 1 minuto.
2. Use a busca e os filtros (Gratuito, Com cadastro, Manual, por estado).
3. Ligue o Lichess: leia o diálogo e **cancele** uma vez, depois ligue de verdade.

Critérios de aceite:

- [ ] Nenhuma conexão sai enquanto tudo está desligado.
- [ ] O diálogo diz o que vai ser enviado antes de ligar.
- [ ] Os filtros e a busca funcionam e o estado aparece em palavras, sem código HTTP.

Registre:

- Textos claros, sem jargão: Sim / Alguns confusos / Muitos confusos

Detalhe: [docs/homologacao/integracoes.md#cx-hom-01-offline-por-padrão-e-estado-da-rede](integracoes.md#cx-hom-01-offline-por-padrão-e-estado-da-rede)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-28. Vincular nome público e testar a Conexão

**Critérios:** CX-HOM-04/05/06 · **Risco:** Conferência · **Precisa de:** Internet, Nome público no Lichess, no Chess.com e ID da FIDE

Passos:

1. Vincule seu nome público no Lichess e no Chess.com e o ID da FIDE.
2. Use **Testar** em cada um. Teste também com maiúsculas diferentes no nome.
3. Remova uma Conexão vinculada.

Critérios de aceite:

- [ ] Cada Provedor responde dentro do prazo.
- [ ] Nome com maiúsculas diferentes funciona.
- [ ] Se aparecer **Limitado**, é pausa do Provedor e some sozinho.

Registre:

- Provedores que responderam: Os três / Dois / Um / Nenhum

Detalhe: [docs/homologacao/integracoes.md#cx-hom-05-testar-a-conexão](integracoes.md#cx-hom-05-testar-a-conexão)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-29. Importar Partidas do Lichess e do Chess.com

**Critérios:** CX-HOM-22/23/24 · **Risco:** Formato · **Precisa de:** Internet, Conta pública com Partidas

Passos:

1. Ação **Importar do Lichess ou Chess.com** (aba Biblioteca): escolha o jogador, período e máximo. Repita para o Chess.com.
2. Compare a contagem com o site. Aperte **Atualizar** e confira que não duplica.
3. Veja Origem, ritmo, Elo e abertura de algumas Partidas.

Critérios de aceite:

- [ ] A contagem bate com o site.
- [ ] Variantes são puladas e contadas, nunca importadas.
- [ ] **Atualizar** não duplica e a Partida mais recente chega logo.

Registre:

- Contagem no app × no site (Lichess e Chess.com): ______
- Velocidade: Boa / Lenta / Travou

Detalhe: [docs/homologacao/integracoes.md#cx-hom-22-importar-partidas-do-lichess](integracoes.md#cx-hom-22-importar-partidas-do-lichess)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-30. Importar arquivo PGN real e problemas do Lichess

**Critérios:** CX-HOM-25/26/27/44/78/79 · **Risco:** Formato · **Precisa de:** Arquivos PGN reais, Internet

Passos:

1. Escolha um PGN real (Queen Alice, ChessBase, TWIC ou PGN Mentor). Veja a **Prévia do arquivo** e confirme os direitos.
2. Tente um arquivo grande (dezenas de milhares de Partidas) e cancele no meio.
3. Treino > Pacotes > **Baixar do Lichess…** com 200 problemas; depois o arquivo completo, se tiver.

Critérios de aceite:

- [ ] A Prévia mostra a fonte detectada e a de uso restrito pede a confirmação.
- [ ] Acentos e duplicatas são tratados e a janela fica responsiva durante a importação.
- [ ] O download só fala com `database.lichess.org` e termina com o pacote e a Origem CC0.

Registre:

- Arquivo grande: Fluiu / Lento, mas terminou / Travou / Não testei
- Tamanho do arquivo e tempo: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-25-importar-arquivos-por-origem](integracoes.md#cx-hom-25-importar-arquivos-por-origem)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-31. Rating FIDE e licença dos dados

**Critérios:** CX-HOM-32/73 · **Risco:** Conferência · **Precisa de:** Internet, ID FIDE real

Passos:

1. Vincule o ID FIDE e abra o Perfil local; use **Atualizar**.
2. Compare os três ritmos com o site da FIDE. Veja a Origem e a licença do dado.

Critérios de aceite:

- [ ] Os ratings batem com o site da FIDE (o Lichess pode estar um mês atrás).
- [ ] A Origem e a licença aparecem junto do valor.

Registre:

- Rating no app × no site (padrão, rápido, relâmpago): ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-32-rating-fide-no-perfil-local](integracoes.md#cx-hom-32-rating-fide-no-perfil-local)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-32. PlayStrategy: importar

**Critérios:** CX-HOM-46/47/48 · **Risco:** Formato · **Precisa de:** Internet, Usuário PlayStrategy (opcional)

Passos:

1. Ligue o PlayStrategy na Central, vincule um nome e use **Testar**.
2. Biblioteca > Importar > PlayStrategy (online).

Critérios de aceite:

- [ ] O teste responde e as Partidas de xadrez entram, as de variante são puladas e contadas.
- [ ] A Origem diz PlayStrategy.

Registre:

- Resultado: Funcionou / Falhou / Não tenho conta

Detalhe: [docs/homologacao/integracoes.md#cx-hom-46-playstrategy-cartão-na-central-ligar-e-testar](integracoes.md#cx-hom-46-playstrategy-cartão-na-central-ligar-e-testar)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 6 · Provedores com conta de teste

Autorização, Partida online, Engines online e FICS.

### RC1-33. Cofre: sistema e senha-mestra

**Critérios:** CX-HOM-07/08 · **Risco:** Base · **Precisa de:** Linux, Windows ou macOS com cofre do sistema

Passos:

1. Inicie a autorização do Lichess (item seguinte) e observe onde o segredo é guardado.
2. Com o chaveiro do sistema bloqueado, tente de novo.
3. Sem cofre do sistema (ou ao escolher o próprio), crie a senha-mestra e desbloqueie.

Critérios de aceite:

- [ ] O segredo vai ao cofre do sistema (Secret Service, Credential Manager ou Keychain), nunca para um arquivo na pasta de dados.
- [ ] Com o chaveiro bloqueado o app não trava.
- [ ] A senha-mestra nunca aparece em log.

Registre:

- Cofre usado: Do sistema / Do Chess Studio (senha-mestra) / Os dois / Nenhum funcionou
- Sistema e versão do chaveiro: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-07-cofre-do-sistema](integracoes.md#cx-hom-07-cofre-do-sistema)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-34. Autorizar o Lichess pelo navegador

**Critérios:** CX-HOM-09 · **Risco:** Base · **Precisa de:** Conta de TESTE do Lichess, Navegador

Passos:

1. Arquivo > Conexões > Lichess > **Autorizar…**. O navegador abre na página do Lichess.
2. Confira as permissões pedidas e autorize. Volte ao app.
3. Nas preferências do Lichess, veja a autorização do Chess Studio.

Critérios de aceite:

- [ ] O navegador abre só a página de autorização do Lichess.
- [ ] As permissões são só as de jogar pelo tabuleiro.
- [ ] O retorno em 127.0.0.1 funciona (com firewall ou VPN) e o app volta ao primeiro plano.
- [ ] O app NUNCA pede a sua senha do Lichess.

Registre:

- Firewall ou VPN no caminho: Nenhum / Firewall / VPN / Os dois
- O que apareceu de estranho: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-09-autorizar-o-lichess-pelo-navegador](integracoes.md#cx-hom-09-autorizar-o-lichess-pelo-navegador)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-35. Recusar, expirar, chave pessoal e desconectar

**Critérios:** CX-HOM-10/11/12/13 · **Risco:** Conferência · **Precisa de:** Conta de teste

Passos:

1. Inicie e recuse no navegador; inicie e deixe vencer; cancele pelo app.
2. Autorize com chave pessoal (token) e compare as permissões.
3. **Desconectar** e confira nas preferências do Lichess.

Critérios de aceite:

- [ ] Recusa e prazo vencido mostram mensagens claras e **Autorizar de novo…**.
- [ ] A revogação some da lista do Lichess (com internet); sem internet o app avisa.
- [ ] O segredo local é apagado sempre.

Detalhe: [docs/homologacao/integracoes.md#cx-hom-10-recusar-deixar-vencer-e-cancelar-a-autorização](integracoes.md#cx-hom-10-recusar-deixar-vencer-e-cancelar-a-autorização)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-36. Explorador, Finais e avaliação em cache

**Critérios:** CX-HOM-14..20 · 75 · 76 · **Risco:** Formato · **Precisa de:** Conexão do Lichess autorizada

Passos:

1. Abra o Painel Explorador: Minha Biblioteca; depois Mestres, Todos os jogadores e Um jogador (com seu nome).
2. Aba Finais com `4k3/6KP/8/8/8/8/7p/8 w - - 0 1` e outra posição conhecida; confira num site de tablebase.
3. Clique numa Partida de referência: ela deve abrir a Partida certa.
4. Desligue a internet e veja o dado em cache com a idade.

Critérios de aceite:

- [ ] As três fontes respondem e os números fazem sentido.
- [ ] O veredito dos Finais bate com o site de tablebase, inclusive com roque (`e1h1`).
- [ ] O rodapé mostra Origem, Período e o aviso de amostra.
- [ ] Sem internet, o cache aparece com a idade na hora local.

Registre:

- Fontes que responderam: As três / Duas / Uma / Nenhuma
- Divergências com o site: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-15-explorador-aberturas-fontes-do-lichess](integracoes.md#cx-hom-15-explorador-aberturas-fontes-do-lichess)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-37. Log detalhado sem segredo

**Critérios:** CX-HOM-77 · SEC-01 · **Risco:** Integridade · **Precisa de:** Conexão autorizada, Terminal

Passos:

1. Abra o app pelo terminal com `RUST_LOG=trace`, autorize o Lichess e use o Explorador.
2. Salve o log e procure o token, o código de autorização e o verificador:
`grep -iE 'bearer|lip_|code_verifier|code=' log.txt`.

Critérios de aceite:

- [ ] Nenhuma linha do log (nem em despejo hexadecimal) tem o token, o código, o verificador PKCE ou `Authorization: Bearer`.

Registre:

- Encontrou segredo no log: Não / Sim (abra issue já!)

Detalhe: [docs/homologacao/integracoes.md#cx-hom-77-log-detalhado-sem-segredo](integracoes.md#cx-hom-77-log-detalhado-sem-segredo)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-38. Partida online no Lichess: contra a Engine e contra pessoa

**Critérios:** CX-HOM-35..39 · **Risco:** Integridade · **Precisa de:** Conta de teste, Segunda conta (adversário)

Passos:

1. Aba Início, grupo Online (Lichess): jogue **Contra a Engine do Lichess** e termine a Partida.
2. Busque um adversário (Rápida ou mais lenta), jogue alguns Lances e termine.
3. Teste empate, abandono, abortar; derrube a internet no meio de uma Partida e volte.

Critérios de aceite:

- [ ] Os Lances no app e no site são os mesmos, sem duplicar nem perder nenhum.
- [ ] O relógio e o resultado vêm do Provedor.
- [ ] Ao cair a conexão, a peça volta se o Lance não chegou e o aviso diz **Reconectando**.
- [ ] A busca só aceita ritmo a partir de Rápida.

Registre:

- Lance duplicado ou fantasma: Nunca / Aconteceu
- Observações: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-36-partida-online-contra-a-engine-do-lichess](integracoes.md#cx-hom-36-partida-online-contra-a-engine-do-lichess)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-39. Desafios e correspondência

**Critérios:** CX-HOM-40/41/69/71 · **Risco:** Integridade · **Precisa de:** Duas contas, Alguns dias

Passos:

1. Desafie a segunda conta pelo nome; receba um desafio e recuse com motivo; tente Chess960.
2. Inicie uma Partida por correspondência, feche o app e deixe o adversário jogar.
3. Reabra e use **Atualizar**.

Critérios de aceite:

- [ ] Chess960 não oferece **Aceitar**.
- [ ] O Lance feito com o app fechado aparece ao reabrir, sem duplicar na Biblioteca.
- [ ] Trocar a Conexão padrão no meio da Partida não afeta a Partida.

Detalhe: [docs/homologacao/integracoes.md#cx-hom-40-desafiar-por-nome-e-desafios-recebidos](integracoes.md#cx-hom-40-desafiar-por-nome-e-desafios-recebidos)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-40. Engine remota (Chess-API.com)

**Critérios:** CX-HOM-49/50/51/73 · **Risco:** Formato · **Precisa de:** Internet

Passos:

1. Painel Análise > lista de Engines > **Stockfish (Chess-API.com) · Remota**. Leia o aviso.
2. Antes de **Ligar**, monitore a rede: nada pode ter ido para `chess-api.com`.
3. Ligue e analise 3 posições; veja o selo e a Origem.

Critérios de aceite:

- [ ] Nada vai ao serviço antes do **Ligar**.
- [ ] Só a Posição e a profundidade saem.
- [ ] O Lance devolvido é sempre legal e a pausa por limite aparece quando existe.

Registre:

- Com várias variantes: Funcionou / Falhou / Não apareceu

Detalhe: [docs/homologacao/integracoes.md#cx-hom-49-engine-remota-escolher-aviso-e-consentimento](integracoes.md#cx-hom-49-engine-remota-escolher-aviso-e-consentimento)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-41. FICS como visitante

**Critérios:** CX-HOM-55..60 · 67 · 68 · 70 · **Risco:** Formato · **Precisa de:** Internet

Passos:

1. Ligue o FICS na Central (leia o aviso: sem criptografia) e **Entrar como visitante**.
2. Busque adversário, jogue, ofereça empate e abandone; observe uma Partida.
3. Derrube a rede no meio de uma Partida.

Critérios de aceite:

- [ ] A entrada termina em até 60 s e o selo diz **FICS · visitante**.
- [ ] Os Lances seguem o formato do servidor e o tabuleiro do servidor manda.
- [ ] Ao cair, a Partida fica **sem resultado confirmado** até **Encerrar sem resultado**.
- [ ] Login com senha é recusado antes da rede.

Registre:

- Entrada no servidor: Entrou / Demorou / Não entrou
- Se algo não entrou: o que o servidor mostrou: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-55-fics-na-central-aviso-ligar-e-desligar](integracoes.md#cx-hom-55-fics-na-central-aviso-ligar-e-desligar)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-42. Engine online (usuário BOT): só com conta NOVA

**Critérios:** CX-HOM-52/53/61..64 · **Risco:** Irreversível · **Precisa de:** Conta NOVA do Lichess criada só para isso

Passos:

1. Crie uma conta nova no Lichess, sem Partidas. NUNCA use a sua conta.
2. Autorize a Conexão de Engine (outra permissão) e leia o aviso de conversão em BOT, que não tem volta.
3. Ligue a Engine online, receba um desafio e jogue. Teste **Parar** e **Parar já…**.
4. Feche o app com uma Partida em andamento.

Critérios de aceite:

- [ ] A conversão em BOT só acontece depois do aviso e da confirmação marcada.
- [ ] Os Lances são sempre legais; a força segue o Elo configurado.
- [ ] Ao fechar com Partida em andamento, o app avisa antes.
- [ ] Ao religar, a Partida em andamento é retomada.

Registre:

- Conversão em BOT: Feita e confirmada / Recusada pelo Lichess / Não testei
- Observações: ______

Detalhe: [docs/homologacao/integracoes.md#cx-hom-52-engine-online-ligar-esperar-jogar-e-parar](integracoes.md#cx-hom-52-engine-online-ligar-esperar-jogar-e-parar)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-43. Trava de fair play

**Critérios:** CX-HOM-21/42/51/81 · #305 · **Risco:** Integridade · **Precisa de:** Monitor de rede, Conexão autorizada · **Ticket:** #305

Passos:

1. Comece uma Partida competitiva (local ou online) e tente abrir Explorador, Finais, avaliação em cache, Engine remota e a Análise.
2. Com o monitor de rede, confirme que nada saiu.
3. Caso #305: inicie uma Partida competitiva local, depois escolha Nova Partida ou Biblioteca sem terminar, e tente o Explorador. Anote o que acontece.

Critérios de aceite:

- [ ] Durante a Partida competitiva ou online, tudo o que calcula recusa com "Trava de fair play" e nenhum pedido sai.
- [ ] A trava só cai com o fim confirmado.
- [ ] O que acontece no caso #305 está anotado (a decisão sobre a regra é sua).

Registre:

- Caso #305: Explorador depois de sair da Partida interrompida: Continuou travado / Liberou / Não testei
- Regra que você quer para #305: Travar até terminar ou abandonar / Liberar ao sair / Perguntar ao sair

Detalhe: [docs/homologacao/integracoes.md#cx-hom-21-trava-de-fair-play](integracoes.md#cx-hom-21-trava-de-fair-play)

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-44. Desligar tudo e apagar

**Critérios:** CX-HOM-29/31/33/34 · **Risco:** Conferência · **Precisa de:** Perfil local de teste

Passos:

1. Desligue cada Provedor; use **Limpar dados guardados**; apague as Partidas importadas por Origem.
2. Exclua o Perfil local de teste.
3. Nas preferências do Lichess, confirme que nenhuma autorização do Chess Studio sobrou.

Critérios de aceite:

- [ ] Com tudo desligado, nenhuma conexão sai.
- [ ] Excluir o Perfil apaga antes o segredo no cofre.
- [ ] Nada sobra no Lichess nem no cofre.

Detalhe: [docs/homologacao/integracoes.md#cx-hom-31-desligar-tudo-e-apagar](integracoes.md#cx-hom-31-desligar-tudo-e-apagar)

Resultado: PASS / FAIL / N/A · Observação: ______

## Grupo 7 · Fechamento

Aprovação visual e veredito do RC-1.

### RC1-45. Aprovação visual geral

**Critérios:** VIS-01/02 · CX-HOM-30 · **Risco:** Conferência

Passos:

1. Depois de usar o app, dê a sua nota ao visual, à linguagem das telas e à janela estreita.

Critérios de aceite:

- [ ] Você revisou ao menos o Tabuleiro 3D, a Central de Conexões, o Explorador e a Partida online.

Registre:

- Visual: Aprovo / Aprovo com ajustes / Não aprovo
- Linguagem das telas: Aprovo / Aprovo com ajustes / Não aprovo
- Ajustes pedidos: ______

Resultado: PASS / FAIL / N/A · Observação: ______

### RC1-46. Veredito do RC-1

**Critérios:** GO · Gate D · **Risco:** Base · **Ticket:** #57

Passos:

1. Revise os itens acima. Todo FAIL vira issue; todo BLOQUEADO diz o que faltou.

Critérios de aceite:

- [ ] Os itens de Base e Irreversível estão PASS ou N/A com justificativa.
- [ ] Você decidiu se este pacote é o RC-1.

Registre:

- Veredito: GO / GO com ressalvas / NO-GO
- Linux: Aprovado / Com ressalvas / Reprovado / Não testei
- Windows: Aprovado / Com ressalvas / Reprovado / Não testei
- macOS: Aprovado / Com ressalvas / Reprovado / Não testei
- Bloqueadores para o RC-2: ______

Detalhe: [docs/relatorios/gate-d.md](../relatorios/gate-d.md)

Resultado: PASS / FAIL / N/A · Observação: ______
