# Central de Conexões (CX-3, #214; acabamento #225): bastidores > Conexões, estado da rede e o
# diálogo "Ligar <Provedor>?". Termos do glossário: Provedor, Conexão, Perfil local.
# Linguagem de quem usa: nada de etapa, código de erro, token, nome de host solto em frase.

backstage-connections = Conexões

connections-title = Central de Conexões
connections-intro = Cada Provedor nasce desligado. Nada sai do computador para um Provedor que você não ligou.
connections-profile = Perfil local: { $name }
connections-search-hint = Buscar Provedor, ex.: Lichess
connections-empty = Nenhum Provedor combina com a busca e o filtro.

connections-filter-all = Todos
connections-filter-free = Gratuitos
connections-filter-signup = Com cadastro
connections-filter-manual = Manual
connections-filter-paid = Pagos ou sob aprovação
connections-filter-chip = { $name } · { $count }
connections-state-filter-label = Estado
connections-state-filter-all = Qualquer estado
connections-state-filter-off = Desligados
connections-state-filter-on = Ligados
connections-state-filter-authorized = Com autorização
connections-state-filter-manual = Importação manual
connections-state-filter-soon = Em breve

connections-cat-play = Jogar online
connections-cat-data = Dados e consultas
connections-cat-engine = Engines
connections-cat-import = Importação de arquivos
connections-cat-ratings = Ratings
connections-section-count-one = 1 Provedor
connections-section-count-many = { $count } Provedores

connections-tier-soon = Em breve
connections-tier-approval = Requer aprovação

# Estados: sempre uma palavra e um ícone, nunca só a cor.
connections-state-off = Desligado
connections-state-on = Ligado
connections-state-linked = Ligado · nome vinculado
connections-state-authorized = Autorizado
connections-state-expired = Autorização expirada
connections-state-locked = Cofre bloqueado
connections-state-limited = Limitado
connections-state-limited-in = Limitado · volta em { $when }
connections-state-failing = Com falha
connections-state-manual = Importação manual
connections-state-approval = Requer aprovação
connections-state-soon = Em breve
connections-state-evaluating = Em avaliação

connections-hint-off = Nada sai do computador. Ligue para usar este Provedor.
connections-hint-on = Ligado. Só o que você pedir é enviado.
connections-hint-linked = Ligado, com um nome público vinculado. Isso só identifica você no Provedor; não é uma autorização.
connections-hint-authorized = Ligado, com a sua conexão autorizada: o Lichess confirmou quem você é.
connections-hint-expired = O Lichess não aceita mais esta autorização. Autorize de novo; seus dados locais continuam aqui.
connections-hint-locked = A autorização existe, mas o cofre do Chess Studio está bloqueado. Desbloqueie para voltar a usá-la.
connections-hint-limited = O Provedor pediu uma pausa. O Chess Studio volta a consultar sozinho.
connections-hint-failing = A última consulta não deu certo. Tente de novo mais tarde.
connections-hint-manual = Este Provedor não tem consulta direta: você baixa o arquivo e importa.
connections-hint-approval = Só funciona com aprovação do Provedor. Nada sai do computador.
connections-hint-planned = Ainda não disponível. Chega numa próxima versão.
connections-hint-unconfirmed = Ainda estamos avaliando os termos deste serviço.

# Nomes dos serviços (substantivos), usados no custo e nos requisitos.
connections-svc-play = Jogar online
connections-svc-online-engine = Engine online
connections-svc-explore = Explorador
connections-svc-endgames = Finais exatos
connections-svc-cached-eval = Avaliações prontas
connections-svc-analyze = Análise remota
connections-svc-query = Perfis e Partidas públicas
connections-svc-download = Pacotes e bases
connections-svc-import-pgn = Importação de PGN
connections-svc-identity = Nome público
connections-svc-manual-file = Arquivos baixados por você

# O que você pode fazer (verbos, em minúscula: a frase junta com vírgulas).
connections-do-link = vincular seu nome público
connections-do-link-id = vincular seu ID público
connections-do-test = testar a Conexão
connections-do-import-file = importar arquivos baixados por você
connections-do-import-pgn = importar PGN baixado por você
connections-do-play = jogar online
connections-do-online-engine = jogar com uma Engine
connections-do-explore = explorar aberturas
connections-do-explore-after = explorar aberturas, depois de autorizar
connections-do-endgames = consultar finais exatos
connections-do-cached-eval = ver avaliações prontas
connections-do-analyze = pedir análise a uma Engine remota
connections-do-query = importar Partidas e ver perfis públicos
connections-do-download = baixar pacotes e bases

connections-face-label = O que você pode fazer agora
connections-face-list = { $list }.
connections-face-planned = Disponível numa próxima versão.
connections-face-planned-list = { $list }, numa próxima versão.
connections-face-unconfirmed = Ainda estamos avaliando este serviço.
connections-face-approval = Requer aprovação do Provedor. Nada sai do computador.
connections-list-and = { $head } e { $last }

connections-cost-free-nosignup = Gratuito, sem cadastro
connections-cost-free-signup = Gratuito, com cadastro no Provedor
connections-cost-free-limited = Gratuito, com limites
connections-cost-paid = Pago, opcional
connections-cost-approval = Requer aprovação do Provedor
connections-cost-unconfirmed = Custo ainda não confirmado

connections-auth-none = Sem identificação
connections-auth-public-id = Nome ou ID público, sem senha
connections-auth-oauth = Autorizar pelo navegador
connections-auth-key-personal = Chave pessoal
connections-auth-key = Chave do serviço
connections-auth-oauth-soon = Autorizar pelo navegador (em breve)
connections-auth-key-personal-soon = Chave pessoal (em breve)
connections-auth-key-soon = Chave do serviço (em breve)
connections-auth-protocol = Acesso próprio do Provedor
connections-auth-manual = Importação manual
connections-auth-partner = Aprovação do Provedor

# O que é enviado: texto próprio da interface, sem endereços em frase.
connections-sends-lichess = Os nomes públicos que você consulta, as Posições que você explora ou analisa e os meses de Partidas que você pede. Nada do seu Perfil local vai junto.
connections-sends-chesscom = Os nomes públicos que você consulta e os meses de Partidas que você pede. Nenhuma senha.
connections-sends-fide = Nada vai para a FIDE. O ID fica neste computador; o rating vem pelo Lichess, se você ligar o Lichess.
connections-sends-generic = As consultas que você fizer a este Provedor. Nada do seu Perfil local vai junto.
connections-sends-manual = Nada. Você baixa o arquivo no site e importa do disco.
connections-sends-info = Nada. Este cartão é só informativo.
connections-sends-soon = Nada por enquanto. Este serviço ainda não está disponível.
connections-sends-off-lead = Nada sai do computador enquanto estiver desligado.
connections-sends-when-on = Ao ligar: { $text }

connections-detail-sends = O que é enviado
connections-detail-identifies = Como se identifica
connections-detail-cost = Custo
connections-detail-requires = Requisito
connections-detail-later = O que vem depois
connections-detail-last = Última consulta
connections-detail-status = Situação
connections-detail-links = Instruções e termos
connections-last-never = Nenhuma nesta sessão
connections-ago-now = agora há pouco
connections-ago-minutes = há { $count } min
connections-ago-hours = há { $count } h
connections-ago-days = há { $count } d
connections-minutes = { $count } min
connections-minutes-under-one = menos de 1 min
connections-failing = Falhou: { $detail }
connections-err-timeout = o Provedor não respondeu no prazo.
connections-err-unreachable = sem conexão com o Provedor.
connections-err-http = o Provedor respondeu com erro.
connections-err-other = a última consulta não deu certo.
connections-err-malformed = a resposta do Provedor veio num formato inesperado.
connections-test-not-turned-on = o Provedor continua desligado.
connections-limited-until = Em pausa a pedido do Provedor. Volta em { $when }.

connections-test-feature = O teste da Conexão
connections-test-running = Testando a Conexão padrão…
connections-test-waiting = Esperando você decidir se liga o Provedor.
connections-test-confirmed = Testado { $when }: encontrado como { $name } em { $source }.
connections-test-confirmed-id = Testado { $when }: ID confirmado ({ $name }). O dado vem do { $source }.
connections-test-not-found = Testado { $when }: não encontramos esse nome em { $source }.
connections-test-not-found-id = Testado { $when }: não encontramos esse ID.
connections-test-limited = Testado { $when }: { $source } pediu uma pausa. Tente de novo em { $until }.
connections-test-invalid = Testado { $when }: o ID FIDE precisa ser um número.
connections-test-failed = Testado { $when }: não deu certo, { $detail }

connections-switch = Ligado
connections-switch-error = Não foi possível gravar a escolha. Tente de novo.

connections-act-link = Vincular nome
connections-act-link-id = Vincular ID
connections-act-link-another = Vincular outro nome…
connections-act-link-another-id = Vincular outro ID…
connections-act-test = Testar
connections-act-clear-cache = Limpar dados guardados
connections-act-authorize = Autorizar…
connections-act-reauthorize = Autorizar de novo…
connections-act-unlock = Desbloquear cofre…
connections-act-key = Usar chave pessoal…
connections-act-engine = Engine online…
connections-act-docs = Abrir instruções
connections-act-terms = Abrir termos no navegador
connections-reason-authorize = Em breve: autorizar sua conexão do { $provider } pelo navegador.
connections-reason-key = Em breve: guardar uma chave pessoal neste computador, com segurança.
connections-more = Mais ações
connections-details = Detalhes
connections-details-show = Mostrar detalhes
connections-details-hide = Ocultar detalhes
connections-cache-cleared-none = Não havia dados guardados.
connections-cache-cleared-one = 1 item guardado removido.
connections-cache-cleared-many = { $count } itens guardados removidos.
connections-open-failed = Não foi possível abrir o navegador. Endereço: { $url }

# Conexões vinculadas.
connections-list-title = Conexões deste Perfil local
connections-none-yet = Nenhuma Conexão vinculada.
connections-default-badge = Padrão
connections-make-default = Tornar padrão
connections-conn-more = Ações da Conexão
connections-rename = Renomear apelido
connections-remove = Remover
connections-name-in = Nome no { $provider }: { $name }
connections-id-in = ID: { $name }
connections-not-tested = Ainda não testada.
connections-rename-label = Apelido
connections-rename-hint = ex.: Principal
connections-rename-error = O apelido pode ter até 64 caracteres, sem caracteres de controle.
connections-rename-save = Salvar
connections-rename-cancel = Cancelar
connections-remove-confirm = Remover “{ $name }” deste Perfil local? O nome continua existindo no Provedor.
connections-remove-yes = Remover
connections-remove-no = Manter

connections-link-name-label = Nome público
connections-link-id-label = ID público
connections-link-nick-label = Apelido (opcional)
connections-link-nick-hint = ex.: Principal
connections-link-hint-lichess = ex.: ana_exemplo
connections-link-hint-chesscom = ex.: ana_exemplo
connections-link-hint-fide = ex.: 9999991
connections-link-hint-default = ex.: seu nome público
connections-link-note = Só o nome ou ID público é guardado, neste computador.
connections-link-save = Vincular
connections-link-cancel = Cancelar
connections-link-error-empty = Digite o nome ou ID público.
connections-link-error-invalid = Use só o nome ou ID, sem espaços, com até 64 caracteres.
connections-link-error-token = Isto parece uma chave de acesso, não um nome. O campo foi limpo. Para usar uma chave, use «Chave pessoal» no cartão do Provedor.
connections-link-error-exists = Este Perfil local já vincula esse nome neste Provedor.
connections-link-error-other = Não foi possível gravar a Conexão. Tente de novo.

# Diálogo "Ligar <Provedor>?" (API para os recursos que precisam de um Provedor desligado).
consent-title = Ligar { $provider }?
consent-intro-feature = { $feature } precisa do { $provider }, que está desligado.
consent-intro = { $provider } está desligado. Ligar permite consultas a ele.
consent-sends-label = O que será enviado
consent-where-label = Para onde vai
# Uma frase por uso real de rede (todo serviço pronto do catálogo tem a sua; há teste que garante).
consent-sends-lichess-explore = As Posições que você abre no Explorador de aberturas.
consent-sends-lichess-cached-eval = As Posições que você analisa, para buscar uma avaliação que o Lichess já calculou.
consent-sends-lichess-endgames = As Posições de finais com poucas peças, para saber o resultado exato.
consent-sends-lichess-query = Os nomes de usuário e os IDs FIDE públicos que você consulta e os meses de Partidas que você pede.
consent-sends-lichess-download = Ao baixar problemas, só o pedido do pacote: nada seu vai junto, e o arquivo vem para este computador.
consent-sends-lichess-play = Em Partidas online, com a sua Conexão autorizada, os seus Lances e os desafios que você fizer.
consent-sends-lichess-online-engine = Com a Conexão de Engine, os Lances que a sua Engine calcula, as respostas aos desafios que você recebe e, só se você pedir, o pedido ao Lichess para virar uma Engine (BOT), que não tem volta.
consent-sends-chesscom-query = Os nomes de usuário públicos que você consulta e os meses de Partidas que você pede.
consent-sends-fics-play = Em Partidas online como visitante, os seus Lances e os comandos de jogo (buscar adversário, aceitar desafio, empate, abandono, observar), para freechess.org em texto puro: a conexão com o FICS não tem criptografia.
consent-sends-chessapi-analyze = Só quando você escolhe a Engine remota na análise: a Posição analisada (onde estão as peças, de quem é a vez e os direitos de roque) e a profundidade pedida. Sem nomes e sem histórico da Partida.
consent-sends-playstrategy-query = Os nomes de usuário públicos que você consulta e os pedidos das Partidas públicas que você faz.
consent-sends-tail-playstrategy = Nenhuma senha. Nesta versão não há Partida online no PlayStrategy.
consent-sends-tail-local-profile = Nada do seu Perfil local vai junto.
consent-sends-tail-no-password = Nenhuma senha.
consent-footer = Você desliga quando quiser, em Arquivo > Conexões. Nada é enviado antes disso.
consent-turn-on = Ligar
consent-not-now = Agora não
consent-open-center = Ver na Central

# Estado da rede na barra de status.
statusbar-net-offline = Rede: offline
statusbar-net-one = Rede: 1 Provedor ligado
statusbar-net-many = Rede: { $count } Provedores ligados
statusbar-net-limited-one = 1 limitado
statusbar-net-limited-many = { $count } limitados
statusbar-net-failing-one = 1 com falha
statusbar-net-failing-many = { $count } com falha
statusbar-net-fairplay = Trava de fair play
statusbar-net-hint = Abrir a Central de Conexões

# Licença dos dados e restrições de cada Provedor (Origem, ADR-0006): linguagem de usuário,
# nunca o texto técnico do catálogo.
connections-detail-license = Licença dos dados
connections-license-lichess = Base de dados, problemas e aberturas em domínio público (CC0). Explorador e consultas seguem os termos do Lichess.
connections-restrict-lichess = Um pedido por vez, com pausa quando o Lichess pedir. Engine é proibida em Partida contra pessoa. O usuário BOT é separado do usuário de jogador. O app nunca pede a senha do Lichess.
connections-license-chesscom = Dados públicos do Chess.com, sob os termos do serviço. Sem redistribuição.
connections-restrict-chesscom = Somente leitura: o app nunca envia Lances nem automatiza o site. Um pedido por vez; os dados podem vir de uma cópia de até 12 horas. O aviso de que o app não é afiliado ao Chess.com aparece quando exigido.
connections-license-fide = Dados da FIDE, reproduzidos pelo Lichess. Os direitos são da FIDE: reprodução e armazenamento restritos, só para uso pessoal.
connections-restrict-fide = Nenhuma senha: só o ID público. As listas não podem ser republicadas.
connections-license-chessigma = Uso com atribuição ao Chessigma.
connections-restrict-chessigma = No máximo 30 pedidos por minuto. A atribuição é obrigatória.
connections-license-twic = Edição gratuita só para uso pessoal. Publicar ou compartilhar exige autorização do TWIC.
connections-restrict-twic = Sem coleta automática. Nada é enviado a um catálogo público.
connections-license-pgnmentor = Direitos de redistribuição e de uso comercial não confirmados.
connections-restrict-pgnmentor = Sem coleta automática nem redistribuição em massa.
connections-license-playstrategy = Dados sob os termos do PlayStrategy.
connections-restrict-playstrategy = Um pedido por vez; depois de um aviso de excesso, espera um minuto. Só xadrez padrão: damas, shogi, xiangqi e outras variantes são recusadas.
connections-license-fics = Dados sob os termos do FICS.
connections-restrict-fics = A senha nunca viaja sem criptografia, e o segredo do FICS nunca vai para outro Provedor.
connections-license-queenalice = Só as suas próprias Partidas.
connections-restrict-queenalice = Sem API externa: o app não entra no serviço nem joga Lances por você. Engine é proibida em Partida em andamento.
connections-license-chessbase = Depende do seu plano. Assinar o ChessBase não dá licença para esta integração.
connections-restrict-chessbase = Sem API pública confirmada. O app nunca pede a sua senha do ChessBase.
connections-license-chess-api = Resultado de Engine sob os termos do Chess-API.com.
connections-restrict-chess-api = Só quando você escolhe a Engine remota. Sem garantia de disponibilidade.
connections-license-stockfishapi = Resultado de Engine sob os termos do StockfishAPI.com.
connections-restrict-stockfishapi = Nada fica habilitado até o plano gratuito ser validado.
connections-license-stockfish-online = Resultado de Engine sob os termos do Stockfish.online.
connections-restrict-stockfish-online = Nada fica habilitado até os termos e as cotas serem validados.
connections-license-icc = Comercial.
connections-restrict-icc = Fica fora do modo padrão de custo zero.
connections-license-pgnbase = Licença não confirmada.
connections-restrict-pgnbase = O app não coleta dados do site automaticamente.
connections-license-chesstempo = Licença não confirmada.
connections-restrict-chesstempo = O app não coleta dados do site automaticamente.
connections-license-chessify = Licença não confirmada.
connections-restrict-chessify = O cadastro gratuito do site não dá acesso a uma API.
connections-license-chessurge = Licença não confirmada.
connections-restrict-chessurge = Não é usado como serviço do app.
connections-license-pychess-variants = O servidor é AGPL; reutilizar o código exige um estudo à parte.
connections-restrict-pychess-variants = Foco em variantes, que o app não joga.
