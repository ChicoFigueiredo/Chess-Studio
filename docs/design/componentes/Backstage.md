# Backstage

Os bastidores, aberto pela aba Arquivo, ocupa a janela inteira: Nova, Abrir PGN, Biblioteca, Perfis, Perfil local e Opções.

- Navegação à esquerda em `accent`/`on-accent`; o conteúdo em `surface-1`, com título `display`.
- "Voltar" e Esc fecham. Nada nos bastidores altera a Partida sem uma Ação explícita (ADR-0001).
- **egui:** `CentralPanel` sobre tudo, com `SidePanel::left` pintado de `accent`.
