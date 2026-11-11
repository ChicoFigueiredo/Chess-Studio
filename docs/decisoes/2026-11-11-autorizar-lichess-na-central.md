# Decisões: autorizar o Lichess pela Central (#231, 11/11/2026)

Etapa 3 do épico #211 (interface). Junta o Cofre e a autorização (#224), a Central (#214, #225) e o Explorador (#226). Valem a [ADR-0006](../adr/0006-rede-offline-por-padrao-provedores-e-conexoes.md), as decisões [CX-09, CX-10 e CX-12](2026-11-11-conexoes.md) e [CF-01 a CF-12](2026-11-11-cofre-e-autorizacao.md).

| # | Decisão | Escolha |
|---|---|---|
| AZ-01 | Onde mora o Cofre | O `NetService` abre o `SecretVault` ao lado do banco (`cofre/`), o passa à rede (`NetConfig::vault`) e lê os segredos numa thread de fundo (`preload`). Enquanto lê, nenhuma Conexão vale como autorizada: o gate nunca espera o sistema na thread da interface. |
| AZ-02 | Cofre falso | Capturas, benchmark e testes usam um cofre do sistema **falso** em memória e um diretório temporário (`secrets::volatile_vault`). O cofre real só é aberto com o banco em disco. |
| AZ-03 | Senha-mestra | Só quando um segredo é necessário (CX-10) e só sem cofre do sistema: diálogo de criar (confirmação, aviso de que não há recuperação e confirmação de entendimento), desbloquear e trocar. O Argon2id roda em fundo; os campos são mascarados, zerados ao descartar e o `egui` esquece o histórico de desfazer deles. Nunca em log, `Debug` ou área de transferência. |
| AZ-04 | Reserva da Conexão | O `id` da Conexão é a chave do segredo, então a linha é **reservada** antes do navegador (`begin_authorization`) e concluída (`record_authorization`) só quando o Lichess confirma. Reserva esquecida (app fechado no meio) é descartada, com o segredo, no quadro seguinte. |
| AZ-05 | Nome já vinculado | Autorizar um nome que o Perfil local já vinculava **absorve** a linha "só o nome" (o apelido passa para a autorizada), em vez de duplicar. |
| AZ-06 | Ciclo de vida | O segredo é apagado no Cofre **antes** da Conexão ou do Perfil local; se o Cofre recusa, nada é removido. A migração 16 refaz `connections` com `AUTOINCREMENT`: o `id` apagado nunca volta. |
| AZ-07 | Desconectar | `revoke` (apaga o segredo local sempre), remove a Conexão e diz se o Lichess não foi avisado, com onde revogar à mão. |
| AZ-08 | Autorização recusada | `401`/`403` marca a referência; o cartão mostra "Autorização expirada" com "Autorizar de novo", que renova a mesma Conexão (mesmo `id`); os dados locais ficam. Cofre próprio bloqueado: "Cofre bloqueado". |
| AZ-09 | Jogador e Engine online | Entradas separadas: outra Conexão, outra permissão mínima, outro segredo, outra seção. Nunca compartilham autorização. |
| AZ-10 | Navegador | A página de autorização só abre se passar por uma regra explícita (`https://lichess.org/oauth?`, retorno em `127.0.0.1`, desafio S256, uma só permissão de jogo). |
| AZ-11 | Explorador | Os filtros de ritmo (Lichess e Um jogador) e de faixa de rating (Lichess) entram no pedido; a aba, a fonte, o jogador, a cor e os filtros são lembrados por Perfil local (preferência `explorer.panel`). |
| AZ-12 | Catálogo | Os serviços que já têm adaptador (Explorador, avaliações prontas, finais, consultas públicas do Lichess; consultas do Chess.com) passam a `Ready`, e o cartão lista o que já funciona. |
