//! Intérprete de gestos (máquina de estados pura que traduz eventos de ponteiro,
//! teclado e foco em `Command`s) e geometria pura de tabuleiro/picking
//! (raio para casa, casa para transform) nas quatro Vistas.

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
