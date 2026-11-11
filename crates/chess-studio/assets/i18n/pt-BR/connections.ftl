# Central de Conexões (CX-3, #214): bastidores > Conexões, estado da rede e o diálogo
# "Ligar <Provedor>?". Termos do glossário: Provedor, Conexão, Perfil local.

backstage-connections = Conexões

connections-title = Central de Conexões
connections-intro = Cada Provedor nasce desligado. Nada sai do computador para um Provedor que você não ligou.
connections-profile = Perfil local: { $name }
connections-search-hint = Buscar Provedor, ex.: Lichess
connections-empty = Nenhum Provedor combina com a busca e o filtro.

connections-filter-all = Todos
connections-filter-free = Gratuitos
connections-filter-paid = Pagos ou pendentes
connections-count-one = 1 Provedor
connections-count-many = { $count } Provedores

connections-cat-play = Jogar online
connections-cat-data = Dados e consultas
connections-cat-engine = Engines remotas
connections-cat-import = Importação de arquivos
connections-cat-ratings = Ratings

connections-state-off = Desligado
connections-state-available = Disponível
connections-state-connected = Conectado
connections-state-limited = Limitado
connections-state-failing = Falha
connections-state-manual = Importação manual
connections-state-approval = Requer aprovação
connections-state-planned = Planejado
connections-state-unconfirmed = Não confirmado

connections-cost-free-nosignup = Gratuito, sem cadastro
connections-cost-free-signup = Gratuito, com cadastro
connections-cost-free-limited = Gratuito, com limites
connections-cost-paid = Pago, opcional
connections-cost-approval = Requer aprovação do Provedor
connections-cost-unconfirmed = Custo não confirmado

connections-auth-none = Sem identificação
connections-auth-public-id = Nome ou ID público
connections-auth-oauth = Autorização pelo navegador
connections-auth-token = Token pessoal
connections-auth-key = Chave de API
connections-auth-protocol = Protocolo próprio do Provedor
connections-auth-manual = Importação manual
connections-auth-partner = Aprovação do Provedor

connections-cap-play = Jogar
connections-cap-online-engine = Engine online
connections-cap-explore = Explorador
connections-cap-endgames = Finais exatos
connections-cap-cached-eval = Avaliação em cache
connections-cap-analyze = Análise remota
connections-cap-query = Consultar perfis e Partidas públicas
connections-cap-download = Baixar pacotes e bases
connections-cap-import-pgn = Importar PGN baixado por você
connections-cap-identity-only = Vincular nome ou ID público
connections-cap-manual-download = Importar a lista baixada por você

connections-now = Permite agora: { $list }
connections-now-nothing = Permite agora: nada, só informação
connections-planned = Chega na etapa { $stage }: { $list }
connections-identifies = Identifica-se por: { $list }
connections-cost = Custo: { $cost }
connections-sends-off = Ao ligar, vai para o Provedor: { $text }
connections-sends-on = Enviado ao Provedor: { $text }
connections-sends-never = O que sai do computador: { $text }
connections-manual-phrase = Conexão para importação; jogo remoto não disponível.
connections-last-never = Última consulta: nenhuma nesta sessão
connections-last-ago = Última consulta: { $ago }
connections-ago-now = agora há pouco
connections-ago-minutes = há { $count } min
connections-ago-hours = há { $count } h
connections-ago-days = há { $count } d
connections-failing = Falha: { $detail }
connections-err-timeout = o Provedor não respondeu no prazo.
connections-err-unreachable = sem conexão com o Provedor.
connections-err-http = o Provedor respondeu com erro.
connections-err-other = a última consulta não deu certo.
connections-limited-until = O Provedor pediu uma pausa.

connections-switch = Ligado
connections-switch-soon = Chega na etapa { $stage }.
connections-switch-error = Não foi possível gravar a escolha. Tente de novo.

connections-act-link = Vincular nome ou ID público
connections-act-test = Testar (etapa { $stage })
connections-act-clear-cache = Limpar cache
connections-act-authorize = Autorizar (etapa 3)
connections-act-key = Inserir chave (etapa 3)
connections-act-docs = Abrir instruções
connections-act-terms = Abrir termos no navegador
connections-soon-stage = Chega na etapa { $stage }.
connections-cache-cleared-none = O cache já estava vazio.
connections-cache-cleared-one = 1 item do cache removido.
connections-cache-cleared-many = { $count } itens do cache removidos.
connections-open-failed = Não foi possível abrir o navegador. Endereço: { $url }

connections-list-title = Conexões deste Perfil local
connections-none-yet = Nenhuma Conexão vinculada.
connections-default-badge = Padrão
connections-make-default = Tornar padrão
connections-remove = Remover
connections-link-name-label = Nome público
connections-link-id-label = ID público
connections-link-nick-label = Apelido (opcional)
connections-link-nick-hint = ex.: Principal
connections-link-hint-lichess = ex.: DrNykterstein
connections-link-hint-chesscom = ex.: MagnusCarlsen
connections-link-hint-fide = ex.: 1503014
connections-link-hint-default = ex.: seu nome público
connections-link-note = Só o nome ou ID público é guardado, neste computador.
connections-link-save = Vincular
connections-link-cancel = Cancelar
connections-link-error-empty = Digite o nome ou ID público.
connections-link-error-invalid = Use só o nome ou ID, sem espaços, com até 64 caracteres.
connections-link-error-exists = Este Perfil local já vincula esse nome neste Provedor.
connections-link-error-other = Não foi possível gravar a Conexão. Tente de novo.

# Diálogo "Ligar <Provedor>?" (API para os recursos que precisam de um Provedor).
consent-title = Ligar { $provider }?
consent-intro-feature = { $feature } precisa de { $provider }, que está desligado.
consent-intro = { $provider } está desligado. Ligar permite consultas a ele.
consent-sends-label = O que será enviado
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
