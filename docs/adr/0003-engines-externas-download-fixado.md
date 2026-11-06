# Engines são processos externos UCI, nunca embutidos; download fixado com SHA-256

O Chess Studio não embute nem redistribui executáveis de Engine. Ele fala UCI com processos filhos isolados (timeouts, filas, falha não derruba a interface) e valida todo lance proposto pelo núcleo de regras. No primeiro uso, um assistente encontra uma Engine instalada ou, com consentimento explícito, **baixa o Stockfish do site oficial numa versão fixada no código, verificando o SHA-256 embutido** para o SO e a CPU detectados, mostrando a licença GPL e o link da fonte antes.

A alternativa de sempre baixar a última release via API do GitHub foi rejeitada: sem hash conhecido de antemão, a integridade dependeria só do HTTPS. Embutir o Stockfish no instalador foi rejeitado porque misturaria a distribuição de um binário GPL com o app MIT/Apache (ver ADR-0004).

## Consequências

- Atualizar a versão do Stockfish no P0 exige uma nova release do app; o gerenciador livre de Engines é da V1.
- O app usa a rede apenas nessa ação explícita; jogar continua 100% offline.
