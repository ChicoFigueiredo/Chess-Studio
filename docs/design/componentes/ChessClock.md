# ChessClock

O relógio mostra cada lado com nome e tempo em `clock` (números tabulares).

- O lado que está jogando ganha o contorno `accent` e um traço à esquerda. Tempo baixo fica em `warning`, sempre com o valor visível.
- A marca do lado (`crown` ou `horse`) fica na mesma linha do nome. Os dígitos são tabulares: a largura não muda de um segundo para o outro.
- **egui:** `RichText` com `FontId` de 32px e `TextStyle` próprio `Clock`.
