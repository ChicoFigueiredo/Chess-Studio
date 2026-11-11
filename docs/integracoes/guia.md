# Guia: usar o Chess Studio com outros serviços de xadrez

Este guia ensina a usar o que já existe no app. Ele vale para o commit `1626ebb` da `main` (11/11/2026). O que ainda não existe está na seção [Em breve](#em-breve).

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

A página mostra um cartão por Provedor. Os gratuitos vêm primeiro. Use os filtros **Todos**, **Gratuitos** e **Pagos ou sob aprovação**, ou digite na busca (por exemplo, Lichess).

Cada cartão mostra:

- o nome e um selo de estado, sempre com ícone e palavra;
- **O que você pode fazer agora**;
- o interruptor **Ligado**;
- uma ação principal;
- o botão **Detalhes**, com o que é enviado, como você se identifica, o custo e as instruções;
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

Autorizar prova ao Lichess que você é você. Hoje isso é necessário para o **Explorador de aberturas** do Lichess (Mestres, Todos os jogadores e Um jogador). Sem autorização, essas fontes não respondem.

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

Veja também **O que esta autorização permite**. É a lista do que o Lichess liberou. O app pede só a permissão de jogar Partidas pelo tabuleiro. Se a lista mostrar mais, o app avisa e sugere criar outra autorização só com essa permissão.

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
4. Clique numa das **Partidas de referência** para procurá-la na Biblioteca.

Filtros:

- **Ritmos** (Bullet, Blitz, Rápida, Clássica, Correspondência): valem para **Todos os jogadores** e **Um jogador**. Sem nenhum marcado, vale todos.
- **Faixas de rating**: valem só para **Todos os jogadores**.

O app lembra a aba, a fonte, o jogador, a cor e os filtros por Perfil local.

As três fontes do Lichess precisam do Lichess **ligado e autorizado**. Sem isso, o Painel mostra:

- **O Lichess está desligado**, com o botão **Ligar Lichess**; ou
- **Requer autorizar sua conexão do Lichess**, com os botões **Abrir a Central de Conexões** e **Usar Minha Biblioteca**.

Sem nenhuma Partida sua na Biblioteca, a fonte **Minha Biblioteca** mostra **Nenhuma Partida sua passou por aqui** e o botão **Abrir a Biblioteca**.

### Aba Finais

1. Clique na aba **Finais**.
2. Leve o tabuleiro a uma Posição com **7 peças ou menos**. Com mais peças aparece **Disponível com 7 peças ou menos**, e nada é pedido.
3. Leia o veredito (por exemplo, **Vitória das Brancas** ou **Empate**), o **Mate em N Lances** e os Lances **do melhor ao pior**.

Os finais não pedem autorização. Pedem só o Lichess ligado. O veredito leva em conta a regra dos 50 Lances. Quando ela muda o resultado, o Painel explica.

### A Origem e os avisos

O rodapé do Painel diz de onde veio o dado, por exemplo: Lichess, **consultado há 3 min**, licença. Se o dado veio do cache, diz **do cache, de** uma data.

Avisos que você pode ver:

- **Sem conexão com a internet**: o Painel mostra o que estava salvo, com a idade.
- **O serviço pediu uma pausa**: o Explorador volta sozinho.
- **Sem dados para esta Posição**: ninguém jogou essa Posição na fonte. Tente outra fonte ou volte um Lance.
- **Indisponível durante a Partida**: veja [a Trava de fair play](#o-que-a-trava-de-fair-play-bloqueia-e-por-quê).
- **Indisponível durante um Exercício**: termine ou saia do Exercício.

## Ver avaliações prontas

Para algumas Posições o Lichess já tem uma avaliação calculada por outras pessoas. O app mostra essa avaliação no **Painel Análise**, numa linha separada: **Em cache (Lichess)**.

1. Abra o Painel Análise.
2. Procure a linha **Em cache (Lichess)**.
3. Leia o valor, a **Profundidade** e a linha principal.

Detalhes:

- O valor é sempre do ponto de vista das Brancas. Passe o mouse para ver **Do ponto de vista das Brancas**.
- A linha **nunca** se mistura com a sua Engine local, nem a substitui.
- **Sem avaliação em cache para esta Posição** é normal: o Lichess não calcula sob demanda.
- Com o Lichess desligado, a linha diz **O Lichess está desligado**. Ligue-o na Central.
- O app confere os Lances da linha pelas regras do xadrez e corta a linha no primeiro Lance que as regras recusam.

## Importar Partidas do Lichess e do Chess.com

O app traz as Partidas **públicas** de um jogador para a sua Biblioteca. Cada Partida guarda a Origem. Nada é baixado sozinho: só quando você pede.

1. Aba **Biblioteca**, botão **Importar do Lichess ou Chess.com**.
   (Também serve: abra o Painel Biblioteca, aba **Partidas**, seção **Importar PGN…**, e escolha em **De onde importar?** o item **Lichess (online)** ou **Chess.com (online)**.)
2. Se o Provedor estiver desligado, clique em **Ligar Lichess** (ou **Ligar Chess.com**).
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

Durante uma Partida competitiva em andamento, a importação fica indisponível.

## Importar Partidas de arquivos

Serve para arquivos PGN que você baixou ou recebeu: de Queen Alice, ChessBase, TWIC (The Week in Chess), PGN Mentor, do Lichess, do Chess.com ou de qualquer outro lugar. O app não entra nesses sites e nunca pede a sua senha deles.

1. Baixe o arquivo no site de origem. No cartão do Provedor, na Central, o botão **Abrir instruções** abre a página que explica como.
2. Abra o Painel Biblioteca, aba **Partidas**, e a seção **Importar PGN…**.
3. Em **De onde importar?**, deixe **Arquivo PGN**.
4. Em **De onde veio o arquivo?**, escolha a origem: **Queen Alice**, **ChessBase**, **TWIC (The Week in Chess)**, **PGN Mentor**, **Lichess**, **Chess.com** ou **Outro ou não sei**.
5. Leia a nota de licença que aparece. Ela diz o que você pode fazer com aquelas Partidas.
6. Digite o caminho do arquivo `.pgn` ou `.pgn.gz`, ou clique num dos arquivos listados em **Arquivos .pgn nos documentos:**.
7. Clique em **Importar**.
8. Acompanhe o andamento. Para parar, clique em **Cancelar**.

O resumo mostra quantas Partidas são **novas**, **repetidas** e **com erro**. As com erro são puladas, e as primeiras são listadas.

O que acontece por baixo:

- Importar o mesmo arquivo duas vezes não duplica nada.
- A mesma Partida vinda de dois Provedores fica uma só, com as duas Origens.
- Arquivos grandes são lidos aos poucos.
- Se o arquivo não estiver em UTF-8, o app o converte e avisa: confira os acentos.

Depois, você filtra a Biblioteca por origem, por ECO e por ritmo. A Origem aparece no detalhe da Partida. Para tirar uma Partida importada da Biblioteca, abra-a e clique em **Excluir da biblioteca**.

## Instalar problemas do Lichess como Exercícios

O Lichess publica um arquivo grande de problemas, com licença livre (CC0). Você escolhe uma parte dele e o app a transforma em Exercícios. **O app não baixa o arquivo**: você já precisa tê-lo no computador.

1. Obtenha o arquivo de problemas do Lichess (`.csv` ou `.csv.zst`).
2. Abra o Painel Treino e clique em **Pacotes**.
3. Em **Instalar problemas do Lichess**, digite o caminho do arquivo ou clique num dos listados em **Arquivos .csv nos documentos:**.
4. Se quiser, digite os **Temas** separados por espaço (vazio: todos). Exemplo: `fork`.
5. Se quiser, digite a **Dificuldade mín.**, a **Dificuldade máx.** e a **Quantidade máxima**.
6. Clique em **Instalar pacote**.
7. Acompanhe **Lendo: N problemas, N selecionados**. Para parar, clique em **Cancelar leitura**: nada é instalado.
8. Ao final aparece **Pacote … instalado**, com quantos Exercícios entraram e quantos foram descartados.

Os Exercícios do pacote ficam numa seção própria, **Pacote: …**, separados dos de fábrica, e mostram a Origem (Provedor e licença). Na lista de Exercícios, os filtros de pacote, tema e dificuldade ajudam a achar o que treinar.

Para remover: em **Pacotes instalados**, clique em **Remover pacote** e confirme. Os Exercícios que você nunca resolveu saem. Os que você já resolveu ficam ocultos, mas as suas Resoluções continuam no histórico. Reinstalar o pacote traz os Exercícios de volta com o histórico.

## O que a Trava de fair play bloqueia, e por quê

Em Partida competitiva, ninguém deve receber ajuda de fora. A **Trava de fair play** garante isso.

Enquanto uma Partida competitiva está em andamento (Simulação FIDE ou Eliminação Imediata), o app bloqueia tudo o que calcula ou consulta a Posição:

- o **Explorador**, nas Aberturas e nos Finais;
- as **avaliações prontas** do Painel Análise;
- a **importação online** e a abertura da Biblioteca;
- a análise por Engine, a barra de avaliação, a dica e as marcas do HUD que calculam;
- a estatística da sua Biblioteca para a Posição (Explorador e Painel Aberturas).

O Painel Explorador não some. Ele diz **Indisponível durante a Partida** e não consulta nada. O indicador da barra de status mostra **Trava de fair play**.

A trava vale nos próprios serviços, não só na tela: a análise, a dica, a estatística da Biblioteca, o Explorador, os finais e as avaliações prontas perguntam a ela antes de calcular. Por isso um Painel escondido ou um atalho não a contorna. Ela cai quando a Partida termina.

Numa **Partida online contra uma pessoa** a trava também vale, e só cai quando o Lichess confirma o fim (abandonar ou fechar o Painel não a soltam). Continuam os Auxílios que só mostram regras: destinos legais, último Lance e coordenadas. Contra a Engine do Lichess a trava não liga.

Na **correspondência**, com a Partida fora do tabuleiro, a trava vale para ela e para as Posições dela: a Posição atual, as que estão a até dois Lances dela (com as mesmas peças e peões) e tudo o que se joga a partir delas, venha de onde vier. As outras Partidas e Posições ficam livres; o Explorador diz **Posição de uma Partida em andamento** quando for o caso.

## Jogar no Lichess

Precisa do Lichess ligado e da **Conexão do jogador** autorizada (a de Engine nunca joga como pessoa). Sem ela, a tela explica e leva à Central.

1. Início > Nova Partida > **Online (Lichess)**, ou o grupo **Online (Lichess)** do Início: **Buscar adversário**, **Desafiar**, **Contra a Engine do Lichess** e **Partidas em andamento**.
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

## Desligar tudo e apagar o que ficou

### Desligar

Não há um botão único para desligar tudo. Cada Provedor tem o seu interruptor, porque assim você vê para onde cada dado vai.

1. Abra a Central de Conexões.
2. Apague o interruptor **Ligado** de cada Provedor.
3. Confira a barra de status: deve dizer **Rede: offline**.

Desligar não apaga nada: as Partidas importadas, as Conexões e o cache continuam aqui.

### Apagar o cache

O cache guarda respostas dos Provedores para o app não repetir pedidos.

1. No cartão do Provedor, abra **Mais ações**.
2. Clique em **Limpar cache**.
3. O app diz quantos itens removeu (**1 item do cache removido.**, **N itens do cache removidos.** ou **O cache já estava vazio.**).

O cache é separado por Perfil local. Cada Provedor tem o seu.

### Apagar o resto

| O que | Como |
|---|---|
| Uma autorização | Na Conexão, **Ações da Conexão** e **Desconectar…**. |
| Um nome vinculado | Na Conexão, **Ações da Conexão** e **Remover**. |
| Uma Partida importada | Abra-a na Biblioteca e clique em **Excluir da biblioteca**. |
| Um pacote de problemas | No Painel Treino, **Pacotes**, **Remover pacote**. |
| Tudo de um Perfil local | Exclua o Perfil local (Arquivo > Perfil local). A autorização guardada no cofre é apagada antes. |

Apagar uma Conexão ou um Perfil local apaga primeiro a autorização guardada. Se o cofre recusar, nada é removido.

## Se algo não funcionar

| Você vê | O que fazer |
|---|---|
| **Desligado** | Ligue o interruptor **Ligado** no cartão. |
| **Limitado · volta em N min** | O Provedor pediu uma pausa. Espere: o app volta sozinho. |
| **Com falha** | A última consulta não deu certo. Tente de novo mais tarde. |
| **Autorização expirada** | Clique em **Autorizar de novo…**. |
| **Cofre bloqueado** | Clique em **Desbloquear…** e digite a senha-mestra. |
| **O cofre de segredos não abriu** | Nada é guardado nem lido. Seus dados locais estão intactos. |
| **O cofre do sistema não respondeu** | Desbloqueie o chaveiro do sistema e abra o Chess Studio de novo. |
| **Sem conexão com a internet** | O app mostra o que estava salvo, quando houver. Tente de novo quando a internet voltar. |
| **Importação manual** | Esse Provedor não tem consulta direta. Baixe o arquivo e importe. |

## Em breve

Estas coisas **ainda não existem** no app. Os cartões e os textos da Central podem citá-las, mas nenhuma funciona hoje.

- **Engine online**: jogar com uma Engine no Lichess. A Central já deixa criar uma autorização separada para isso, mas nada a usa ainda.
- **PlayStrategy** e **FICS**.
- **Engine remota** para análise.
- **Baixar problemas e bases pela rede**. Hoje você instala um arquivo que já tem.
- **Rating da FIDE** na tela. Hoje você só vincula e testa o ID.
- **Escolher o arquivo numa janela do sistema**. Hoje você digita o caminho ou clica num arquivo listado.
