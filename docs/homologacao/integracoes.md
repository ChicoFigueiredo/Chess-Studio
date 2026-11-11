# Homologação das integrações (épico #211, Gates das etapas 1 a 3)

Roteiro manual das integrações com Provedores: Central de Conexões, Cofre, autorização do Lichess, Explorador, importações e pacotes de problemas. Reúne as verificações humanas pedidas nos PRs [#223](https://github.com/ChicoFigueiredo/Chess-Studio/pull/223), [#227](https://github.com/ChicoFigueiredo/Chess-Studio/pull/227), [#229](https://github.com/ChicoFigueiredo/Chess-Studio/pull/229) e [#235](https://github.com/ChicoFigueiredo/Chess-Studio/pull/235), e as decisões [CX](../decisoes/2026-11-11-conexoes.md), [CF](../decisoes/2026-11-11-cofre-e-autorizacao.md) e [AZ](../decisoes/2026-11-11-autorizar-lichess-na-central.md).

Vale para o commit `1626ebb` da `main`. O passo a passo para quem usa está em [`docs/integracoes/guia.md`](../integracoes/guia.md); os rótulos abaixo são os mesmos do guia e dos arquivos Fluent.

**Por que um roteiro manual.** A suíte automatizada nunca chama o serviço real (CX-17): usa servidor falso, relógio falso e cofre falso. Só uma pessoa, com internet e usuário de verdade no Lichess e no Chess.com, confirma que o serviço real ainda responde como as respostas gravadas. A autorização do Lichess e o cofre do sistema só se provam no computador do usuário.

Legenda: **UNVERIFIED** = pendente do usuário (não é PASS). Marque PASS ou FAIL, com a data, ao verificar. Em FAIL, anote o que apareceu e abra uma issue.

## Como preparar

1. Compile e abra o app: `cargo run -p chess-studio`.
2. Crie um Perfil local novo para o roteiro (Arquivo > Perfil local), para não misturar com seus dados.
3. Tenha à mão: um usuário do Lichess (de preferência sem nada importante) e um usuário público do Chess.com; um arquivo `.pgn` de cada origem (Queen Alice, ChessBase, TWIC, PGN Mentor); o arquivo de problemas do Lichess (`.csv` ou `.csv.zst`); um ID da FIDE.
4. Para o cofre próprio, use um computador (ou usuário do sistema) sem cofre do sistema, ou aceite que o item CX-HOM-08 só vale onde o sistema não oferece cofre.
5. Sempre que o item pedir "com o serviço real", ligue a internet. Nos demais, o servidor falso dos testes já cobre.

## Resumo

| ID | Recurso | Serviço real? | Status |
|---|---|---|---|
| CX-HOM-01 | Offline por padrão e estado da rede | não | UNVERIFIED |
| CX-HOM-02 | Central de Conexões: cartões, filtros e busca | não | UNVERIFIED |
| CX-HOM-03 | Ligar e desligar um Provedor | não | UNVERIFIED |
| CX-HOM-04 | Vincular nome ou ID público | não | UNVERIFIED |
| CX-HOM-05 | Testar a Conexão | sim | UNVERIFIED |
| CX-HOM-06 | Gerir Conexões vinculadas | não | UNVERIFIED |
| CX-HOM-07 | Cofre do sistema | sim (sistema) | UNVERIFIED |
| CX-HOM-08 | Cofre do Chess Studio e senha-mestra | não | UNVERIFIED |
| CX-HOM-09 | Autorizar o Lichess pelo navegador | sim | UNVERIFIED |
| CX-HOM-10 | Recusar, deixar vencer e cancelar a autorização | sim | UNVERIFIED |
| CX-HOM-11 | Autorizar com chave pessoal | sim | UNVERIFIED |
| CX-HOM-12 | Autorização expirada e renovação | sim | UNVERIFIED |
| CX-HOM-13 | Desconectar | sim | UNVERIFIED |
| CX-HOM-14 | Explorador, Aberturas: Minha Biblioteca | não | UNVERIFIED |
| CX-HOM-15 | Explorador, Aberturas: fontes do Lichess | sim | UNVERIFIED |
| CX-HOM-16 | Explorador sem Provedor ou sem autorização | não | UNVERIFIED |
| CX-HOM-17 | Explorador: filtros e memória por Perfil local | sim | UNVERIFIED |
| CX-HOM-18 | Explorador, Finais | sim | UNVERIFIED |
| CX-HOM-19 | Avaliação em cache na Análise | sim | UNVERIFIED |
| CX-HOM-20 | Origem, cache e sem internet | sim | UNVERIFIED |
| CX-HOM-21 | Trava de fair play | sim | UNVERIFIED |
| CX-HOM-22 | Importar Partidas do Lichess | sim | UNVERIFIED |
| CX-HOM-23 | Importar Partidas do Chess.com | sim | UNVERIFIED |
| CX-HOM-24 | Atualizar sem duplicar | sim | UNVERIFIED |
| CX-HOM-25 | Importar arquivos por origem | não | UNVERIFIED |
| CX-HOM-26 | Importação de arquivo: duplicatas, acentos, grande, cancelar | não | UNVERIFIED |
| CX-HOM-27 | Instalar problemas do Lichess | não | UNVERIFIED |
| CX-HOM-28 | Pacote: filtros, treino e remoção | não | UNVERIFIED |
| CX-HOM-29 | Limpar cache | não | UNVERIFIED |
| CX-HOM-30 | Linguagem, Temas e janela estreita | não | UNVERIFIED |
| CX-HOM-31 | Desligar tudo e apagar | não | UNVERIFIED |
| CX-HOM-32 | Rating FIDE no Perfil local | sim | UNVERIFIED |
| CX-HOM-33 | Limpar dados guardados | não | UNVERIFIED |
| CX-HOM-34 | Apagar Partidas importadas por origem | não | UNVERIFIED |
| CX-HOM-35 | Partida online: sem Conexão e Conexão de Engine | não | UNVERIFIED |
| CX-HOM-36 | Partida online contra a Engine do Lichess | sim | UNVERIFIED |
| CX-HOM-37 | Buscar adversário, jogar e terminar | sim | UNVERIFIED |
| CX-HOM-38 | Intenção enviada, recusa e queda de conexão | sim | UNVERIFIED |
| CX-HOM-39 | Empate, abandono, abortar e adversário ausente | sim | UNVERIFIED |
| CX-HOM-40 | Desafiar por nome e desafios recebidos | sim | UNVERIFIED |
| CX-HOM-41 | Correspondência: lista, fechar e reabrir o app | sim | UNVERIFIED |
| CX-HOM-42 | Trava de fair play na Partida online | sim | UNVERIFIED |
| CX-HOM-43 | Seletor de arquivo: escolher e Salvar como… | não (sistema) | UNVERIFIED |
| CX-HOM-44 | Baixar problemas do Lichess pela rede | sim | UNVERIFIED |
| CX-HOM-45 | Consentimento do Lichess cita tudo o que o app envia | não | UNVERIFIED |
| CX-HOM-46 | PlayStrategy: cartão na Central, ligar e testar | sim | UNVERIFIED |
| CX-HOM-47 | PlayStrategy: importar Partidas públicas e variantes puladas | sim | UNVERIFIED |
| CX-HOM-48 | PlayStrategy: Origem, Atualizar e duplicatas | sim | UNVERIFIED |
| CX-HOM-49 | Engine remota: escolher, aviso e consentimento | sim | UNVERIFIED |
| CX-HOM-50 | Engine remota: análise, limite e Provedor desligado | sim | UNVERIFIED |
| CX-HOM-51 | Engine remota sob a Trava de fair play | sim | UNVERIFIED |
| CX-HOM-52 | Engine online: ligar, esperar, jogar e parar | sim | UNVERIFIED |
| CX-HOM-53 | Engine online: limites, conta BOT e falha da Engine | sim | UNVERIFIED |
| CX-HOM-55 | FICS na Central: aviso, ligar e desligar | não | UNVERIFIED |
| CX-HOM-56 | FICS: entrar como visitante e sair | sim | UNVERIFIED |
| CX-HOM-57 | FICS: buscar adversário, jogar e terminar | sim | UNVERIFIED |
| CX-HOM-58 | FICS: desafios recebidos e variantes | sim | UNVERIFIED |
| CX-HOM-59 | FICS: queda de conexão durante a Partida | sim | UNVERIFIED |
| CX-HOM-60 | FICS: observar uma Partida | sim | UNVERIFIED |
| CX-HOM-72 | Central: filtros Com cadastro, Manual e por estado | não | UNVERIFIED |
| CX-HOM-73 | Licença dos dados e Origem (Central, rating FIDE, Engine remota) | não | UNVERIFIED |
| CX-HOM-74 | Textos das integrações sem "conta", "chaveiro" nem código HTTP | não | UNVERIFIED |

## Itens

### CX-HOM-01. Offline por padrão e estado da rede

- **Pré-condição:** instalação limpa ou Perfil local novo; nenhum Provedor ligado.
- **Passos:**
  1. Abra o app sem internet.
  2. Olhe a barra de status.
  3. Abra Arquivo > Conexões e confira o selo de cada cartão.
  4. Ligue um Provedor (CX-HOM-03) e olhe a barra de novo.
- **Esperado:** o app abre direto no tabuleiro, sem aviso de conexão. A barra diz **Rede: offline**. Todos os cartões que têm interruptor mostram **Desligado**. Depois de ligar um, a barra diz **Rede: 1 Provedor ligado**. Clicar no indicador abre a Central.
- **Com o serviço real:** com um monitor de rede (por exemplo, `ss -tnp` no Linux), o app não abre conexão nenhuma enquanto todos estão desligados.
- **Evidência:** [`no_network_without_consent.rs`](../../crates/chess-studio/tests/no_network_without_consent.rs), [`consent.rs`](../../crates/chess-net/tests/consent.rs), [`status.rs`](../../crates/chess-net/tests/status.rs).

### CX-HOM-02. Central de Conexões: cartões, filtros e busca

- **Pré-condição:** CX-HOM-01.
- **Passos:**
  1. Arquivo > Conexões.
  2. Role a página. Confira as seções **Jogar online**, **Dados e consultas**, **Engines**, **Importação de arquivos** e **Ratings**.
  3. Clique nos filtros **Gratuitos** e **Pagos ou sob aprovação**.
  4. Digite `Lichess` na busca.
  5. Abra **Detalhes** num cartão e feche.
- **Esperado:** gratuitos primeiro. Em cada seção: o que funciona, depois **Em breve**, depois **Requer aprovação**, os dois últimos em cartões menores sem interruptor. Cada selo tem ícone e palavra. Nenhum texto fala em "etapa", código de erro, "token" ou "conta".
- **Com o serviço real:** não se aplica.
- **Evidência:** [`docs/evidencias/central-conexoes/`](../evidencias/central-conexoes/README.md), [`connections/view.rs`](../../crates/chess-studio/src/connections/view.rs) (testes), [`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs) (testes).

### CX-HOM-03. Ligar e desligar um Provedor

- **Pré-condição:** Lichess desligado.
- **Passos:**
  1. Na Central, clique em **Detalhes** do Lichess e leia **O que é enviado**.
  2. Ligue o interruptor **Ligado**.
  3. Desligue-o.
  4. Abra o Painel Explorador e, na fonte **Todos os jogadores (Lichess)**, clique em **Ligar Lichess**.
  5. Na janela **Ligar Lichess?**, teste **Agora não**, depois repita e escolha **Ligar**.
- **Esperado:** com o Provedor desligado, o texto começa com "Nada sai do computador enquanto estiver desligado." e **Ao ligar:**. O selo muda para **Ligado** e volta a **Desligado**. A janela mostra **O que será enviado** e **Para onde vai**. **Agora não** não liga nada. A escolha vale por Perfil local.
- **Com o serviço real:** ao ligar, nenhuma conexão sai até você pedir uma consulta.
- **Evidência:** [`provider_consent.rs`](../../crates/chess-studio/src/provider_consent.rs), [`connections/mod.rs`](../../crates/chess-studio/src/connections/mod.rs) (testes), [`gate.rs`](../../crates/chess-net/tests/gate.rs).

### CX-HOM-04. Vincular nome ou ID público

- **Pré-condição:** Perfil local do roteiro.
- **Passos:**
  1. No cartão do Lichess, **Vincular nome**; digite seu usuário e um apelido; **Vincular**.
  2. Repita com o mesmo nome. Depois com um nome com espaço.
  3. Faça o mesmo no Chess.com.
  4. No cartão da FIDE, **Vincular ID** com um número.
- **Esperado:** o selo do cartão passa a **Ligado · nome vinculado** (e nunca a **Autorizado**). Nome repetido: "Este Perfil local já vincula esse nome neste Provedor." Nome com espaço: "Use só o nome ou ID, sem espaços, com até 64 caracteres." A Conexão aparece em **Conexões deste Perfil local**, com o selo **Padrão** na primeira.
- **Com o serviço real:** o vínculo não envia nada.
- **Evidência:** [`connections.rs`](../../crates/chess-store/tests/connections.rs), [`connections/mod.rs`](../../crates/chess-studio/src/connections/mod.rs) (testes).

### CX-HOM-05. Testar a Conexão

- **Pré-condição:** CX-HOM-04; internet.
- **Passos:**
  1. No cartão do Lichess, clique em **Testar**. Se o Provedor estiver desligado, responda **Ligar**.
  2. Repita no Chess.com e na FIDE (com o Lichess ligado).
  3. Vincule um nome que não existe e teste.
- **Esperado:** "Testado há N min: encontrado como NOME em Lichess." (e equivalentes). Nome inexistente: "não encontramos esse nome". FIDE: "ID confirmado".
- **Com o serviço real:** observe se o Lichess ou o Chess.com respondem no prazo; se pedem pausa (selo **Limitado · volta em N min**); se o nome é achado mesmo com maiúsculas diferentes. Se o ID da FIDE não vier, anote o texto exibido.
- **Evidência:** [`connections/probe.rs`](../../crates/chess-studio/src/connections/probe.rs), [`lichess_queries.rs`](../../crates/chess-providers/tests/lichess_queries.rs).

### CX-HOM-06. Gerir Conexões vinculadas

- **Pré-condição:** duas Conexões do mesmo Provedor.
- **Passos:**
  1. Em uma delas, **Tornar padrão**.
  2. Em **Ações da Conexão**, **Renomear apelido**; digite um apelido de 65 caracteres e depois um válido; **Salvar**.
  3. Em **Ações da Conexão**, **Remover**; teste **Manter** e depois **Remover**.
- **Esperado:** só uma Conexão tem **Padrão**. Apelido longo: "O apelido pode ter até 64 caracteres, sem caracteres de controle." Remover pede confirmação e diz que o nome continua existindo no Provedor.
- **Com o serviço real:** não se aplica.
- **Evidência:** [`connections.rs`](../../crates/chess-store/tests/connections.rs).

### CX-HOM-07. Cofre do sistema

- **Pré-condição:** computador com cofre do sistema (Secret Service no Linux, Credential Manager no Windows, Keychain no macOS), desbloqueado.
- **Passos:**
  1. Abra a Central e olhe a faixa do topo.
  2. Autorize o Lichess (CX-HOM-09).
  3. Feche e reabra o app.
  4. Bloqueie o cofre do sistema, reabra o app e abra a Central.
- **Esperado:** a faixa diz **Segredos guardados no cofre do sistema**. O app não pede senha-mestra. Depois de reabrir, o cartão continua **Autorizado**. Com o cofre do sistema bloqueado, o sistema pode pedir a senha dele; sem resposta, a faixa mostra **O cofre do sistema não respondeu** e nada novo é guardado.
- **Com o serviço real:** é o único modo de provar a integração com o cofre do sistema. Anote o sistema e a versão. Confira também que nenhum arquivo com a chave aparece na pasta de dados.
- **Evidência:** [`vault.rs`](../../crates/chess-vault/tests/vault.rs), [`static_secrets.rs`](../../crates/chess-vault/tests/static_secrets.rs), [`docs/evidencias/autorizar-lichess/`](../evidencias/autorizar-lichess/README.md).

### CX-HOM-08. Cofre do Chess Studio e senha-mestra

- **Pré-condição:** computador sem cofre do sistema, ou aceite o resultado como N/A.
- **Passos:**
  1. A faixa deve dizer **Sem cofre para guardar autorizações**. Clique em **Criar cofre…**.
  2. Tente criar com senha de 3 caracteres, com senhas diferentes e sem marcar **Entendi que não há como recuperar a senha**.
  3. Crie com uma senha válida.
  4. Clique em **Bloquear**, depois em **Desbloquear…** com a senha errada e com a certa.
  5. **Trocar senha-mestra…** e desbloqueie de novo com a nova.
  6. Feche e reabra o app.
- **Esperado:** cada erro tem sua mensagem (**A senha-mestra precisa ter pelo menos 8 caracteres.**, **As duas senhas não são iguais. Digite de novo.**, **Marque que você entendeu que a senha não pode ser recuperada.**, **Senha-mestra errada. Tente de novo.**). A janela não congela enquanto o cofre é criado (leva alguns segundos). Os campos são mascarados e não aceitam copiar. Depois de reabrir, a faixa diz **Cofre do Chess Studio bloqueado**.
- **Com o serviço real:** observe o tempo de criar e de desbloquear em máquina fraca. Observe que a senha não aparece em nenhum arquivo de log.
- **Evidência:** [`own_vault.rs`](../../crates/chess-vault/tests/own_vault.rs), [`static_secrets.rs`](../../crates/chess-vault/tests/static_secrets.rs), [`connections/auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs).

### CX-HOM-09. Autorizar o Lichess pelo navegador

- **Pré-condição:** CX-HOM-07 ou CX-HOM-08; internet; usuário do Lichess com sessão aberta no navegador.
- **Passos:**
  1. No cartão do Lichess (desligado), clique em **Autorizar…**.
  2. Responda **Ligar** em **Ligar Lichess?**.
  3. Se pedido, crie ou desbloqueie o cofre.
  4. Na página do Lichess que abre no navegador, confira o que ela pede e confirme.
  5. Volte ao app.
- **Esperado:** o app mostra **Esperando você autorizar no navegador** com o tempo restante, e depois **Autorizado como** SEU_USUÁRIO, **O Lichess confirmou que é você.** e a lista **O que esta autorização permite**. O cartão passa a **Autorizado**. Em **Conexões deste Perfil local** aparece **Autorizada pelo navegador**, **Usuário no Lichess** e o selo **Autorizada**.
- **Com o serviço real:** é o item mais importante do roteiro. Observe: se a página abre no navegador padrão; quais permissões o Lichess lista (só a de jogar Partidas pelo tabuleiro, nada mais); se o retorno ao `127.0.0.1` funciona com firewall ou VPN ligados; se o app volta ao primeiro plano; o tempo total. Se o navegador não abrir, teste **Copiar endereço**. Depois, confira nas preferências do Lichess que a autorização aparece.
- **Evidência:** [`auth.rs`](../../crates/chess-providers/tests/auth.rs), [`loopback.rs`](../../crates/chess-net/tests/loopback.rs), [`connections/auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs), [`docs/evidencias/autorizar-lichess/`](../evidencias/autorizar-lichess/README.md).

### CX-HOM-10. Recusar, deixar vencer e cancelar a autorização

- **Pré-condição:** CX-HOM-09 funcionando.
- **Passos:**
  1. Inicie **Autorizar…** e, no navegador, recuse.
  2. Inicie e clique em **Cancelar** no app.
  3. Inicie e deixe o tempo acabar.
  4. Inicie, copie o endereço e abra-o duas vezes.
- **Esperado:** **Você recusou a autorização** / "Nada foi guardado."; cancelar volta ao cartão sem guardar nada; **O tempo para autorizar acabou**; a segunda abertura do mesmo endereço não vale. O botão **Tentar de novo** reinicia. O cartão nunca fica **Autorizado**.
- **Com o serviço real:** observe se o Lichess mostra algo diferente de recusa quando se recusa; quanto tempo o app espera de verdade.
- **Evidência:** [`auth.rs`](../../crates/chess-providers/tests/auth.rs), [`loopback.rs`](../../crates/chess-net/tests/loopback.rs).

### CX-HOM-11. Autorizar com chave pessoal

- **Pré-condição:** CX-HOM-07 ou CX-HOM-08; chave criada no site do Lichess.
- **Passos:**
  1. Crie no Lichess uma chave só com a permissão de jogar Partidas pelo tabuleiro.
  2. No cartão, **Mais ações** > **Usar chave pessoal…**.
  3. Cole um texto sem sentido e clique em **Conferir e guardar**.
  4. Cole a chave certa e confirme.
  5. Repita com uma chave que tenha também a permissão de jogar como Engine.
- **Esperado:** o campo é mascarado. Texto sem sentido: "Isto não parece uma chave pessoal do Lichess." Chave certa: **Autorizado como** USUÁRIO e **O Lichess confirmou a chave pessoal.** Chave com permissão de Engine: "Esta chave não serve para o jogador…" e nada é guardado.
- **Com o serviço real:** observe se o Lichess devolve a lista de permissões da chave e se ela bate com o que o app exibe em **O que esta autorização permite**.
- **Evidência:** [`auth.rs`](../../crates/chess-providers/tests/auth.rs), [`connections/auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs).

### CX-HOM-12. Autorização expirada e renovação

- **Pré-condição:** uma Conexão autorizada.
- **Passos:**
  1. No site do Lichess, revogue a autorização (preferências, acesso por chaves).
  2. No app, na Conexão autorizada, **Ações da Conexão** > **Testar**.
  3. Clique em **Autorizar de novo…** e conclua.
- **Esperado:** o teste diz "O Lichess não aceita mais esta autorização. Autorize de novo." O cartão mostra **Autorização expirada**. Depois de renovar, é a mesma Conexão (mesmo apelido, mesma posição) e o selo volta a **Autorizado**.
- **Com o serviço real:** observe se o Lichess recusa na hora ou só depois de um tempo (cache).
- **Evidência:** [`authorized_connections.rs`](../../crates/chess-store/tests/authorized_connections.rs), [`gate.rs`](../../crates/chess-net/tests/gate.rs).

### CX-HOM-13. Desconectar

- **Pré-condição:** uma Conexão autorizada.
- **Passos:**
  1. **Ações da Conexão** > **Desconectar…** > **Desconectar**, com internet.
  2. Autorize de novo, desligue a internet e desconecte.
- **Esperado:** com internet: **Desconectado. O Lichess revogou a autorização e a chave foi apagada deste computador.** Sem internet: "A chave foi apagada deste computador, mas não conseguimos avisar o Lichess…", com a orientação de revogar à mão. A Conexão some nos dois casos.
- **Com o serviço real:** confira nas preferências do Lichess que a autorização sumiu no primeiro caso e continua no segundo (até você revogá-la).
- **Evidência:** [`auth.rs`](../../crates/chess-providers/tests/auth.rs) (revogação), [`connections/auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs).

### CX-HOM-14. Explorador, Aberturas: Minha Biblioteca

- **Pré-condição:** Biblioteca com algumas Partidas importadas (CX-HOM-25) e outra, vazia, em outro Perfil local.
- **Passos:**
  1. Aba Análise > grupo Explorador > **Abrir Explorador**.
  2. Aba **Aberturas**, fonte **Minha Biblioteca**.
  3. Jogue ou navegue pela Notação para mudar a Posição.
  4. Clique num Lance da tabela; depois em **Voltar à Partida**.
  5. Troque para o Perfil local vazio.
- **Esperado:** a tabela segue a Posição, inclusive ao navegar pela Notação. O rodapé diz **Origem: sua Biblioteca, sem internet**. Clicar num Lance mostra **Mostrando uma variante do Explorador, só para ver. A Partida não muda.** No Perfil vazio: **Nenhuma Partida sua passou por aqui**, com o botão **Abrir a Biblioteca**.
- **Com o serviço real:** não se aplica; confira que funciona sem internet.
- **Evidência:** [`docs/evidencias/explorador/`](../evidencias/explorador/README.md), [`openings.rs`](../../crates/chess-store/tests/openings.rs).

### CX-HOM-15. Explorador, Aberturas: fontes do Lichess

- **Pré-condição:** CX-HOM-09 (Lichess autorizado); internet.
- **Passos:**
  1. Fonte **Mestres (Lichess)** na Posição inicial e depois depois de `1.e4 c5`.
  2. Fonte **Todos os jogadores (Lichess)**.
  3. Fonte **Um jogador (Lichess)**: digite seu usuário, escolha **Como Brancas** e **Como Pretas**.
  4. Clique numa das **Partidas de referência**.
- **Esperado:** a tabela chega (aos poucos, com **Recebendo mais resultados…**) com Lance, Partidas, barra Brancas/Empates/Pretas e rating. O rodapé diz **Lichess · consultado agora** e a licença. Uma Partida de referência abre a busca na Biblioteca. Navegar rápido com a seta não dispara um pedido por Posição (espera cerca de 0,3 s).
- **Com o serviço real:** é aqui que se confirma o que a suíte só simula: o Explorador do Lichess só responde com autorização (desde 11/11/2026). Observe: se as três fontes respondem; o formato dos números (milhares); o tempo de resposta; se algum pedido volta como "pausa"; se "Um jogador" aceita nomes com maiúsculas.
- **Evidência:** [`explorer.rs`](../../crates/chess-providers/tests/explorer.rs), [`explorer/`](../../crates/chess-studio/src/explorer/mod.rs).

### CX-HOM-16. Explorador sem Provedor ou sem autorização

- **Pré-condição:** Lichess desligado e sem Conexão autorizada.
- **Passos:**
  1. Escolha a fonte **Mestres (Lichess)**.
  2. Clique em **Ligar Lichess** e responda **Ligar**.
  3. Observe o novo estado.
  4. Clique em **Abrir a Central de Conexões** e depois, de volta, em **Usar Minha Biblioteca**.
- **Esperado:** primeiro **O Lichess está desligado** com **Ligar Lichess**. Depois de ligar: **Requer autorizar sua conexão do Lichess** com **Abrir a Central de Conexões** e **Usar Minha Biblioteca**. Nenhuma consulta de Posição sai antes da autorização.
- **Com o serviço real:** com o monitor de rede, confirme que nada é enviado ao `explorer` enquanto o estado for "Requer autorizar".
- **Evidência:** [`docs/evidencias/explorador/`](../evidencias/explorador/README.md), [`gate.rs`](../../crates/chess-net/tests/gate.rs).

### CX-HOM-17. Explorador: filtros e memória por Perfil local

- **Pré-condição:** CX-HOM-15.
- **Passos:**
  1. Em **Todos os jogadores (Lichess)**, marque os ritmos **Blitz** e **Rápida** e uma faixa de rating.
  2. Em **Um jogador (Lichess)**, marque **Bullet**.
  3. Feche o app e reabra.
  4. Troque de Perfil local e volte.
- **Esperado:** os números mudam com os filtros. **Faixas de rating** só existem na fonte **Todos os jogadores**. Depois de reabrir, o Painel volta com a aba, a fonte, o jogador, a cor e os filtros. Outro Perfil local tem as suas próprias escolhas.
- **Com o serviço real:** confira se a contagem de Partidas diminui de forma plausível com cada filtro.
- **Evidência:** [`explorer/prefs.rs`](../../crates/chess-studio/src/explorer/prefs.rs).

### CX-HOM-18. Explorador, Finais

- **Pré-condição:** Lichess ligado; internet. Não precisa de autorização.
- **Passos:**
  1. Abra a aba **Finais**.
  2. Carregue uma Posição de 5 peças (por exemplo, Rei e Dama contra Rei e Torre).
  3. Carregue uma de 7 peças e outra de 8.
  4. Carregue um final em que a regra dos 50 Lances muda o resultado.
- **Esperado:** veredito em palavras (**Vitória das Brancas**, **Empate**, **Vitória das Pretas**), **Mate em N Lances**, DTZ e os Lances **do melhor ao pior**. Com 8 peças: **Disponível com 7 peças ou menos**, sem pedido de rede. Com a regra dos 50 Lances, aparece a nuance (**A vitória existe, mas a regra dos 50 Lances a transforma em empate.**).
- **Com o serviço real:** confira o veredito de 2 ou 3 Posições conhecidas num site de tablebase. Observe se o serviço de finais responde no mesmo prazo do Explorador.
- **Evidência:** [`lichess_queries.rs`](../../crates/chess-providers/tests/lichess_queries.rs), [`explorer/`](../../crates/chess-studio/src/explorer/mod.rs).

### CX-HOM-19. Avaliação em cache na Análise

- **Pré-condição:** Lichess ligado; internet; Painel Análise aberto.
- **Passos:**
  1. Na Posição inicial, olhe a linha **Em cache (Lichess)**.
  2. Navegue por uma abertura popular (`1.e4 e5 2.Nf3 Nc6`).
  3. Vá a uma Posição rara.
  4. Desligue o Lichess.
- **Esperado:** a linha mostra o valor (do ponto de vista das Brancas), **Profundidade** e a linha principal. Posição rara: **Sem avaliação em cache para esta Posição.** (estado normal). Lichess desligado: **O Lichess está desligado.** A linha nunca substitui nem se mistura com a Engine local.
- **Com o serviço real:** confira o sinal do valor contra um site de análise. Observe linhas com roque (o serviço pode mandar `e1h1`): a linha deve aparecer correta ou ser cortada sem erro.
- **Evidência:** [`eval.rs`](../../crates/chess-providers/src/eval.rs), [`lichess_queries.rs`](../../crates/chess-providers/tests/lichess_queries.rs).

### CX-HOM-20. Origem, cache e sem internet

- **Pré-condição:** CX-HOM-15 ou CX-HOM-18 feitos para uma Posição.
- **Passos:**
  1. Volte à mesma Posição.
  2. Desligue a internet e recarregue a Posição.
  3. Vá a uma Posição nunca consultada, ainda sem internet.
- **Esperado:** o rodapé diz **consultado há N min** ou **do cache, de** DD/MM. Sem internet, com cache: aviso **Sem conexão com a internet. Mostrando o que estava salvo, de** data. Sem internet e sem cache: **Sem conexão com a internet** e "Não há nada salvo para esta Posição."
- **Com o serviço real:** observe a idade do cache e se ele é renovado quando a internet volta.
- **Evidência:** [`cache.rs`](../../crates/chess-net/tests/cache.rs), [`limits.rs`](../../crates/chess-net/tests/limits.rs).

### CX-HOM-21. Trava de fair play

- **Pré-condição:** Lichess ligado e autorizado; Explorador e Painel Análise abertos.
- **Passos:**
  1. Inicie uma Partida na Simulação FIDE (ou Eliminação Imediata) e faça um Lance.
  2. Olhe o Explorador, a linha **Em cache (Lichess)**, a aba Biblioteca e a barra de status.
  3. Tente a Ação **Importar do Lichess ou Chess.com**.
  4. Termine a Partida (abandone).
  5. Repita com uma Partida Assistida.
- **Esperado:** na Partida competitiva, o Explorador mostra **Indisponível durante a Partida**, a avaliação em cache diz **Indisponível durante a Partida.**, a Biblioteca e a importação ficam desabilitadas com o motivo, e a barra de status mostra **Trava de fair play**. Nenhuma consulta sai. Ao terminar a Partida, tudo volta. Na Partida Assistida, a trava não liga.
- **Com o serviço real:** com o monitor de rede, confirme que nenhum pedido ao Lichess sai durante a Partida competitiva, nem com o Painel reaberto, nem pelo menu.
- **Evidência:** [`guard.rs`](../../crates/chess-net/src/guard.rs), [`gate.rs`](../../crates/chess-net/tests/gate.rs), [`net_service.rs`](../../crates/chess-studio/src/net_service.rs).
- **Limite conhecido:** este item cobre a Partida competitiva local. A Partida online contra uma pessoa tem o item próprio, CX-HOM-39.

### CX-HOM-22. Importar Partidas do Lichess

- **Pré-condição:** Lichess ligado; internet; um usuário com Partidas públicas.
- **Passos:**
  1. Aba Biblioteca > **Importar do Lichess ou Chess.com**.
  2. Em **De onde importar?**, confira **Lichess (online)**.
  3. Digite o usuário (ou **Usar … (sua Conexão)**), período **Últimos 30 dias**, máximo 50, **Importar**.
  4. Repita e clique em **Cancelar** no meio.
  5. Digite um usuário que não existe.
- **Esperado:** aparecem **Recebidas**, **Novas** e **Repetidas** subindo. Ao final: **Importação de … concluída**. Cada Partida tem a Origem (Lichess, API, a data, licença CC0). Cancelar mantém o que já veio e diz que cancelou. Usuário inexistente: **Não existe um jogador chamado …**.
- **Com o serviço real:** observe a velocidade; se o Lichess pede pausa (**O serviço pediu uma pausa**); se as Partidas chegam com ritmo, Elo e abertura (filtre a Biblioteca por ritmo e ECO); se o número bate com o do site.
- **Evidência:** [`games.rs`](../../crates/chess-providers/tests/games.rs), [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs), [`online_import.rs`](../../crates/chess-studio/src/online_import.rs).

### CX-HOM-23. Importar Partidas do Chess.com

- **Pré-condição:** Chess.com ligado; internet; um usuário público com Partidas.
- **Passos:** como CX-HOM-22, escolhendo **Chess.com (online)**. Teste também o filtro **Cor** e o **Ritmo**.
- **Esperado:** o mesmo comportamento. A nota de licença do Chess.com fala em termos de uso e uso pessoal.
- **Com o serviço real:** o Chess.com não tem filtro de período no servidor: o app pede os arquivos mensais, um de cada vez, e filtra aqui. Observe o tempo de um histórico longo e se o app respeita as pausas do serviço. Anote Partidas de variantes (devem ser recusadas, não importadas).
- **Evidência:** [`games.rs`](../../crates/chess-providers/tests/games.rs), [`chesscom.rs`](../../crates/chess-providers/src/chesscom.rs).

### CX-HOM-24. Atualizar sem duplicar

- **Pré-condição:** CX-HOM-22 feito para um usuário.
- **Passos:**
  1. Jogue uma Partida no Lichess com esse usuário (ou espere uma nova).
  2. Volte à importação e clique em **Atualizar**.
  3. Clique em **Atualizar** de novo.
- **Esperado:** o botão **Atualizar** aparece, com a dica "Traz só as Partidas jogadas desde DATA". A primeira vez traz só a Partida nova; a segunda traz zero novas. Nada duplica.
- **Com o serviço real:** observe se a Partida mais recente chega logo depois de terminar no site.
- **Evidência:** [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs), [`online_import_ui.rs`](../../crates/chess-studio/src/online_import_ui.rs).

### CX-HOM-25. Importar arquivos por origem

- **Pré-condição:** um arquivo `.pgn` de cada origem: Queen Alice, ChessBase, TWIC, PGN Mentor.
- **Passos:**
  1. Painel Biblioteca > aba Partidas > **Importar PGN…**, com **Arquivo PGN** em **De onde importar?**.
  2. Em **De onde veio o arquivo?**, escolha a origem do arquivo.
  3. Leia a nota de licença.
  4. Digite o caminho, ou clique num arquivo listado, e clique em **Importar**.
  5. Repita para cada origem. Importe também um `.pgn.gz`.
- **Esperado:** a nota de licença muda com a origem (por exemplo, **TWIC: uso pessoal. Não redistribua as Partidas.**). O resumo diz **N novas, N repetidas, N com erro**. A Origem aparece no detalhe de cada Partida, com o modo **importação manual guiada** (ou **arquivo**).
- **Com o serviço real:** é aqui que se vê arquivos de verdade. Anote, para cada origem: se o arquivo abriu; quantas Partidas deram erro e por quê; se os acentos ficaram certos; se o botão **Abrir instruções** do cartão correspondente leva à página certa.
- **Evidência:** [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs), [`pgn_import.rs`](../../crates/chess-store/tests/pgn_import.rs), [`library_ui.rs`](../../crates/chess-studio/src/library_ui.rs).

### CX-HOM-26. Importação de arquivo: duplicatas, acentos, arquivo grande, cancelar

- **Pré-condição:** CX-HOM-25.
- **Passos:**
  1. Importe de novo o mesmo arquivo.
  2. Importe um arquivo em Windows-1252/Latin-1 com acentos.
  3. Importe um arquivo grande (dezenas de milhares de Partidas, por exemplo um TWIC).
  4. Clique em **Cancelar** no meio.
- **Esperado:** a repetição diz **0 novas** e N repetidas. O arquivo antigo mostra o aviso **N Partidas não estavam em UTF-8 e foram convertidas…**. A janela continua respondendo e a barra de andamento avança. Cancelar mantém o que foi lido.
- **Com o serviço real:** anote o tempo e se a janela travou em máquina fraca.
- **Evidência:** [`stream.rs`](../../crates/chess-pgn/tests/stream.rs), [`stream_boundaries.rs`](../../crates/chess-pgn/tests/stream_boundaries.rs), [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs).

### CX-HOM-27. Instalar problemas do Lichess

- **Pré-condição:** o arquivo de problemas do Lichess no computador (`.csv` ou `.csv.zst`).
- **Passos:**
  1. Painel Treino > **Pacotes**.
  2. Em **Instalar problemas do Lichess**, digite o caminho do arquivo.
  3. Em **Temas**, digite `fork`; em **Quantidade máxima**, `200`; **Instalar pacote**.
  4. Repita e clique em **Cancelar leitura**.
  5. Tente com um arquivo que não é de problemas.
- **Esperado:** **Lendo: N problemas, N selecionados**, e no fim **Pacote … instalado: N Exercícios**, com os descartados. Cancelar: **Leitura cancelada: nada foi instalado.** Arquivo errado: **Não foi possível instalar: …** ou **Nenhum problema atende à seleção**. O pacote aparece em **Pacotes instalados** com **Origem: Lichess, licença CC0**.
- **Com o serviço real:** use o arquivo completo (milhões de linhas). Anote o tempo, o pico de memória e se a janela responde. Confira se o primeiro Exercício bate com o problema no site (o primeiro Lance do arquivo é do adversário).
- **Evidência:** [`lichess.rs`](../../crates/chess-training/tests/lichess.rs), [`packs.rs`](../../crates/chess-training/tests/packs.rs), [`training_ui/packs.rs`](../../crates/chess-studio/src/training_ui/packs.rs).

### CX-HOM-28. Pacote: filtros, treino e remoção

- **Pré-condição:** CX-HOM-27.
- **Passos:**
  1. Na lista de Exercícios, filtre por pacote, tema e dificuldade.
  2. Resolva um Exercício do pacote.
  3. Em **Pacotes**, **Remover pacote** > **Remover**.
  4. Veja o histórico de Resoluções e a Evolução.
  5. Reinstale o mesmo pacote.
- **Esperado:** os Exercícios do pacote ficam sob **Pacote: …**, separados dos de fábrica, com a Origem. Depois de remover, o Exercício resolvido some da lista, mas a Resolução continua no histórico e na Evolução. Reinstalar traz o Exercício de volta com o histórico.
- **Com o serviço real:** não se aplica.
- **Evidência:** [`training_packs.rs`](../../crates/chess-store/tests/training_packs.rs), [`training_ui/packs.rs`](../../crates/chess-studio/src/training_ui/packs.rs).

### CX-HOM-29. Limpar cache

- **Pré-condição:** CX-HOM-15 ou CX-HOM-18 feitos.
- **Passos:**
  1. No cartão do Lichess, **Mais ações** > **Limpar cache**.
  2. Clique de novo.
  3. Volte a uma Posição já consultada, com a internet desligada.
- **Esperado:** a primeira vez diz **N itens do cache removidos.**; a segunda, **O cache já estava vazio.**. Sem internet, a Posição não tem mais dado salvo. Os dados de outro Perfil local não mudam.
- **Com o serviço real:** não se aplica.
- **Evidência:** [`cache.rs`](../../crates/chess-net/tests/cache.rs).

### CX-HOM-30. Linguagem, Temas e janela estreita

- **Pré-condição:** nenhuma.
- **Passos:**
  1. Percorra a Central, o Explorador e as janelas de autorização, cofre e **Ligar Lichess?** nos Temas Escuro, Claro, Pretas e Brancas.
  2. Alterne a Densidade (Confortável e Compacta).
  3. Reduza a janela a 800 x 600 e a 1024 x 576.
  4. Navegue só pelo teclado (Tab, Espaço, Enter, Esc).
- **Esperado:** nenhum texto de interface fala em "etapa", "token", "401", "conta", "login" ou "credencial". Todo selo tem ícone e palavra. Nenhum texto passa da borda; abaixo de 980 pt a Central usa uma coluna e rola inteira. O foco do teclado aparece em todo controle, e Esc fecha menus e janelas.
- **Com o serviço real:** não se aplica. É aprovação visual do usuário (PRs com `ready-for-human`).
- **Evidência:** [`docs/evidencias/central-conexoes/`](../evidencias/central-conexoes/README.md), [`docs/evidencias/autorizar-lichess/`](../evidencias/autorizar-lichess/README.md), [`docs/evidencias/explorador/`](../evidencias/explorador/README.md).

### CX-HOM-31. Desligar tudo e apagar

- **Pré-condição:** os itens anteriores feitos no Perfil local do roteiro.
- **Passos:**
  1. Desligue o interruptor de cada Provedor ligado.
  2. Confira a barra de status.
  3. Desconecte a autorização (CX-HOM-13).
  4. Exclua o Perfil local do roteiro (Arquivo > Perfil local).
  5. Olhe a pasta de dados do app.
- **Esperado:** a barra diz **Rede: offline**. Nenhuma conexão sai (monitor de rede). Excluir o Perfil apaga antes qualquer autorização guardada no cofre; se o cofre recusar, o Perfil não é excluído. Na pasta `cofre`, não sobra segredo daquele Perfil.
- **Com o serviço real:** confira nas preferências do Lichess que nenhuma autorização do Chess Studio sobrou.
- **Evidência:** [`authorized_connections.rs`](../../crates/chess-store/tests/authorized_connections.rs), [`connections/auth_tests.rs`](../../crates/chess-studio/src/connections/auth_tests.rs).

### CX-HOM-32. Rating FIDE no Perfil local

- **Pré-condição:** um ID FIDE real (ou o de um amigo); CX-HOM-04 feito.
- **Passos:**
  1. No cartão da FIDE, busque o jogador pelo nome e clique em **Vincular este ID**. Com o Lichess desligado, responda **Ligar** ao diálogo.
  2. Confira nome, federação, título e as caixas **Standard**, **Rápido** e **Blitz**, e a linha **FIDE, via Lichess, consultado ...**.
  3. Clique em **Atualizar**.
  4. Abra **Arquivo > Perfil local**.
  5. Desligue o Lichess e a internet, feche e abra o app, e olhe de novo.
  6. Inicie uma Partida competitiva e clique em **Atualizar**.
- **Esperado:** os três ritmos são caixas separadas; um ritmo sem rating mostra **—**. O texto avisa que a data é a da consulta. **Atualizar** diz que atualizou ou que os ratings continuam os mesmos. Sem internet o último valor continua à vista, com a data. Na Partida competitiva a consulta funciona (não é bloqueada). **Agora não** no diálogo não envia nada.
- **Com o serviço real:** compare com o site da FIDE. O Lichess pode estar um mês atrás; anote a diferença, se houver.
- **Evidência:** [`fide_and_bulk.rs`](../../crates/chess-store/tests/fide_and_bulk.rs), [`fide_profile.rs`](../../crates/chess-studio/src/fide_profile.rs), [`lichess_queries.rs`](../../crates/chess-providers/tests/lichess_queries.rs), [`docs/evidencias/rating-fide-dados-origem/`](../evidencias/rating-fide-dados-origem/README.md).

### CX-HOM-33. Limpar dados guardados

- **Pré-condição:** CX-HOM-15 e CX-HOM-22 feitos (há cache e Partidas importadas).
- **Passos:**
  1. Na Central, clique em **Limpar dados guardados…** e leia a janela.
  2. Clique em **Cancelar** e confira que nada mudou.
  3. Repita e clique em **Limpar dados guardados**.
  4. Abra a Biblioteca, as Partidas jogadas e o rating FIDE do Perfil local.
- **Esperado:** a janela diz o que some e o que fica. Cancelar não apaga. Depois de limpar, o app diz quantas respostas apagou; as Partidas jogadas e importadas, os Exercícios, as Conexões e o último rating FIDE continuam.
- **Com o serviço real:** não se aplica.
- **Evidência:** [`stored_data.rs`](../../crates/chess-studio/src/connections/stored_data.rs).

### CX-HOM-34. Apagar Partidas importadas por origem

- **Pré-condição:** Partidas importadas de dois Provedores e de um arquivo (CX-HOM-22, 23 e 25) e ao menos uma Partida jogada no app.
- **Passos:**
  1. Na Biblioteca, abra a aba **Origens**.
  2. Marque uma importação e clique em **Apagar marcadas…**; leia a janela e confirme.
  3. Marque **Tudo de um Provedor** e confirme.
  4. Se tiver uma Partida que veio de dois lugares, apague uma das origens.
- **Esperado:** a janela traz a contagem e a lista do que será apagado. Marcar o Provedor cobre as importações dele. As Partidas jogadas não aparecem na aba e continuam. A Partida de duas origens fica, e a janela diz que ficou.
- **Com o serviço real:** não se aplica.
- **Evidência:** [`fide_and_bulk.rs`](../../crates/chess-store/tests/fide_and_bulk.rs), [`library_bulk.rs`](../../crates/chess-studio/src/library_bulk.rs).
### CX-HOM-35. Partida online: sem Conexão e Conexão de Engine

- **Pré-condição:** Perfil local do roteiro sem Conexão do Lichess autorizada.
- **Passos:**
  1. Início > Nova Partida > **Online (Lichess)** (ou a Paleta: "Buscar adversário").
  2. Clique em **Abrir a Central**.
  3. Autorize só a **Engine online** (Conexão de Engine) e repita o passo 1.
- **Esperado:** a tela **Partida online no Lichess** explica **Autorize o Lichess para jogar** e leva à Central. Com só a Conexão de Engine, continua pedindo a Conexão do jogador: a de Engine nunca joga como pessoa. Nada sai para o Lichess.
- **Evidência:** [`online/tests.rs`](../../crates/chess-studio/src/online/tests.rs) (`only_the_authorized_player_connection_plays_never_the_engine_one`).

### CX-HOM-36. Partida online contra a Engine do Lichess

- **Pré-condição:** Lichess ligado e a Conexão do jogador autorizada.
- **Passos:**
  1. Início > grupo **Online (Lichess)** > **Contra a Engine do Lichess**.
  2. Nível 1, cor Brancas, ritmo Rápida 10+5; **Jogar**.
  3. Jogue alguns Lances pelo tabuleiro 3D e pelo 2D, arrastando e com dois cliques. Promova um peão se der.
  4. Ligue a Análise.
  5. Abandone (confirme) ou jogue até o fim.
- **Esperado:** a Partida aparece no tabuleiro do seu lado, com o Painel **Partida online** (selos **Lichess**, **Ao vivo**, **Sem rating**), os relógios do Lichess no Painel Relógio e no relógio 3D, os nomes do Lichess no Painel Relógio e a Notação. Cada Lance aparece com a peça no destino e **Enviando …** até o Lichess confirmar. A Análise roda (contra a Engine do Lichess não há Trava). O fim vem do Lichess; a Partida fica na Biblioteca com **Partida online · Lichess · lichess.org/…**.
- **Com o serviço real:** confira no site que a Partida tem os mesmos Lances.

### CX-HOM-37. Buscar adversário, jogar e terminar

- **Passos:**
  1. Início > **Buscar adversário**; Rápida 10+5, sem rating; **Buscar adversário**.
  2. Espere (o tempo de espera conta em números que não pulam) e, uma vez, **Cancelar busca**; busque de novo.
  3. Jogue a Partida até o fim (mate, tempo ou abandono de alguém).
  4. Depois do fim, **Analisar** e **Ver na Biblioteca** no Painel Partida online.
- **Esperado:** a busca mostra **Procurando adversário** e some ao cancelar. Pareado, a Partida vai ao tabuleiro sozinha. Blitz não aparece na busca (o Lichess só busca a partir do rápido). Durante a Partida, Engines, Explorador, finais e análise ficam desligados e a barra de status mostra **Trava de fair play**. O resultado e o motivo são os do Lichess; só então a Análise liga. A Partida está na Biblioteca com a Origem.

### CX-HOM-38. Intenção enviada, recusa e queda de conexão

- **Passos:**
  1. Numa Partida online contra pessoa, faça um Lance e observe o Painel até a confirmação.
  2. Desligue a internet por ~20 s no meio da Partida (de preferência logo depois de soltar uma peça) e religue.
  3. Faça o adversário terminar a Partida (abandono) enquanto você está sem internet, e religue.
- **Esperado:** o Lance fica no destino com **Enviando …** até o Lichess confirmar. Sem internet, o selo vira **Reconectando em … s** e o Painel diz que nenhum Lance é reenviado. Ao voltar, o tabuleiro fica como o Lichess diz: se o Lance não chegou, a peça volta e o aviso pede para jogar de novo; nunca há Lance duplicado. Se a Partida terminou sem você, o resultado aparece ao reconectar.

### CX-HOM-39. Empate, abandono, abortar e adversário ausente

- **Passos:**
  1. Logo no começo, **Abortar** (antes de cada lado jogar).
  2. Em outra Partida, **Propor empate**; peça ao adversário para propor e use **Aceitar** e **Recusar**.
  3. **Abandonar** e responda **Continuar jogando**; depois **Abandonar** de novo e confirme.
  4. Peça ao adversário para fechar o navegador e espere.
- **Esperado:** nada muda até o Lichess confirmar. A Partida abortada fica sem resultado. **Abandonar** sempre pede confirmação. Com o adversário fora, aparece **O adversário saiu da Partida** e **Reclamar vitória** só fica disponível quando o Lichess permitir.

### CX-HOM-40. Desafiar por nome e desafios recebidos

- **Passos:**
  1. **Desafiar** um usuário seu de teste, em Blitz 3+2; depois **Cancelar desafio**; desafie de novo e aceite no outro usuário.
  2. Do outro usuário, desafie você: em xadrez padrão e em Chess960.
  3. Com uma Partida competitiva local em andamento, peça outro desafio.
- **Esperado:** o desafio aparece como **Aguardando resposta** e some ao cancelar. O desafio recebido traz um aviso discreto no canto (**… desafia você**, com **Ver**), que nunca aparece durante outra Partida competitiva (o desafio espera na aba **Desafiar**). **Recusar** usa o motivo escolhido. O de Chess960 diz que o app só joga xadrez padrão e não tem **Aceitar**.

### CX-HOM-41. Correspondência: lista, fechar e reabrir o app

- **Passos:**
  1. **Buscar adversário** > Correspondência, 3 dias por Lance (ou desafie seu usuário de teste em correspondência).
  2. Quando a Partida começar, faça um Lance e feche o app.
  3. Do outro usuário, responda no site.
  4. Abra o app.
  5. **Atualizar** no aviso; depois **Partidas em andamento** > **Abrir**.
- **Esperado:** ao abrir, nada sai para a rede: só o aviso **Partidas online em andamento** com **Atualizar**. A lista mostra de quem é a vez e quanto falta. **Abrir** traz o Lance que o adversário fez com o app fechado, na mesma Partida (sem duplicar na Biblioteca). Uma Partida terminada enquanto o app estava fechado mostra o resultado ao atualizar.

### CX-HOM-42. Trava de fair play na Partida online

- **Passos:**
  1. Numa Partida online contra pessoa em tempo real: tente ligar a Análise, abrir o Explorador, ver a avaliação em cache, a aba Finais, a Biblioteca do Explorador e o Painel Aberturas; revise Lances anteriores.
  2. Abra outra Partida da Biblioteca com a Partida online ainda em andamento.
  3. Com uma correspondência em andamento fora do tabuleiro: estude outra abertura com a Análise; depois cole o FEN da Posição atual da correspondência num tabuleiro de estudo.
- **Esperado:** em tempo real nada calcula, em Posição nenhuma, enquanto o Lichess não confirmar o fim (abandonar ou fechar o Painel não solta). Destinos legais, último Lance e coordenadas continuam. Na correspondência, outras Partidas e Posições ficam livres; a Posição da Partida (e o que se joga a partir dela) mostra **Posição de uma Partida em andamento** e nada é consultado.
- **Com o serviço real:** com o monitor de rede, nenhum pedido ao Explorador, à avaliação em cache ou aos finais sai nesses casos.
- **Evidência:** [`fair_play.rs`](../../crates/chess-studio/src/fair_play.rs), [`online/tests.rs`](../../crates/chess-studio/src/online/tests.rs), [decisões PO-13 a PO-19](../decisoes/2026-11-11-partida-online.md).

### CX-HOM-43. Seletor de arquivo: escolher e Salvar como…

- **Pré-condição:** um computador com janela de escolha de arquivo do sistema (portal do desktop no Linux; nativa no Windows e no macOS). Repita, se puder, num sistema sem ela (Linux sem portal e sem `zenity`).
- **Passos:**
  1. Em **Importar arquivo** (Biblioteca) e em **Instalar problemas** (Treino > Pacotes), clique em **Escolher arquivo…**: escolha um arquivo, depois abra de novo e cancele.
  2. No Painel Revisão, em **Exportar PGN**, clique em **Salvar como…**, mude a pasta e o nome, confirme. Repita escolhendo um arquivo que já existe (o sistema pergunta se substitui).
  3. Termine uma Partida (xeque-mate ou abandono): no diálogo de fim, clique em **Salvar como…**; depois em **Exportar PGN** (vai para Documentos).
  4. No Ribbon, aciona **Exportar PGN**: a janela de **Salvar como** abre com o nome `partida-AAAA-MM-DD-HHMMSS.pgn`.
  5. Com a Súmula aberta e a notação oculta, confira o diálogo de fim e o Painel Revisão.
  6. Sem janela de escolha no sistema: confira que os botões **Salvar como…** somem, o campo de caminho continua e **Exportar PGN** do Ribbon grava em Documentos.
- **Esperado:** o caminho escolhido aparece no campo (itens 1) ou na mensagem **Partida salva em …** (itens 2 a 4); cancelar não muda nada; o arquivo sai com a extensão `.pgn` mesmo se você não a digitou. Com a notação oculta pela Súmula, o **Salvar como…** do fim de Partida fica desativado e o do Painel Revisão não aparece (o PGN revelaria a notação).
- **Evidência:** [`file_picker.rs`](../../crates/chess-studio/src/file_picker.rs), [`fen_pgn.rs`](../../crates/chess-studio/src/fen_pgn.rs) (testes com seletor falso), [capturas](../evidencias/acabamento-etapa-5/README.md).

### CX-HOM-44. Baixar problemas do Lichess pela rede

- **Pré-condição:** internet; Lichess desligado na Central; Perfil local novo.
- **Passos:**
  1. Treino > Pacotes > **Baixar problemas do Lichess**: escolha poucos temas e uma quantidade pequena (200); clique em baixar. O diálogo **Ligar Lichess?** abre: confira que a lista **O que será enviado** cita o download de problemas; clique em **Ligar**.
  2. Acompanhe o andamento; deixe terminar e abra o pacote em **Pacotes instalados** (**Origem: Lichess, licença CC0**).
  3. Repita e **Cancele** no meio: nada é instalado.
  4. Desligue a internet no meio de outro download: a mensagem manda conferir a internet e diz que nada foi instalado.
  5. Se o Lichess mandar o arquivo para outro endereço (redirecionamento), observe o resultado.
- **Esperado:** o arquivo vem do endereço de downloads do Lichess; só é aceito redirecionamento para outro endereço de download do próprio Lichess (até 3 saltos, sem porta nem parâmetros); qualquer outro destino mostra **O Lichess mandou o arquivo para um endereço que o Chess Studio não aceita, por segurança … nada foi instalado.** (decisão RD-1).
- **Com o serviço real:** com o monitor de rede, as conexões vão só para `database.lichess.org` e nenhum dado do Perfil local sai. Anote se houve redirecionamento e para qual endereço: o comportamento real é UNVERIFIED até este passo.
- **Evidência:** [`download.rs`](../../crates/chess-net/tests/download.rs) (redirecionamento com servidor falso), [`2026-11-11-acabamento-etapa-5.md`](../decisoes/2026-11-11-acabamento-etapa-5.md), [capturas de #238](../evidencias/baixar-problemas/README.md).

### CX-HOM-45. Consentimento do Lichess cita tudo o que o app envia

- **Passos:**
  1. Com o Lichess desligado, abra o Explorador (ou **Baixar problemas**): o diálogo **Ligar Lichess?** abre.
  2. Leia **O que será enviado** e **Para onde vai**; abra também o cartão do Lichess na Central (Detalhes).
- **Esperado:** uma frase para cada uso: Explorador, avaliação em cache, finais, consulta de Partidas e IDs FIDE públicos, download de problemas e Partidas online (com a Conexão autorizada); nenhuma sigla técnica (sem "HTTP", "etapa" ou nome de endpoint). O mesmo texto vale no cartão da Central.
- **Evidência:** teste `every_ready_network_service_has_a_consent_line` em [`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs); [capturas](../evidencias/acabamento-etapa-5/README.md).
### CX-HOM-46. PlayStrategy: cartão na Central, ligar e testar

- **Passos:**
  1. **Arquivo > Conexões**: abra o cartão do **PlayStrategy**. Com tudo desligado, leia "O que será enviado".
  2. Ligue o Provedor pelo Switch e confirme o aviso. Vincule o nome de um usuário público do PlayStrategy e use **Testar**.
  3. Teste também um nome que não existe.
- **Esperado:** o cartão está completo (não mais em "Em breve"), com Switch, e diz que não há Partida online no PlayStrategy nesta versão. Desligado, nada sai. O teste confirma o nome e mostra "PlayStrategy" como fonte; o nome inexistente dá "Nome não encontrado".
- **Com o serviço real:** os formatos do PlayStrategy são os da família do Lichess, mas **só foram escritos a partir do contrato e de respostas sintéticas** (a página de documentação não entrega o texto das rotas): confirme que `/api/user/{nome}` responde assim.
- **Evidência:** [`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs), [`probe.rs`](../../crates/chess-studio/src/connections/probe.rs), [capturas](../evidencias/playstrategy-central-importacao/README.md).

### CX-HOM-47. PlayStrategy: importar Partidas públicas e variantes puladas

- **Passos:**
  1. **Biblioteca > Importar > PlayStrategy (online)**, com o Provedor desligado: o botão pede para ligar e nada é pedido.
  2. Ligue, informe um usuário público que jogue também variantes (damas, shogi, Crazyhouse...) e importe com período e ritmo.
- **Esperado:** só as Partidas de xadrez padrão entram; o resumo diz quantas Partidas de variantes foram puladas. O formulário avisa que só xadrez padrão é importado e que não há Partida online no PlayStrategy.
- **Com o serviço real:** confirme que `/api/games/user/{nome}` aceita os filtros (`since`, `perfType`, `max`) e que o campo `variant` de cada linha vem como `standard`/`fromPosition` para xadrez; se o PlayStrategy usar outros nomes, as Partidas de xadrez seriam puladas por engano.
- **Evidência:** [`online_import.rs`](../../crates/chess-studio/src/online_import.rs) (testes com servidor falso), [`catalog.rs`](../../crates/chess-net/src/catalog.rs).

### CX-HOM-48. PlayStrategy: Origem, Atualizar e duplicatas

- **Passos:**
  1. Depois do item anterior, abra uma Partida importada: a Origem diz "PlayStrategy · API".
  2. Na aba **Origens** da Biblioteca, confira o grupo **PlayStrategy**.
  3. Volte ao formulário e use **Atualizar**; depois importe o mesmo nome de novo.
- **Esperado:** Origem e aba Origens mostram "PlayStrategy" (não "Lichess" nem "Outro"). **Atualizar** pede só o que veio depois da última Partida; repetir a importação não duplica nada (as Partidas voltam como repetidas).
- **Evidência:** [`origin.rs`](../../crates/chess-store/src/origin.rs), [`import_origin.rs`](../../crates/chess-store/tests/import_origin.rs).
### CX-HOM-49. Engine remota: escolher, aviso e consentimento

- **Passos:**
  1. Com o Chess-API.com **desligado** na Central, abra o Painel **Análise** e abra a lista **Engine**.
  2. Escolha **Stockfish (Chess-API.com) · Remota**.
  3. Leia o aviso e clique em **Cancelar**; repita e clique em **Continuar**.
  4. No diálogo **Ligar Chess-API.com?**, clique em **Agora não**; repita até **Ligar**.
  5. Feche e reabra o app.
- **Esperado:** a Engine remota aparece numa linha separada, nunca marcada como padrão. O aviso diz que a Posição sai do computador, para o Chess-API.com, e que só o FEN vai. **Cancelar** não muda nada e nada sai para a rede. **Continuar** abre o consentimento do Provedor; **Agora não** deixa a Engine local. Só **Ligar** escolhe a Engine remota e deixa o selo **Remota · Chess-API.com** com o lembrete no Painel. Ao reabrir o app, a Engine local volta a ser a escolhida e o Chess-API.com continua ligado.
- **Com o serviço real:** no monitor de rede, nada vai ao `chess-api.com` antes do **Ligar**.
- **Evidência:** [`remote_engine.rs`](../../crates/chess-studio/src/remote_engine.rs), [`remote_engine_tests.rs`](../../crates/chess-studio/src/remote_engine_tests.rs).

### CX-HOM-50. Engine remota: análise, limite e Provedor desligado

- **Passos:**
  1. Com a Engine remota escolhida, ligue a análise na Posição inicial e depois navegue rápido pelos Lances de uma Partida.
  2. Em **Conexões**, desligue o Chess-API.com e volte ao Painel.
  3. Clique em **Ligar o Chess-API.com**.
  4. Faça muitas análises seguidas até o serviço pedir pausa (se acontecer).
- **Esperado:** a avaliação chega como a de uma Engine local (placar das Brancas, linha principal clicável) com o selo da Engine remota; navegar rápido não dispara um pedido por Lance (a Posição precisa parar). Com o Provedor desligado, o Painel diz que ele está desligado e que nada foi enviado, e oferece **Ligar o Chess-API.com** (que repete o aviso e o consentimento). No limite, o Painel diz em quantos segundos tentar de novo e a análise local segue disponível. Nenhum texto mostra código HTTP ou termo técnico.
- **Com o serviço real:** o Lance devolvido é sempre legal na Posição (o app recusa e não mostra o ilegal).
- **Evidência:** [`remote_engine_tests.rs`](../../crates/chess-studio/src/remote_engine_tests.rs).

### CX-HOM-51. Engine remota sob a Trava de fair play

- **Passos:**
  1. Com a Engine remota escolhida e a análise ligada, comece uma Partida online contra pessoa (tempo real).
  2. Tente ligar a análise de novo e trocar para a Engine remota na lista.
  3. Termine a Partida, aguarde o resultado confirmado pelo Lichess e ligue a análise.
- **Esperado:** a análise some e o Painel diz **Trava de fair play** e que a análise volta quando a Partida online terminar. Nenhum pedido sai ao Chess-API.com enquanto a Partida durar. Depois do fim confirmado, a análise liga de novo (a escolha da Engine é a mesma da sessão).
- **Com o serviço real:** no monitor de rede, nenhum pedido ao `chess-api.com` durante a Partida.
- **Evidência:** [`fair_play.rs`](../../crates/chess-studio/src/fair_play.rs), [`remote_engine_tests.rs`](../../crates/chess-studio/src/remote_engine_tests.rs).

### CX-HOM-52. Engine online: ligar, esperar, jogar e parar

- **Passos:**
  1. Em **Conexões** > Lichess > **Engine online**, autorize a Conexão de Engine com um usuário BOT de teste (ver CX-HOM-53) e clique em **Abrir a tela da Engine online**.
  2. Com uma Engine instalada, escolha-a e não clique em Ligar; feche e reabra o app.
  3. Clique em **Ligar a Engine online**; do site, com outro usuário, desafie o BOT em blitz, sem rating.
  4. Durante a Partida, clique em **Parar**; depois ligue de novo e clique em **Parar já…**.
- **Esperado:** a tela abre parada e nada sai para o Lichess até **Ligar** (reabrir o app volta a **Parada**). Ligada, mostra **Esperando desafios**; o desafio aceito vira **Jogando 1** com a Partida na lista (adversário, cor, Lances, quem pensa). **Parar** muda para **Parando** e a Engine fecha sozinha quando a Partida termina; desafios novos nesse meio-tempo são recusados. **Parar já…** pede confirmação e abandona a Partida em andamento. A Partida terminada fica na Biblioteca como Partida da Engine, e a Trava de fair play nunca liga.
- **Com o serviço real:** a Partida joga Lances legais a cada vez (ver CX-HOM-53 para a falha da Engine).
- **Evidência:** [`engine_online/tests.rs`](../../crates/chess-studio/src/engine_online/tests.rs), [`docs/evidencias/engine-remota-online`](../evidencias/engine-remota-online).

### CX-HOM-53. Engine online: limites, conta BOT e falha da Engine

- **Passos:**
  1. Ligue com só **Blitz**, **Só sem rating**, **Só pessoas**, **1** Partida ao mesmo tempo e máximo **1**; desafie com bullet, depois com rating, depois de outro BOT, depois em Chess960, e por fim com blitz sem rating.
  2. Durante a Partida, desafie de novo.
  3. Com uma conta nova do Lichess que nunca jogou, abra **A conta ainda não é uma conta BOT?**, leia o aviso, marque a confirmação e clique em **Converter em conta BOT**. Tente também com uma conta que já tem Partidas.
  4. Escolha uma Engine cujo arquivo foi movido ou apagado e deixe aceitar uma Partida.
- **Esperado:** cada desafio fora dos limites é recusado com o motivo certo e aparece em **Atividade recente**; o último é aceito e, atingido o máximo, a tela passa a **Parando** com a razão **Limite de Partidas atingido**; desafios durante a Partida são recusados. A conversão só acontece depois do aviso e da confirmação marcada, uma vez; numa conta com Partidas o Lichess recusa e a tela diz que a conta precisa ser nova. Se a Engine não inicia, a Partida é abandonada e a atividade diz que a Engine não iniciou (o relógio não corre). Nunca há Lance ilegal enviado.
- **Com o serviço real:** a conversão em BOT é **irreversível**: use só uma conta criada para o teste.
- **Evidência:** [`engine_online/tests.rs`](../../crates/chess-studio/src/engine_online/tests.rs).

### CX-HOM-55. FICS na Central: aviso, ligar e desligar

- **Passos:**
  1. Arquivo > Conexões; em **Jogar online**, encontre o cartão **FICS** e abra **Detalhes**.
  2. Ligue o Switch; depois desligue.
  3. Início > Nova Partida > **Online (Lichess ou FICS)**; escolha **FICS · visitante** com o FICS desligado e clique em **Ligar o FICS…**.
- **Esperado:** o cartão diz, antes de ligar, **Conexão sem criptografia** (texto puro), **Só como visitante** (nome GuestXXXX, sem rating, **nunca pede nem envia senha**), **Sem reconexão automática** e **Sem timeseal**. Não há campo de senha, chave nem botão de autorizar. **Como se identifica: Visitante, sem senha**; **O que é enviado** fala em texto puro e **nenhuma senha**. O diálogo **Ligar FICS?** mostra o mesmo texto e **freechess.org**. Desligado, nada sai: a barra de status não mostra conexão com o FICS.
- **Evidência:** [`connections/model.rs`](../../crates/chess-studio/src/connections/model.rs) (`the_fics_card_plays_as_a_guest_never_asks_for_a_password_and_warns_of_plain_text`), [`online/fics_tests.rs`](../../crates/chess-studio/src/online/fics_tests.rs) (`with_the_fics_off_no_connection_is_ever_opened`, `the_fics_screen_draws_every_state_in_plain_language_with_no_password_field`), capturas em [`docs/evidencias/fics-visitante/`](../evidencias/fics-visitante/).

### CX-HOM-56. FICS: entrar como visitante e sair

- **Pré-condição:** FICS ligado (CX-HOM-55); internet.
- **Passos:**
  1. Na tela da Partida online, **FICS · visitante** > **Entrar como visitante**.
  2. Espere **No FICS como Guest…**; depois **Sair do FICS**.
- **Esperado:** a entrada mostra **Entrando como visitante** e termina com o nome de visitante que o FICS deu. Nenhuma senha é pedida em momento algum. **Sair do FICS** encerra a conexão na hora.
- **Com o serviço real (UNVERIFIED):** os textos de entrada do FICS real (`login:`, "Press return to enter the server as…", "Starting FICS session as…") são os que o app reconhece; até aqui só foram testados com transcrições **sintéticas**. Se a entrada não terminar em 60 s, o app diz que o FICS não confirmou a entrada a tempo; anote o que apareceu.

### CX-HOM-57. FICS: buscar adversário, jogar e terminar

- **Passos:**
  1. Conectado como visitante, **Buscar adversário** > Blitz 5+0 > **Buscar adversário**; uma vez, **Cancelar busca**, e busque de novo.
  2. Quando a Partida começar, jogue pelos gestos de sempre (3D e 2D, arrastando e com dois cliques), inclusive um roque e, se der, uma promoção.
  3. Use **Propor empate**, **Abortar** (logo no começo de outra Partida) e **Abandonar** (confirme).
  4. Depois do fim, **Analisar** e **Ver na Biblioteca**.
- **Esperado:** pareado, a tela fecha e a Partida vai ao tabuleiro com o Painel **Partida online** (selos **FICS · visitante** e **Ao vivo**, **sem rating**). Cada Lance aparece no destino com **Enviando …** até o FICS confirmar e nunca é reenviado. Durante a Partida a Trava de fair play desliga Engines, Explorador, finais e análise; só cai com o fim confirmado pelo FICS. A Partida está na Biblioteca com **Partida online · FICS (visitante) · Partida N**.
- **Com o serviço real (UNVERIFIED):** o **formato do Lance enviado** (`e2e4`, `o-o`, `o-o-o`, `e7e8=q`) é aceito pelo FICS real; os textos de criação (`{Game N (…) Creating …}`), de fim, de empate e de abortar são reconhecidos. Um Lance recusado aparece como **O FICS recusou o Lance** e a peça volta.

### CX-HOM-58. FICS: desafios recebidos e variantes

- **Passos:**
  1. Conectado como visitante, de outro cliente do FICS (outro visitante), desafie o nome mostrado em xadrez padrão (por exemplo `match GuestXXXX 3 0`) e, depois, numa variante (`match GuestXXXX 3 0 wild 1`).
  2. Na aba **Desafios**, **Recusar** um e **Aceitar** o outro.
- **Esperado:** o desafio aparece na aba **Desafios (1)** e no Painel (**1 desafio recebido no FICS**). O de variante diz que o app só joga xadrez padrão e não tem **Aceitar**. Uma Partida de variante que comece mesmo assim é recusada com aviso e não vai ao tabuleiro nem à Biblioteca.
- **Com o serviço real (UNVERIFIED):** o texto do desafio (`Challenge: … unrated blitz 3 0.`) do FICS real é reconhecido.

### CX-HOM-59. FICS: queda de conexão durante a Partida

- **Passos:**
  1. Numa Partida de visitante, solte uma peça e desligue a internet logo em seguida (ou feche a conexão pelo sistema).
  2. Tente mover de novo; tente ligar a Análise.
  3. Religue a internet. No Painel, **Descartar a sessão**.
- **Esperado:** o Painel e a tela avisam: **A conexão com o FICS caiu durante a Partida. No FICS, a Partida de visitante que cai é dada como perdida; o resultado não pôde ser confirmado e o app não reconecta sozinho.** O Lance sem confirmação volta e não é reenviado; o tabuleiro não aceita Lance. A Trava de fair play continua até **Descartar a sessão**; só então a Análise liga. Nada reconecta sozinho; **Entrar de novo como visitante** é outra sessão, outro nome. Na Biblioteca, a Partida fica sem resultado.

### CX-HOM-60. FICS: observar uma Partida

- **Passos:**
  1. Conectado como visitante, aba **Observar**: digite o número de uma Partida em andamento no FICS (veja no site ou em outro cliente) e **Observar**.
  2. Espere alguns Lances; **Parar de observar**.
- **Esperado:** um tabuleiro pequeno mostra a Partida, com nomes, relógios e o último Lance. Nada vai ao tabuleiro principal nem à Biblioteca, e a Trava de fair play não liga. Uma Partida de variante é recusada com aviso.
- **Com o serviço real (UNVERIFIED):** os textos de observação (`You are now observing game N.`, `Game N: …`, `Removing game N from observation list.`) do FICS real são reconhecidos.
- **Evidência:** [`online/fics_tests.rs`](../../crates/chess-studio/src/online/fics_tests.rs), [decisões FC-13 a FC-21](../decisoes/2026-11-11-fics.md).

### CX-HOM-72. Central: filtros Com cadastro, Manual e por estado

- **Pré-condição:** CX-HOM-02.
- **Passos:**
  1. Arquivo > Conexões. Clique em **Com cadastro** e depois em **Manual**.
  2. Volte a **Todos**. Na linha **Estado**, clique em **Desligados**, **Ligados**, **Com autorização**, **Importação manual** e **Em breve**.
  3. Combine um filtro de custo com um de estado e digite um nome na busca.
- **Esperado:** cada botão mostra só os Provedores daquele grupo; o número ao lado dos filtros de custo bate com a lista. **Manual** e **Importação manual** mostram só quem não tem API (TWIC, PGN Mentor, Queen Alice…). **Com autorização** mostra só os Provedores com Conexão autorizada. Combinar filtros estreita a lista; sem resultado aparece "Nenhum Provedor combina com a busca e o filtro."
- **Com o serviço real:** não se aplica.
- **Evidência:** `connections/model.rs` (testes `signup_and_manual_filters_follow_the_catalog` e `the_state_filter_uses_the_state_of_each_card`), [`docs/evidencias/central-origem-textos/`](../evidencias/central-origem-textos/README.md).

### CX-HOM-73. Licença dos dados e Origem

- **Pré-condição:** CX-HOM-02; para o rating, um ID da FIDE vinculado (CX-HOM-04 e a consulta do rating).
- **Passos:**
  1. Na Central, abra **Detalhes** nos cartões do Lichess, Chess.com e TWIC e leia a linha **Licença dos dados**.
  2. Em Perfil local, veja o rating FIDE consultado.
  3. No Painel Análise, escolha a Engine remota (Chess-API.com) e espere uma resposta.
- **Esperado:** **Licença dos dados** mostra a licença e, em cinza, as restrições, em linguagem de usuário (TWIC: uso pessoal; Chess.com: sem redistribuição; FIDE: reprodução restrita). O rating FIDE traz "Dados da FIDE reproduzidos pelo Lichess; os direitos são da FIDE…" junto de "consultado …". Com a Engine remota, o Painel mostra "Origem: Chess-API.com, consultado …" e os termos do resultado, e **não** mostra Threads, Hash, Carga nem o aviso da segunda Engine.
- **Com o serviço real (UNVERIFIED):** a resposta real do Chess-API.com (instante e termos) e o rating real da FIDE via Lichess.
- **Evidência:** `connections/model.rs` e `fide_profile.rs` e `remote_engine_tests.rs` (testes), `chess-store/tests/fide_and_bulk.rs` (migração 18), [`docs/evidencias/central-origem-textos/`](../evidencias/central-origem-textos/README.md).

### CX-HOM-74. Textos sem "conta", "chaveiro" nem código HTTP

- **Pré-condição:** nenhuma.
- **Passos:**
  1. Percorra a Central, as janelas de autorização e cofre, a Engine online (incluindo **Converter em usuário BOT**), o aviso da Engine remota e o download de problemas do Lichess (com a internet desligada e com um erro do Provedor).
- **Esperado:** nenhum texto fala em "conta", "chaveiro", "login", "credencial" nem mostra um número de erro; aparecem "Conexão", "usuário BOT", "cofre do sistema" e "O Lichess não respondeu como esperado."
- **Com o serviço real:** não se aplica.
- **Evidência:** `connections/model.rs` (teste `integration_texts_avoid_the_glossary_banned_words_and_http_codes`).

## Fora deste roteiro

Chat, premove e reclamar empate por ausência não existem no app (ver [`2026-11-11-partida-online.md`](../decisoes/2026-11-11-partida-online.md)).

## O que falta

Todos os itens acima são do usuário: percorrer o roteiro com o serviço real, marcar PASS ou FAIL e aprovar o visual. Itens "sim" em **Serviço real?** nunca serão PASS só pela suíte.
