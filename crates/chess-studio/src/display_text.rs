//! Texto vindo de fora (Provedor, servidor FICS, arquivo) que vai para a interface passa por
//! aqui antes de ser desenhado (SEC-08, #282).
//!
//! O `egui` não reordena texto bidirecional, então o risco não é execução, é confusão visual:
//! um nome com U+202E ou espaços de largura zero parece outro nome. A função única
//! [`clean_display`] tira controles (Cc) e marcas de formatação (Cf) e corta o comprimento.
//! Todo argumento do Fluent passa por [`clean_argument`] dentro de `I18n::t_with`; texto de
//! Provedor desenhado sem Fluent chama [`clean_display`] no ponto de exibição.

/// Comprimento máximo, em caracteres, de um nome ou rótulo vindo de fora.
pub const MAX_DISPLAY_CHARS: usize = 64;

/// Comprimento máximo de um argumento livre do Fluent (mensagens, motivos de recusa).
pub const MAX_ARGUMENT_CHARS: usize = 400;

/// Caracteres que enganam a leitura: controles, marcas bidi, espaços de largura zero,
/// separadores invisíveis e o BOM.
fn is_deceptive(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// Tira controles e marcas de formatação e corta em `max` caracteres (com reticências).
pub fn clean_with_limit(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max * 4));
    let mut count = 0;
    for c in text.chars().filter(|c| !is_deceptive(*c)) {
        if count == max {
            out.push('…');
            return out;
        }
        out.push(c);
        count += 1;
    }
    out
}

/// O filtro único dos nomes e textos curtos do Provedor (64 caracteres).
pub fn clean_display(text: &str) -> String {
    clean_with_limit(text, MAX_DISPLAY_CHARS)
}

/// O filtro dos argumentos do Fluent (mensagens maiores, mesma limpeza).
pub fn clean_argument(text: &str) -> String {
    clean_with_limit(text, MAX_ARGUMENT_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_and_bidi_and_invisible_marks_are_removed() {
        let hostile = "ab\u{202E}cd\u{2066}e\u{2069}f\u{200E}\u{200F}g\u{200B}h\u{FEFF}i\u{0007}j\n\u{202A}k\u{202C}";
        assert_eq!(clean_display(hostile), "abcdefghijk");
    }

    #[test]
    fn valid_names_do_not_change() {
        for name in [
            "Magnus",
            "DrNykterstein",
            "Jürgen_Müller",
            "李小龍",
            "o'neil-3",
            "GuestXYZW(U)",
        ] {
            assert_eq!(clean_display(name), name);
        }
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis() {
        let cut = clean_display(&"x".repeat(200));
        assert_eq!(cut.chars().count(), MAX_DISPLAY_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(clean_display(&"y".repeat(64)), "y".repeat(64));
    }

    /// Varredura: os argumentos do Fluent passam por `clean_argument` em `I18n::t_with`; por
    /// isso nenhum outro arquivo pode montar `FluentArgs` por conta própria.
    #[test]
    fn nobody_builds_fluent_arguments_outside_the_filtered_entry_point() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        scan(&src, &mut |path, text| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "i18n.rs" && name != "display_text.rs" && text.contains("FluentArgs") {
                offenders.push(path.display().to_string());
            }
        });
        assert!(
            offenders.is_empty(),
            "FluentArgs fora de i18n.rs: {offenders:?}"
        );
    }

    /// Pontos de exibição de texto do Provedor que não passam pelo Fluent: o arquivo precisa
    /// chamar `clean_display`. Quem criar um ponto novo acrescenta o arquivo aqui.
    const DISPLAY_POINTS: &[&str] = &["online/fics_view.rs"];

    #[test]
    fn every_listed_display_point_uses_the_filter() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for point in DISPLAY_POINTS {
            let text = std::fs::read_to_string(src.join(point)).unwrap();
            assert!(
                text.contains("clean_display"),
                "{point} desenha texto do Provedor sem `clean_display`"
            );
        }
    }

    fn scan(dir: &std::path::Path, visit: &mut dyn FnMut(&std::path::Path, &str)) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan(&path, visit);
            } else if path.extension().is_some_and(|e| e == "rs") {
                visit(&path, &std::fs::read_to_string(&path).unwrap());
            }
        }
    }
}
