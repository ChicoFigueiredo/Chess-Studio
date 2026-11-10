# StatusBar

A barra de status mostra o estado do sistema: o Perfil de interface ativo, a Engine (consentida ou não, profundidade), de quem é a vez e o número do lance. À direita ficam avisos, FPS (opcional) e a Escala.

- Texto `caption` em `ink-muted`. Estado com ponto colorido **e** palavra (`success`, `warning`).
- Clicar no Perfil abre o seletor; clicar na Engine abre o Painel Análise.
- **egui:** `TopBottomPanel::bottom` com altura `statusbar-h`.
