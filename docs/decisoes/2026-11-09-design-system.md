# Decisões: design system e nova interface (09/11/2026)

Rodada de questionamento com o usuário (grill-with-docs), a partir do pedido de um design system e de uma interface no estilo Office com Painéis encaixáveis em janelas. Termos novos estão no [GLOSSARY](../../GLOSSARY.md); a decisão difícil de reverter está na [ADR-0005](../adr/0005-interface-ribbon-acoes-design-system-egui.md). A pesquisa de referência (Office ribbon, Fluent 2, docking em VS, JetBrains, Blender e Photoshop, GUIs de xadrez e viabilidade em egui) fica nas notas da sessão.

| # | Decisão | Escolha |
|---|---|---|
| DS-01 | Menu superior | **Ribbon** com modo clássico (3 linhas), modo simples (1 linha) e recolhido (Ctrl+F1). **Arquivo** abre os bastidores: Nova, Abrir PGN, Biblioteca, Perfis, Opções. |
| DS-02 | Abas do Ribbon | Por tarefa: Arquivo · Início · Análise · Biblioteca · Treino · Exibir. Abas contextuais aparecem só quando cabem (ex.: Súmula aberta). |
| DS-03 | Janelas | Painel **encaixado** (dock), **flutuante** (dentro da janela principal) ou **destacado** (janela do SO). Primeiro o dock interno; a janela do SO vem na 2ª etapa. Destacar e Encaixar são Ações; **não há arrastar entre janelas do SO** (bevy_egui #454). |
| DS-04 | Painel esquerdo | Deixa de existir: comandos vão para o Ribbon, alternâncias frequentes para Exibir e preferências persistentes para **Arquivo > Opções**. |
| DS-05 | Tema | Escuro e claro, seguindo o SO por padrão. Alto contraste fica para depois. |
| DS-06 | Termo | **Painel** é o nome oficial. Quadro, card e gadget ficam como sinônimos a evitar. |
| DS-07 | Perfis de interface | Passam a guardar também o estado do Ribbon, a Barra de Acesso Rápido e os Painéis flutuantes e destacados. Há 5 de fábrica (tabela abaixo). Tema, Densidade e Escala ficam fora do perfil: são do Perfil local. |
| DS-08 | Revisão | Primeiro o artefato de design system com as telas, para comentários; depois uma galeria egui no app (`--gallery`) e os tickets. |
| DS-09 | Familiaridade | Barra de status, Barra de Acesso Rápido, KeyTips (Alt) e Paleta de Ações (Ctrl+K). |
| DS-10 | Ícones | Phosphor (`egui-phosphor` 0.14, MIT OR Apache-2.0, egui 0.36). |
| DS-11 | Accent | Âmbar tirado da madeira do tabuleiro; azul fica para seleção e informação. |
| DS-12 | Fonte | Inter (OFL) na interface; JetBrains Mono (OFL) só no log UCI. As duas embutidas, com licença registrada. |
| DS-13 | Densidade | Confortável (padrão) e Compacta, mais Escala de 80% a 200% (Ctrl+= e Ctrl+-). |
| DS-14 | Ações | **Uma lista única de Ações** (rótulo, ícone, atalhos, estado) para Ribbon, Barra de Acesso Rápido, KeyTips, Paleta e Mapa de controles. |
| DS-15 | Transição | Troca única, atrás de um flag de desenvolvimento, quando o Ribbon e as Opções cobrirem tudo. |
| DS-16 | Avaliação | Barra de avaliação vertical colada ao tabuleiro, visível só com a Análise ligada e a Engine consentida (ADR-0003). As linhas ficam no Painel Análise. |

## Perfis de interface de fábrica (DS-07)

| Perfil | Apresentação e Painéis | Auxílios | Ribbon |
|---|---|---|---|
| Jogo físico | 3D profissional · Relógio + Notação | nenhum | simples |
| Treinamento assistido | Treino + Notação + Tabuleiro 2D | ligados | simples |
| Estudo e análise | Análise + Aberturas + Notação + Biblioteca (abas) · barra de avaliação | conforme o perfil de auxílios | clássico |
| Competição | Súmula + Relógio · Notação oculta | nenhum | recolhido |
| Mínimo | só o tabuleiro e o Relógio, sem dock | nenhum | recolhido |

## Hierarquia da informação

1. **Tabuleiro**: a tarefa principal, sempre o maior elemento. Dele só ficam colados a barra de avaliação (DS-16) e as coordenadas.
2. **Estado da Partida**: Relógio e Notação, no primeiro grupo do dock.
3. **Ferramentas da tarefa** (Análise, Aberturas, Biblioteca, Treino, Súmula), em abas, que o Perfil de interface escolhe.
4. **Ações**: no Ribbon, agrupadas por tarefa; as frequentes também na Barra de Acesso Rápido.
5. **Preferências** em Opções, nunca na tela de jogo.
6. **Estado do sistema** (Perfil ativo, Engine e consentimento, mensagens) na barra de status.
