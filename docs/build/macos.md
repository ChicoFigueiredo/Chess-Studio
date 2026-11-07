# Build e testes no macOS (VM QEMU)

Contexto (DEC-25): o macOS roda em VM QEMU local para compilar e rodar testes. Homologação visual e de desempenho só no Mac físico.

> **Risco aceito pelo usuário (DEC-25):** executar macOS em QEMU fora de hardware Apple contraria a EULA da Apple, e a VM não oferece Metal acelerado. O usuário assumiu esse risco, registrado no relatório do Gate A. Nenhum resultado desta VM substitui a homologação em um Mac real.

## Pré-requisitos

```bash
xcode-select --install          # Xcode Command Line Tools (clang, linker, SDK do macOS)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
```

O git vem com as Command Line Tools. Não é preciso instalar o Xcode completo.

## Compilar e testar

```bash
git clone https://github.com/ChicoFigueiredo/Chess-Studio.git
cd Chess-Studio
rustc --version                  # o rustup instala o canal 1.98 do rust-toolchain.toml
cargo build --workspace 2>&1 | tee build-macos.log
cargo test --workspace 2>&1 | tee test-macos.log
cargo test -p chess-core --release --test perft -- --ignored   # perft d5/d6 (opcional)
```

Os testes de domínio (regras, partida, UCI com Engine falsa) não dependem de GPU e continuam válidos mesmo que a janela não abra. Anote `uname -m` (x86_64 ou arm64).

## Executar

```bash
cargo run -p chess-studio
cargo run -p chess-studio -- --screenshot docs/evidencias/gate-a-macos
```

## Notas específicas de VM

- Sem Metal acelerado, o wgpu pode não encontrar adaptador utilizável e a janela **pode não abrir** (erro como "No adapters found" ou panic na criação da superfície). Isso é esperado na VM e não reprova o build.
- Se houver um dispositivo Metal por software ou paravirtualizado, o log (`RUST_LOG=info`) mostra `AdapterInfo` com nome e tipo; registre-o.
- Tentativa opcional: `WGPU_BACKEND=metal cargo run -p chess-studio` (Metal é o único backend nativo do macOS).
- Pinça e dois dedos (`PinchGesture`/`PanGesture`) só são emitidos pelo winit em macOS físico; CAM-02 fica UNVERIFIED até o Mac real.
- Critério de sucesso na VM: `cargo build` e `cargo test --workspace` sem falhas. Janela aberta é bônus.

## O que anotar no relatório

Copie para os campos `PENDENTE (usuário)` de `docs/relatorios/gate-a.md`:

- Versão do macOS (`sw_vers`), arquitetura (`uname -m`), CPU, RAM, hipervisor (QEMU, versão).
- `rustc --version` e a versão das Command Line Tools (`xcode-select -p`).
- `cargo build`: ok ou erro (as 20 últimas linhas se falhar).
- `cargo test --workspace`: total, aprovados, falhas (anexe `test-macos.log`).
- A janela abriu? Se não, a mensagem de erro literal e o resultado com `WGPU_BACKEND=metal`.
