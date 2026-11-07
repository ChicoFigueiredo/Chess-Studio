# Instalar o Chess Studio

O Chess Studio roda **sem internet e sem login**: tudo (partidas, treino, configurações) fica num banco SQLite local. A rede só é usada se você mesmo pedir o download de uma Engine (por exemplo, o Stockfish). Os pacotes não são assinados (veja os avisos de cada sistema).

Requisito comum: placa de vídeo com driver Vulkan (Linux), DirectX 12 ou Vulkan (Windows) ou Metal (macOS).

Cada pacote traz a pasta `assets/` (modelos, texturas e fontes) e os arquivos `LICENSE-MIT`, `LICENSE-APACHE` e `THIRD-PARTY-LICENSES`. O app procura os assets, nesta ordem: variável `CHESS_STUDIO_ASSETS`, pasta `assets` ao lado do executável, `../share/chess-studio/assets` (Linux instalado), `../Resources/assets` (macOS `.app`). Se você mover o executável sem a pasta `assets/`, o tabuleiro 3D aparece quase preto; mantenha-os juntos.

## Linux

**Opção 1: pacote `.deb` (Ubuntu, Debian e derivados)**

```bash
sudo apt install ./chess-studio_0.1.0_amd64.deb
chess-studio
```

Para remover: `sudo apt remove chess-studio`.

**Opção 2: tarball portátil (qualquer distribuição)**

```bash
tar xzf chess-studio-0.1.0-linux-x86_64.tar.gz
cd chess-studio-0.1.0-linux-x86_64
./chess-studio
```

Precisa de `libvulkan1`, `libxkbcommon0` e um driver Vulkan (NVIDIA ou Mesa).

## Windows

1. Extraia `chess-studio-0.1.0-windows-x86_64.zip` (botão direito, "Extrair tudo") numa pasta sua, por exemplo `C:\Programas\ChessStudio`.
2. Execute `chess-studio.exe` dentro da pasta extraída (não separe o `.exe` da pasta `assets`).
3. Aviso do SmartScreen ("O Windows protegeu o computador"): como o pacote não é assinado, clique em **Mais informações** e depois em **Executar mesmo assim**.

## macOS

1. Abra `chess-studio-0.1.0-macos-<arquitetura>.dmg` (ou extraia o `.tar.gz`) e arraste **Chess Studio.app** para `Aplicativos`.
2. O app não é assinado nem notarizado, então o Gatekeeper bloqueia a primeira abertura ("não pode ser aberto porque o desenvolvedor não pode ser verificado"). Para liberar, use um destes caminhos:
   - Clique com o botão direito (ou Control + clique) em **Chess Studio.app**, escolha **Abrir** e confirme **Abrir**; ou
   - `Ajustes do Sistema > Privacidade e Segurança`, role até a mensagem sobre o Chess Studio e toque em **Abrir Mesmo Assim**; ou
   - No Terminal: `xattr -dr com.apple.quarantine "/Applications/Chess Studio.app"`.

## Onde ficam os seus dados

| Sistema | Banco de dados |
|---|---|
| Linux | `~/.local/share/ChessStudio/chess-studio.db` |
| Windows | `%APPDATA%\ChessStudio\chess-studio.db` |
| macOS | `~/Library/Application Support/ChessStudio/chess-studio.db` |

Desinstalar não apaga esse arquivo; apague-o à mão se quiser recomeçar.

## Conferir que funciona offline (PLAT-02)

1. Desligue a rede (modo avião, ou desconecte o cabo/Wi-Fi) e **não** entre em nenhuma conta.
2. Abra o Chess Studio. Ele deve abrir direto no tabuleiro, sem tela de login, aviso de conexão ou espera.
3. Jogue alguns lances (Assistido, dois jogadores), troque de Apresentação com `Tab` e feche. Reabra e confira que a partida foi gravada.
4. Opcional, no Linux: `unshare -rn ./chess-studio` roda o app num ambiente sem nenhuma interface de rede.

## Jogar contra uma Engine

O Chess Studio não distribui Engines. Instale o Stockfish pelo gerenciador do sistema ou pelo [site oficial](https://stockfishchess.org/download/) e deixe `stockfish` no `PATH` (ou aponte `CHESS_STUDIO_ENGINE` para o executável). Isso é a única parte que pode precisar de rede, e só durante o download.

## Problemas comuns

- **Tabuleiro quase preto ou sem peças:** o app não encontrou a pasta `assets/`. Mantenha-a ao lado do executável ou aponte `CHESS_STUDIO_ASSETS` para ela.
- **Janela não abre:** teste o backend gráfico, por exemplo `WGPU_BACKEND=vulkan` (Linux/Windows), `dx12` (Windows) ou `gl`.
