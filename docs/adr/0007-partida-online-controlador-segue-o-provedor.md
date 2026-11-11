# Na Partida online o controlador só registra o que o Provedor confirma; trava de fair play no serviço

Numa **Partida online** o Provedor decide Lances, relógio e resultado. Ela continua sendo uma Partida do domínio, com log local (ADR-0002), e o controlador continua sendo o único que a escreve (ADR-0001). O que muda é a origem dos comandos: o Lance do jogador local é uma **intenção enviada** e só vira Lance feito quando o Provedor confirma; os Lances do adversário, o relógio e o resultado entram como eventos vindos do Provedor. Sem confirmação, o app consulta o estado remoto antes de reenviar; nunca repete um envio às cegas e nunca deixa um evento fora de ordem regredir a Partida. As Políticas de arbitragem locais (toque, Infrações) não se aplicam: valem as regras do Provedor.

Enquanto uma Partida online contra uma pessoa está em andamento (inclusive por correspondência), a **trava de fair play** bloqueia, para aquela Partida, tudo o que calcula: Engine local e remota, avaliação em cache, finais exatos, Explorador de aberturas e análise. O bloqueio é da camada de serviço, não da interface: esconder botão não basta, e não existe chave de depuração que o contorne. Continuam os Auxílios que só mostram regras (destinos legais, último Lance, coordenadas). A análise só é liberada quando o Provedor confirma o fim da Partida. Uma Engine só joga online por uma Conexão de Engine própria, separada da Conexão do jogador.

Alternativas rejeitadas: um espelho do estado remoto fora do domínio, convertido em Partida só no fim (duplicaria tabuleiro, relógio e Notação); registrar o Lance na hora e desfazer se o Provedor recusar (criaria estado local que nunca existiu no servidor); liberar o Explorador em correspondência, como as regras do Lichess permitem (uma exceção a mais para errar; pode ser revista).

## Consequências

- O controlador ganha uma fonte de comandos remota; o tempo da Partida online vem do Provedor, nunca do relógio local.
- Reconexão: reabrir o fluxo, buscar o estado atual, descartar eventos repetidos e reconciliar o relógio.
- Todo serviço que calcula (Engines, Explorador, finais, avaliação em cache) consulta a trava antes de responder; há testes negativos de que ela não é contornável.
