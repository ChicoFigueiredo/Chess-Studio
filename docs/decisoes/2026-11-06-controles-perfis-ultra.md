# Controles remapeáveis, vários perfis locais e Ultra (C12, issue #54)

Registra as escolhas de implementação de US-28, US-29 e US-30. Estende a DEC-50 (um perfil local no P0) e a DEC-51 (mapeamento fixo no P0): a V1 remapeia e aceita vários perfis.

## US-28 — mapa de controles

- **Um recurso único.** `chess_input::controls::ControlMap` (puro, sem Bevy) guarda ação -> atalhos. Todos os sistemas de entrada consultam `Controls` (`crates/chess-studio/src/controls.rs`): câmera, jogo, relógio, Apresentação, Foco e tela cheia. Nenhuma tecla fixa sobrou nesses sistemas.
- **Padrões = DEC-51.** Seleção (esquerdo), Ajuste (Alt+esquerdo), órbita (meio / dois dedos), pan (Shift+meio, direito, Shift+dois dedos), zoom (roda / pinça), Esc, Espaço, 1–4/0 (e teclado numérico), Tab, F10, F11.
- **Resolução.** Vale o atalho **mais específico** cujos modificadores estão apertados (Alt+esquerdo é Ajuste, esquerdo é seleção; Shift+esquerdo continua selecionando).
- **Conflito.** Um mesmo atalho (entrada + modificadores) não serve a duas ações. A tela "Controles" avisa e oferece "Substituir" (tira da outra ação) ou cancelar. A Seleção nunca perde o último atalho.
- **Tipo de entrada por ação.** Teclas só em ações de teclado; botões/dois dedos em órbita e pan; roda/pinça no zoom.
- **Persistência.** Preferência `controls` por perfil local, texto estável (`Shift+Mouse:Middle`, `Key:Digit1`, `Gesture:TwoFinger`). Ilegível, de tipo errado ou com conflito volta ao padrão da DEC-51.
- **Captura.** Enquanto a tela espera um atalho, `Controls::is_capturing` silencia todas as consultas (nada move a câmera nem o tabuleiro). Esc cancela a captura.
- **Fora do mapa (de propósito).** O editor de posição (C11) e a edição de texto do egui seguem fixos.

## US-29 — vários perfis locais

- As tabelas `games`, `exercise_runs` e `imported_games` já tinham `profile_id`; a migração V7 só acrescenta `profiles.last_used_at_ms` (o perfil existente continua o ativo). Nenhum dado é movido.
- `Store`: `list_profiles`, `create_profile` (nome de 1 a 40 letras, único), `rename_profile`, `delete_profile` (apaga as Partidas à mão, pois `games.profile_id` não tem `ON DELETE CASCADE`; o resto cai em cascata; o último perfil não se exclui), `active_profile`/`set_active_profile`. `default_profile()` passou a ser o ativo.
- No app, `Persistence::switch_profile` grava a Partida que sai, abre a do perfil que entra (em Pausa, DEC-42) e reaplica preferências, layout, Perfis de interface, controles e Anotações. Trocar nunca altera regras (ADR-0001).
- Seletor na abertura (só com mais de um perfil), seção "Perfil local" na janela principal e janela "Perfis locais". O perfil em uso não se exclui (troque antes). Trocar é bloqueado durante um Exercício.

## US-30 — qualidade Ultra

- **Ultra** = MSAA 8x, sombras 4096 com filtro gaussiano e, opcional, **bloom suave** (HDR + `Tonemapping::TonyMcMapface`). Preferência por perfil (`ultra_effects`, ligada por padrão).
- **Suporte e fallback.** `GpuSupport` lê do adaptador as contagens de amostras aceitas para o formato LDR e para `Rgba16Float`. O preset usa o maior MSAA suportado `<=` o pedido (Ultra sem 8x vira 4x; sem MSAA nenhum, desligado) e o bloom só liga se o HDR renderiza com o MSAA efetivo. Tudo é função pura testada.
- **Regra de MSAA** (issue #82): `apply_quality` continua aplicando o mesmo `Msaa` a **todas** as câmeras da janela. Hdr e bloom ficam só na câmera 3D principal.
- **SSAO descartado.** No Bevy 0.20 o SSAO exige `Msaa::Off`, o que contradiz o MSAA 8x e a regra acima. O filtro temporal de sombras também (precisa de TAA). Revisitar só se o Ultra passar a usar TAA no lugar de MSAA.
- **Picking inalterado:** é geometria pura de `chess_input`.
- Custo de dependência: a feature `bevy_post_process` (crate MIT/Apache do próprio Bevy), só para o bloom.
