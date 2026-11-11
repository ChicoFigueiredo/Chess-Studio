# SecretField (campo de segredo)

O campo de segredo recebe o que ninguém deve ver nem copiar: a senha-mestra do Cofre e a chave pessoal de um Provedor. É um `TextField.md` com garantias a mais.

- **Mascarado.** Cada caractere aparece como ponto. Não existe botão "mostrar".
- **Sem área de transferência.** O `egui` não deixa copiar nem recortar o texto de um campo mascarado, e o widget nunca escreve na área de transferência. Colar é permitido, porque a chave nasce em outro lugar.
- **Sem rastro.** Quem consome o segredo o move para o Cofre e chama `forget_secret_field`: o `egui` guarda cursor e histórico de desfazer com cópias do texto digitado, e a chamada apaga ambos. O segredo nunca entra em log, em `Debug` nem em mensagem de erro.
- Aparência do `TextField.md`: `surface-2`, anel `focus-ring`, contorno `accent` no foco. Erro em `danger`, de 2px, com a mensagem logo abaixo e o ícone de aviso. A mensagem diz o que fazer ("Isto não parece uma chave pessoal do Lichess. Confira se copiou a chave inteira, sem espaços."), sem repetir o que foi digitado.
- O placeholder diz o que colar ou digitar ("Cole a chave aqui", "Pelo menos 8 caracteres"), nunca um exemplo de segredo.
- O rótulo vem acima, em `body`: "Senha-mestra", "Repita a senha-mestra", "Chave". Uma nota logo abaixo diz para onde o segredo vai ("A chave fica mascarada, vai direto para o cofre e só é guardada depois que o Lichess a confirmar.").
- Ao criar uma senha, o botão que confirma só vale depois de marcar "Entendi que não há como recuperar a senha". O trabalho pesado de cifrar roda fora da interface; a janela mostra "Protegendo o cofre com a senha-mestra… isso leva alguns segundos." e não congela.
- Nunca guarde o valor em preferência nem em estado de Painel.
- **egui:** `cs_secret_field(ui, id, &mut texto, dica, erro)` e `forget_secret_field(ctx, id)` em `ui_theme`. Usado em `connections/view_auth.rs`.
