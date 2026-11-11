# OverflowMenu (menu "⋯")

O menu "⋯" guarda as Ações secundárias de um cartão ou de uma linha, para que o resto da superfície mostre **uma** ação principal.

- O gatilho é um botão `Subtle` só de ícone (`dots-three-outline`), com tooltip "Mais ações" (num cartão) ou "Ações da Conexão" (numa linha de Conexão). Abre com clique, Enter ou Espaço e fecha com Esc ou ao escolher um item.
- Cada item é um verbo (`Button.md`), com ícone de 16 à esquerda; altura mínima 24, raio `radius-m`, hover `surface-3`, pressionado `surface-4`. O menu usa `radius-l` e `shadow-flyout`, largura mínima de 220.
- O que não está disponível agora **continua na lista**, desabilitado em `ink-disabled`, e o tooltip diz o motivo em linguagem de usuário ("Em breve: guardar uma chave pessoal neste computador, com segurança.").
- Ação destrutiva (Remover, Desconectar…) em `danger`, no fim da lista, e sempre confirma depois.
- Sem submenus e sem separadores: se a lista passa de seis itens, o item principal está errado.
- **egui:** `cs_overflow_menu(ui, id, tooltip, &[MenuItem])` devolve o índice escolhido.
