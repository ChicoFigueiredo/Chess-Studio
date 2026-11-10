# Interface com Ribbon, lista única de Ações e design system próprio em egui

A interface deixa o painel esquerdo de configurações e passa a ter: **Ribbon** no estilo Office, com modo clássico, modo simples (uma linha) e recolhido; o diálogo **Opções** para preferências persistentes; Barra de Acesso Rápido; KeyTips; Paleta de Ações; e barra de status. Tudo isso é desenhado em **egui** com um design system próprio: tokens de cor, espaçamento, raio e tipografia, Tema claro e escuro que segue o sistema, accent âmbar tirado da madeira do tabuleiro, fonte Inter (OFL) e ícones Phosphor (MIT OR Apache-2.0). Não há HTML nem webview (DEC-01, DEC-02).

O Ribbon, a Barra de Acesso Rápido, os KeyTips, a Paleta e o Mapa de controles leem a **mesma lista de Ações**. Uma Ação tem rótulo, ícone, atalhos e estado. Assim, mudar um atalho atualiza o Ribbon, e nenhuma Ação existe só num desses lugares.

Todo Painel pode ficar no dock, ser **flutuante** (dentro da janela principal) ou **destacado** (numa janela do sistema operacional). Destacar e Encaixar são Ações explícitas: **não existe arrastar um Painel de uma janela do SO para outra**. O bevy_egui mantém um contexto egui por janela, mas não suporta viewports do egui (bevy_egui #454), então esse arrasto exigiria reimplementar o docking entre janelas. A janela do SO vem numa segunda etapa, depois do dock interno.

A troca é **única**: a nova interface é construída atrás de um flag de desenvolvimento e substitui a antiga quando cobrir todas as Ações e preferências. Nada foi lançado, então não há usuários para migrar.

## Consequências

- Uma nova janela do SO tem câmera própria e segue a regra das câmeras do CLAUDE.md (mesmo `Msaa` e `Hdr`).
- Tema, Densidade e Escala são do Perfil local; o Perfil de interface guarda layout, Painéis flutuantes e destacados, estado do Ribbon e Barra de Acesso Rápido.
- Fonte e ícones entram como assets com licença registrada (ADR-0004). O `egui_material_icons` foi descartado por exigir egui 0.37.
- Componentes como botão, alternância, caixa de seleção e grupo do Ribbon são widgets do projeto, não `egui::Button` cru com estilo ad hoc, para que os estados (repouso, hover, pressionado, desabilitado, foco) sejam iguais em toda a interface.
