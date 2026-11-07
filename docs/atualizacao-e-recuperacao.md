# Atualização e recuperação

Como atualizar o Chess Studio sem perder dados, fazer backup e restaurar, e o que fazer se o banco ou as Engines estragarem. Os caminhos abaixo vêm de `crates/chess-studio/src/paths.rs` e `crates/chess-uci/src/detect.rs`.

## Onde ficam os dados

Tudo do usuário está numa única pasta, `ChessStudio`, separada do programa instalado:

| Sistema | Pasta de dados |
|---|---|
| Linux | `$XDG_DATA_HOME/ChessStudio` (se for caminho absoluto), senão `~/.local/share/ChessStudio` |
| Windows | `%APPDATA%\ChessStudio` (ex.: `C:\Users\<você>\AppData\Roaming\ChessStudio`) |
| macOS | `~/Library/Application Support/ChessStudio` |

Conteúdo:

- `chess-studio.db` — o banco SQLite (Partidas, log de Eventos, perfis, preferências, biblioteca, Exercícios, registro de Engines). Junto dele podem aparecer `chess-studio.db-wal` e `chess-studio.db-shm` (veja WAL abaixo).
- `engines/` — Engines baixadas pelo assistente/gerenciador (`stockfish-<versão>/`, `networks/` do Lc0, `preferred.txt`). Pode ser apagada e baixada de novo.

PGN exportado vai para a pasta Documentos do usuário, fora desta pasta.

Desinstalar o programa não apaga a pasta de dados (ver `docs/instalacao.md`). `--db <arquivo>` troca o caminho do banco (uso avançado e testes).

## Atualizar sem perder dados

1. Feche o Chess Studio (isso fecha o banco de forma limpa; não existe "Salvar", a Partida é reconstruída do log de Eventos, ADR-0002).
2. Instale a nova versão por cima (instalador, ou substitua o executável e a pasta `assets/`). A pasta de dados não é tocada.
3. Abra o app. O banco é migrado sozinho para o esquema novo (`PRAGMA user_version`; cada migração roda na própria transação, então uma migração que falha não deixa o banco pela metade). A Partida em andamento é retomada.

Regras de compatibilidade:

- **Atualizar** (versão nova abre banco antigo): sempre suportado, migrações automáticas e em ordem.
- **Voltar a uma versão antiga** com banco já migrado: **não suportado**. A versão antiga recusa o banco com a mensagem "banco na versão de esquema N, mas este app só entende até M; atualize o Chess Studio" e segue sem persistência (nada é gravado nem apagado). Para voltar, restaure um backup feito antes da atualização.
- Por precaução, **faça um backup antes de atualizar** (próxima seção). É barato e é a única forma de voltar atrás.

## Backup e restauração

O banco usa WAL (`journal_mode=WAL`, `synchronous=FULL`): um Evento confirmado já está em disco, mas parte dos dados recentes pode estar em `chess-studio.db-wal` e ainda não no `.db`. Por isso:

**Backup (app fechado, forma mais simples):** copie a pasta `ChessStudio` inteira, ou pelo menos `chess-studio.db` **e** os arquivos `-wal`/`-shm` se existirem. Copiar só o `.db` com o app aberto pode perder os últimos Eventos.

Exemplos (app fechado):

```bash
# Linux
cp -a ~/.local/share/ChessStudio ~/ChessStudio-backup-$(date +%F)
# macOS
cp -a ~/Library/Application\ Support/ChessStudio ~/ChessStudio-backup-$(date +%F)
```

```powershell
# Windows (PowerShell)
Copy-Item "$env:APPDATA\ChessStudio" "$HOME\ChessStudio-backup-$(Get-Date -Format yyyy-MM-dd)" -Recurse
```

**Backup com o app aberto (opcional, requer `sqlite3`):** `sqlite3 chess-studio.db ".backup 'copia.db'"` gera uma cópia consistente, incluindo o que está no WAL.

**Restauração:** feche o app, apague (ou renomeie) a pasta `ChessStudio` atual e copie o backup de volta com o mesmo nome. Ao restaurar um banco de uma versão **mais antiga**, a versão atual do app o migra ao abrir. Apague `chess-studio.db-wal` e `-shm` que sobrarem da pasta anterior, para não misturar com o banco restaurado.

Para guardar só as Partidas em formato aberto, use a exportação de PGN do app; ela não substitui o backup do banco (não leva perfis, preferências nem o log completo).

## Banco corrompido ou que não abre

O WAL com `synchronous=FULL` torna corrupção por queda de energia ou fechamento forçado muito improvável: ao reabrir, o SQLite recupera o WAL sozinho. Se mesmo assim o banco não abrir, o app **não trava**: registra um aviso ("banco ... indisponível, o app segue sem persistência") e abre sem gravar nada. Então:

1. **Não apague nada ainda.** Feche o app e faça uma cópia de toda a pasta `ChessStudio`.
2. Se for só a mensagem de esquema novo demais, instale a versão atual ou mais recente do app (veja "Voltar a uma versão antiga").
3. Verifique o arquivo (requer `sqlite3`): `sqlite3 chess-studio.db "PRAGMA integrity_check;"`. Resposta `ok` significa banco íntegro (o problema é outro: permissão de pasta, disco cheio, arquivo aberto por outro processo).
4. Se não for `ok`, tente recuperar o que for legível: `sqlite3 chess-studio.db ".recover" | sqlite3 recuperado.db`. Depois substitua `chess-studio.db` por `recuperado.db` (com o app fechado, sem os arquivos `-wal`/`-shm` antigos) e abra o app, que aplica as migrações que faltarem.
5. Se a recuperação não servir, restaure o último backup.
6. Último recurso (perde Partidas, perfis e preferências): renomeie `chess-studio.db` (e `-wal`/`-shm`) para `chess-studio.db.corrompido` e abra o app; ele cria um banco novo. Guarde o arquivo renomeado, ele ainda pode ser lido por ferramentas SQLite.

## Reinstalar Engines

O Chess Studio não distribui Engines (ADR-0003). Stockfish e Lc0 (GPL) são só baixados por você, depois de aceitar a licença e o tamanho no assistente, para `ChessStudio/engines/`.

- **Baixar de novo:** no gerenciador de Engines (painel Engines) use *Atualizar* ou *Remover* e instale outra vez, ou abra o assistente de Engine (o botão de download pede o consentimento). O download é verificado por SHA-256; falha de rede, de hash ou de disco deixa a instalação anterior intacta.
- **Reinstalação limpa:** feche o app, apague `ChessStudio/engines/` (ou só a pasta da Engine, ex. `stockfish-<versão>/`) e abra o app; o gerenciador mostra a Engine como ausente e o assistente oferece o download. Pesos de rede do Lc0 ficam em `engines/networks/`.
- **Engine instalada por você** (gerenciador de pacotes, site oficial): continua valendo; o app a encontra no `PATH`, ou use `CHESS_STUDIO_ENGINE=/caminho/do/executavel`. Remover pelo app nunca apaga Engines que não foram baixadas por ele.
- Sem Engine, o app continua funcionando (Assistido, dois jogadores, análise manual e estudo).

Perder a pasta `engines/` não afeta Partidas nem perfis: elas ficam só no banco.
