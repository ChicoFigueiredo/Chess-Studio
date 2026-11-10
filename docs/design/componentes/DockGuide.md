# DockGuide

Ao arrastar uma aba, o guia mostra onde o Painel vai encaixar: acima, abaixo, ao lado ou como aba (centro). A área de destino fica em `accent-subtle`, com contorno tracejado `accent`.

- Soltar fora de um guia deixa o Painel flutuante.
- Arrastar para outra janela do SO não existe (ADR-0005): use Destacar e Encaixar.
- **egui:** os guias são desenhados por cima do `egui_tiles::Tree` durante o arrasto (`Behavior::paint_on_top_of_tile`).
