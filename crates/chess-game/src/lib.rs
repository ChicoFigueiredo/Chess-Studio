//! A Partida: recebe `Command` mais um instante injetado e devolve `Event`s;
//! `replay(events)` reconstrói o estado. Contém Políticas de arbitragem, relógio,
//! Obrigação de peça tocada, empates, desfazer e exportação PGN (ADR-0001/0002).

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
