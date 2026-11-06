//! Núcleo de regras: Posição canônica, Lance, geração e validação de lances,
//! FEN, SAN, mate/afogamento/material insuficiente/posição morta, chave de
//! repetição e perft. Não depende de Bevy, SQLite nem de relógio.

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
