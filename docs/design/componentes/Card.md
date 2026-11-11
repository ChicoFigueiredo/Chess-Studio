# Card (cartão com detalhes sob demanda)

O cartão é a superfície de uma página de bastidores que representa uma coisa com estado e ações, como um Provedor na Central de Conexões. (No glossário do projeto o termo de interface é "cartão", não "card".)

- **Resumido por padrão:** nome, selo de estado (`StatusBadge.md`), uma frase sobre o que se pode fazer agora, o Switch e **uma** ação principal. O resto vai no menu "⋯" (`OverflowMenu.md`).
- **Detalhes sob demanda:** o botão `Subtle` "Detalhes" com seta abre linhas rótulo-valor (rótulo `caption` em `ink-muted`, 148 de largura; abaixo de 340 o rótulo vai acima do valor). Só o que a pessoa raramente precisa: o que é enviado, como se identifica, custo por serviço, o que vem depois, última consulta, instruções e termos.
- **Completo:** `surface-2`, contorno `border-strong`, `radius-l`, margem `space-4` (16). **Discreto** (`CardEmphasis::Quiet`): sem fundo, contorno `border`, margem `space-3` (12). Serve ao que ainda não funciona (sem Switch nem detalhes), às Conexões listadas dentro de um cartão e à faixa do Cofre no topo da Central.
- Cartões de uma faixa têm a mesma altura quando resumidos (a frase reserva a linha), para os rodapés se alinharem. Duas colunas a partir de 980 pt; abaixo, uma.
- A ação principal é `Secondary`: o `Primary` (âmbar) é um por tela ou diálogo.
- Dentro do cartão, uma lista (como as Conexões vinculadas) usa cartões discretos, um por linha.
- **egui:** `cs_card_frame(p, CardEmphasis)`, `cs_disclosure`, `cs_detail_row`.
