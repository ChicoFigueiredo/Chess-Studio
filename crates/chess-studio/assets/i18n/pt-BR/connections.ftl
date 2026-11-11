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
connections-filter-paid = Pagos ou sob aprovação
connections-filter-chip = { $name } · { $count }

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
connections-cost-line = { $cost }: { $services }

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
connections-detail-later-list = { $list }.
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
connections-act-clear-cache = Limpar cache
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
connections-cache-cleared-none = O cache já estava vazio.
connections-cache-cleared-one = 1 item do cache removido.
connections-cache-cleared-many = { $count } itens do cache removidos.
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
connections-link-error-exists = Este Perfil local já vincula esse nome neste Provedor.
connections-link-error-other = Não foi possível gravar a Conexão. Tente de novo.

# Diálogo "Ligar <Provedor>?" (API para os recursos que precisam de um Provedor desligado).
consent-title = Ligar { $provider }?
consent-intro-feature = { $feature } precisa de { $provider }, que está desligado.
consent-intro = { $provider } está desligado. Ligar permite consultas a ele.
consent-sends-label = O que será enviado
consent-where-label = Para onde vai
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
