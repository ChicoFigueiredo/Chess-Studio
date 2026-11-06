# Licença MIT OR Apache-2.0 e veto a dependências GPL

O código do Chess Studio é público sob **MIT OR Apache-2.0**, o padrão do ecossistema Rust/Bevy. Por isso nenhuma dependência copyleft (GPL/AGPL/LGPL estática) entra no binário. A consequência mais visível: o núcleo de regras usa **cozy-chess** (MIT) para gerar lances e escreve SAN, PGN, repetição e regras de empate no próprio projeto, **em vez de usar shakmaty/pgn-reader**, que já fazem isso mas são GPL-3.0.

O app é desktop nativo em Rust com Bevy/wgpu/egui e SQLite embarcado, sem servidor; a alternativa web/Electron foi descartada no prompt mestre (DEC-01, DEC-02) por exigir navegador e por desempenho gráfico.

## Consequências

- Toda dependência nova passa por verificação de licença (cargo-deny no Gate A).
- Engines GPL como o Stockfish são aceitáveis só como processos externos que o usuário obtém (ADR-0003).
- Assets 3D e texturas precisam ser CC0 ou compatíveis, com licença registrada.
