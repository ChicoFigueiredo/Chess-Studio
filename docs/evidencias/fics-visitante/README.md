# Evidência: FICS como visitante na interface (#254)

Capturas nos Temas Escuro (`escuro/`) e Claro (`claro/`), geradas pelo modo `--screenshot` do app. As cenas usam dados **fictícios** (o visitante `GuestDEMO`, apelidos inventados, a Partida 142): nenhuma conexão ao FICS, nenhum banco.

## Tela da Partida online, Provedor FICS (`--online-tour`)

- `fics-off.png`: FICS desligado. "O FICS está desligado", o aviso honesto (conexão sem criptografia, só visitante e nunca senha, sem reconexão, sem timeseal) e **Ligar o FICS…** (abre o diálogo de consentimento), **Abrir a Central** e **Fechar**.
- `fics-guest.png`: FICS ligado, ainda sem conexão. "Jogar no FICS como visitante", o mesmo aviso e **Entrar como visitante**. Não há campo de senha.
- `fics-lobby.png`: conectado como `GuestDEMO`, aba **Buscar adversário** (Relâmpago, Blitz e Padrão; visitante só joga sem rating) e **Sair do FICS**.
- `fics-waiting.png`: "Procurando adversário no FICS", 5+3 sem rating, o tempo de espera e **Cancelar busca**.
- `fics-challenges.png`: aba **Desafios (2)**: um desafio em xadrez padrão (Aceitar, Recusar) e um em `wild/fr`, que diz que o app só joga xadrez padrão e não tem Aceitar.
- `fics-observe.png`: aba **Observar** com a Partida 77: tabuleiro pequeno só de leitura, nomes, relógios, Lances vistos e o último Lance, e o aviso de que a observada fica na memória, não vai à Biblioteca e não liga a Trava.

## O Painel Partida online com a Partida de visitante (o mesmo Painel do Lichess)

Nestas cenas o Painel fica no grupo do Relógio (o mais alto), para caber inteiro na captura; o Painel Relógio, ao lado, diz "Relógio do FICS".

- `fics-playing.png`: Partida em andamento, selos **FICS · visitante** e **Ao vivo**, "Enviando Ba4…" (intenção enviada, a peça no destino), Propor empate, Abandonar, "Contra Peao_Passado · Blitz 5+3 · sem rating" e "Trava de fair play ligada… até o fim confirmado pelo FICS".
- `fics-desync.png`: a Partida no app não confere com o tabuleiro do FICS; nada foi mudado e nenhum Lance foi inventado.
- `fics-dropped.png`: a conexão caiu durante a Partida: "No FICS, a Partida de visitante que cai é dada como perdida; o resultado não pôde ser confirmado e o app não reconecta sozinho", a Trava continua até **Descartar a sessão**.
- `fics-ended.png`: fim confirmado pelo FICS, **Analisar**, **Ver na Biblioteca** e "Origem FICS (visitante)".

## Central de Conexões

- `central-fics.png`: o cartão do FICS em **Jogar online**, completo, com o Switch (consentimento por Provedor), "Jogar online." e o aviso; os **Detalhes** abertos mostram "Visitante, sem senha", o que é enviado (texto puro, nenhuma senha) e o custo. Não há campo de senha, chave nem autorização.

## Como gerar de novo

```sh
cargo run -p chess-studio -- --screenshot <dir> --size 1600x900 \
  --online-tour fics-off,fics-guest,fics-lobby,fics-waiting,fics-challenges,fics-observe,fics-playing,fics-desync,fics-dropped,fics-ended
cargo run -p chess-studio -- --screenshot <dir> --single central --size 1920x1080 \
  --backstage-page connections --connections-demo --connections-open fics --theme escuro   # ou --theme claro
```
