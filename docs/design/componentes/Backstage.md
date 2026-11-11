# Backstage

Os bastidores, aberto pela aba Arquivo, ocupa a janela inteira. A navegação tem oito entradas, nesta ordem: Nova, Abrir PGN, Biblioteca, Perfis, Perfil local, Conexões, Controles e Opções. Perfis, Perfil local e Conexões abrem uma página no corpo; as outras executam uma Ação.

- Navegação à esquerda em `accent`/`on-accent`; o conteúdo em `surface-1`, com título `display`.
- As escolhas de uma página (tipo de Partida) são cartões com contorno `border-strong`; o escolhido fica em `accent-subtle` com contorno `accent`.
- "Voltar" e Esc fecham. Nada nos bastidores altera a Partida sem uma Ação explícita (ADR-0001).
- A página **Conexões** (a Central de Conexões) é longa e rola inteira: título, Perfil local ativo, busca e filtros, a faixa do Cofre (um cartão discreto de `Card.md`, com as ações do Cofre à direita), as seções de Provedores e as Conexões deste Perfil local. Abaixo de 980 pt usa uma coluna.
- **egui:** `CentralPanel` sobre tudo, com `SidePanel::left` pintado de `accent`.
