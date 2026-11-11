# Roteiro de testes de usabilidade do V4

Roteiro para a fase de testes e usabilidade do V4 (épico [#211](https://github.com/ChicoFigueiredo/Chess-Studio/issues/211), ticket [#275](https://github.com/ChicoFigueiredo/Chess-Studio/issues/275)): Central de Conexões, Provedores, importação, Explorador, Exercícios, Partida online, FICS, Engine remota e Engine online.

**Para que serve.** Descobrir onde uma pessoa hesita, erra ou não entende um texto. Não é o roteiro de homologação ([`docs/homologacao/integracoes.md`](../homologacao/integracoes.md)), que confere se cada recurso funciona. Aqui o app é a cobaia.

**Quem usa.** Quem testa lê só o **objetivo**. Quem observa (pode ser a mesma pessoa, depois) usa as demais linhas. O roteiro nunca diz onde clicar: se a pessoa não achar o caminho sozinha, isso é o resultado.

## Como preparar

1. Compile e abra com os dados de demonstração:

   ```
   cargo run -p chess-studio -- --demo-data
   ```

   O app abre com o Perfil local **Demonstração**, num banco temporário que some ao fechar (o seu banco real não é aberto nem alterado; `--db` é ignorado nesse modo). Dentro dele há Partidas importadas de várias Origens, o pacote de Exercícios **Demonstração: táticas**, três Conexões fictícias por nome público (Lichess, Chess.com e PlayStrategy), nenhuma autorizada, e respostas do Explorador já em cache. **Nenhum Provedor está ligado e nenhum pedido de rede sai** enquanto você não ligar um.
2. Para a jornada 1 (primeira vez), use um banco novo e vazio, sem a demonstração: `cargo run -p chess-studio -- --db /caminho/novo.db`.
3. Marcas nas tarefas:
   - **[demo]**: roda inteira com `--demo-data`, sem internet.
   - **[internet]**: precisa de internet, mas não de conta.
   - **[conta]**: precisa de conta de verdade (Lichess; para a jornada 9, uma conta criada só para o teste). Veja os riscos em [`verificar-com-servico-real.md`](verificar-com-servico-real.md).
4. Se for gravar a tela, peça permissão a quem testa. Não grave senhas nem chaves.

## Regras de condução

- Leia o **cenário** em voz alta; não explique a tela.
- Peça para a pessoa pensar em voz alta. Se ela ficar parada mais de **30 s**, anote a hesitação e, só depois de 1 min, dê uma dica (e anote que deu).
- Não corrija erros. Anote o que a pessoa esperava que acontecesse.
- Tempo muito acima do esperado, mesmo com sucesso, é um problema de usabilidade.

## Como anotar

Cada tarefa tem estes campos (copie o bloco para uma planilha ou imprima):

| Campo | Valores |
|---|---|
| Resultado | **OK** (sozinho), **OK com dica**, **Falhou**, **Desistiu** |
| Tempo | segundos até concluir |
| Hesitação | onde parou, mais de 30 s |
| Erro | o que fez de errado, e se percebeu |
| Texto | frase da tela que não entendeu (copie a frase) |
| Comentário | frase da pessoa, do jeito que ela falou |

Sobre os termos: os rótulos da tela seguem o [glossário](../../GLOSSARY.md) (Provedor, Conexão, Central de Conexões, Origem, Cofre, Trava de fair play, Explorador, Engine remota, Engine online). Anote quando a pessoa usar outra palavra para a mesma coisa (por exemplo "conta" para Conexão): é sinal de rótulo ruim.

---

## Jornada 1. Primeira vez, offline

Banco novo e vazio. Computador pode estar sem internet.

### 1.1 Abrir o app e entender que está offline **[demo]**

- **Cenário:** "Você acabou de instalar o programa de xadrez e vai abri-lo pela primeira vez, sem internet."
- **Objetivo:** abrir o app e dizer, com suas palavras, se ele fala com algum serviço agora.
- **Sucesso:** diz que não fala com ninguém; acha o indicador de rede (**Rede: offline**) ou a Central de Conexões como prova.
- **Observar:** se procura aviso de conexão ou de login; se estranha não ser pedida conta; se entende o indicador de rede.
- **Tempo esperado:** 1 min.
- **Anotações:** resultado __ tempo __ hesitação __ erro __ texto __ comentário __

### 1.2 Jogar uma Partida a dois e fechar o app

- **Cenário:** "Você quer jogar uma partida rápida com um amigo no mesmo computador."
- **Objetivo:** começar uma Partida, fazer cinco Lances cada lado e fechar o app no meio.
- **Sucesso:** faz os Lances; ao reabrir, a Partida volta pausada, sem ter procurado "Salvar".
- **Observar:** se procura por Salvar; se entende que a Partida volta em Pausa; se confunde o tabuleiro 3D e o 2D.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 1.3 Descobrir de onde vêm os dados de fora

- **Cenário:** "Um colega disse que o programa pode usar o Lichess, mas você não quer que nada seja enviado sem querer."
- **Objetivo:** descobrir quais serviços o app sabe usar e se algum está ligado.
- **Sucesso:** chega à Central de Conexões e diz que todos os Provedores estão desligados.
- **Observar:** qual caminho tenta primeiro; se o selo (ícone e palavra) basta ou se ele abre os Detalhes; se entende a diferença entre **Gratuitos** e **Pagos ou sob aprovação** e o que é **Em breve**.
- **Tempo esperado:** 2 min.
- **Anotações:** (campos acima)

---

## Jornada 2. Ligar um Provedor

### 2.1 Ler o que seria enviado antes de ligar **[demo]**

- **Cenário:** "Você quer usar o Lichess, mas antes quer saber o que o programa mandaria para lá."
- **Objetivo:** dizer, com suas palavras, o que seria enviado ao Lichess e para onde.
- **Sucesso:** acha o texto de **O que é enviado** (ou o da janela **Ligar Lichess?**) e resume corretamente: só o que a pessoa pedir; nada do Perfil local.
- **Observar:** se encontra o texto sozinha; se entende "Para onde vai"; se algum termo técnico atrapalha.
- **Tempo esperado:** 2 min.
- **Anotações:** (campos acima)

### 2.2 Ligar e desligar o Lichess **[demo]**

- **Cenário:** "Decidiu que o Lichess pode ser usado."
- **Objetivo:** ligar o Lichess, conferir que ficou ligado e desligar de novo.
- **Sucesso:** o indicador de rede passa a **Rede: 1 Provedor ligado** e volta a **Rede: offline**.
- **Observar:** se percebe que ligar não envia nada ainda; se entende o que **Agora não** faz, quando aparecer; se confia no interruptor.
- **Tempo esperado:** 2 min.
- **Anotações:** (campos acima)

### 2.3 Vincular o seu nome público **[demo]**

- **Cenário:** "Quer que o programa conheça o seu usuário do Lichess, sem dar senha."
- **Objetivo:** vincular um nome público a um Provedor e depois dar a ele um apelido e torná-lo o padrão.
- **Sucesso:** a Conexão aparece em **Conexões deste Perfil local** com o apelido e o selo **Padrão**; a pessoa diz que **não** autorizou nada.
- **Observar:** se confunde "vincular nome" com "autorizar"; se erra o nome (com espaço) e entende a mensagem de erro; se o selo **Ligado · nome vinculado** é claro.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 2.4 Testar a Conexão **[internet]**

- **Cenário:** "Quer saber se o nome que digitou existe mesmo."
- **Objetivo:** conferir se o seu usuário é encontrado no Provedor.
- **Sucesso:** vê "Testado há N min: encontrado como NOME" (ou entende a mensagem de nome inexistente).
- **Observar:** se aceita que o app peça para ligar o Provedor antes; tempo de espera percebido; se a mensagem de pausa (**Limitado · volta em N min**) é compreendida.
- **Tempo esperado:** 2 min.
- **Anotações:** (campos acima)

---

## Jornada 3. Importar Partidas e estudar

### 3.1 Achar uma Partida de uma Origem **[demo]**

- **Cenário:** "Você importou Partidas de vários lugares e quer rever só as que vieram do TWIC."
- **Objetivo:** achar uma Partida do TWIC na Biblioteca e dizer de onde ela veio e com qual licença.
- **Sucesso:** diz "TWIC, arquivo, uso pessoal" a partir da Origem exibida.
- **Observar:** se usa busca, filtro ou a lista de Origens; se entende **Origem** (Provedor, modo, data, licença); se a nota de licença é clara.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 3.2 Estudar uma Partida importada **[demo]**

- **Cenário:** "Quer ver uma Partida do Lichess lance a lance e entender onde ela virou."
- **Objetivo:** abrir uma Partida importada, navegar pelos Lances e ligar a análise da Engine local.
- **Sucesso:** vê os Lances no tabuleiro e a avaliação da Engine (ou entende que precisa instalar uma Engine).
- **Observar:** se encontra a navegação por Lances; se a Notação e o tabuleiro ficam sincronizados; se a Anotação de estudo é descoberta.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 3.3 Importar Partidas de um arquivo **[demo]**

- **Cenário:** "Baixou um arquivo `.pgn` do ChessBase e quer trazê-lo para o programa." (Prepare um arquivo pequeno; o roteiro de homologação tem a lista.)
- **Objetivo:** importar o arquivo dizendo de onde ele veio, e depois importar o mesmo arquivo de novo.
- **Sucesso:** a primeira vez traz "N novas"; a segunda diz "0 novas" e N repetidas; a pessoa entende por que nada duplicou.
- **Observar:** se encontra o formulário; se escolhe a Origem certa; se lê a nota de licença; se o resumo (**N novas, N repetidas, N com erro**) é claro.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 3.4 Importar Partidas do seu usuário no Lichess ou no Chess.com **[conta]**

- **Cenário:** "Quer trazer as suas partidas dos últimos 30 dias."
- **Objetivo:** importar as Partidas públicas de um usuário e, depois, trazer só as novas.
- **Sucesso:** vê **Recebidas / Novas / Repetidas** subirem; ao atualizar, vem só o que é novo.
- **Observar:** se entende o que acontece com o Provedor desligado; se percebe e usa **Cancelar**; se **Atualizar** é compreendido; reação a "O serviço pediu uma pausa".
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

---

## Jornada 4. Explorador

### 4.1 Ver o que a sua Biblioteca diz sobre uma abertura **[demo]**

- **Cenário:** "Você joga 1.e4 e quer saber o que costuma acontecer depois nas suas Partidas."
- **Objetivo:** abrir o Explorador, jogar 1.e4 e dizer qual resposta das Pretas apareceu mais nas suas Partidas.
- **Sucesso:** lê a tabela de Lances e a barra Brancas/Empates/Pretas; sabe que os números vêm da própria Biblioteca, sem internet.
- **Observar:** se acha o Explorador; se entende as colunas; se nota a Origem no rodapé (**Origem: sua Biblioteca, sem internet**); se percebe que clicar num Lance não muda a Partida.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 4.2 Pedir dados do Lichess sem autorização **[demo]**

- **Cenário:** "Quer ver o que os mestres jogam nessa posição."
- **Objetivo:** tentar a fonte de mestres e explicar por que não aparece nada ainda e o que fazer.
- **Sucesso:** entende que o Provedor está desligado (e, ligado, que falta autorizar), e que existe a alternativa de usar a própria Biblioteca.
- **Observar:** se os botões **Ligar Lichess**, **Abrir a Central de Conexões** e **Usar Minha Biblioteca** são encontrados e entendidos; se o texto de "Requer autorizar" assusta.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 4.3 Finais exatos e avaliação em cache **[demo]**

- **Cenário:** "Está estudando um final de peão e quer saber se é vitória."
- **Objetivo:** carregar a posição `4k3/6KP/8/8/8/8/7p/8 w - - 0 1` (Rei e peão contra Rei e peão) e descobrir o resultado teórico; depois olhar a avaliação da posição inicial no Painel Análise.
- **Sucesso:** lê "Vitória das Brancas" e **Mate em N Lances**; diz de onde veio a resposta (do cache, de uma data). Os dados vêm do cache de demonstração, só depois de ligar o Lichess, e nenhum pedido sai.
- **Observar:** se carrega a posição por FEN sem ajuda; se entende o veredito e o DTZ; se a linha **Em cache (Lichess)** é distinta da Engine local; se nota o aviso de idade do dado.
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

### 4.4 Consultar o Explorador com autorização **[conta]**

- **Cenário:** "Autorizou o Lichess e quer ver o que os jogadores do site fazem."
- **Objetivo:** ver a tabela de Todos os jogadores filtrando por Blitz e por uma faixa de rating, e fechar e reabrir o app.
- **Sucesso:** a tabela chega aos poucos; os filtros mudam os números; ao reabrir, as escolhas voltaram.
- **Observar:** tempo até a primeira linha; se **Recebendo mais resultados…** é compreendido; se os filtros se descobrem sem ajuda.
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

---

## Jornada 5. Baixar e resolver problemas

### 5.1 Resolver um Exercício do pacote **[demo]**

- **Cenário:** "Quer treinar um pouco de tática."
- **Objetivo:** escolher o pacote **Demonstração: táticas**, resolver um Exercício e ver o histórico da Resolução.
- **Sucesso:** resolve o Exercício (ou erra e entende o feedback) e acha a Resolução no histórico.
- **Observar:** se distingue os Exercícios de fábrica do pacote; se encontra o filtro por pacote, tema e dificuldade; se entende o feedback de erro e a Origem do pacote.
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

### 5.2 Remover e reinstalar um pacote **[demo]**

- **Cenário:** "Não quer mais esse pacote, mas não quer perder o que já resolveu."
- **Objetivo:** remover o pacote de demonstração e conferir que o histórico continua.
- **Sucesso:** o pacote some da lista; as Resoluções continuam no histórico e na Evolução; a pessoa entende o motivo.
- **Observar:** se a confirmação de **Remover pacote** é clara; se há medo de perder dados; se entende que reinstalar traz o histórico de volta.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 5.3 Baixar problemas do Lichess **[internet]**

- **Cenário:** "Quer mais Exercícios: uns 200 de garfo."
- **Objetivo:** baixar um pacote pequeno de problemas do Lichess e conferir de onde veio.
- **Sucesso:** o pacote é instalado e mostra **Origem: Lichess, licença CC0**; a pessoa percebeu o pedido de consentimento antes do download.
- **Observar:** se o diálogo **Ligar Lichess?** faz sentido para um download de arquivo; tempo de espera; se **Cancelar** é achado; reação a erro de internet.
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

---

## Jornada 6. Partida online no Lichess

Exige conta do Lichess **[conta]**. Use uma conta sem importância. Veja o risco de cada passo em [`verificar-com-servico-real.md`](verificar-com-servico-real.md).

### 6.1 Descobrir o que falta para jogar online

- **Cenário:** "Quer jogar uma partida contra alguém no Lichess, de dentro do programa."
- **Objetivo:** começar o caminho de uma Partida online sem estar autorizado e dizer o que falta.
- **Sucesso:** vê a explicação **Autorize o Lichess para jogar** e chega à Central.
- **Observar:** se encontra o lugar de Partida online; se o texto leva à ação certa; se confunde Conexão do jogador e Conexão de Engine.
- **Tempo esperado:** 2 min.
- **Anotações:** (campos acima)

### 6.2 Autorizar o Lichess

- **Cenário:** "Concorda em deixar o programa jogar por você no Lichess."
- **Objetivo:** autorizar o Lichess pelo navegador e conferir o que foi permitido.
- **Sucesso:** o cartão passa a **Autorizado** e a pessoa lê, sem ajuda, a lista **O que esta autorização permite**.
- **Observar:** se entende o cofre (criar ou usar o do sistema) e a senha-mestra, se aparecer; se a espera por "Esperando você autorizar no navegador" é clara; reação à página do Lichess; se sabe como cancelar.
- **Tempo esperado:** 6 min.
- **Anotações:** (campos acima)

### 6.3 Jogar contra a Engine do Lichess

- **Cenário:** "Quer uma partida fácil para testar."
- **Objetivo:** jogar alguns Lances contra a Engine do Lichess no nível mais baixo e abandonar.
- **Sucesso:** os Lances aparecem; entende **Enviando …**; abandona com confirmação.
- **Observar:** se percebe o atraso até o Lichess confirmar; se entende por que a análise não está bloqueada nesta Partida; se o abandono é difícil de achar ou fácil demais de acionar.
- **Tempo esperado:** 6 min.
- **Anotações:** (campos acima)

### 6.4 Buscar adversário e entender a Trava de fair play

- **Cenário:** "Agora quer jogar contra uma pessoa."
- **Objetivo:** buscar um adversário (Rápida 10+5, sem rating), jogar e, no meio, tentar abrir a análise ou o Explorador.
- **Sucesso:** entende que Engines e Explorador estão desligados durante a Partida e por quê; a Trava some só quando o Lichess confirma o fim.
- **Observar:** se lê o aviso de **Trava de fair play**; se acha que é defeito; se entende por que Blitz não aparece na busca.
- **Tempo esperado:** 10 min (depende do pareamento).
- **Anotações:** (campos acima)

### 6.5 Perder a internet no meio da Partida

- **Cenário:** "O Wi-Fi cai durante a partida."
- **Objetivo:** desligar a internet por ~20 s no meio de uma Partida e voltar.
- **Sucesso:** a pessoa diz o estado em que está o tabuleiro e se o último Lance foi enviado.
- **Observar:** se **Reconectando em … s** é claro; se tenta mover sem confirmação; se entende que "nenhum Lance é reenviado".
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

---

## Jornada 7. FICS como visitante

Exige internet **[internet]**; não exige conta (o FICS é só como visitante, sem senha).

### 7.1 Entender o aviso antes de ligar o FICS

- **Cenário:** "Um amigo falou do FICS, um servidor antigo."
- **Objetivo:** ler o que o app diz do FICS antes de ligar e explicar duas diferenças em relação ao Lichess.
- **Sucesso:** diz que a conexão não é criptografada e que só entra como visitante, sem senha.
- **Observar:** se lê o cartão; se procura um campo de senha; se o texto de **Conexão sem criptografia** é entendido.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 7.2 Entrar como visitante e jogar

- **Cenário:** "Quer jogar uma blitz no FICS."
- **Objetivo:** entrar como visitante, buscar um adversário em Blitz 5+0, jogar alguns Lances e sair.
- **Sucesso:** vê o nome de visitante dado pelo FICS, joga, e **Sair do FICS** encerra a conexão.
- **Observar:** se estranha o nome Guest; tempo de espera da busca; se entende que a Partida é sem rating; reação a "o FICS não confirmou a entrada a tempo".
- **Tempo esperado:** 8 min.
- **Anotações:** (campos acima)

### 7.3 A conexão cai no meio da Partida

- **Cenário:** "A internet cai durante a partida no FICS."
- **Objetivo:** provocar a queda e dizer o que o app informa e o que fazer.
- **Sucesso:** entende que a Partida de visitante que cai é dada como perdida, que o app não reconecta sozinho e que **Descartar a sessão** libera a análise.
- **Observar:** se o aviso é compreendido sem ansiedade; se tenta reconectar; se entende que "Entrar de novo" é outra sessão.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

---

## Jornada 8. Engine remota

### 8.1 Escolher a Engine remota **[internet]**

- **Cenário:** "Seu computador é lento e você quer uma análise mais forte, de um serviço."
- **Objetivo:** escolher a Stockfish do Chess-API.com no Painel Análise, lendo o aviso antes.
- **Sucesso:** diz que a Posição sai do computador (só o FEN) e vai ao Chess-API.com; escolhe **Continuar** e **Ligar**; vê o selo **Remota · Chess-API.com**.
- **Observar:** se lê o aviso; se entende "só o FEN"; se **Agora não** e **Cancelar** são compreendidos; se espera a Engine remota continuar escolhida ao reabrir (ela não continua).
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 8.2 Analisar com a Engine remota e com o Provedor desligado **[internet]**

- **Cenário:** "Analisa uma Partida e depois desliga o serviço."
- **Objetivo:** navegar rápido por uma Partida com a análise ligada; desligar o Chess-API.com na Central e voltar ao Painel.
- **Sucesso:** a avaliação chega; com o Provedor desligado o Painel diz que nada foi enviado e oferece **Ligar o Chess-API.com**.
- **Observar:** se percebe que navegar rápido não faz um pedido por Lance; se entende o texto de limite (em quantos segundos tentar); se volta à Engine local sem ajuda.
- **Tempo esperado:** 5 min.
- **Anotações:** (campos acima)

---

## Jornada 9. Engine online (Lichess BOT)

**Atenção.** Converter uma conta em BOT é **irreversível**. Use só uma conta criada para o teste e nunca a sua conta de uso. Exige conta **[conta]**, uma Engine instalada e uma segunda conta para desafiar.

### 9.1 Entender o que é a Engine online

- **Cenário:** "Quer deixar o seu computador aceitar desafios no Lichess com uma Engine sua."
- **Objetivo:** abrir a tela da Engine online sem ligar nada e explicar com suas palavras o que ela faz.
- **Sucesso:** diz que a Engine joga sozinha contra quem desafiar e que está **Parada** até ligar.
- **Observar:** se encontra a tela; se distingue Engine online de Engine remota (nomes parecidos); se o termo "BOT" é compreendido.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 9.2 Ligar, receber um desafio e parar

- **Cenário:** "Está pronto para receber um desafio."
- **Objetivo:** ligar a Engine online, receber um desafio de outra conta e, durante a Partida, pedir para parar de forma educada e depois à força.
- **Sucesso:** a tela mostra **Esperando desafios**, depois **Jogando 1**; **Parar** leva a **Parando** e **Parar já…** pede confirmação.
- **Observar:** se entende a diferença entre **Parar** e **Parar já…**; se os limites (ritmo, rating, quantas Partidas) são compreensíveis; se **Atividade recente** explica as recusas.
- **Tempo esperado:** 10 min.
- **Anotações:** (campos acima)

### 9.3 Converter a conta em BOT

- **Cenário:** "Sua conta de teste ainda não é BOT."
- **Objetivo:** ler o aviso e decidir se converte; se converter, usar apenas a conta de teste.
- **Sucesso:** a pessoa diz, antes de marcar, que a conversão não tem volta; só converte depois de marcar a confirmação.
- **Observar:** se o aviso é lido; se a confirmação marcada é entendida como confirmação e não como formalidade; se pensa em desistir.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

---

## Jornada 10. Limpar dados e desligar tudo

### 10.1 Limpar o que ficou guardado **[demo]**

- **Cenário:** "Quer apagar as respostas guardadas dos serviços, mas não as suas Partidas."
- **Objetivo:** limpar os dados guardados e confirmar o que sumiu e o que ficou.
- **Sucesso:** a janela diz o que some e o que fica; depois de limpar, as Partidas, os Exercícios e as Conexões continuam; a linha de avaliação em cache e os finais exatos deixam de vir do cache.
- **Observar:** se lê a janela de confirmação; se **Cancelar** é seguro; se entende "cache" ou se o rótulo precisa de outra palavra.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 10.2 Apagar as Partidas de uma Origem **[demo]**

- **Cenário:** "Não quer mais as Partidas que vieram do Chess.com."
- **Objetivo:** apagar só as Partidas importadas do Chess.com e conferir que as jogadas por você continuam.
- **Sucesso:** a janela mostra a contagem e a lista; só as do Chess.com somem.
- **Observar:** se acha a lista de Origens; se entende "Tudo de um Provedor"; se o medo de apagar Partida jogada aparece.
- **Tempo esperado:** 4 min.
- **Anotações:** (campos acima)

### 10.3 Desligar tudo

- **Cenário:** "Terminou os testes e quer que o programa volte a não falar com ninguém."
- **Objetivo:** deixar todos os Provedores desligados e provar isso.
- **Sucesso:** o indicador de rede diz **Rede: offline**; se houver Conexão autorizada, ela foi desconectada (a pessoa sabe a diferença entre desligar e desconectar).
- **Observar:** se desligar e desconectar são confundidos; se lembra de revogar no site do Lichess; se acredita no indicador.
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

### 10.4 Excluir o Perfil local de teste

- **Cenário:** "Quer apagar tudo o que o teste criou."
- **Objetivo:** excluir o Perfil local do teste, com tudo o que ele guarda.
- **Sucesso:** o Perfil some, junto com qualquer autorização guardada para ele.
- **Observar:** se encontra a gestão de Perfis locais; se a confirmação deixa claro o que será apagado; se entende que o Perfil em uso não pode ser excluído (é preciso trocar antes).
- **Tempo esperado:** 3 min.
- **Anotações:** (campos acima)

---

## Formulário de impressões (final da sessão)

Responda sem consultar o app. Escala: 1 (discordo totalmente) a 5 (concordo totalmente).

| # | Afirmação | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| 1 | Sempre soube se o programa estava falando com algum serviço. | | | | | |
| 2 | Entendi a diferença entre ligar um Provedor, vincular um nome e autorizar. | | | | | |
| 3 | Os textos da tela foram claros, sem palavras técnicas demais. | | | | | |
| 4 | Confiaria no programa para guardar a autorização de uma conta minha. | | | | | |
| 5 | Os avisos de consentimento me ajudaram a decidir (não foram só um obstáculo). | | | | | |
| 6 | Entendi por que algumas ajudas ficam bloqueadas durante uma Partida online. | | | | | |
| 7 | Achei as coisas onde esperava. | | | | | |
| 8 | Sempre soube como cancelar ou voltar atrás. | | | | | |

Perguntas abertas:

1. Qual foi o momento mais confuso da sessão?
2. Qual palavra da tela você trocaria por outra? Por qual?
3. Alguma ação lhe deu medo de perder dados ou de enviar algo sem querer? Qual?
4. O que você mais gostou?
5. O que faltou para você usar este programa no dia a dia?

Observador: nome __ data __ versão (commit) __ sistema __ tema/densidade usados __
