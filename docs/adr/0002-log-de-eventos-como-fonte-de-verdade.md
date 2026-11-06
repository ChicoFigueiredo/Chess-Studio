# Log de eventos no SQLite é a fonte de verdade da Partida

Cada Partida é um **log append-only de eventos** (Toque intencional, Ajuste, Lance feito, Lance concluído, acionamento do relógio, Infração, penalidade, Reclamação de empate, Pausa, desfazer, fim e motivo), com tipo e payload versionados, gravado no SQLite em transação **no momento em que acontece**. Lances, posições e relógios são projeções reconstruídas do log; FEN e PGN são só formatos de interoperabilidade, nunca uma segunda verdade. Não existe botão "Salvar": fechar e reabrir retoma a Partida (pausada) do último evento gravado.

A alternativa era gravar o estado atual por autosave periódico e manter eventos só para auditoria. Rejeitamos porque Infrações, desfazer registrado e reconstrução de relógio precisam de histórico exato (AC-07, DB-01, DB-05), e porque um autosave periódico abre janela para perda de lances numa queda.

## Consequências

- Mudar o formato de um evento exige versão de payload e migração; eventos antigos continuam legíveis.
- O tempo do relógio é derivado dos instantes gravados nos eventos, não de um contador salvo.
