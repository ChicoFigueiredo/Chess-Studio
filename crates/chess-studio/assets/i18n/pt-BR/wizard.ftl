## Assistente de Engine (B10): detectar, escolher ou baixar o Stockfish fixado.

wizard-title = Assistente de Engine
wizard-intro = Para jogar contra o computador é preciso uma Engine de xadrez (programa que fala o protocolo UCI). O Chess Studio não traz nenhuma embutida.
wizard-without-engine = Sem Engine você continua usando a Partida a dois no mesmo computador e os Exercícios. Só o modo contra a Engine fica indisponível.
wizard-open = Procurar ou baixar Engine…

wizard-detected-heading = Engines encontradas neste computador
wizard-detecting = Procurando Engines instaladas…
wizard-none-detected = Nenhuma Engine instalada foi encontrada.
wizard-engine-author = Autor: { $author }
wizard-use = Usar esta Engine

wizard-choose-heading = Escolher executável
wizard-choose-hint = Caminho do executável da Engine
wizard-choose-button = Verificar e usar
wizard-checking = Verificando a Engine…
wizard-check-failed = Esse arquivo não respondeu como uma Engine UCI: { $detail }

wizard-download-heading = Baixar o Stockfish { $version }
wizard-download-button = Baixar Stockfish { $version }
wizard-download-unavailable = Não há download oficial do Stockfish para este sistema ({ $os }, { $arch }). Instale uma Engine por conta própria e escolha o executável acima.
wizard-license = O Stockfish é software livre sob a licença { $license }. Ele é baixado do site oficial e fica numa pasta do Chess Studio; o aplicativo não o inclui nem o modifica.
wizard-license-link = Ler a licença
wizard-source-link = Código-fonte do Stockfish { $version }
wizard-download-size = Arquivo: { $file } ({ $size })
wizard-download-network-notice = O download usa a internet só agora, por sua escolha. O arquivo é conferido por uma assinatura SHA-256 embutida no aplicativo; se não coincidir, é apagado.
wizard-download-confirm = Aceito a licença e quero baixar
wizard-download-cancel = Cancelar
wizard-retry = Tentar de novo

wizard-step-downloading = Baixando… { $done } de { $total }
wizard-step-downloading-unknown = Baixando… { $done }
wizard-step-verifying = Conferindo a assinatura SHA-256…
wizard-step-extracting = Extraindo…
wizard-step-validating = Testando a Engine…

wizard-error-title = O download não foi concluído
wizard-error-network = Sem conexão com a internet (ou o servidor não respondeu). Confira a rede e tente de novo.
wizard-error-http = O servidor recusou o download (HTTP { $code }). Tente de novo mais tarde.
wizard-error-hash = O arquivo baixado não confere com a assinatura SHA-256 esperada e foi apagado. Nada foi instalado.
wizard-error-disk = Não foi possível gravar no disco. Verifique o espaço livre e as permissões.
wizard-error-invalid = O arquivo baixado está íntegro, mas a Engine não pôde ser usada.
wizard-error-nodir = Não foi possível descobrir a pasta de dados do aplicativo.
wizard-error-detail = Detalhe: { $detail }

wizard-ready = Engine pronta: { $name }
wizard-ready-hint = Clique em "Iniciar contra a Engine" no painel para jogar.
wizard-close = Fechar
wizard-continue-without = Continuar sem Engine
