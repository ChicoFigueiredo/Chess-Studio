# DockGuide

Ao arrastar uma aba, o guia mostra onde o Painel vai encaixar: acima, abaixo, ao lado ou como aba (centro). A área de destino fica em `accent-subtle`, com contorno tracejado `accent`.

- Soltar fora de um guia, dentro da janela, deixa o Painel flutuante.
- Arrastar um Painel flutuante para fora da janela principal cria um Painel destacado (janela do SO), que nasce com o Painel e acompanha o mouse; serve para levar Painéis a outro monitor.
- Encaixar arrastando de uma janela do SO para outra não existe (ADR-0005): use a Ação Encaixar.
- **egui:** os guias são desenhados por cima do `egui_tiles::Tree` durante o arrasto (`Behavior::paint_on_top_of_tile`).

## Arrastar um Painel flutuante de volta ao dock

- Arrastar o cabeçalho de um Painel flutuante até a região do dock mostra os mesmos guias: o centro vira aba do grupo, a borda do grupo divide, fora do dock continua flutuante na posição solta (com o tamanho que tinha).
- Com o dock vazio (todos os Painéis flutuantes), a borda direita vira uma zona de soltura com o rótulo "Solte aqui para encaixar"; soltar nela recria o dock.
- **egui:** ao entrar no dock o Painel volta à árvore e o arrasto passa ao `egui_tiles` (`Context::set_dragged_id`); o egui não aborta o arrasto quando o widget some.
