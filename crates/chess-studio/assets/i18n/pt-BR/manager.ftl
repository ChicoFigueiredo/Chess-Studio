## Gerenciador de Engines (C9): instalar, atualizar, remover e configurar.

manager-open = Gerenciar Engines…
manager-title = Engines
manager-intro = Aqui você vê as Engines que o Chess Studio conhece, instala as oficiais do catálogo, atualiza, remove e ajusta threads, memória (hash) e nível. Nada é baixado nem executado sem a sua confirmação.

manager-list-heading = Engines instaladas e detectadas
manager-list-empty = Nenhuma Engine registrada ainda. Instale uma do catálogo, procure no computador ou informe um executável.
manager-version = versão { $version }
manager-protocol = protocolo { $protocol }
manager-checksum = SHA-256: { $sha256 }
manager-origin-catalog = origem: instalada pelo Chess Studio
manager-origin-detected = origem: detectada no computador
manager-origin-manual = origem: escolhida por você
manager-state-ready = pronta
manager-state-missing = arquivo não encontrado
manager-use = Usar esta Engine
manager-configure = Configurar…
manager-update = Atualizar para { $version }
manager-remove = Remover…

manager-catalog-heading = Instalar do catálogo oficial
manager-catalog-entry = { $name } { $version }
manager-catalog-unavailable = { $name } não tem download oficial para este sistema ({ $os }, { $arch }). Se você já tem o programa instalado (pelo gerenciador de pacotes ou compilado), use "Procurar no computador".
manager-install = Instalar…
manager-installed-already = Já instalada

manager-add-heading = Procurar ou adicionar
manager-detect = Procurar Engines no computador
manager-add-path = Verificar e adicionar
manager-add-warning = Ao adicionar um executável, o Chess Studio o executa uma vez para conferir se fala o protocolo UCI. Só adicione programas em que você confia.

manager-confirm-heading = Instalar { $name } { $version }
manager-confirm-variant = Versão do programa: { $label }
manager-confirm-license = Licença: { $license }. O programa é baixado do site oficial e fica numa pasta do Chess Studio; o aplicativo não o inclui nem o modifica.
manager-source-link = Código-fonte de { $name }
manager-confirm-network = Baixar também a rede neural { $name } ({ $size })
manager-confirm-accept = Aceito a licença e quero instalar

manager-remove-heading = Remover { $name }?
manager-remove-app = A pasta { $dir } e tudo o que o Chess Studio instalou nela será apagado.
manager-remove-unregister = Esta Engine não foi instalada pelo Chess Studio: ela só sai da lista. O arquivo { $path } continua onde está.
manager-remove-confirm = Remover

manager-group-resources = Recursos
manager-group-level = Nível
manager-group-advanced = Parâmetros avançados
manager-group-network = Rede neural
manager-network-none = Nenhuma rede neural baixada. Instale o Lc0 com a rede recomendada.
manager-config-heading = Configurar { $name }
manager-config-invalid = Valor inválido: { $detail }
manager-config-save = Salvar
manager-config-default = Padrão

manager-removed = Engine removida.
manager-installed = Engine instalada e testada.
manager-updated = Engine atualizada. A versão anterior foi removida.
manager-cancelled = Cancelado.
manager-detected = Busca concluída: { $detail } Engine(s) encontrada(s).
manager-in-use = Engine escolhida. Clique em "Iniciar contra a Engine" no painel para jogar.
manager-config-saved = Configuração salva.

manager-error-nodb = O banco de dados não está disponível; o gerenciador não pode guardar as Engines nesta sessão.
manager-error-nodir = Não foi possível descobrir a pasta de dados do aplicativo.
manager-error-store = Não foi possível gravar no banco de dados.
manager-error-not-uci = Esse arquivo não respondeu como uma Engine UCI e não foi adicionado.
manager-error-remove-refused = Por segurança, nada foi apagado: a pasta não é uma instalação do Chess Studio dentro da pasta de Engines.
manager-error-remove-io = Não foi possível apagar a pasta da Engine. Feche programas que a usem e tente de novo.
