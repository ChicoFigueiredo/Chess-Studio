## Treino: lista de Exercícios, Resolução, revisão e métricas.

training-window-title = Treino
training-open = Abrir Treino
training-close = Sair do Treino
training-note-no-effect = O treino não altera o resultado de nenhuma Partida.

training-list-heading = Exercícios
training-start = Resolver
training-exercise-count = acertos: { $correct } | resoluções: { $runs }
training-kind-stalemate = Afogamento
training-kind-mate = Xeque-mate
training-kind-check = Xeque
training-kind-illegal_pawn = Peão ilegal
training-kind-castling = Roque
training-kind-promotion = Promoção
training-kind-memory = Memória de sequência

training-objective = Objetivo: { $text }
training-side-white = Você joga com as Brancas.
training-side-black = Você joga com as Pretas.
training-errors-count = Erros nesta resolução: { $count }
training-error-brief = Não foi dessa vez: a posição foi restaurada.
training-error-illegal = Lance ilegal ({ $from }-{ $to }): { $reason }
training-error-off-objective = O Lance { $uci } é legal, mas não cumpre o objetivo. A posição foi restaurada.
training-feedback-hidden = O feedback aparece na revisão.
training-hint-undo = Desfazer continua permitido.
training-give-up = Desistir e ver a revisão
training-back = Voltar à lista

training-review-heading = Revisão
training-result-correct = Resolvido!
training-result-incorrect = Não resolvido
training-review-outcome = Resultado da posição final: { $outcome }
training-review-explanation = Explicação: { $text }
training-review-solution = Solução: { $san }
training-not-mate-heading = Por que não é xeque-mate
training-not-mate-no-check = O rei não está em xeque e, mesmo assim, não tem nenhum Lance legal: isso é afogamento e a Partida empata.
training-not-mate-rule = No xeque-mate o rei está em xeque e sem saída; no afogamento ele não está em xeque, mas não há Lance legal. Xeque-mate decide a Partida; afogamento empata.
training-review-errors = Erros ({ $count }):
training-review-no-errors = Nenhum erro.
training-review-error-line = { $n }. { $text }
training-review-aids-used = Auxílios usados: { $list }
training-review-aids-none = Nenhum Auxílio usado.
training-review-aids-off = Desligados: { $list }
training-review-feedback = Explicação de erro: { $mode }
training-review-duration = Tempo: { $seconds } s
training-repeat = Repetir (posição original)

training-metrics-heading = Métricas
training-metrics-total = Resoluções: { $runs } | acertos: { $correct } | erros: { $errors } | acertos sem Auxílios: { $noaids }
training-metrics-by-kind = Por tipo
training-metrics-by-session = Por sessão
training-metrics-line = { $name }: { $runs } resoluções, { $correct } acertos, { $errors } erros, { $noaids } sem Auxílios
training-metrics-session-current = Sessão atual
training-metrics-session-previous = Sessão anterior { $n }
training-metrics-empty = Nenhuma Resolução ainda.

training-memory-present-start = Memorize a sequência: posição inicial.
training-memory-present = Memorize: Lance { $n } de { $total }
training-memory-present-hint = O tabuleiro mostra a sequência sozinho; depois ele volta ao início e você a reproduz.
training-memory-recall = Reprodução: Lance { $n } de { $total }
training-memory-turn-white = Vez das Brancas.
training-memory-turn-black = Vez das Pretas.
training-memory-errors = Lances errados: { $count }
training-memory-show-again = Mostrar de novo
training-memory-review-heading = Memória de sequência
training-memory-review-hits = Acertos por Lance (na primeira tentativa): { $hits } de { $total }
training-memory-review-first-error = Primeiro erro: Lance { $n }.
training-memory-review-no-error = Nenhum erro na reprodução.
training-memory-review-replays = Você pediu para ver a sequência de novo { $count } vez(es).
training-memory-ply-line = { $n }. { $san }: { $result }
training-memory-ply-hit = acertou
training-memory-ply-miss = errou
training-memory-ply-pending = não jogado
training-metrics-memory = Memória: { $hits } de { $plies } Lances certos na primeira tentativa
