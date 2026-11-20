# Instalar o Chess Studio

Este guia vale para a versão **0.1.0**. Os pacotes ficam na página de [Releases](https://github.com/ChicoFigueiredo/Chess-Studio/releases); o primeiro publicado é o pre-release `v0.1.0-rc1`.

O Chess Studio roda **sem internet e sem cadastro**. Partidas, treino e preferências ficam num banco SQLite no seu computador. A rede só é usada quando você liga um Provedor (Lichess, Chess.com e outros) na Central de Conexões ou pede um download, como o de uma Engine. Antes disso, o app diz o que vai sair.

Os pacotes **não são assinados** e, no macOS, também não são notarizados. Por isso cada sistema avisa na primeira abertura; cada seção abaixo diz como liberar. O ícone do app ainda é provisório.

## Qual arquivo baixar

| Sistema | Arquivo | O que é |
|---|---|---|
| macOS 12 Monterey ou mais novo, Apple Silicon (M1 ou mais novo) | `chess-studio-0.1.0-macos-arm64.dmg` | Imagem de disco com o **Chess Studio** |
| Windows 10 e 11, x86_64 | `chess-studio-0.1.0-windows-x86_64-setup.exe` | Instalador (recomendado) |
| | `chess-studio-0.1.0-windows-x86_64.zip` | Portátil, sem instalar |
| Linux, x86_64 | `chess-studio-0.1.0-linux-x86_64.AppImage` | Arquivo único, para qualquer distribuição |
| | `chess-studio_0.1.0_amd64.deb` | Pacote para Debian, Ubuntu e derivados |
| | `chess-studio-0.1.0-linux-x86_64.tar.gz` | Portátil, sem instalar |
| Todos | `SHA256SUMS` | Somas para [conferir o download](#conferir-o-download) |
| | `THIRD-PARTY-LICENSES` | Licenças das dependências do app |

Não há pacote para Mac com processador Intel. Nesse caso, compile a partir do código (veja [Mac com processador Intel](#mac-com-processador-intel)).

## Requisitos de vídeo

- **macOS:** Metal, que todo Mac com Apple Silicon tem.
- **Windows:** DirectX 12 ou Vulkan.
- **Linux:** driver Vulkan, da NVIDIA ou da Mesa (pacote `mesa-vulkan-drivers`). O comando `vulkaninfo --summary` deve listar a sua placa.

## Conferir o download

O arquivo `SHA256SUMS` traz uma linha por pacote, com a soma SHA-256 e o nome do arquivo. Baixe-o na mesma pasta do pacote e compare a soma do arquivo baixado com a linha dele. Se o navegador mudou o nome do arquivo (por exemplo, acrescentando "(1)"), devolva o nome original antes de conferir.

**macOS** (Terminal):

```bash
cd ~/Downloads
grep macos-arm64.dmg SHA256SUMS | shasum -a 256 -c
```

A resposta deve ser `chess-studio-0.1.0-macos-arm64.dmg: OK`. Para ver a soma, use `shasum -a 256 chess-studio-0.1.0-macos-arm64.dmg`.

**Linux:**

```bash
cd ~/Downloads
sha256sum -c --ignore-missing SHA256SUMS
```

Cada pacote presente na pasta deve aparecer com `OK`.

**Windows** (PowerShell):

```powershell
cd $HOME\Downloads
(Get-FileHash .\chess-studio-0.1.0-windows-x86_64-setup.exe -Algorithm SHA256).Hash.ToLower()
Select-String "windows-x86_64-setup.exe" .\SHA256SUMS
```

As duas sequências de letras e números precisam ser iguais. Para o `.zip`, troque o nome do arquivo nas duas linhas.

Se a soma não bater, não abra o arquivo: baixe de novo.

## macOS

### Confira o chip do seu Mac

Abra o menu Apple > **Sobre Este Mac**. Se aparecer **Chip** Apple M1, M2, M3, M4 ou mais novo, use o `.dmg`. Se aparecer **Processador** Intel, veja [Mac com processador Intel](#mac-com-processador-intel).

### Instalar

1. Baixe `chess-studio-0.1.0-macos-arm64.dmg` e dê dois cliques nele. O Finder abre o disco "Chess Studio".
2. A janela do disco mostra o **Chess Studio** e um atalho **Applications** (a pasta Aplicativos). Arraste o Chess Studio sobre o atalho. Isso copia o app para `/Applications`.
3. Ejete o disco pelo botão de ejetar ao lado de "Chess Studio" na barra lateral do Finder, ou arrastando o disco para o Lixo. Depois disso, o `.dmg` pode ser apagado.
4. Abra o Chess Studio pela pasta Aplicativos, pelo Launchpad ou pelo Spotlight. Não o abra de dentro do disco do `.dmg`.

### Liberar a primeira abertura (Gatekeeper)

Como o app não é assinado nem notarizado, a primeira tentativa de abrir mostra um aviso de que o macOS não conseguiu verificar o app. Feche o aviso **sem** escolher **Mover para o Lixo**. A liberação é feita uma vez por versão: ao baixar uma versão nova, repita.

**macOS Sequoia 15, Tahoe 26 e mais novos.** Este é o único caminho que a Apple descreve para essas versões ([Abra um app do Mac de um desenvolvedor desconhecido](https://support.apple.com/pt-br/guide/mac-help/mh40616/mac) e [Abrir apps com segurança no Mac](https://support.apple.com/pt-br/102445)):

1. Tente abrir o Chess Studio uma vez, como acima.
2. Abra o menu Apple > **Ajustes do Sistema** e clique em **Privacidade e Segurança** na barra lateral. Talvez seja preciso rolar a barra lateral.
3. Na seção **Segurança** aparece uma mensagem sobre o Chess Studio. Clique em **Abrir Mesmo Assim**. Segundo a Apple, o botão fica disponível por cerca de uma hora depois da tentativa de abrir o app. Se ele não estiver lá, tente abrir o app de novo e volte.
4. Confirme no aviso seguinte. Quando o macOS pedir, digite a sua senha de início de sessão e clique em **OK**.
5. O app abre e fica salvo como exceção. Daí em diante, ele abre com dois cliques.

A partir do macOS Sequoia 15, o Control + clique > **Abrir** deixou de liberar um app que não é notarizado. A Apple anunciou a mudança em [Updates to runtime protection in macOS Sequoia](https://developer.apple.com/news/?id=saqachfa) (06/08/2024): agora é preciso passar por Ajustes do Sistema > Privacidade e Segurança. A página de ajuda do macOS 15 e mais novos não traz mais o Control + clique; a do macOS Sonoma 14 ainda traz.

**macOS Monterey 12, Ventura 13 e Sonoma 14.** No Finder, abra a pasta Aplicativos e dê Control + clique no Chess Studio. Escolha **Abrir** no menu de atalho e clique em **Abrir** no aviso ([página da Apple para o macOS 14](https://support.apple.com/pt-br/guide/mac-help/mh40616/14.0/mac/14.0)). O caminho por Ajustes do Sistema também funciona nessas versões.

**Pelo Terminal, em qualquer versão:**

```bash
xattr -dr com.apple.quarantine "/Applications/Chess Studio.app"
```

O comando tira do app a marca de "baixado da internet" (a quarentena), que é o que o Gatekeeper confere. Use-o também se o aviso disser que o Chess Studio "está danificado e não pode ser aberto". Esse aviso pode aparecer com apps sem assinatura baixados pela internet e não oferece o botão **Abrir Mesmo Assim**. O `xattr` é uma ferramenta do próprio macOS (`man xattr`), mas esse uso não está nas páginas da Apple citadas acima.

### Bom saber no Mac

- **Atalhos:** os atalhos com Ctrl usam a tecla **Control**, não a Command. Alt é a tecla **Option**: Option + clique faz o Ajuste (j'adoube), e Option sozinha mostra os KeyTips do Ribbon.
- **Teclas F:** no teclado do Mac, F10 (Modo Foco) e F11 (Tela cheia) podem exigir a tecla Fn. O macOS pode usar o F11 para outra coisa; nesse caso, use **Tela cheia** na aba Exibir do Ribbon ou troque o atalho em Arquivo > Controles….
- **Trackpad:** dois dedos orbitam a câmera, Shift + dois dedos deslocam e a pinça dá zoom.
- **Engine:** o assistente de Engine baixa o Stockfish 19 para macOS, com o seu consentimento. Se preferir o Homebrew, `brew install stockfish` (ou `brew install lc0`) também serve: o app procura Engines em `/opt/homebrew/bin`.
- **Pasta Documentos:** ao importar ou exportar PGN, o macOS pode perguntar se o Chess Studio pode acessar a pasta Documentos. Se você recusar, o botão **Escolher arquivo…** continua funcionando.

### Atualizar no Mac

Feche o Chess Studio, abra o `.dmg` da versão nova, arraste o app sobre **Applications** e escolha **Substituir**. Depois libere a primeira abertura de novo, como acima. Os seus dados não são tocados. Por precaução, faça um backup antes de atualizar ([atualização e recuperação](https://github.com/ChicoFigueiredo/Chess-Studio/blob/main/docs/atualizacao-e-recuperacao.md)).

### Desinstalar no Mac

1. Se você autorizou o Lichess, abra antes a Central de Conexões e use **Desconectar…** em cada Conexão autorizada. Isso revoga a autorização no Lichess e apaga o segredo guardado no Acesso às Chaves.
2. Feche o Chess Studio.
3. No Finder, abra a pasta Aplicativos e arraste o **Chess Studio** para o Lixo. Depois esvazie o Lixo.
4. Os seus dados **ficam** em `~/Library/Application Support/ChessStudio`. Para apagá-los também, abra no Finder o menu **Ir > Ir para a Pasta…** (Shift + Command + G), cole `~/Library/Application Support/ChessStudio` e arraste a pasta `ChessStudio` para o Lixo. Isso apaga Partidas, Perfis locais e preferências; faça um backup antes, se quiser guardá-los.
5. Se pulou o passo 1, apague no Acesso às Chaves os itens do serviço "Chess Studio" e revogue a chave nas preferências do Lichess, na área de acesso por chaves.

### Mac com processador Intel

Não há pacote para Intel. Siga os pré-requisitos e os comandos de [docs/build/macos.md](https://github.com/ChicoFigueiredo/Chess-Studio/blob/main/docs/build/macos.md). O guia foi escrito para a VM do projeto, mas os passos valem num Mac físico. Depois rode `cargo run -p chess-studio --release`.

## Windows

### Instalador (recomendado)

1. Baixe `chess-studio-0.1.0-windows-x86_64-setup.exe`. Se o navegador avisar que o arquivo não é baixado com frequência, escolha manter o arquivo.
2. Abra o instalador. Como ele não é assinado, o SmartScreen pode mostrar **O Windows protegeu o computador**. Clique em **Mais informações** e depois em **Executar mesmo assim**.
3. O instalador instala **só para o usuário atual**, sem pedir administrador, em `%LOCALAPPDATA%\Chess Studio` (por exemplo, `C:\Users\<você>\AppData\Local\Chess Studio`). Ele cria o atalho **Chess Studio** no Menu Iniciar, com o ícone.
4. Na última página, marque se quiser um atalho na Área de Trabalho, e **Executar Chess Studio** para abrir o app na hora.

Para atualizar, rode o instalador da versão nova; os seus dados não são tocados.

### Portátil (.zip)

1. Extraia `chess-studio-0.1.0-windows-x86_64.zip` (botão direito > **Extrair tudo**) numa pasta sua.
2. Rode `chess-studio.exe` dentro da pasta extraída. Não separe o `.exe` da pasta `assets`.
3. Se o SmartScreen avisar, faça como no instalador: **Mais informações** > **Executar mesmo assim**.

### Desinstalar no Windows

1. Se você autorizou o Lichess, use antes **Desconectar…** na Central de Conexões (veja o passo 1 do Mac).
2. Instalador: abra **Configurações > Aplicativos > Aplicativos instalados** (no Windows 10, **Aplicativos e recursos**), escolha **Chess Studio** e **Desinstalar**. Portátil: apague a pasta extraída.
3. Os seus dados **ficam** em `%APPDATA%\ChessStudio`. Para apagá-los, abra essa pasta pelo Win + R (digite `%APPDATA%`) e apague `ChessStudio`.
4. Autorizações que sobraram ficam no Gerenciador de Credenciais do Windows, em itens com "Chess Studio" no nome.

## Linux

### AppImage (qualquer distribuição)

```bash
chmod +x chess-studio-0.1.0-linux-x86_64.AppImage
./chess-studio-0.1.0-linux-x86_64.AppImage
```

Pelo gerenciador de arquivos, o mesmo é feito em Propriedades > Permissões > "Permitir executar como programa". Guarde o arquivo numa pasta fixa, como `~/Aplicativos`. O AppImage não cria entrada no menu sozinho.

Possível requisito: o AppImage precisa da biblioteca FUSE 2. Se aparecer um erro com `libfuse.so.2` ou "AppImages require FUSE to run", instale-a ([documentação do AppImage](https://docs.appimage.org/user-guide/troubleshooting/fuse.html)):

- Ubuntu 22.04 e 23.x: `sudo apt install libfuse2`
- Ubuntu 24.04 ou mais novo, onde o pacote mudou de nome: `sudo apt install libfuse2t64`
- Sem FUSE: `./chess-studio-0.1.0-linux-x86_64.AppImage --appimage-extract-and-run` (mais lento para abrir)

### Pacote .deb (Debian, Ubuntu e derivados)

```bash
sudo apt install ./chess-studio_0.1.0_amd64.deb
chess-studio
```

O pacote cria a entrada **Chess Studio**, com ícone, no menu de aplicativos. Para atualizar, instale o `.deb` da versão nova do mesmo jeito.

### Portátil (.tar.gz)

```bash
tar xzf chess-studio-0.1.0-linux-x86_64.tar.gz
cd chess-studio-0.1.0-linux-x86_64     # a pasta criada pela extração
./chess-studio
```

Precisa de `libvulkan1`, `libxkbcommon0` e um driver Vulkan (NVIDIA ou Mesa). Mantenha o executável ao lado da pasta `assets`.

### Desinstalar no Linux

1. Se você autorizou o Lichess, use antes **Desconectar…** na Central de Conexões (veja o passo 1 do Mac).
2. `.deb`: `sudo apt remove chess-studio`. AppImage: apague o arquivo. `.tar.gz`: apague a pasta extraída.
3. Os seus dados **ficam** em `~/.local/share/ChessStudio` (ou em `$XDG_DATA_HOME/ChessStudio`). Para apagá-los: `rm -rf ~/.local/share/ChessStudio`.
4. Autorizações que sobraram ficam no Secret Service (GNOME Senhas e Chaves ou KWallet), em itens do serviço "Chess Studio".

## Onde ficam os seus dados

| Sistema | Pasta de dados |
|---|---|
| macOS | `~/Library/Application Support/ChessStudio` |
| Windows | `%APPDATA%\ChessStudio` (por exemplo, `C:\Users\<você>\AppData\Roaming\ChessStudio`) |
| Linux | `$XDG_DATA_HOME/ChessStudio`, se for caminho absoluto; senão, `~/.local/share/ChessStudio` |

A pasta fica separada do programa instalado e guarda:

- `chess-studio.db`, o banco das Partidas, Perfis locais, preferências, Biblioteca, Exercícios e Conexões (só nomes públicos e o que cada autorização permite, nunca a chave). Junto dele podem aparecer `chess-studio.db-wal` e `chess-studio.db-shm`.
- `rede.db`, com o que está ligado em cada Provedor e os dados guardados (cache) das consultas.
- `engines/`, com as Engines baixadas pelo app.
- `exercise-packs/`, com os pacotes de problemas instalados.
- `cofre/`, só se o computador não tem cofre do sistema e você criou o cofre do Chess Studio, com senha-mestra.

Desinstalar o programa **não apaga** essa pasta. O PGN exportado vai para a pasta Documentos, fora dela. Backup, restauração e banco corrompido: [atualização e recuperação](https://github.com/ChicoFigueiredo/Chess-Studio/blob/main/docs/atualizacao-e-recuperacao.md).

## Onde o app procura os assets

A pasta `assets/` traz modelos, texturas e fontes. O app a procura nesta ordem:

1. a variável de ambiente `CHESS_STUDIO_ASSETS`, se existir;
2. a pasta `assets` ao lado do executável (pacotes portáteis `.zip` e `.tar.gz`);
3. no Linux instalado, `../lib/chess-studio/assets` e depois `../share/chess-studio/assets`, relativas ao executável. O `.deb` e o AppImage usam `/usr/lib/chess-studio/assets`;
4. no macOS, `../Resources/assets`, dentro do `Chess Studio.app`.

Se o executável for movido sem a pasta `assets/`, o tabuleiro 3D aparece quase preto. Mantenha os dois juntos.

## Conferir que funciona offline (PLAT-02)

1. Desligue a rede (modo avião, ou desconecte o cabo ou o Wi-Fi).
2. Abra o Chess Studio. Ele deve abrir direto no tabuleiro, sem tela de entrada, aviso de conexão ou espera. A barra de status mostra **Rede: offline**.
3. Jogue alguns Lances (Assistido, dois jogadores), troque de Apresentação com `Tab` e feche. Reabra e confira que a Partida foi gravada.
4. Opcional, no Linux com o `.tar.gz`: `unshare -rn ./chess-studio` roda o app num ambiente sem nenhuma interface de rede.

## Jogar contra uma Engine

O Chess Studio não distribui Engines. Na Nova Partida, escolha a Engine como adversário. Se não houver nenhuma, o **assistente de Engine** oferece duas saídas:

- **Procurar no computador:** acha Engines instaladas no `PATH` e em pastas comuns de cada sistema, como `/opt/homebrew/bin` no Mac.
- **Baixar o Stockfish 19** do site oficial: o app mostra a licença e o tamanho, só baixa depois de você aceitar e confere o arquivo com o SHA-256 embutido. É a única parte do jogo offline que usa a rede, e só durante o download.

Antes de executar um arquivo pela primeira vez, o app pede a sua confirmação e verifica se ele fala UCI. O gerenciador de Engines (Painel Engines) instala, atualiza e remove Engines, inclusive o Lc0 no Windows. Quem abre o app pelo terminal também pode apontar `CHESS_STUDIO_ENGINE` para o executável da Engine.

## Problemas comuns

- **macOS: "não pode ser aberto" ou "não foi possível verificar":** veja [Liberar a primeira abertura](#liberar-a-primeira-abertura-gatekeeper).
- **macOS: o botão Abrir Mesmo Assim não aparece:** tente abrir o app de novo e volte a Privacidade e Segurança em até uma hora, ou use o comando `xattr` acima.
- **macOS: "está danificado e não pode ser aberto":** use o comando `xattr` acima. Não é preciso baixar de novo se a soma do `SHA256SUMS` bateu.
- **Windows: "O Windows protegeu o computador":** clique em **Mais informações** > **Executar mesmo assim**.
- **Linux: o AppImage não abre e cita `libfuse.so.2`:** instale a FUSE 2 (veja [AppImage](#appimage-qualquer-distribuição)).
- **Tabuleiro quase preto ou sem peças:** o app não encontrou a pasta `assets/`. Mantenha-a ao lado do executável ou aponte `CHESS_STUDIO_ASSETS` para ela (veja [Onde o app procura os assets](#onde-o-app-procura-os-assets)).
- **A janela não abre:** teste outro backend gráfico pelo terminal, por exemplo `WGPU_BACKEND=vulkan` (Linux e Windows), `dx12` (Windows) ou `gl`.
