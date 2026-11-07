# Build e testes no Windows (VM QEMU)

Contexto (DEC-25): o Windows roda em VM QEMU local, onde o usuário abre o Claude para compilar e testar. FPS medido em VM **não vale como evidência** de desempenho; a VM serve para provar que compila, que os testes passam e, se possível, que a janela abre. Homologação visual e de desempenho fica para a máquina Windows física.

## Pré-requisitos

Em PowerShell:

```powershell
winget install --id Git.Git -e
winget install --id Rustlang.Rustup -e
# Compilador MSVC + Windows SDK (caminho padrão do Rust no Windows):
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

A carga de trabalho "Desenvolvimento para desktop com C++" (`VCTools`) inclui o MSVC e o Windows 10/11 SDK; confirme que o SDK foi marcado. Feche e reabra o terminal depois.

**Caminho alternativo (sem MSVC):** toolchain GNU com MSYS2/MinGW-w64 (`winget install MSYS2.MSYS2`, depois `pacman -S mingw-w64-x86_64-toolchain` e `mingw64\bin` no PATH), `rustup toolchain install 1.98-x86_64-pc-windows-gnu` e `$env:RUSTUP_TOOLCHAIN="1.98-x86_64-pc-windows-gnu"`. Não foi validado neste projeto; use só se o MSVC não instalar e anote no relatório.

## Compilar e testar

```powershell
git clone https://github.com/ChicoFigueiredo/Chess-Studio.git
cd Chess-Studio
rustup show                      # o rustup instala o canal 1.98 do rust-toolchain.toml
rustc --version; cargo --version
cargo test --workspace *>&1 | Tee-Object test-windows.log
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p chess-core --release --test perft -- --ignored   # perft d5/d6 (opcional)
```

Os testes de domínio (regras, partida, UCI com Engine falsa) não precisam de GPU. A Engine falsa `fake_uci` é compilada pelo próprio `cargo test -p chess-uci`.

## Executar

```powershell
cargo run -p chess-studio
cargo run -p chess-studio -- --screenshot docs\evidencias\gate-a-windows
cargo run --release -p chess-studio -- --benchmark --minutes 1 --out docs\evidencias\gate-a-windows
```

Na VM use `--benchmark --minutes 1`: o objetivo é provar que roda, não medir.

## Notas específicas de VM (QEMU sem GPU)

Sem passthrough de GPU, o Windows enxerga um adaptador de vídeo básico (VGA, virtio ou QXL). O wgpu escolhe entre:

- **DX12 com WARP**, o rasterizador por software da Microsoft (aparece como "Microsoft Basic Render Driver", tipo `Cpu`). Funciona quando o DirectX 12 está presente, mas é lento.
- **Vulkan via Mesa** (lavapipe, também por software), só se houver um driver Vulkan instalado na VM.
- **GL**, se houver um `opengl32.dll` com OpenGL 3.3 ou superior (por exemplo, um build Mesa llvmpipe).

Forçar o backend (PowerShell, antes do `cargo run`):

```powershell
$env:WGPU_BACKEND = "dx12"     # ou "vulkan" ou "gl"
$env:RUST_LOG = "info"         # imprime AdapterInfo (nome, backend, tipo de dispositivo)
cargo run -p chess-studio
```

Se a janela não abrir ou o app terminar com "No adapters found" ou "Unable to find a GPU", teste os três backends em sequência e registre o resultado de cada um. Isso é informação válida para o relatório (risco de compatibilidade conhecido) e não invalida o build.

Se `WGPU_BACKEND=dx12` falhar, `$env:WGPU_ADAPTER_NAME="Microsoft"` seleciona o adaptador WARP pelo nome.

## O que anotar no relatório

Copie para os campos `PENDENTE (usuário)` de `docs/relatorios/gate-a.md`:

- Versão do Windows (`winver`), CPU, RAM, hipervisor (QEMU, versão, acelerador KVM ou WHPX) e tipo do adaptador de vídeo da VM.
- `rustc --version` e a versão do MSVC/SDK (ou do toolchain GNU, se usado).
- Resultado de `cargo build`: ok ou erro (as 20 últimas linhas se falhar).
- `cargo test --workspace`: total, aprovados, falhas (anexe `test-windows.log`).
- A janela abriu? Com qual `WGPU_BACKEND` e qual adaptador (`AdapterInfo` do log)? Capturas geradas por `--screenshot` (anexe 1 ou 2 PNGs).
- Qualquer erro de wgpu, DX12 ou Vulkan, copiado literalmente.
