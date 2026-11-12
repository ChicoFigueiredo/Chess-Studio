# Autorizar o Lichess pela Central (CX-11, #231): o cofre de segredos, a senha-mestra, a
# autorização pelo navegador, a chave pessoal e a Engine online.
# Linguagem de quem usa: "autorização", "chave pessoal", "o que esta autorização permite". Nunca
# as palavras técnicas do protocolo, nem "conta" (confunde com Conexão) nem o código de erro.

## Faixa do cofre, no topo da Central
vault-system-title = Segredos guardados no cofre do sistema
vault-system-text = O seu computador protege as autorizações. O Chess Studio não guarda nenhuma senha.
vault-loading-title = Abrindo o cofre do sistema…
vault-loading-text = O sistema pode pedir que você desbloqueie o cofre do sistema. As autorizações voltam quando ele responder.
vault-system-failed-title = O cofre do sistema não respondeu
vault-system-failed-text = As autorizações ficam suspensas e nada novo é guardado. Desbloqueie o cofre do sistema e abra o Chess Studio de novo.
vault-own-open-title = Cofre do Chess Studio desbloqueado
vault-own-open-text = As autorizações estão disponíveis nesta sessão. Bloqueie o cofre quando terminar.
vault-own-locked-title = Cofre do Chess Studio bloqueado
vault-own-locked-text = Suas autorizações só voltam a valer quando você desbloquear o cofre com a senha-mestra.
vault-own-locked-text-empty = Ele guarda as autorizações deste computador, protegidas por uma senha-mestra.
vault-none-title = Sem cofre para guardar autorizações
vault-none-text = Este computador não oferece um cofre do sistema, então nada é guardado. Para autorizar o Lichess, crie o cofre do Chess Studio, protegido por uma senha-mestra.
vault-broken-title = O cofre de segredos não abriu
vault-broken-text = Nada é guardado nem lido até isso ser resolvido. Seus dados locais continuam intactos.
vault-act-create = Criar cofre…
vault-act-unlock = Desbloquear…
vault-act-lock = Bloquear
vault-act-change = Trocar senha-mestra…

## Diálogo da senha-mestra
vault-dialog-create-title = Criar o cofre do Chess Studio
vault-dialog-create-intro = O cofre guarda as suas autorizações neste computador, cifradas por uma senha-mestra que só você sabe.
vault-dialog-create-warning-title = Não há como recuperar esta senha
vault-dialog-create-warning = Se você esquecer a senha-mestra, as autorizações guardadas se perdem e precisam ser refeitas. Nem o Chess Studio nem ninguém consegue recuperá-la. Ele não a grava, não a registra e não a copia para a área de transferência.
vault-dialog-unlock-title = Desbloquear o cofre
vault-dialog-unlock-intro = Digite a senha-mestra para usar as suas autorizações nesta sessão.
vault-dialog-change-title = Trocar a senha-mestra
vault-dialog-change-intro = Digite a senha atual e escolha uma nova. As autorizações guardadas continuam valendo.
vault-field-current = Senha-mestra atual
vault-field-password = Senha-mestra
vault-field-new = Nova senha-mestra
vault-field-confirm = Repita a senha-mestra
vault-hint-password = Pelo menos 8 caracteres
vault-hint-unlock = Digite a senha-mestra
vault-hint-confirm = Digite a mesma senha de novo
vault-understood = Entendi que não há como recuperar a senha
vault-submit-create = Criar cofre
vault-submit-unlock = Desbloquear
vault-submit-change = Trocar senha
vault-cancel = Cancelar
vault-working-create = Protegendo o cofre com a senha-mestra… isso leva alguns segundos.
vault-working-unlock = Conferindo a senha-mestra…
vault-error-empty = Digite a senha-mestra.
vault-error-empty-current = Digite a senha-mestra atual.
vault-error-weak = A senha-mestra precisa ter pelo menos 8 caracteres.
vault-error-mismatch = As duas senhas não são iguais. Digite de novo.
vault-error-understand = Marque que você entendeu que a senha não pode ser recuperada.
vault-error-wrong = Senha-mestra errada. Tente de novo.
vault-error-corrupted = O arquivo do cofre foi alterado ou está corrompido. Nada foi lido dele.
vault-error-state = O cofre já está configurado de outra forma. Feche esta janela e tente de novo.
vault-error-other = Não foi possível concluir. Tente de novo.

## Autorizar pelo navegador
auth-feature = A autorização
auth-waiting-title = Esperando você autorizar no navegador
auth-waiting-title-engine = Esperando você autorizar a Engine online no navegador
auth-waiting-text = A página do Lichess abriu no seu navegador. Confirme lá e depois volte aqui. O Chess Studio nunca vê a sua senha do Lichess.
auth-waiting-left = Vale por mais { $when }.
auth-waiting-open-failed = Não foi possível abrir o navegador sozinho. Copie o endereço e abra você mesmo.
auth-act-reopen = Abrir o navegador de novo
auth-act-copy = Copiar endereço
auth-act-cancel = Cancelar
auth-checking-title = Conferindo a chave com o Lichess…
auth-checking-text = A chave só é guardada depois que o Lichess a confirmar.

## Como terminou
auth-done-title = Autorizado como { $name }
auth-done-browser = O Lichess confirmou que é você.
auth-done-key = O Lichess confirmou a chave pessoal.
auth-permits = O que esta autorização permite
auth-permits-extra = Esta autorização permite mais do que o Chess Studio precisa. Se puder, crie outra só com a permissão de jogar.
auth-refused-title = Você recusou a autorização
auth-refused-text = Nada foi guardado. Você pode autorizar de novo quando quiser.
auth-timeout-title = O tempo para autorizar acabou
auth-timeout-text = O Lichess não recebeu a sua confirmação a tempo. Nada foi guardado.
auth-declined-title = O Lichess continua desligado
auth-declined-text = Sem ligar o Lichess não há como autorizar. Nada foi enviado.
auth-problem-title = Não foi possível autorizar
auth-act-retry = Tentar de novo
auth-act-dismiss = Fechar

auth-problem-off = O Lichess está desligado. Ligue-o e tente de novo.
auth-problem-locked = O cofre está bloqueado. Desbloqueie-o e tente de novo.
auth-problem-no-vault = Não há cofre para guardar a autorização. Crie o cofre do Chess Studio e tente de novo.
auth-problem-vault = O cofre não conseguiu guardar a autorização. Nada foi guardado.
auth-problem-vault-broken = O cofre de segredos não abriu, então nada pode ser guardado. Veja a faixa no topo da página.
auth-problem-system-vault = O cofre do sistema não respondeu, então nada pode ser guardado agora.
auth-problem-local = O Chess Studio não conseguiu preparar o retorno do navegador neste computador. Tente de novo.
auth-problem-callback = O retorno do navegador não corresponde a esta autorização, então nada foi aceito. Comece de novo.
auth-problem-code = O Lichess recusou o código desta autorização (já usado ou vencido). Comece de novo.
auth-problem-key-format = Isto não parece uma chave pessoal do Lichess. Confira se copiou a chave inteira, sem espaços.
auth-problem-key-rejected = O Lichess não aceita esta chave (expirada, revogada ou inexistente). Nada foi guardado.
auth-problem-wrong-player = Esta chave não serve para o jogador: ela precisa permitir jogar Partidas pelo tabuleiro e não pode permitir jogar como Engine. Nada foi guardado.
auth-problem-wrong-engine = Esta chave não serve para a Engine online: ela precisa permitir jogar como Engine (usuário BOT) e não pode permitir jogar como jogador. Nada foi guardado.
auth-problem-no-permissions = O Lichess não informou o que esta autorização permite, então ela não foi guardada.
auth-problem-malformed = A resposta do Lichess veio num formato inesperado. Tente de novo mais tarde.
auth-problem-net = Sem resposta do Lichess: { $detail }
auth-problem-duplicate = Este Perfil local já tem uma Conexão autorizada para esse usuário do Lichess. Nada novo foi guardado.
auth-problem-save = Não foi possível gravar a Conexão neste computador. Nada foi guardado.
auth-problem-other = Não deu certo. Tente de novo.

## Chave pessoal (fluxo avançado)
auth-key-title = Chave pessoal do Lichess
auth-key-title-engine = Chave pessoal da Engine online
auth-key-intro = Fluxo avançado. Crie a chave na página de chaves do Lichess, só com a permissão de jogar Partidas pelo tabuleiro, e cole aqui.
auth-key-intro-engine = Fluxo avançado. Crie a chave na página de chaves do Lichess, só com a permissão de jogar como Engine (usuário BOT), e cole aqui.
auth-key-label = Chave
auth-key-hint = Cole a chave aqui
auth-key-note = A chave fica mascarada, vai direto para o cofre e só é guardada depois que o Lichess a confirmar. Não é gravada no banco nem copiada para lugar nenhum.
auth-key-save = Conferir e guardar
auth-key-cancel = Cancelar
auth-key-error-empty = Cole a chave pessoal.
auth-key-error-format = Isto não parece uma chave do Lichess: sem espaços e com até 4096 caracteres.

## Conexões autorizadas
auth-row-browser = Autorizada pelo navegador, { $when }
auth-row-key = Autorizada por chave pessoal, { $when }
auth-row-user = Usuário no Lichess: { $name }
auth-badge-authorized = Autorizada
auth-badge-expired = Expirada
auth-badge-locked = Bloqueada
auth-act-test = Testar
auth-act-reauth = Autorizar de novo…
auth-act-disconnect = Desconectar…
auth-disconnect-confirm = Desconectar “{ $name }”? O Chess Studio pede ao Lichess que revogue a autorização e apaga a chave daqui. Sem internet, avisamos o que fazer.
auth-disconnect-yes = Desconectar
auth-disconnect-no = Manter
auth-disconnecting = Desconectando…
auth-expired-text = O Lichess não aceita mais esta autorização (ela venceu ou foi revogada). Seus dados locais continuam aqui; basta autorizar de novo.
auth-locked-text = Desbloqueie o cofre do Chess Studio para voltar a usar esta autorização.
auth-check-running = Conferindo com o Lichess…
auth-check-ok = Conferido { $when }: o Lichess confirmou { $name }.
auth-check-rejected = O Lichess não aceita mais esta autorização. Autorize de novo.
auth-check-other = Não foi possível conferir agora.

## Engine online (entrada separada da de jogador)
engine-section-title = Engine online
engine-section-text = Joga como Engine no Lichess. Exige um usuário BOT, que é separado do seu usuário de jogador e não volta a ser comum. Esta autorização nunca se mistura com a de jogador.
engine-none = Nenhuma Engine online autorizada.
engine-act-authorize = Autorizar Engine online…
engine-act-open-screen = Abrir a tela da Engine online
engine-act-key = Usar chave pessoal…
connections-requires-explorer = Para usar o Explorador de aberturas, autorize sua conexão do { $provider }.

## Desconectar
connections-revoke-ok = Desconectado. O Lichess revogou a autorização e a chave foi apagada deste computador.
connections-revoke-remote-failed = A chave foi apagada deste computador, mas não conseguimos avisar o Lichess. Para garantir, revogue-a nas preferências do Lichess, na área de acesso por chaves.
connections-revoke-local-failed = Não foi possível apagar a chave guardada. A Conexão não foi removida.
connections-remove-error = Não foi possível remover a Conexão: a chave guardada não foi apagada, então nada foi removido.

## O que cada permissão do Lichess quer dizer
perm-board-play = Jogar Partidas pelo tabuleiro
perm-bot-play = Jogar como Engine online (usuário BOT)
perm-challenge-read = Ver seus desafios
perm-challenge-write = Criar, aceitar e recusar desafios
perm-preference-read = Ler suas preferências
perm-preference-write = Alterar suas preferências
perm-email-read = Ler o seu e-mail
perm-study-read = Ler seus estudos
perm-study-write = Criar e alterar estudos
perm-puzzle-read = Ler seu histórico de problemas
perm-follow-read = Ver quem você segue
perm-follow-write = Seguir e deixar de seguir
perm-msg-write = Enviar mensagens
perm-team-read = Ler seus times
perm-team-write = Gerenciar times
perm-tournament-write = Criar e gerenciar torneios
perm-racer-write = Criar corridas de problemas
perm-web-mod = Ferramentas de moderação
perm-other = Outra permissão: { $name }
