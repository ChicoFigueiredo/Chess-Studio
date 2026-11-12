# Treino: trocar a Partida do tabuleiro sai do Treino (#307)

## Problema

Com um Exercício no tabuleiro, o Treino guarda a Sessão do usuário (`TrainingUi.stashed`) e a persistência não grava a Sessão do Exercício. Nova Partida, Retomar, Biblioteca, FEN e a Partida online que começa continuavam possíveis e trocavam a Sessão do Exercício por outra. Ao sair, a Partida guardada voltava por cima da recém-aberta, que se perdia.

Havia ainda um defeito de base: o Exercício e a Partida do usuário começavam na mesma época (0). A troca por outra Partida dá `época do Exercício + 1`, que podia coincidir com a época que a persistência acompanha; ela não notava a troca e gravava os Eventos da Partida nova no log da guardada.

## Regra (uma só)

**Trocar a Partida do tabuleiro durante o Treino sai do Treino**, como no modo espectador (#302). Nenhuma Ação fica desabilitada: a Partida recém-aberta é a do usuário e ocupa o tabuleiro; a Resolução em andamento é abandonada (o registro dela já existe e não é concluído).

- A Partida guardada **não volta** por cima da nova e **não é gravada de novo**: ela já está no banco, onde estava, e continua retomável (Retomar e Biblioteca).
- A detecção é única, para todos os caminhos (inclusive uma Partida online que começa sozinha): a `Session` do Exercício tem uma época própria, diferente da do usuário (`Session::cover`); se a época do tabuleiro deixa de ser a dela, outra Ação trocou a Partida (`TrainingUi::leave_if_board_replaced`, sistema `training_guard` antes de `persist_session`).
- Não muda a regra de fair play: a Partida competitiva guardada passa a ser interrompida como em Nova Partida/Retomar sem Treino (pendente em #305).

## Por que não desabilitar

A Partida online pode começar sem Ação do usuário (um desafio aceito, uma busca que casou). Desabilitar Ações não cobriria esse caminho e deixaria a regra em oito lugares; a detecção da troca cobre todos de uma vez.

## Evidência

`crates/chess-studio/src/training_ui/swap_tests.rs`: Nova Partida, Retomar/Biblioteca e Partida online durante o Exercício; nenhum Evento duplicado, nenhuma Partida some, a guardada continua igual no banco e retomável. Homologação: CX-HOM-82.
