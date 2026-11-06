//! Engine UCI como processo filho: handshake, opções, `position`/`go`/`stop`,
//! timeouts, detecção de crash, detecção de Engines instaladas e download
//! fixado com SHA-256 (ADR-0003).

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
