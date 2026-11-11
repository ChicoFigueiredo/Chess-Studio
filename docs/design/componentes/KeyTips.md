# KeyTips

Com Alt pressionado, aparecem letras sobre as abas; depois de escolher uma aba, sobre as Ações dela. Assim, todo o Ribbon se opera pelo teclado, como no Office.

- Selo `ink` sobre `surface-1` invertido, em `eval` (12/16 negrito). Nas abas fica logo abaixo do rótulo; nos botões grandes, sobre o ícone. O selo nunca cobre o rótulo.
- As letras são estáveis (definidas na Ação, nunca geradas pela posição) e únicas dentro de cada aba. Esc volta um nível.
- **egui:** uma camada `egui::Area` em `Order::Tooltip` desenhando os selos sobre os `Rect` dos itens, que o Ribbon registra a cada quadro.
