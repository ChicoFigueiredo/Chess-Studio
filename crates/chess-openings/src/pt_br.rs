//! Nomes de abertura em português (#239). O conjunto CC0 de origem só tem nomes em inglês
//! ("Sicilian Defense: Najdorf Variation"); aqui eles viram "Defesa Siciliana: Variante
//! Najdorf" por regras de gramática, sem tabela de 3 mil nomes:
//!
//! - o substantivo final passa para a frente ("<Nome> Defense" -> "Defesa <Nome>") e os
//!   adjetivos concordam em gênero com ele ("Sicilian Defense" -> "Defesa Siciliana");
//! - expressões fixas ("Queen's Gambit" -> "Gambito da Dama", "Two Knights" -> "dos Dois
//!   Cavalos") vêm de uma tabela curta;
//! - nomes próprios (Najdorf, Ruy Lopez, Giuoco Piano) ficam como estão, sem o "'s";
//! - o que não se reconhece fica em inglês, sem arriscar uma tradução errada.
//!
//! O nome original continua em `Opening::name`; a tradução é só para mostrar.

/// Substantivo final: (inglês, singular, plural, feminino?).
const NOUNS: &[(&str, &str, &str, bool)] = &[
    ("Variation", "Variante", "Variantes", true),
    ("Variations", "Variantes", "Variantes", true),
    ("Defense", "Defesa", "Defesas", true),
    ("Opening", "Abertura", "Aberturas", true),
    ("Gambit", "Gambito", "Gambitos", false),
    ("Countergambit", "Contragambito", "Contragambitos", false),
    ("Attack", "Ataque", "Ataques", false),
    ("Counterattack", "Contra-ataque", "Contra-ataques", false),
    ("System", "Sistema", "Sistemas", false),
    ("Line", "Linha", "Linhas", true),
    ("Game", "Jogo", "Jogos", false),
    ("Formation", "Formação", "Formações", true),
    ("Trap", "Armadilha", "Armadilhas", true),
];

/// Estado do gambito: concorda com o substantivo. (inglês, masculino, feminino)
const STATUS: &[(&str, &str, &str)] = &[
    ("Accepted", "Aceito", "Aceita"),
    ("Declined", "Recusado", "Recusada"),
    ("Deferred", "Adiado", "Adiada"),
];

/// Modificadores: (inglês, masculino, feminino). Os que não mudam repetem o texto.
const MODIFIERS: &[(&str, &str, &str)] = &[
    // Preposições e expressões fixas.
    ("King's English", "Inglês do Rei", "Inglesa do Rei"),
    ("King's Indian", "Índia do Rei", "Índia do Rei"),
    ("Queen's Indian", "Índia da Dama", "Índia da Dama"),
    ("Nimzo-Indian", "Nimzo-Índia", "Nimzo-Índia"),
    ("Bogo-Indian", "Bogo-Índia", "Bogo-Índia"),
    ("Old Indian", "Índia Antiga", "Índia Antiga"),
    ("East Indian", "Índia Oriental", "Índia Oriental"),
    ("Anglo-Indian", "Anglo-Indiano", "Anglo-Indiana"),
    ("Anglo-Slav", "Anglo-Eslavo", "Anglo-Eslava"),
    ("Semi-Slav", "Semi-Eslavo", "Semi-Eslava"),
    ("King's Knight", "do Cavalo do Rei", "do Cavalo do Rei"),
    ("Queen's Knight", "do Cavalo da Dama", "do Cavalo da Dama"),
    ("King's Pawn", "do Peão do Rei", "do Peão do Rei"),
    ("Queen's Pawn", "do Peão da Dama", "do Peão da Dama"),
    ("King's", "do Rei", "do Rei"),
    ("Queen's", "da Dama", "da Dama"),
    ("Bishop's", "do Bispo", "do Bispo"),
    ("Bishop", "do Bispo", "do Bispo"),
    ("Knight", "do Cavalo", "do Cavalo"),
    ("Knights", "dos Cavalos", "dos Cavalos"),
    ("Queen", "da Dama", "da Dama"),
    ("Wayward Queen", "da Dama Errante", "da Dama Errante"),
    ("Two Pawns", "dos Dois Peões", "dos Dois Peões"),
    ("Three Pawns", "dos Três Peões", "dos Três Peões"),
    ("Three Pawn", "dos Três Peões", "dos Três Peões"),
    ("Six Pawns", "dos Seis Peões", "dos Seis Peões"),
    ("Chinese", "Chinês", "Chinesa"),
    ("Baltic", "Báltico", "Báltica"),
    ("Armenian", "Armênio", "Armênia"),
    ("Swiss", "Suíço", "Suíça"),
    ("Argentinian", "Argentino", "Argentina"),
    ("Flexible", "Flexível", "Flexível"),
    ("Two Knights", "dos Dois Cavalos", "dos Dois Cavalos"),
    ("Three Knights", "dos Três Cavalos", "dos Três Cavalos"),
    ("Four Knights", "dos Quatro Cavalos", "dos Quatro Cavalos"),
    ("Four Pawns", "dos Quatro Peões", "dos Quatro Peões"),
    ("Poisoned Pawn", "do Peão Envenenado", "do Peão Envenenado"),
    ("Exchange", "da Troca", "da Troca"),
    ("Advance", "do Avanço", "do Avanço"),
    ("Wing", "da Ala", "da Ala"),
    ("Center", "do Centro", "do Centro"),
    ("Vienna", "de Viena", "de Viena"),
    (
        "Knight Tour",
        "do Passeio do Cavalo",
        "do Passeio do Cavalo",
    ),
    ("Kingside", "da Ala do Rei", "da Ala do Rei"),
    ("Queenside", "da Ala da Dama", "da Ala da Dama"),
    ("Retreat", "da Retirada", "da Retirada"),
    ("Castling", "do Roque", "do Roque"),
    ("Check", "do Xeque", "do Xeque"),
    ("Pin", "da Cravada", "da Cravada"),
    ("Hedgehog", "Ouriço", "Ouriço"),
    ("Elephant", "do Elefante", "do Elefante"),
    ("New York", "de Nova York", "de Nova York"),
    // Adjetivos.
    ("Positional", "Posicional", "Posicional"),
    ("Sharp", "Agudo", "Aguda"),
    ("Short", "Curto", "Curta"),
    ("Standard", "Padrão", "Padrão"),
    ("Dragon", "Dragão", "Dragão"),
    ("Classical", "Clássico", "Clássica"),
    ("Modern", "Moderno", "Moderna"),
    ("Closed", "Fechado", "Fechada"),
    ("Open", "Aberto", "Aberta"),
    ("Normal", "Normal", "Normal"),
    ("Symmetrical", "Simétrico", "Simétrica"),
    ("Orthodox", "Ortodoxo", "Ortodoxa"),
    ("Traditional", "Tradicional", "Tradicional"),
    ("Quiet", "Tranquilo", "Tranquila"),
    ("Central", "Central", "Central"),
    ("Double", "Duplo", "Dupla"),
    ("Main", "Principal", "Principal"),
    ("Old", "Antigo", "Antiga"),
    ("Deferred", "Adiado", "Adiada"),
    ("Delayed", "Adiado", "Adiada"),
    ("Accelerated", "Acelerado", "Acelerada"),
    ("Reversed", "Invertido", "Invertida"),
    ("Improved", "Melhorado", "Melhorada"),
    ("Simplified", "Simplificado", "Simplificada"),
    ("English", "Inglês", "Inglesa"),
    ("Spanish", "Espanhol", "Espanhola"),
    ("Russian", "Russo", "Russa"),
    ("Czech", "Tcheco", "Tcheca"),
    ("Dutch", "Holandês", "Holandesa"),
    ("Scandinavian", "Escandinavo", "Escandinava"),
    ("Italian", "Italiano", "Italiana"),
    ("French", "Francês", "Francesa"),
    ("Sicilian", "Siciliano", "Siciliana"),
    ("Scotch", "Escocês", "Escocesa"),
    ("Portuguese", "Português", "Portuguesa"),
    ("Hungarian", "Húngaro", "Húngara"),
    ("Polish", "Polonês", "Polonesa"),
    ("Danish", "Dinamarquês", "Dinamarquesa"),
    ("Latvian", "Letão", "Letã"),
    ("Austrian", "Austríaco", "Austríaca"),
    ("Indian", "Indiano", "Indiana"),
    ("German", "Alemão", "Alemã"),
    ("Slav", "Eslavo", "Eslava"),
    ("Catalan", "Catalão", "Catalã"),
    ("Basque", "Basco", "Basca"),
    ("Irish", "Irlandês", "Irlandesa"),
    ("Mexican", "Mexicano", "Mexicana"),
    ("Australian", "Australiano", "Australiana"),
];

/// Trechos inteiros, sem substantivo, que têm tradução própria.
const WHOLE: &[(&str, &str)] = &[
    ("Main Line", "Linha principal"),
    ("Closed", "Fechada"),
    ("Open", "Aberta"),
    ("Classical", "Clássica"),
    ("Modern", "Moderna"),
    ("Sicilian", "Siciliana"),
    ("Accelerated Dragon", "Dragão Acelerado"),
    ("Queen's Pawn", "Peão da Dama"),
    ("King's Pawn", "Peão do Rei"),
    ("Queen's Indian Accelerated", "Índia da Dama Acelerada"),
    ("Slav Indian", "Índia Eslava"),
    ("Rare Defenses", "Defesas raras"),
    ("Advance", "Avanço"),
    ("Queen's Knight", "Cavalo da Dama"),
    ("King's Knight", "Cavalo do Rei"),
    ("Two Knights", "Dois Cavalos"),
    ("Three Knights", "Três Cavalos"),
    ("Four Knights", "Quatro Cavalos"),
    ("Dragon", "Dragão"),
    ("Dragon Transfer", "Transferência para o Dragão"),
    ("Hyperaccelerated Dragon", "Dragão Hiperacelerado"),
    ("Reversed Dragon", "Dragão invertido"),
    ("Kingside Fianchetto", "Fianchetto da Ala do Rei"),
    ("Queenside Fianchetto", "Fianchetto da Ala da Dama"),
    ("Fool's Mate", "Mate do Tolo"),
    ("Endgame Offer", "Oferta de final"),
    ("Sicilian Invitation", "Convite Siciliano"),
    ("Old Benoni", "Benoni Antiga"),
    ("English Orangutan", "Orangotango Inglês"),
    ("Franco-Sicilian Hybrid", "Híbrido Franco-Siciliano"),
    ("Vienna Hybrid", "Híbrido de Viena"),
    ("Maróczy Bind", "Estrutura Maróczy"),
    ("Closed Sicilian", "Siciliana Fechada"),
    ("Giuoco Piano", "Giuoco Piano"),
    ("Giuoco Pianissimo", "Giuoco Pianissimo"),
];

fn modifier(word: &str, feminine: bool) -> Option<&'static str> {
    MODIFIERS
        .iter()
        .find(|(en, _, _)| *en == word)
        .map(|(_, m, f)| if feminine { *f } else { *m })
}

/// Um trecho entre separadores (`: ` e `, `).
fn piece(text: &str) -> String {
    if let Some((_, pt)) = WHOLE.iter().find(|(en, _)| *en == text) {
        return (*pt).to_owned();
    }
    // "Hungarian Dragon": o Dragão com o adjetivo concordando no masculino.
    if let Some(adjective) = text
        .strip_suffix(" Dragon")
        .and_then(|rest| modifier(rest, false))
    {
        return format!("Dragão {adjective}");
    }
    if let Some(rest) = text.strip_prefix("with ") {
        return format!("com {}", piece(rest));
    }
    // "Reversed Sicilian": a mesma ideia com as cores trocadas.
    if let Some(rest) = text.strip_prefix("Reversed ") {
        return format!("{} invertida", piece(rest));
    }
    // "Botvinnik System Reversed": o mesmo, com a palavra no fim; concorda com o substantivo.
    if let Some(rest) = text.strip_suffix(" Reversed") {
        let translated = piece(rest);
        let masculine = ["Sistema", "Gambito", "Ataque", "Jogo", "Dragão"]
            .iter()
            .any(|noun| translated.starts_with(noun));
        return format!(
            "{translated} {}",
            if masculine { "invertido" } else { "invertida" }
        );
    }
    let mut words: Vec<&str> = text.split(' ').collect();
    let status = words
        .last()
        .and_then(|w| STATUS.iter().find(|(en, _, _)| en == w))
        .copied();
    if status.is_some() {
        words.pop();
    }
    let Some(&last) = words.last() else {
        return text.to_owned();
    };
    let Some(&(_, singular, plural, feminine)) = NOUNS.iter().find(|(en, ..)| *en == last) else {
        // Sem substantivo ("Neo-Catalan Declined"): o estado concorda no masculino.
        return match status {
            Some((_, masculine, _)) => format!("{} {masculine}", piece(&words.join(" "))),
            None => text.to_owned(),
        };
    };
    words.pop();
    let noun = if last == "Variations" {
        plural
    } else {
        singular
    };
    let mut out = vec![noun.to_owned()];
    let mut i = 0;
    while i < words.len() {
        // Expressões de até três palavras, da mais longa para a mais curta.
        let found = (1..=3.min(words.len() - i)).rev().find_map(|n| {
            let phrase = words[i..i + n].join(" ");
            modifier(&phrase, feminine).map(|pt| (n, pt))
        });
        match found {
            Some((n, pt)) => {
                out.push(pt.to_owned());
                i += n;
            }
            None => {
                out.push(words[i].trim_end_matches("'s").to_owned());
                i += 1;
            }
        }
    }
    if let Some((_, masculine, feminine_form)) = status {
        out.push((if feminine { feminine_form } else { masculine }).to_owned());
    }
    out.join(" ")
}

/// O nome em português de uma abertura do livro. Separadores `: ` e `, ` são mantidos.
pub fn name_pt_br(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    let mut rest = name;
    loop {
        let cut = rest
            .find([':', ','])
            .filter(|i| rest[i + 1..].starts_with(' '));
        match cut {
            Some(i) => {
                out.push_str(&piece(&rest[..i]));
                out.push_str(&rest[i..i + 2]);
                rest = &rest[i + 2..];
            }
            None => {
                out.push_str(&piece(rest));
                return out;
            }
        }
    }
}
