# Segurança do gerenciador de Engines (PEND-08, issue #51)

Estado: **proposto para aprovação** do supervisor/usuário (critério "o documento de segurança está aprovado" da issue #51). Complementa o [ADR-0003](../adr/0003-engines-externas-download-fixado.md) (download fixado, nunca embutir) e as decisões DEC-27/DEC-28. Escopo: V1, múltiplas Engines UCI (Stockfish, Lc0 e outras).

## Por que isto importa

Uma Engine é um **programa nativo que o app baixa e executa**. Qualquer falha na cadeia de instalação vira execução de código arbitrário com os privilégios do usuário. O gerenciador de Engines é, portanto, a superfície de ataque mais sensível do Chess Studio, e todas as decisões abaixo partem de um princípio: **o app só executa o que o usuário confirmou, com origem e conteúdo verificados de antemão**.

## Ativos protegidos

1. A conta do usuário e seus arquivos (código nativo roda com os mesmos privilégios do app).
2. Os recursos da máquina (CPU, memória, disco, rede).
3. A integridade da Partida (a Engine só **propõe** Lances; o núcleo de regras valida, ADR-0003).
4. Os arquivos que o usuário já tinha (Engines instaladas por ele nunca são alteradas ou apagadas).

## Modelo de ameaças e mitigações

| # | Ameaça | Como ocorreria | Mitigação | Teste |
|---|---|---|---|---|
| T1 | **Binário adulterado** | Arquivo trocado no servidor, no CDN ou no meio do caminho | Catálogo **fixado e embutido no app**, com SHA-256 de cada arquivo calculado por nós ao baixá-lo. O hash é conferido **antes** de extrair; arquivo diferente é apagado e nada é executado. O hash não vem do servidor, vem do binário do app | `install.rs`: hash errado, corpo adulterado |
| T2 | **URL trocada / redirecionada** | Catálogo apontando para host malicioso; redirecionamento para outro host ou para HTTP | URLs só `https://` e só de **hosts oficiais** (lista fechada em código: `github.com` e `storage.lczero.org`); um teste falha se qualquer entrada do catálogo sair da lista. O cliente HTTP recusa rebaixar de HTTPS para HTTP em redirecionamentos. Mesmo que o host fosse comprometido, o hash (T1) barra o conteúdo | `catalog`: todas as URLs oficiais e HTTPS |
| T3 | **MITM** | Interceptação da conexão | TLS com `rustls` e raízes da Mozilla (`webpki-roots`); o SHA-256 embutido torna o TLS uma segunda camada, não a única | hash errado rejeitado; ver T1 |
| T4 | **Path traversal na extração** (zip-slip, links simbólicos, caminhos absolutos) | Arquivo com entradas `../../.bashrc`, `/etc/x`, links | A extração **nunca usa o caminho do arquivo**. Só entradas cujo caminho coincide **exatamente** com a lista esperada (executável, licença e DLLs declaradas no catálogo) são lidas, e gravadas por **nome-base** num diretório novo. Links e diretórios são ignorados. Cada entrada tem teto de tamanho (anti zip-bomb) | `install.rs`: entradas maliciosas ignoradas; teto |
| T5 | **Executável que não é Engine** | Arquivo escolhido pelo usuário (ou substituído) que não fala UCI, ou faz outra coisa | **Handshake UCI antes de registrar** (`uci` -> `uciok` em prazo curto), inclusive para Engines baixadas (executadas só depois do hash, na pasta temporária). A detecção só executa arquivos com **nome de Engine conhecida**; nunca varre diretórios rodando tudo. Executável escolhido à mão passa pelo handshake **após confirmação explícita** do usuário, que é avisado de que o arquivo será executado | `detect.rs`, `manager`: arquivo sem UCI recusado |
| T6 | **Engine maliciosa/defeituosa consumindo recursos** | Laço infinito, memória ilimitada, saída gigante, nunca responde | Prazos em toda espera (handshake, `isready`, busca, `stop`, `quit`) e **kill** do processo ao estourar; log circular limitado; um processo por vez por Partida; **limites de Threads e Hash** aplicados antes de enviar ao `setoption` (Threads <= núcleos lógicos, Hash <= teto do app e o máximo anunciado). Isolamento de SO (job objects, cgroups, sandbox) **não** faz parte da V1 (ver "Fora de escopo") | `engine.rs` (timeouts), `manager`: limites |
| T7 | **Injeção de comandos UCI** | Valor de opção com quebra de linha (`Hash value 1\nsetoption ...`/`debug`) enviado à Engine | Todo nome e valor de opção é **validado**: sem caracteres de controle, tipo e faixa conferidos contra a opção **anunciada no handshake**; opção não anunciada é recusada | `manager`: valores inválidos |
| T8 | **Opções que tocam o disco** | `Debug Log File`, `SyzygyPath`, `EvalFile` apontando para caminhos arbitrários (sobrescrever arquivos do usuário) | Opções de **arquivo/caminho** não são editáveis livremente: `Debug Log File` é bloqueada; a rede neural do Lc0 (`WeightsFile`) só aceita arquivos que o **app instalou** (com hash) em `<dados>/engines/networks/` | `manager`: opções restritas |
| T9 | **Atualização forçada ou silenciosa** | Servidor ou app que troca a versão sem o usuário saber; atualização que quebra a Engine | **Sem verificação em segundo plano**: a rede só é tocada numa ação do usuário (Instalar/Atualizar). "Atualizar" só oferece a versão **do catálogo embutido** (nova versão = nova release do app). A versão anterior fica no disco até a nova passar no hash **e** no handshake; se a nova falhar, **a anterior permanece intacta e registrada** | `manager`: atualização que falha mantém a anterior |
| T10 | **Remoção destrutiva** | Registro adulterado (`install_dir = ~`), link simbólico, `..`, remover a Engine do sistema do usuário | Só se remove um diretório que (a) está **estritamente dentro** de `<dados>/engines/` após canonicalização, (b) não é link simbólico e (c) contém o **marcador de instalação** gravado pelo app. Engines detectadas ou escolhidas à mão são **só desregistradas**, nunca apagadas do disco | `manager`: remoção fora do diretório, link, sem marcador, Engine do usuário |
| T11 | **Consentimento ausente** | Download ou execução automáticos | **Nada é baixado nem executado sem confirmação por ação**: a janela mostra Engine, versão, **licença** (com link), **fonte**, URL, **tamanho** e o aviso de rede antes do botão "Aceito e quero instalar". O consentimento vale para aquela ação, não é lembrado | UI + testes de chaves Fluent |
| T12 | **Banco adulterado** | `engine_installations` com caminho para executável arbitrário | O caminho gravado só é executado depois de o usuário **iniciar** a Engine; o registro de Engine instalada pelo app guarda o hash, e a janela mostra "arquivo não encontrado" se o executável sumiu. Registros nunca levam o app a apagar nada sem o marcador (T10) | `manager` |

## Princípios aplicados

- **Catálogo fixado, embutido, auditável.** Versão, URL, tamanho e SHA-256 vivem no código (`crates/chess-uci/src/catalog.rs`) e passam por revisão de PR. Não existe "última versão" nem catálogo remoto.
- **Hash antes de tudo.** Download -> SHA-256 em fluxo -> comparação -> só então extração. Teto de bytes no download.
- **Menor privilégio de extração.** Lista branca de arquivos, nome-base, diretório de staging (`.<nome>.part`) apagado por *guard* em qualquer falha.
- **Validar antes de registrar.** Nenhum executável entra na lista sem handshake UCI.
- **Instalação atômica e reversível.** Nova pasta com nome versionado (`<engine>-<versão>[-<variante>]`); a anterior só sai depois da validação.
- **A Engine propõe, o núcleo decide.** Inalterado do ADR-0003: Lance ilegal é erro recuperável.
- **Licenças visíveis.** Stockfish e Lc0 são GPL-3.0: o app **não os embute nem os liga** (processos separados baixados do site oficial), logo o ADR-0004 continua valendo. A licença e o link da fonte aparecem antes do download, e os arquivos de licença do pacote são mantidos junto do binário.

## Catálogo da V1 e como ele é mantido

Ver a tabela no PR e `catalog.rs`. Regras de manutenção:

1. Mudar versão/URL/hash exige PR, com os hashes **recalculados baixando o arquivo** (não copiados só do rodapé da release). Se existir `digest` publicado pelo fornecedor, os dois devem coincidir.
2. Cada entrada declara os arquivos que serão extraídos (executável, licença, DLLs).
3. Redes neurais do Lc0 são ativos separados, com hash próprio e licença registrada.
4. Testes com rede real ficam `#[ignore]`; a suíte padrão roda offline.

## O que fica fora (riscos aceitos)

- **Assinatura de código do fornecedor.** Stockfish e Lc0 não publicam assinaturas verificáveis (GPG/Authenticode/notarização) dos binários das releases. A confiança vem do **hash que nós calculamos** em um download feito de fonte oficial: protege contra adulteração **posterior**, não contra um binário já comprometido na origem no momento em que fixamos o hash. Mitigação: conferir com o `digest` publicado pela release e rever a cada atualização do catálogo.
- **Sandbox do SO.** A Engine roda com os privilégios do usuário; não há *job object*, cgroup, `seccomp` ou AppContainer na V1. Candidato a V2.
- **Engines escolhidas à mão e Engines do sistema** (apt, brew). São executadas como o usuário as indicou; o app só garante o handshake e a validação de Lances. O app não altera nem remove esses arquivos.
- **Linux e macOS do Lc0.** O projeto Lc0 não publica binários oficiais para esses sistemas (só Windows e Android). Nesses sistemas o app **detecta** um Lc0 instalado pelo usuário (pacote da distribuição, Homebrew ou compilação própria), mas não o baixa. A rede neural pode ser baixada em qualquer SO.
- **Bibliotecas de terceiros dentro dos pacotes do Lc0** (`libopenblas.dll`, `dnnl.dll`, `onnxruntime.dll`) são cobertas pelo mesmo hash do pacote e extraídas por nome; o app não as substitui. O `directml.dll` (pacote ONNX-DML) **não** é baixado pelo app, por licença; o usuário o obtém como descreve o README do pacote.
- **Atualização do próprio catálogo** depende de uma nova release do app (decisão DEC-28 mantida).
- **Origem comprometida e hash recalculado por um mantenedor desatento**: coberto por revisão de PR, não por código.

## Verificação (resumo)

`cargo test -p chess-uci` cobre T1, T2, T4, T5, T6 (prazos), T7, T8, T9, T10 com servidor HTTP local e Engine falsa; a rede real roda só sob `--ignored`.
