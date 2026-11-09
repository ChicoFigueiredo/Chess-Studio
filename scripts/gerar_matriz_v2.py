#!/usr/bin/env python3
"""Gera docs/homologacao/matriz-v2.md (Súmula manual, issues #58 a #60) e valida as referências.

Mesmo padrão de `gerar_matriz_p0.py` e `gerar_matriz_v1.py`: cada teste citado precisa existir
como `fn <nome>` no arquivo indicado (caminho relativo a `crates/`); cada evidência citada entre
crases (`docs/...`, `scripts/...`, `crates/...`) precisa existir no disco. Se algo não existir, o
script falha e não escreve a matriz: a matriz nunca cita teste ou arquivo inventado.

Uso (na raiz do repositório):
    python3 scripts/gerar_matriz_v2.py            # valida e reescreve a matriz
    python3 scripts/gerar_matriz_v2.py --check    # só valida (sem escrever)
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
SAIDA = RAIZ / "docs" / "homologacao" / "matriz-v2.md"

# Atalhos de caminho (relativos a crates/).
STU = "chess-studio/src/"
STO = "chess-store/tests/"
TRN = "chess-training/tests/"
TRS = "chess-training/src/"

STATUS_VALIDOS = ("PASS", "UNVERIFIED", "FAIL", "N/A")

# IDs da V2 no prompt mestre (§14), mais as linhas complementares abaixo.
IDS_OBRIGATORIOS = ["V2-SUM-01", "V2-SUM-02", "V2-SUM-03"]


def t(arquivo, *nomes):
    return [(arquivo, n) for n in nomes]


# (id, requisito, onde está implementado, testes, evidência, status, nota)
LINHAS = [
    ("V2-SUM-01", "Súmula manual suporta anotação sem mostrar resposta pronta.",
     "#58 (PR #129): `chess-training` (`scoresheet.rs`, `ManualScoresheet`), `chess-store` (`scoresheet.rs`, migração v10, registro por perfil e Partida), `chess-studio` (`scoresheet.rs`, painel Súmula; `notation.rs` e `study.rs` ocultam a notação automática)",
     t(TRS + "scoresheet.rs", "writing_trims_keeps_the_raw_text_and_blank_clears", "entries_come_in_ply_order",
       "invalid_writes_are_rejected_and_change_nothing", "delivering_freezes_the_sheet_and_ends_the_notation_hiding",
       "from_parts_round_trips_and_validates")
     + t(STO + "scoresheet.rs", "a_game_without_scoresheet_has_none", "the_scoresheet_round_trips_and_saving_again_replaces_it",
         "two_profiles_never_see_each_others_scoresheets", "saving_a_scoresheet_never_touches_the_game_events_or_pgn",
         "deleting_the_game_or_the_scoresheet_removes_only_that_record", "an_unknown_game_is_an_error")
     + t(STU + "scoresheet.rs", "notation_is_hidden_exactly_while_the_sheet_is_being_written",
         "discarding_while_writing_releases_the_notation_and_deletes_the_record", "writes_are_saved_once_and_unchanged_text_saves_nothing",
         "a_new_game_starts_without_a_sheet_and_waits_for_the_database", "an_exercise_session_suspends_the_hiding",
         "a_failed_database_read_keeps_hiding_and_blocks_start_and_writes", "a_failed_save_goes_back_to_the_queue_without_losing_text",
         "entries_beyond_the_played_moves_are_listed_and_can_be_erased", "a_game_switch_hides_notation_until_the_sheet_loads",
         "rows_follow_the_standard_layout", "rows_start_with_black_after_a_fen_with_black_to_move")
     + t(STU + "persistence.rs", "scoresheet_without_a_stored_game_is_an_error_and_consumes_nothing",
         "scoresheet_failures_are_not_hidden_by_the_events_failing_flag"),
     "PR #129 (issue #58); `docs/evidencias/e3/sumula-em-branco.pdf`", "PASS",
     "A Súmula é um registro separado: nunca altera a Partida, o log de eventos nem o PGN (ADR-0001/0002), provado em `saving_a_scoresheet_never_touches_the_game_events_or_pgn`. A notação automática fica oculta (painéis Notação e Estudo) enquanto a Súmula está em anotação e volta ao entregar ou descartar. A verificação visual na janela está na linha V2-SUM-01-UI."),
    ("V2-SUM-02", "Conferência compara súmula manual com lances reais.",
     "#59 (PR #129): `chess-training` (`scoresheet_check.rs`, só leitura da Partida, fora do Bevy), `chess-store` (migração v11, conferência em registro à parte com histórico conferido), `chess-studio` (resultado no painel Súmula)",
     t(TRN + "scoresheet_check.rs", "exact_canonical_san_is_correct_and_clean", "accepted_notation_variations_table", "promotion_variations",
       "wrong_notation_versus_swapped_table", "missing_disambiguation_is_wrong_notation_unneeded_is_caveat", "r_is_read_as_rook_or_pt_br_king",
       "missing_and_extra_entries", "an_empty_delivered_sheet_is_all_missing", "only_delivered_sheets_are_checked", "verdict_keys_round_trip",
       "checking_a_game_does_not_touch_its_events", "extra_normalizations_are_accepted_with_caveat_table",
       "a_unicode_rook_is_never_read_as_the_pt_br_king", "an_unavailable_start_position_is_an_error", "the_check_records_which_history_it_covers")
     + t(STO + "scoresheet.rs", "the_check_round_trips_with_every_verdict", "two_profiles_never_see_each_others_checks",
         "a_check_needs_a_delivered_sheet_and_follows_its_fate", "saving_a_check_never_touches_the_game_events_or_pgn")
     + t(STU + "scoresheet.rs", "a_delivered_sheet_is_checked_once_and_the_result_is_saved_once", "the_check_follows_the_sheet_and_the_game",
         "a_stored_check_is_not_recomputed", "delivering_mid_game_is_rechecked_when_the_history_changes"),
     "PR #129 (issue #59)", "PASS",
     "Cada meio-lance vira correto, aceito com ressalva, notação errada, trocado, omitido ou a mais. A conferência guarda o histórico conferido (nº de meios-lances e hash) e é refeita quando ele muda. A regra de normalização está na linha V2-SUM-DEC-02 (decisão do usuário)."),
    ("V2-SUM-03", "Impressão A4 e revisão de erros, sem bloquear P0/V1.",
     "#60 (PR #129): `chess-training` (`scoresheet_pdf.rs`, PDF A4 determinístico com fonte em subconjunto; `scoresheet_history.rs`, histórico de erros), `chess-store` (histórico por perfil e período), `chess-studio` (exportar PDF anotado ou em branco, histórico no painel Súmula)",
     t(TRN + "scoresheet_pdf.rs", "is_a_pdf_with_a4_pages_and_the_header_text", "annotated_sheet_prints_what_the_player_wrote_not_the_real_san",
       "page_count_follows_the_number_of_moves", "blank_sheet_has_the_grid_but_no_annotations", "generation_is_deterministic_and_the_font_is_a_subset",
       "invalid_font_is_an_error_not_a_panic")
     + t(TRS + "scoresheet_history.rs", "civil_round_trip_and_month_buckets", "window_is_inclusive_and_buckets_by_day")
     + t(STO + "scoresheet_history.rs", "recalculated_check_replaces_the_previous_one_and_counts_once", "empty_history_has_no_sheets",
         "totals_per_class_and_evolution_for_one_profile", "periods_cut_by_the_injected_instant", "two_profiles_have_separate_histories",
         "unchecked_or_undelivered_sheets_do_not_count_and_history_is_read_only", "unknown_profile_is_an_error")
     + t(STU + "scoresheet.rs", "pdf_is_saved_in_the_folder_and_never_overwrites", "export_writes_a_real_pdf_for_annotated_and_blank",
         "br_date_is_day_month_year", "history_is_refreshed_on_period_change_new_game_and_new_check"),
     "PR #129 (issue #60); `docs/evidencias/e3/sumula-anotada.pdf`; `docs/evidencias/e3/sumula-em-branco.pdf`", "PASS",
     "O PDF anotado imprime o que o jogador escreveu, não a SAN real. O arquivo nunca sobrescreve outro. Impressão em papel A4 real e a tela do histórico estão nas linhas V2-SUM-03-UI e V2-SUM-01-UI. P0 e V1 não dependem da V2."),

    # ---- Linhas complementares (não são IDs do prompt mestre): verificação visual e decisões ----
    ("V2-SUM-01-UI", "Painel Súmula (anotar, entregar, descartar, resultado da conferência, histórico) usável e legível na janela real.",
     "Complementar a V2-SUM-01/02/03: painel Súmula do dock em pt-BR (`crates/chess-studio/assets/i18n/pt-BR/scoresheet.ftl`)",
     [],
     "Nenhuma captura da janela com o painel Súmula; só os PDFs `docs/evidencias/e3/sumula-em-branco.pdf` e `docs/evidencias/e3/sumula-anotada.pdf`", "UNVERIFIED",
     "A interface da Súmula nunca foi aberta em janela (a lógica do painel é testada sem Bevy). Falta o usuário abrir uma Partida, anotar, entregar, ver a conferência e o histórico, e aprovar o visual: a notação some e volta, a digitação não perde texto, a conferência é legível."),
    ("V2-SUM-03-UI", "A súmula impressa em A4 (papel real) fica legível e com o layout esperado.",
     "Complementar a V2-SUM-03: PDF A4 gerado por `scoresheet_pdf.rs`",
     [],
     "`docs/evidencias/e3/sumula-em-branco.pdf`; `docs/evidencias/e3/sumula-anotada.pdf`", "UNVERIFIED",
     "Os PDFs de exemplo existem e os testes provam páginas A4, cabeçalho, grade e determinismo, mas não houve impressão em papel nem aprovação do layout (colunas, tamanho da fonte, nº de lances por folha) pelo usuário."),
    ("V2-SUM-DEC-01", "Decisão: com a Súmula aberta, as setas e dicas do HUD/Análise continuam visíveis ou ficam ocultas?",
     "Complementar a V2-SUM-01: hoje só a notação automática é ocultada (painéis Notação e Estudo); HUD, setas e Análise não são tocados pela Súmula",
     [],
     "Sem teste: é uma decisão de produto", "UNVERIFIED",
     "DECISÃO DO USUÁRIO pendente. Em Partida competitiva o HUD e a Análise já ficam bloqueados pela política (UI-08); o ponto é o treino de escrita em Partida assistida ou de estudo, onde setas e dicas revelariam o lance. Proposta de engenharia (não é decisão): ocultar setas de lance e dicas enquanto a Súmula estiver em anotação, como com a notação."),
    ("V2-SUM-DEC-02", "Decisão: a regra de normalização da conferência (o que conta como correto, aceito com ressalva ou notação errada).",
     "Complementar a V2-SUM-02: `scoresheet_check.rs` aceita alfabeto inglês e pt-BR (R D T B C), sufixos, `x`, roque com zeros, promoção, número do lance, símbolos Unicode e `e.p.`",
     t(TRN + "scoresheet_check.rs", "accepted_notation_variations_table", "extra_normalizations_are_accepted_with_caveat_table",
       "missing_disambiguation_is_wrong_notation_unneeded_is_caveat"),
     "Sem aprovação registrada", "UNVERIFIED",
     "A regra implementada tem teste, mas os limites foram escolhidos pela engenharia e o usuário ainda não os aprovou: por exemplo, desambiguação a mais vira ressalva, a faltante vira notação errada, e a ambiguidade de R (torre ou Rei em pt-BR) é lida como torre. Aprovar ou ajustar a tabela."),
]

MODELO = """# Matriz de rastreabilidade da V2 — Súmula manual (issues #58, #59 e #60)

Gerada por `scripts/gerar_matriz_v2.py`, que **falha** se algum teste ou evidência citado não existir. Para atualizar: edite os dados no script, rode `python3 scripts/gerar_matriz_v2.py` e confira o diff.

- **Base:** `main` em `{{COMMIT}}`. A V2 chegou à `main` no PR #129 (issues #58, #59 e #60, uma entrega só), sobre o P0 e a V1.
- **Escopo:** os três IDs da V2 do prompt mestre (V2-SUM-01, V2-SUM-02, V2-SUM-03) e quatro linhas **complementares** (sufixos `-UI` e `-DEC-`) que não são IDs do prompt mestre: registram a verificação visual que falta e as duas decisões que são do usuário. Não há outro ID V2 no prompt mestre.
- **Ambiente:** Linux físico (Ubuntu 26.04, Ryzen 7 3700X, RTX 4080 SUPER); a Súmula não foi aberta em janela real.
- **Suíte:** ver `docs/relatorios/v2.md` (TESTES_EXECUTADOS).
- **Resumo:** {{RESUMO}}

Legenda: **PASS** = teste automatizado verde ou evidência verificável; **UNVERIFIED** = depende de verificação manual, de hardware ou de decisão do usuário (UNVERIFIED não é PASS); **FAIL** = defeito ou lacuna conhecida; **N/A** = não se aplica (nenhum neste marco). Caminhos de teste são relativos a `crates/`. Números `#NN` são PRs ou issues.

{{TABELA}}

## O que falta para a V2 ser GO

Itens que só o usuário resolve.

1. **V2-SUM-01-UI:** abrir o app, jogar uma Partida, abrir a Súmula, anotar, entregar, ver a conferência e o histórico; conferir que a notação some e volta; aprovar o visual.
2. **V2-SUM-03-UI:** abrir os PDFs de `docs/evidencias/e3/` (ou exportar pelo app) e imprimir em A4; aprovar o layout.
3. **V2-SUM-DEC-01:** decidir se setas e dicas ficam ocultas com a Súmula aberta.
4. **V2-SUM-DEC-02:** aprovar ou ajustar a regra de normalização da conferência.
5. Aprovar a V2 e fechar o épico depois dos itens acima.
"""


def valida():
    erros = []
    ids = [l[0] for l in LINHAS]
    for obrigatorio in IDS_OBRIGATORIOS:
        if obrigatorio not in ids:
            erros.append(f"ID obrigatório ausente: {obrigatorio}")
    if len(ids) != len(set(ids)):
        erros.append("IDs duplicados")
    for (idn, _r, impl, testes, evid, st, nota) in LINHAS:
        if st not in STATUS_VALIDOS:
            erros.append(f"{idn}: status inválido {st}")
        if st == "PASS" and not testes and "`docs/evidencias/" not in evid:
            erros.append(f"{idn}: PASS sem teste nem evidência")
        if st == "UNVERIFIED" and not nota:
            erros.append(f"{idn}: UNVERIFIED sem nota")
        if len(set(testes)) != len(testes):
            erros.append(f"{idn}: teste citado duas vezes")
        for (arq, nome) in testes:
            p = RAIZ / "crates" / arq
            if not p.is_file():
                erros.append(f"{idn}: arquivo inexistente crates/{arq}")
                continue
            if not re.search(rf"\bfn {re.escape(nome)}\b", p.read_text()):
                erros.append(f"{idn}: teste `{nome}` não existe em crates/{arq}")
        for caminho in re.findall(r"`((?:docs|scripts|crates)/[^`\s]+)`", " ".join([evid, nota, impl])):
            if "*" in caminho:
                if not list((RAIZ / Path(caminho).parent).glob(Path(caminho).name)):
                    erros.append(f"{idn}: evidência sem correspondência {caminho}")
            elif not (RAIZ / caminho).exists():
                erros.append(f"{idn}: caminho inexistente {caminho}")
    return erros


def fmt_testes(testes):
    if not testes:
        return "—"
    return "<br>".join(f"`{a}::{n}`" for a, n in testes)


def gera():
    cont = {s: 0 for s in STATUS_VALIDOS}
    for linha in LINHAS:
        cont[linha[5]] += 1
    cab = "| ID | Requisito | Implementação | Teste(s) automatizado(s) | Evidência | Status | Nota |\n|---|---|---|---|---|---|---|\n"
    corpo = ""
    for (idn, req, impl, testes, evid, st, nota) in LINHAS:
        corpo += f"| **{idn}** | {req} | {impl} | {fmt_testes(testes)} | {evid} | **{st}** | {nota or '—'} |\n"
    resumo = " · ".join(f"{k}: {v}" for k, v in cont.items())
    return cont, resumo, cab + corpo


def commit_base():
    """Hash curto de origin/main (informativo; vazio se o git não estiver disponível)."""
    try:
        return subprocess.run(["git", "rev-parse", "--short=12", "origin/main"], cwd=RAIZ, capture_output=True,
                              text=True, check=True).stdout.strip()
    except Exception:
        return "(desconhecido)"


def main():
    erros = valida()
    if erros:
        print("\n".join(erros), file=sys.stderr)
        sys.exit(1)
    cont, resumo, tabela = gera()
    print(f"{len(LINHAS)} IDs válidos. {resumo}")
    if "--check" in sys.argv:
        return
    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    texto = (MODELO.replace("{{RESUMO}}", resumo).replace("{{TABELA}}", tabela)
             .replace("{{COMMIT}}", commit_base()))
    SAIDA.write_text(texto)
    print(f"escrito {SAIDA.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
