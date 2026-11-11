# Aba e fonte do Explorador: do Perfil local, não do Perfil de interface (#239)

**Pergunta.** A aba (Aberturas ou Finais) e a fonte (Biblioteca, Mestres, Lichess, Um jogador) do Painel Explorador devem ser lembradas dentro do Perfil de interface?

**Decisão.** Não. Continuam nas preferências do **Perfil local** (`ui_preferences`, chave `explorer.panel`, em `crates/chess-studio/src/explorer/prefs.rs`).

**Por quê.**

- O `GLOSSARY.md` define o Perfil de interface como o que muda a *aparência e a disposição*: Apresentação, layout do dock, Painéis flutuantes e destacados, estado do Ribbon, Perfil de auxílios, modo Foco e Vista da câmera. Aba e fonte do Explorador não estão nessa lista.
- Trocar de Perfil de interface (por exemplo, de "Jogo físico" para "Estudo e análise") "nunca altera a Partida nem o Tema", e não deve mexer no que a pessoa está investigando. Quem consulta "Um jogador: ana_exemplo" não espera perder a consulta ao voltar para "Jogo físico".
- A fonte e o jogador consultado dizem respeito a *quem* está estudando (um dado da pessoa, como os ritmos e as faixas de rating), e o glossário já põe as preferências da pessoa no Perfil local.
- O que *é* do Perfil de interface continua sendo dele: se o Painel Explorador está visível, onde está e com que largura.

**Consequências.** Nenhuma mudança de código. Se um dia o glossário passar a incluir "aba e fonte do Explorador" no Perfil de interface, o caminho é mover a chave `explorer.panel` para o registro do Perfil de interface e migrar o valor existente na primeira leitura.
