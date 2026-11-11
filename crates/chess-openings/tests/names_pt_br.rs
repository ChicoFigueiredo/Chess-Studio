//! Nomes de abertura em português: o comportamento externo de `name_pt_br`.

use chess_openings::name_pt_br;

fn pt(name: &str) -> String {
    name_pt_br(name)
}

#[test]
fn the_final_noun_moves_to_the_front_and_adjectives_agree_with_it() {
    assert_eq!(pt("Sicilian Defense"), "Defesa Siciliana");
    assert_eq!(pt("Italian Game"), "Jogo Italiano");
    assert_eq!(pt("English Opening"), "Abertura Inglesa");
    assert_eq!(pt("Scandinavian Defense"), "Defesa Escandinava");
    assert_eq!(pt("Vienna Game"), "Jogo de Viena");
}

#[test]
fn proper_names_stay_and_the_possessive_goes_away() {
    assert_eq!(pt("Alekhine Defense"), "Defesa Alekhine");
    assert_eq!(pt("Petrov's Defense"), "Defesa Petrov");
    assert_eq!(pt("Ruy Lopez"), "Ruy Lopez");
    assert_eq!(pt("Caro-Kann Defense"), "Defesa Caro-Kann");
}

#[test]
fn fixed_expressions_use_the_portuguese_chess_terms() {
    assert_eq!(pt("Queen's Gambit"), "Gambito da Dama");
    assert_eq!(pt("Queen's Gambit Declined"), "Gambito da Dama Recusado");
    assert_eq!(pt("King's Gambit Accepted"), "Gambito do Rei Aceito");
    assert_eq!(pt("King's Indian Defense"), "Defesa Índia do Rei");
    assert_eq!(pt("Two Knights Defense"), "Defesa dos Dois Cavalos");
    assert_eq!(pt("Four Knights Game"), "Jogo dos Quatro Cavalos");
}

#[test]
fn each_part_of_a_long_name_is_translated_and_the_separators_are_kept() {
    assert_eq!(
        pt("Sicilian Defense: Najdorf Variation, English Attack"),
        "Defesa Siciliana: Variante Najdorf, Ataque Inglês"
    );
    assert_eq!(
        pt("Ruy Lopez: Morphy Defense"),
        "Ruy Lopez: Defesa Morphy"
    );
    assert_eq!(
        pt("French Defense: Exchange Variation"),
        "Defesa Francesa: Variante da Troca"
    );
    assert_eq!(pt("Sicilian Defense: Dragon Variation, Classical Variation"),
        "Defesa Siciliana: Variante Dragon, Variante Clássica");
}

#[test]
fn deferred_reversed_and_with_parts_read_naturally() {
    assert_eq!(
        pt("Ruy Lopez: Morphy Defense, Schliemann Defense Deferred"),
        "Ruy Lopez: Defesa Morphy, Defesa Schliemann Adiada"
    );
    assert_eq!(
        pt("English Opening: King's English Variation, Reversed Sicilian"),
        "Abertura Inglesa: Variante Inglesa do Rei, Siciliana invertida"
    );
    assert_eq!(
        pt("Vienna Gambit, with Max Lange Defense"),
        "Gambito de Viena, com Defesa Max Lange"
    );
    assert_eq!(
        pt("Alekhine Defense: Two Pawns Attack"),
        "Defesa Alekhine: Ataque dos Dois Peões"
    );
}

#[test]
fn what_is_not_recognized_stays_as_it_is() {
    assert_eq!(pt("Giuoco Piano"), "Giuoco Piano");
    assert_eq!(pt("Something Unheard Of"), "Something Unheard Of");
    assert_eq!(pt(""), "");
}

#[test]
fn no_name_of_the_embedded_book_keeps_the_commonest_english_words() {
    for source in [
        include_str!("../data/a.tsv"),
        include_str!("../data/b.tsv"),
        include_str!("../data/c.tsv"),
        include_str!("../data/d.tsv"),
        include_str!("../data/e.tsv"),
    ] {
        for line in source.lines().skip(1) {
            let Some(name) = line.split('\t').nth(1) else {
                continue;
            };
            let out = pt(name);
            assert!(
                !out.contains("Variation") && !out.contains("Defense"),
                "{name} => {out}"
            );
        }
    }
}

/// Revisão manual: `cargo test -p chess-openings --test names_pt_br dump -- --ignored --nocapture`.
#[test]
#[ignore = "só para revisar a tradução à mão"]
fn dump() {
    for source in [
        include_str!("../data/a.tsv"),
        include_str!("../data/b.tsv"),
        include_str!("../data/c.tsv"),
        include_str!("../data/d.tsv"),
        include_str!("../data/e.tsv"),
    ] {
        for line in source.lines().skip(1) {
            if let Some(name) = line.split('\t').nth(1) {
                println!("{name} => {}", pt(name));
            }
        }
    }
}
