# Guia: usar o Chess Studio com outros serviços de xadrez

Este guia ensina a usar o que já existe no app. Ele vale para o commit `02cd92a` da `main` (11/11/2026). O que ainda não existe está na seção [O que ainda não existe](#o-que-ainda-não-existe).

Se quiser ensaiar sem conta e sem internet, abra o app com os dados de demonstração: `cargo run -p chess-studio -- --demo-data`. Ele abre um Perfil local **Demonstração** com Partidas e Exercícios fictícios, num banco temporário que some ao fechar; nenhum Provedor vem ligado.

Os nomes entre aspas ou em negrito são os textos que você vê na tela. Os termos seguem o [glossário](../../GLOSSARY.md).

## Antes de tudo: o app é offline

O Chess Studio nasce sem falar com ninguém. Nada sai do seu computador até você ligar um **Provedor**.

- Um Provedor é um serviço de xadrez, como o Lichess ou o Chess.com.
- Cada Provedor nasce **Desligado**.
- Você liga e desliga cada um quando quiser.
- Só o que você pede é enviado. Nada do seu Perfil local vai junto.
- Cada dado que vem de fora mostra a **Origem**: de qual Provedor veio, quando e com qual licença.

Tudo isto fica em um só lugar: a **Central de Conexões**.

## Abrir a Central de Conexões

1. Clique na aba **Arquivo** do Ribbon.
2. Clique em **Conexões**.

Outros caminhos: o botão **Central de Conexões** na aba Arquivo (grupo Conexões), a Paleta de Ações (Ctrl+K) ou um clique no indicador de rede na barra de status.

A página mostra um cartão por Provedor. Os gratuitos vêm primeiro. Para achar um, use:

- os filtros de custo **Todos**, **Gratuitos**, **Com cadastro**, **Manual** (quem não tem consulta direta: você baixa o arquivo e importa) e **Pagos ou sob aprovação**, cada um com o número de Provedores;
- a linha **Estado**: **Qualquer estado**, **Desligados**, **Ligados**, **Com autorização**, **Importação manual** e **Em breve**;
- a busca (por exemplo, Lichess).

Os filtros se combinam. Sem resultado, a página diz **Nenhum Provedor combina com a busca e o filtro.**

Cada cartão mostra:

- o nome e um selo de estado, sempre com ícone e palavra;
- **O que você pode fazer agora**;
- o interruptor **Ligado**;
- uma ação principal;
- o botão **Detalhes**, com o que é enviado, como você se identifica, o custo, a **Licença dos dados** (e o que ela restringe, em palavras simples) e as instruções;
- o menu **Mais ações**.

Os cartões de serviços que ainda não funcionam ficam menores, sob **Em breve** ou **Requer aprovação**. Eles não têm interruptor.

## Ver o que está ligado

A barra de status, no pé da janela, mostra o estado da rede:

- **Rede: offline**: nenhum Provedor está ligado.
- **Rede: 1 Provedor ligado** (ou o número que for).
- **1 limitado** ou **1 com falha**, quando for o caso.
- **Trava de fair play**, quando ela está ativa.

Clique no indicador para abrir a Central.

## Ligar um Provedor

1. Abra a Central de Conexões.
2. No cartão do Provedor, clique em **Detalhes** e leia **O que é enviado**. Com o Provedor desligado, o texto começa com **Ao ligar:**.
3. Ligue o interruptor **Ligado**.

O selo muda para **Ligado**. Para desligar, apague o interruptor.

Se você usar um recurso que precisa de um Provedor desligado, o app pergunta antes. Aparece a janela **Ligar Lichess?** (ou o nome do Provedor), com **O que será enviado** e **Para onde vai**. Você escolhe:

- **Ligar**: liga o Provedor e continua o que você pediu.
- **Agora não**: nada é enviado.
- **Ver na Central**: abre a Central.

## Vincular seu nome público

Vincular guarda o seu nome (ou ID) público neste computador. Serve para sugerir o seu nome nas importações e para testar. **Não é uma autorização**: o Provedor não sabe que é você.

1. No cartão do Provedor, clique em **Vincular nome**. No cartão da FIDE, o botão é **Vincular ID**.
2. Digite o **Nome público** (ou o **ID público**). Exemplo: `ana_exemplo`.
3. Se quiser, digite um **Apelido (opcional)**, como `Principal`.
4. Clique em **Vincular**.

Regras:

- Use só o nome ou ID, sem espaços, com até 64 caracteres.
- O mesmo Perfil local não vincula o mesmo nome duas vezes no mesmo Provedor.
- Você pode vincular vários nomes. Para vincular outro, use **Vincular outro nome…**.

O selo do cartão passa a **Ligado · nome vinculado**.

As Conexões aparecem em **Conexões deste Perfil local**. Em cada uma você pode:

- **Tornar padrão**: a padrão é a que o app sugere e testa.
- **Renomear apelido**, no menu **Ações da Conexão**. O apelido tem até 64 caracteres.
- **Remover**, no mesmo menu. O app pede confirmação. O nome continua existindo no Provedor.

### Testar a Conexão

1. No cartão, clique em **Testar**.
2. Se o Provedor estiver desligado, o app pergunta se você quer ligá-lo.
3. Leia o resultado: encontrado, não encontrado, pausa pedida pelo Provedor ou falha.

O teste existe para o Lichess, o Chess.com e a FIDE. O ID da FIDE é conferido pelo Lichess, então o Lichess precisa estar ligado.

## Autorizar o Lichess

Autorizar prova ao Lichess que você é você. É necessário para o **Explorador de aberturas** do Lichess (Mestres, Todos os jogadores e Um jogador) e para [jogar no Lichess](#jogar-no-lichess). Sem autorização, essas fontes não respondem. Para a [Engine online](#engine-online) existe uma autorização separada.

O Chess Studio nunca vê a sua senha do Lichess. Você confirma na página do próprio Lichess.

### Autorizar pelo navegador

1. Abra a Central de Conexões.
2. No cartão do Lichess, clique em **Autorizar…**.
3. Se o Lichess estiver desligado, responda **Ligar** na janela **Ligar Lichess?**.
4. Se o seu computador não tiver um cofre do sistema, o app abre **Criar o cofre do Chess Studio**. Veja [Criar o cofre](#criar-o-cofre-e-a-senha-mestra).
5. O navegador abre a página do Lichess. Confirme lá.
6. Volte ao Chess Studio. Aparece **Autorizado como** e o seu nome.

Enquanto você não confirma, o app mostra **Esperando você autorizar no navegador** e quanto tempo falta. Você pode clicar em:

- **Abrir o navegador de novo**;
- **Copiar endereço**, se o navegador não abrir sozinho;
- **Cancelar**.

Veja também **O que esta autorização permite**. É a lista do que o Lichess liberou. Para jogar, o app pede só a permissão de jogar Partidas pelo tabuleiro. Se a lista mostrar mais, o app avisa e sugere criar outra autorização só com essa permissão.

Se algo falhar, o app diz o motivo e nada é guardado: **Você recusou a autorização**, **O tempo para autorizar acabou** ou **Não foi possível autorizar**. Use **Tentar de novo** ou **Fechar**.

### Autorizar com uma chave pessoal (avançado)

Use isto só se preferir criar a chave no site do Lichess.

1. No cartão do Lichess, abra **Mais ações** e clique em **Usar chave pessoal…**.
2. No site do Lichess, crie uma chave só com a permissão de jogar Partidas pelo tabuleiro.
3. Cole a chave no campo **Chave**. O campo é mascarado.
4. Clique em **Conferir e guardar**.

O app confere a chave com o Lichess. Só guarda se o Lichess confirmar. Ele recusa uma chave com permissões que não servem ao jogador.

### Criar o cofre e a senha-mestra

Os segredos precisam de um lugar seguro. O app usa, nesta ordem:

1. o **cofre do sistema** do seu computador, se existir;
2. o **cofre do Chess Studio**, protegido por uma senha-mestra, se o sistema não tiver cofre.

Com cofre do sistema, você não precisa fazer nada. A faixa no topo da Central mostra **Segredos guardados no cofre do sistema**.

Sem cofre do sistema, a faixa mostra **Sem cofre para guardar autorizações**. Faça assim:

1. Clique em **Criar cofre…** (o app também abre esta janela quando você tenta autorizar).
2. Digite a **Senha-mestra** (pelo menos 8 caracteres) e repita em **Repita a senha-mestra**.
3. Leia o aviso **Não há como recuperar esta senha**.
4. Marque **Entendi que não há como recuperar a senha**.
5. Clique em **Criar cofre**. Isso leva alguns segundos.

Depois disso:

- **Bloquear** fecha o cofre. As autorizações param de valer até você desbloquear.
- **Desbloquear…** pede a senha-mestra. O selo **Cofre bloqueado** aparece enquanto ele estiver fechado.
- **Trocar senha-mestra…** pede a senha atual e a nova. As autorizações continuam valendo.

Se você esquecer a senha-mestra, as autorizações se perdem. Será preciso autorizar de novo. Nem o Chess Studio consegue recuperá-la.

### Quando a autorização para de valer

Se o Lichess deixar de aceitar a autorização, o cartão mostra **Autorização expirada**. Seus dados locais ficam. Clique em **Autorizar de novo…**. A mesma Conexão é renovada.

### Desconectar

1. Na Conexão autorizada, abra **Ações da Conexão**.
2. Clique em **Desconectar…**.
3. Confirme em **Desconectar**.

O app pede ao Lichess que revogue a autorização e apaga a chave daqui. Sem internet, o app avisa o que fazer: revogar a chave nas preferências do Lichess, na área de acesso por chaves.

## Explorar aberturas e finais

O **Explorador** é um Painel que segue a Posição do tabuleiro. Ele também segue a Posição quando você navega pela Notação.

### Abrir o Explorador

- Aba **Análise**, grupo **Explorador**, botão **Abrir Explorador**.
- Ou, na aba **Exibir**, a caixa **Explorador** no grupo Painéis.

O Painel funciona como qualquer outro: encaixado, flutuante ou destacado.

### Aba Aberturas

1. Na aba **Aberturas**, escolha a **Fonte**:
   - **Minha Biblioteca**: a estatística das suas próprias Partidas, sem internet. É a fonte padrão.
   - **Mestres (Lichess)**.
   - **Todos os jogadores (Lichess)**.
   - **Um jogador (Lichess)**: digite o nome do jogador no Lichess e escolha **Como Brancas** ou **Como Pretas**.
2. Leia a tabela: Lance, Partidas, uma barra com Brancas, Empates e Pretas, e o rating médio.
3. Clique num Lance para ver a variante. Isso é só para olhar: **a Partida não muda**. Use **Voltar à Partida** para sair.
4. Clique numa das **Partidas de referência**. O app mostra **Abrindo a Partida…** e abre aquela Partida mesmo: se ela não estava na Biblioteca, ele a baixa do Lichess, guarda com a Origem e a abre; no clique seguinte ela abre direto, sem internet. Se não der, a linha diz o motivo (a Partida não está mais no Lichess, o Lichess pediu uma pausa, está desligado ou falta autorizar) e você pode clicar de novo.
5. O nome da abertura (por exemplo, **Defesa Siciliana: Variante Najdorf**) aparece em português, com o código ECO. Passe o mouse sobre ele para ver o **Nome original** em inglês.

Filtros:

- **Ritmos** (Relâmpago, Blitz, Rápida, Clássica, Correspondência): valem para **Todos os jogadores** e **Um jogador**. Sem nenhum marcado, vale todos.
- **Faixas de rating**: valem só para **Todos os jogadores**.

O app lembra a aba, a fonte, o jogador, a cor e os filtros por Perfil local.

As três fontes do Lichess precisam do Lichess **ligado e autorizado**. Sem isso, o Painel mostra:

- **O Lichess está desligado**, com o botão **Ligar Lichess**; ou
- **Autorize sua conexão do Lichess**, com os botões **Abrir a Central de Conexões** e **Usar Minha Biblioteca**.

Sem nenhuma Partida sua na Biblioteca, a fonte **Minha Biblioteca** mostra **Nenhuma Partida sua passou por aqui** e o botão **Abrir a Biblioteca**.

### Aba Finais

1. Clique na aba **Finais**.
2. Leve o tabuleiro a uma Posição com **7 peças ou menos**. Com mais peças aparece **Disponível com 7 peças ou menos**, e nada é pedido.
3. Leia o veredito (por exemplo, **Vitória das Brancas** ou **Empate**), o **Mate em N Lances** e os Lances **do melhor ao pior**.

Os finais não pedem autorização. Pedem só o Lichess ligado. O veredito leva em conta a regra dos 50 Lances. Quando ela muda o resultado, o Painel explica.

### A Origem e os avisos

O rodapé do Painel diz de onde veio o dado, por exemplo: Lichess, **consultado às 14:32** (na hora do seu computador; sem o fuso do sistema, **consultado há 3 min**), licença. Se o dado veio de uma avaliação guardada, diz **avaliação guardada em** uma data.

O rodapé diz também o **Período** dos dados: na **Minha Biblioteca**, todas as Partidas dela; nas fontes do Lichess, que o Lichess não informa as datas daquela base e o app não filtra por data. E lembra que a estatística é histórica, de uma amostra, não é avaliação, e que vitórias por cor não mostram vantagem objetiva nem o melhor Lance.

Avisos que você pode ver:

- **Sem conexão com a internet**: o Painel mostra o que estava salvo, com a idade.
- **O serviço pediu uma pausa**: o Explorador volta sozinho.
- **Sem dados para esta Posição**: ninguém jogou essa Posição na fonte. Tente outra fonte ou volte um Lance.
- **Indisponível durante a Partida**: veja [a Trava de fair play](#o-que-a-trava-de-fair-play-bloqueia-e-por-quê).
- **Indisponível durante um Exercício**: termine ou saia do Exercício.

## Ver avaliações prontas

Para algumas Posições o Lichess já tem uma avaliação calculada por outras pessoas. O app mostra essa avaliação no **Painel Análise**, numa linha separada: **Avaliação pronta (Lichess)**.

1. Abra o Painel Análise.
2. Procure a linha **Avaliação pronta (Lichess)**.
3. Leia o valor, a **Profundidade** e a linha principal.

Detalhes:

- O valor é sempre do ponto de vista das Brancas. Passe o mouse para ver **Do ponto de vista das Brancas**.
- A linha **nunca** se mistura com a sua Engine local, nem a substitui.
- **Sem avaliação pronta para esta Posição** é normal: o Lichess não calcula sob demanda.
- Com o Lichess desligado, a linha diz **O Lichess está desligado**. Ligue-o na Central.
- Em Partida competitiva ou online contra uma pessoa, a linha diz **Indisponível: Trava de fair play.**
- O app confere os Lances da linha pelas regras do xadrez e corta a linha no primeiro Lance que as regras recusam.

## Importar Partidas do Lichess, do Chess.com e do PlayStrategy

O app traz as Partidas **públicas** de um jogador para a sua Biblioteca. Cada Partida guarda a Origem. Nada é baixado sozinho: só quando você pede.

1. Aba **Biblioteca**, botão **Importar do Lichess ou Chess.com**.
   (Também serve: abra o Painel Biblioteca, aba **Partidas**, seção **Importar PGN…**, e escolha em **De onde importar?** o item **Lichess (online)**, **Chess.com (online)** ou **PlayStrategy (online)**.)
2. Se o Provedor estiver desligado, clique em **Ligar Lichess** (ou **Ligar Chess.com**, **Ligar PlayStrategy**).
3. Em **Nome do jogador**, digite o nome público. Se você já vinculou o seu, clique em **Usar … (sua Conexão)**.
4. Escolha o **Período**: **Todo o histórico**, **Últimos 7 dias**, **Últimos 30 dias** ou **Último ano**.
5. Escolha o **Ritmo**, a **Cor** e o **Máximo** de Partidas.
6. Clique em **Importar**.
7. Acompanhe **Recebidas**, **Novas** e **Repetidas**. Para parar, clique em **Cancelar**. O que já foi lido fica na Biblioteca.
8. Ao final, o resumo diz **Importação de … concluída**.

Para trazer só o que é novo, clique em **Atualizar**. Ele pede apenas as Partidas jogadas depois da última que você importou daquele nome. Nada se duplica.

Se der erro, o app explica:

- **Não existe um jogador chamado …**: confira a grafia.
- **O serviço pediu uma pausa**: tente de novo em alguns minutos.
- **Sem conexão com a internet**: o que já foi importado fica.
- **Nenhuma Partida encontrada com esses filtros**: use um período maior ou outro ritmo.

O PlayStrategy também tem damas, shogi, xiangqi, Crazyhouse e outras variantes: o app só importa as de xadrez padrão e diz quantas de variantes pulou. Ainda não há Partida online no PlayStrategy.

Durante uma Partida competitiva em andamento, a importação fica indisponível.

## Importar Partidas de arquivos

Serve para arquivos PGN que você baixou ou recebeu: de Queen Alice, ChessBase, TWIC (The Week in Chess), PGN Mentor, do Lichess, do Chess.com ou de qualquer outro lugar. O app não entra nesses sites e nunca pede a sua senha deles.

1. Baixe o arquivo no site de origem. No cartão do Provedor, na Central, o botão **Abrir instruções** abre a página que explica como.
2. Abra o Painel Biblioteca, aba **Partidas**, e a seção **Importar PGN…**.
3. Em **De onde importar?**, deixe **Arquivo PGN**.
4. Em **De onde veio o arquivo?**, escolha a origem: **Queen Alice**, **ChessBase**, **TWIC (The Week in Chess)**, **PGN Mentor**, **Lichess**, **Chess.com** ou **Outro ou não sei**.
5. Leia a nota de licença que aparece. Ela diz o que você pode fazer com aquelas Partidas.
6. Clique em **Escolher arquivo…** e escolha o arquivo `.pgn` ou `.pgn.gz` na janela do sistema. Se o seu sistema não tem essa janela, o botão não aparece: digite o caminho ou clique num dos arquivos listados em **Arquivos .pgn nos documentos:**.
7. Assim que o arquivo é escolhido, aparece a **Prévia do arquivo**: quantas Partidas ele tem (**Cerca de N** se for grande), os cabeçalhos das primeiras e, se as tags indicarem, **As tags indicam Lichess** (ou outra fonte). A fonte do passo 4 já vem escolhida por ela; se as tags indicarem outra, o app avisa para você conferir. Nada foi gravado ainda.
8. Se a fonte for de uso restrito (TWIC, PGN Mentor, ChessBase), marque **Confirmo que posso usar estas Partidas**. Sem isso, **Importar** fica desligado.
9. Clique em **Importar**.
10. Acompanhe o andamento. Para parar, clique em **Cancelar**.

O resumo mostra quantas Partidas são **novas**, **repetidas** e **com erro** e, quando for o caso, quantas têm licença restrita. As com erro são puladas, e as primeiras são listadas.

O que acontece por baixo:

- Importar o mesmo arquivo duas vezes não duplica nada.
- A mesma Partida vinda de dois Provedores fica uma só, com as duas Origens.
- Arquivos grandes são lidos aos poucos.
- Se o arquivo não estiver em UTF-8, o app o converte e avisa: confira os acentos.

Depois, você filtra a Biblioteca por origem, por ECO e por ritmo. A Origem aparece no detalhe da Partida. Para tirar uma Partida importada da Biblioteca, abra-a e clique em **Excluir da biblioteca**.

## Baixar ou instalar problemas do Lichess como Exercícios

O Lichess publica um arquivo grande de problemas, com licença livre (CC0). Você escolhe uma parte dele e o app a transforma em Exercícios. Há dois caminhos: baixar pela internet ou instalar de um arquivo que você já tem.

### Baixar pela internet

1. Abra o Painel Treino e clique em **Pacotes**.
2. Clique em **Baixar do Lichess…**.
3. Escolha os **Temas** (abra um grupo e marque; sem nenhum marcado vale qualquer tema), a **Dificuldade** (**Iniciante**, **Intermediário**, **Avançado**, **Mestre** ou **Faixa própria…**) e a **Quantidade de problemas**.
4. Clique em **Baixar problemas**. Se o Lichess estiver desligado, a janela **Ligar Lichess?** aparece; responda **Ligar** (ou **Agora não**, e nada é baixado).
5. Acompanhe o andamento. O app baixa só o necessário e para sozinho quando encontra a quantidade pedida. **Cancelar** não instala nada.
6. Ao final aparece **Pacote … instalado com N Exercícios**, com a Origem (Lichess, licença CC0, data).

Se a internet cair, o app diz que nada foi instalado. Por segurança, o app só aceita que o Lichess redirecione o arquivo para outro endereço de download do próprio Lichess.

### Instalar de um arquivo

1. Obtenha o arquivo de problemas do Lichess (`.csv` ou `.csv.zst`).
2. Abra o Painel Treino e clique em **Pacotes**.
3. Em **Instalar de um arquivo**, clique em **Escolher arquivo…** (ou digite o caminho, ou clique num dos listados em **Arquivos .csv e .csv.zst nos documentos:**). Esta opção não usa a internet.
4. Se quiser, digite os **Temas do Lichess, separados por espaço** (vazio: todos), por exemplo `fork`, e a **Quantidade máxima**.
5. Clique em **Instalar pacote**.
6. Acompanhe **Lendo: N problemas, N selecionados**. Para parar, clique em **Cancelar leitura**: nada é instalado.
7. Ao final aparece **Pacote … instalado**, com quantos Exercícios entraram e quantos foram descartados.

Um arquivo que não é a base de problemas do Lichess (com uma linha gigante) é recusado e nada é instalado.

### Depois de instalar

Os Exercícios do pacote ficam numa seção própria, **Pacote: …**, separados dos de fábrica, e mostram a Origem (Provedor e licença). Na lista de Exercícios, os filtros de pacote, tema e dificuldade ajudam a achar o que treinar.

Para remover: em **Pacotes instalados**, clique em **Remover pacote** e confirme. Os Exercícios que você nunca resolveu saem. Os que você já resolveu ficam ocultos, mas as suas Resoluções continuam no histórico. Reinstalar o pacote traz os Exercícios de volta com o histórico.

## O que a Trava de fair play bloqueia, e por quê

Em Partida competitiva, ninguém deve receber ajuda de fora. A **Trava de fair play** garante isso.

Enquanto uma Partida competitiva está em andamento (Simulação FIDE ou Eliminação Imediata), o app bloqueia tudo o que calcula ou consulta a Posição:

- o **Explorador**, nas Aberturas e nos Finais;
- as **avaliações prontas** do Painel Análise;
- a **importação online** e a abertura da Biblioteca;
- a análise por Engine (local ou **remota**), a barra de avaliação, a dica e as marcas do HUD que calculam;
- a estatística da sua Biblioteca para a Posição (Explorador e Painel Aberturas) e a busca por Posição na Biblioteca;
- a **Engine online** (ela não liga com uma Partida online contra pessoa em andamento);
- a **Engine adversária** de uma nova Partida enquanto outra Partida online contra pessoa está em andamento: **Contra a Engine** fica desabilitado, com o motivo.

O Painel Explorador não some. Ele diz **Indisponível durante a Partida** e não consulta nada. O indicador da barra de status mostra **Trava de fair play**.

A trava vale nos próprios serviços, não só na tela: a análise, a dica, a estatística e a busca por Posição da Biblioteca, a Engine adversária, a Engine online, o Explorador, os finais e as avaliações prontas perguntam a ela antes de calcular. Por isso um Painel escondido ou um atalho não a contorna. Ela cai quando a Partida termina. Numa Partida competitiva local, a Engine que é a sua adversária continua jogando: ela é a adversária, não uma ajuda.

Numa **Partida online contra uma pessoa** a trava também vale, e só cai quando o Lichess confirma o fim (abandonar ou fechar o Painel não a soltam). Continuam os Auxílios que só mostram regras: destinos legais, último Lance e coordenadas. Contra a Engine do Lichess a trava não liga.

Na **correspondência**, com a Partida fora do tabuleiro, a trava vale para ela e para as Posições dela: a Posição atual, as que estão a até dois Lances dela (com as mesmas peças e peões) e tudo o que se joga a partir delas, venha de onde vier. As outras Partidas e Posições ficam livres; o Explorador diz **Posição de uma Partida em andamento** quando for o caso.

## Jogar no Lichess

Precisa do Lichess ligado e da **Conexão do jogador** autorizada (a de Engine nunca joga como pessoa). Sem ela, a tela explica e leva à Central.

1. Início > Nova Partida > **Online (Lichess ou FICS)**, ou o grupo **Online (Lichess)** do Início: **Buscar adversário**, **Desafiar**, **Contra a Engine do Lichess** e **Partidas em andamento**.
2. **Buscar adversário:** escolha o ritmo (rápida, clássica ou correspondência em dias por Lance) e se vale rating. O app mostra **Procurando adversário** com o tempo de espera e **Cancelar busca**. Blitz só por desafio direto.
3. **Desafiar:** digite o nome no Lichess, o ritmo (blitz inclusive), a cor e o rating. O desafio fica **Aguardando resposta** até ser aceito ou recusado; dá para cancelar.
4. **Desafios recebidos:** aparecem num aviso discreto no canto (nunca durante outra Partida competitiva) e na aba **Desafiar**, com **Aceitar** e **Recusar** (com o motivo). Variante que não é xadrez padrão é recusada.
5. **Contra a Engine do Lichess:** nível de 1 a 8, cor e ritmo.

Durante a Partida, jogue no tabuleiro 3D ou 2D com os gestos de sempre. O seu Lance fica no destino com **Enviando …** no Painel **Partida online** até o Lichess confirmar; se ele recusar, a peça volta com a explicação. Os relógios do Painel Relógio e do relógio 3D são os do Lichess. No Painel: **Propor empate**, aceitar ou recusar a proposta do adversário, **Abandonar** (com confirmação), **Abortar** no começo e **Reclamar vitória** quando o adversário sair e o Lichess permitir. Se a internet cair, o selo mostra **Reconectando** e nenhum Lance é reenviado; ao voltar, vale o que o Lichess diz. Não há desfazer nem as Políticas de arbitragem locais.

No fim, o resultado e o motivo são os do Lichess. A Partida fica na Biblioteca com a Origem (**Partida online · Lichess · lichess.org/…**) e só então **Analisar** fica disponível.

**Correspondência:** **Partidas em andamento** lista de quem é a vez e quanto falta; **Abrir** continua a Partida pelo que o Lichess diz. Ao abrir o app, nada sai sozinho: um aviso oferece **Atualizar**.

## Onde ficam os segredos

Segredos são as autorizações e as chaves pessoais.

- Com **cofre do sistema**: o seu computador os guarda. O Chess Studio não guarda nenhuma senha.
- Sem cofre do sistema: ficam no **cofre do Chess Studio**, cifrado pela sua senha-mestra, na pasta `cofre`, ao lado do banco de dados do app.
- Nunca aparecem em texto, em registro ou em exportação. O campo da chave é mascarado e o app nunca o copia para a área de transferência.
- O banco de dados guarda só o seu nome público, o apelido e o que a autorização permite. Nunca a chave.

A pasta de dados do app é:

| Sistema | Pasta |
|---|---|
| Linux | `~/.local/share/ChessStudio` |
| Windows | `%APPDATA%\ChessStudio` |
| macOS | `~/Library/Application Support/ChessStudio` |

## Rating FIDE

Com um ID da FIDE vinculado, o cartão da FIDE na Central e a página **Arquivo > Perfil local** mostram o jogador: nome, federação, título e os ratings **Standard**, **Rápido** e **Blitz**, cada um na sua caixa. Um ritmo sem rating aparece como **—**. Os três são listas separadas da FIDE: o app nunca os soma nem os compara com ratings do Lichess ou do Chess.com.

A linha de origem diz **FIDE, via Lichess, consultado há N min** e, abaixo, a licença: os dados são da FIDE, reproduzidos pelo Lichess, e os direitos são dela (reprodução restrita, só para uso pessoal). O dado vem do Lichess, que reproduz a FIDE, e o Lichess **não informa de qual mês é a lista**: a data é a da consulta, não a do rating.

### Consultar e atualizar

1. Vincule o ID (veja [Vincular seu nome público](#vincular-seu-nome-público)) ou ache-o pela busca, abaixo.
2. No cartão da FIDE, clique em **Consultar rating**. Nas próximas vezes, o botão é **Atualizar**.
3. Se o Lichess estiver desligado, o app pergunta se você quer ligá-lo. **Agora não** não envia nada.

**Atualizar** sempre pergunta de novo ao Lichess, mesmo que a consulta anterior seja recente. O app diz se os ratings **continuam os mesmos**.

O último valor fica guardado neste computador, por Perfil local. Sem internet, ou com o Lichess desligado, você vê esse valor e a data da consulta. A consulta não é bloqueada na Partida competitiva, porque não calcula nada.

### Achar o ID pelo nome

1. No cartão da FIDE, digite o nome em **Buscar jogador da FIDE pelo nome** (de 2 a 60 letras) e clique em **Buscar**.
2. Cada resultado mostra título, federação, os três ratings e o ID. Clique em **Vincular este ID**.
3. Para trocar de jogador depois, use **Buscar outro jogador pelo nome**.

## Apagar Partidas importadas por origem

Na Biblioteca, a aba **Origens** lista só as Partidas que vieram de fora (arquivos e Provedores). As que você jogou no Chess Studio **não aparecem ali e nunca são apagadas por ela**.

1. Marque **Tudo de um Provedor** (todas as importações dele) ou uma ou mais importações em **Por importação**, que mostra o nome, o modo, a data e quantas Partidas.
2. Clique em **Apagar marcadas…**.
3. A janela diz quantas Partidas serão apagadas e de onde. Confirme com **Apagar N Partidas**, ou cancele.

Se a mesma Partida veio de dois lugares (por exemplo, de um arquivo e do Lichess), ela fica, e a janela diz quantas ficam **porque também vieram de outra origem**. Não dá para desfazer, mas você pode importar de novo.

## Desligar tudo e apagar o que ficou

### Desligar

Não há um botão único para desligar tudo. Cada Provedor tem o seu interruptor, porque assim você vê para onde cada dado vai.

1. Abra a Central de Conexões.
2. Apague o interruptor **Ligado** de cada Provedor.
3. Confira a barra de status: deve dizer **Rede: offline**.

Desligar não apaga nada: as Partidas importadas, as Conexões e o cache continuam aqui.

### Apagar os dados guardados de todos os Provedores de uma vez

Os dados guardados (o cache) são respostas dos Provedores que o app guarda para não repetir pedidos e para funcionar sem rede.

1. No alto da Central, na faixa **Dados guardados**, clique em **Limpar dados guardados…**.
2. A janela diz o que some e o que fica. **Some:** as respostas guardadas de todos os Provedores neste Perfil local (perfis, ratings, avaliações, Explorador e listas de Partidas). **Fica:** suas Partidas jogadas, as Partidas e os Exercícios importados, suas Conexões e autorizações, o último rating FIDE guardado e o que está ligado ou desligado.
3. Clique em **Limpar dados guardados** para confirmar, ou em **Cancelar**.
4. O app diz quantas respostas guardadas apagou. Depois disso, cada consulta pergunta de novo ao Provedor, se ele estiver ligado.

### Apagar os dados guardados de um Provedor só

1. No cartão do Provedor, abra **Mais ações**.
2. Clique em **Limpar dados guardados**.
3. O app diz quantos itens removeu (**1 item guardado removido.**, **N itens guardados removidos.** ou **Não havia dados guardados.**).

O cache é separado por Perfil local. Cada Provedor tem o seu.

### Apagar o resto

| O que | Como |
|---|---|
| Uma autorização | Na Conexão, **Ações da Conexão** e **Desconectar…**. |
| Um nome vinculado | Na Conexão, **Ações da Conexão** e **Remover**. |
| Uma Partida importada | Abra-a na Biblioteca e clique em **Excluir da biblioteca**. |
| Várias Partidas importadas | Na Biblioteca, aba **Origens**: veja [Apagar Partidas importadas por origem](#apagar-partidas-importadas-por-origem). |
| Um pacote de problemas | No Painel Treino, **Pacotes**, **Remover pacote**. |
| Tudo de um Perfil local | Exclua o Perfil local (Arquivo > Perfil local). A autorização guardada no cofre é apagada antes. |

Apagar uma Conexão ou um Perfil local apaga primeiro a autorização guardada. Se o cofre recusar, nada é removido.

## Jogar no FICS como visitante

O FICS é um servidor de xadrez antigo e gratuito. O Chess Studio só entra como **visitante**: o FICS dá um nome do tipo `GuestXXXX`, as Partidas são **sem rating**, e o app **nunca pede nem envia senha**.

Antes de ligar, leia o aviso (no cartão **FICS** da Central, em **Jogar online** > **Detalhes**, e na tela):

- **Conexão sem criptografia**: os Lances, os comandos e o nome de visitante trafegam em texto puro até o FICS.
- **Só como visitante**.
- **Sem reconexão automática**: se a conexão cair durante a Partida, o FICS a dá como perdida.
- **Sem compensação de atraso**: o atraso da rede entra no tempo do seu relógio.

Passo a passo:

1. Abra Início > Nova Partida > **Online (Lichess ou FICS)** e escolha **FICS · visitante**.
2. Se o FICS estiver desligado, clique em **Ligar o FICS…** e responda **Ligar** em **Ligar FICS?**.
3. Clique em **Entrar como visitante**. A tela mostra **Entrando como visitante** e, depois, **No FICS como** o seu nome de visitante. Se o FICS não confirmar em 1 minuto, o app avisa e você tenta de novo.
4. Na aba **Buscar adversário**, escolha o ritmo (**Relâmpago**, **Blitz** ou **Padrão**) e clique em **Buscar adversário**. Para desistir da busca, **Cancelar busca**.
5. Na aba **Desafios** aparecem os desafios que outros jogadores fazem a você pelo seu nome de visitante, com **Aceitar** e **Recusar**. Desafios de variante não têm **Aceitar**: o app só joga xadrez padrão.
6. Na aba **Observar**, digite o número de uma Partida em andamento no FICS e clique em **Observar**. A Partida aparece num tabuleiro pequeno, só para ver: não vai para a Biblioteca e não liga a Trava de fair play. **Parar de observar** encerra.
7. Quando uma Partida começa, ela vai ao tabuleiro, com o Painel **Partida online** (selos **FICS · visitante** e **Ao vivo**). Jogue pelos gestos de sempre; o Lance fica no destino com **Enviando …** até o FICS confirmar. No Painel: **Propor empate**, **Abortar** (no FICS, depois do primeiro Lance, pede a concordância do adversário) e **Abandonar** (com confirmação; o FICS registra a derrota).
8. **Sair do FICS** fecha a conexão. Sair com uma Partida em andamento é perder a Partida, e o app avisa antes.

A Trava de fair play vale como no Lichess: durante a Partida contra uma pessoa, Engines, Explorador, finais e análise ficam desligados até o FICS confirmar o fim.

Se a conexão cair no meio da Partida, o Painel e a tela dizem **A conexão com o FICS caiu**, o Lance sem confirmação volta e não é reenviado, e o tabuleiro não aceita mais Lances. A Trava continua ligada até você clicar em **Descartar a sessão**; então a Análise volta a funcionar. A Partida fica na Biblioteca sem resultado. **Entrar de novo como visitante** é outra sessão, com outro nome.

A Partida fica na Biblioteca com a Origem **Partida online · FICS (visitante) · Partida N**.

## Analisar com uma Engine remota

A **Engine remota** analisa no servidor de um Provedor (hoje, o Chess-API.com), não no seu computador. Para isso a Posição sai da sua máquina. Por isso ela nunca é a padrão e nunca liga sozinha.

1. Abra o Painel **Análise** e a lista **Engine**.
2. Escolha **Stockfish (Chess-API.com) · Remota** (numa linha separada das Engines do seu computador).
3. Leia o aviso **Usar a Engine remota?**: só a Posição (onde estão as peças, de quem é a vez e os direitos de roque) e a profundidade pedida são enviadas, nunca nomes, histórico da Partida, Perfil local ou Conexão. Clique em **Continuar** (ou **Cancelar**: nada muda e nada sai).
4. Se o Chess-API.com estiver desligado, o diálogo **Ligar Chess-API.com?** aparece. **Ligar** escolhe a Engine remota; **Agora não** deixa a Engine local.
5. Ligue a análise. A avaliação chega como a de uma Engine local, com o selo **Remota · Chess-API.com**, a Origem (**Origem: Chess-API.com, consultado …**) e os termos do resultado.

Para voltar à Engine do seu computador, escolha-a na lista. Ao fechar e reabrir o app, a Engine local volta a ser a escolhida.

Avisos que você pode ver no Painel:

- **O Chess-API.com está desligado, então nada foi enviado**, com o botão **Ligar o Chess-API.com**.
- **O Chess-API.com pediu uma pausa. Tente de novo em N s**: a análise local continua disponível.
- **Trava de fair play**: a Engine remota fica bloqueada durante uma Partida online contra pessoa; nada é enviado.
- **A Engine remota não respondeu agora** ou **não pôde analisar esta Posição**: tente de novo ou use uma Engine local.

## Engine online

A **Engine online** deixa uma Engine instalada no seu computador responder a desafios no Lichess, como um usuário BOT. Ela só liga quando você clica em **Ligar a Engine online**, e só funciona com uma autorização **separada** da de jogador: a Conexão de Engine.

Antes de começar, tenha uma Engine instalada (gerenciador de Engines) e um usuário do Lichess **criado só para isso**: a conversão em usuário BOT **não tem volta** e só funciona num usuário novo, que nunca jogou.

1. Na Central, no cartão do Lichess, clique em **Engine online…**. Na seção **Engine online**, clique em **Autorizar Engine online…** (pelo navegador) ou em **Usar chave pessoal…**, com o usuário BOT.
2. Clique em **Abrir a tela da Engine online**. (Atalhos: a Ação **Engine online** na aba Início, grupo **Online (Lichess)**, ou a Paleta, Ctrl+K, digitando "Engine online".)
3. Escolha a **Engine**. Se quiser, marque **Limitar a força da Engine** e escolha o Elo (a mesma força da Engine adversária das Partidas locais).
4. Em **Limites**, escolha os **Ritmos aceitos**, **Valendo rating** (**Os dois**, **Só sem rating** ou **Só valendo**), **Quem pode desafiar** (**Só pessoas** ou **Pessoas e Engines**), **Partidas ao mesmo tempo** e **Máximo de Partidas nesta execução**.
5. Clique em **Ligar a Engine online**. O estado passa a **Esperando desafios**. Quando um desafio dentro dos limites chega, o app o aceita e o estado vira **Jogando N**, com a Partida na lista (adversário, cor, Lances e quem pensa). Desafios fora dos limites são recusados, com o motivo em **Atividade recente**.
6. **Parar** muda para **Parando**: a Engine não aceita mais desafios e fecha sozinha quando as Partidas terminam. **Parar já…** pede confirmação e **abandona** as Partidas em andamento (o adversário recebe a vitória).

A tela abre sempre **Parada**: nada sai para o Lichess até você ligar. Fechar a janela da tela não para a Engine. Se você fechar o Chess Studio com Partida em andamento, o app pergunta (**Sair com Partida em andamento?**): **Cancelar**, **Parar e esperar terminarem** ou **Sair mesmo assim** (as Partidas ficam no Lichess e o relógio da Engine continua correndo, então elas perdem por tempo). Se você ligar a Engine online de novo antes de o relógio acabar, o Lichess reenvia a Partida e ela é retomada de onde parou, no mesmo registro da Biblioteca.

Se o usuário ainda não for um usuário BOT, abra **O usuário ainda não é um usuário BOT?**, leia o aviso, marque **Entendi que isto não tem volta e que vou usar um usuário criado só para a Engine.** e clique em **Converter em usuário BOT**. Num usuário com Partidas jogadas, o Lichess recusa.

Se a Engine falhar (não iniciar, propor dois Lances ilegais seguidos ou fechar), a Partida é abandonada e a **Atividade recente** diz o que houve. Um Lance ilegal nunca é enviado. As Partidas terminadas ficam na Biblioteca como Partidas da Engine.

A **Trava de fair play** vale: a Engine online não liga durante uma Partida online em tempo real contra pessoa, e uma Partida que começa sob a Trava é abandonada pela Engine.

### Ver a Partida da Engine no tabuleiro

Em **Partidas em andamento**, **Ver no tabuleiro** abre o **modo espectador**: o tabuleiro (3D ou 2D) mostra a Partida da Engine e acompanha cada Lance que o Lichess confirma. É só para ver: nenhum gesto move peça, as Ações de jogo (abandonar, empate, relógio, desfazer) ficam desabilitadas, a Trava de fair play não liga e nada é gravado de novo (quem grava é a própria Engine online). Os selos **Engine online** e **Modo espectador** ficam no canto do tabuleiro e no Painel **Partida online**, com os relógios do Lichess e quem joga.

Se a sua Partida estiver em andamento no tabuleiro, o app pergunta antes (**Ver a Partida da Engine?**): ela fica guardada como está, sem ser encerrada; a local com relógio fica em **Pausa**, e a online continua no Lichess (o relógio dela não para). **Sair do modo espectador** devolve o tabuleiro como estava. Começar outra Partida, abrir uma da Biblioteca ou uma Partida online sua que comece também saem do modo espectador. Enquanto ele está aberto, as Anotações e a Súmula ficam indisponíveis (são da sua Partida), e a Trava da sua Partida competitiva, se houver, continua valendo para a Engine online.

## Salvar e exportar uma Partida

- No fim de uma Partida, o diálogo oferece **Salvar como…**: escolha a pasta e o nome na janela do sistema (a extensão `.pgn` é acrescentada se você não a digitar).
- No Painel Revisão, em **Exportar PGN**, **Salvar como…** faz o mesmo.
- A Ação **Exportar PGN** do Ribbon abre a janela já com o nome `partida-AAAA-MM-DD-HHMMSS.pgn`.
- Se o seu sistema não tem a janela de escolha de arquivo, os botões **Salvar como…** não aparecem e **Exportar PGN** grava na pasta Documentos.
- Se a Súmula estiver ocultando a notação, o PGN revelaria os Lances: o **Salvar como…** do fim da Partida fica desativado e o do Painel Revisão não aparece.

## Se algo não funcionar

| Você vê | O que fazer |
|---|---|
| **Desligado** | Ligue o interruptor **Ligado** no cartão. |
| **Limitado · volta em N min** | O Provedor pediu uma pausa. Espere: o app volta sozinho. |
| **Com falha** | A última consulta não deu certo. Tente de novo mais tarde. |
| **Autorização expirada** | Clique em **Autorizar de novo…**. |
| **Cofre bloqueado** | Clique em **Desbloquear…** e digite a senha-mestra. |
| **O cofre de segredos não abriu** | Nada é guardado nem lido. Seus dados locais estão intactos. |
| **O cofre do sistema não respondeu** | Desbloqueie o cofre do sistema e abra o Chess Studio de novo. |
| **Sem conexão com a internet** | O app mostra o que estava salvo, quando houver. Tente de novo quando a internet voltar. |
| **Importação manual** | Esse Provedor não tem consulta direta. Baixe o arquivo e importe. |

## O que ainda não existe

Estas coisas **não existem** no app. Os cartões e os textos da Central podem citá-las como "Em breve", mas nenhuma funciona hoje.

- **Partida online e Engine online no PlayStrategy.** O PlayStrategy só tem perfil, teste da Conexão e importação de Partidas.
- **Entrar no FICS com nome registrado e senha.** O FICS não tem conexão cifrada, então o app só entra como visitante.
- **Chat, premove e reclamar empate por ausência** nas Partidas online.
- **Jogar no Chess.com**, e os Provedores pagos, sob aprovação ou sem API oficial (por exemplo, ChessBase e ICC): aparecem na Central como informativos, sem interruptor.
- **Ler o relógio e a avaliação dos comentários do PGN** (`[%clk]` e `[%eval]`): eles passam como texto.

Está em andamento (ainda não no app): melhorias do Painel da Partida online, do desafio recebido no FICS e da correspondência; a ligação da Partida online à Conexão que a joga; ajustes visuais da Engine online.
