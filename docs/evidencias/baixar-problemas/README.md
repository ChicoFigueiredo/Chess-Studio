# Evidência: baixar problemas do Lichess em Treino > Pacotes (#238)

Capturas com dados fictícios (nenhuma chamada de rede; o Lichess é ligado ou desligado só na rede em memória da captura). Em Escuro (`escuro/`) e Claro (`claro/`).

```
chess-studio --screenshot <pasta> --single <cena> --size 1600x1000 --panel \
  --dock-demo training-float --training <cena> --theme dark|light
```

| Arquivo | Cena | O que mostra |
|---|---|---|
| `download.png` | `download` | Formulário: temas em português por grupo (Motivos táticos e Mate em quantos lances abertos), dificuldade e quantidade. |
| `download-running.png` | `download-running` | Andamento: 311 de 500 encontrados, lidos e baixados, aviso de que cancelar não instala nada, "Cancelar". |
| `download-done.png` | `download-done` | Pacote instalado com a Origem (Lichess, CC0, data) e a lista de pacotes com a data. |
| `download-error.png` | `download-error` | Falha de conexão explicada em linguagem de usuário. |
| `download-consent.png` | `download-consent` | Provedor desligado: o diálogo "Ligar Lichess?" abre ao pedir o download. |
| `download-list.png` | `download-list` | Lista de Exercícios com temas traduzidos e o filtro de tema por nome. |
