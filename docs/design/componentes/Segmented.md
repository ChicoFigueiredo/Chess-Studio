# Segmented

O controle segmentado troca a vista de um Painel entre 2 a 4 opções curtas: Aberturas | Finais no Explorador, Brancas | Pretas na fonte "Um jogador".

- Um campo `surface-2` com contorno `border-strong` e `radius-m`, dividido em partes iguais. A opção escolhida fica em `accent-subtle`, contorno `accent` e texto `accent-ink`; as outras, sem fundo, em `ink`, com `surface-3` no hover e `surface-4` pressionada.
- Rótulos em `body-strong`, sem ícone; altura `control-h`. Para mais de 4 opções ou rótulos longos, use o Dropdown.
- Teclado: o foco passa por cada segmento (anel de 2px em `focus-ring`); Espaço e Enter escolhem.
- **egui:** `cs_segmented(ui, &mut valor, &[(valor, "Rótulo".into())])`; a resposta marca `changed` quando a escolha muda.
