#![allow(clippy::unwrap_used)]
//! Propriedades do enquadramento ICS (#237) com entradas pseudoaleatórias determinísticas
//! (xorshift, sem crate novo): nunca entra em pânico, nunca guarda mais que a linha máxima,
//! nunca entrega quebra de linha, `\r` ou byte de controle, e o resultado não depende de como
//! os bytes chegaram partidos.

use chess_net::{IcsItem, IcsReader};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Bytes hostis: muito `\n`, `\r`, IAC (255) e comandos telnet, controles, UTF-8 partido e
/// trechos longos.
fn hostile(rng: &mut Rng, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        match rng.below(12) {
            0 => out.push(b'\n'),
            1 => out.push(b'\r'),
            2 => out.push(0xff),
            3 => out.extend_from_slice(&[0xff, 0xfa, 0x18]),
            4 => out.extend_from_slice(&[0xff, 0xf0]),
            5 => out.push(u8::try_from(rng.below(32)).unwrap()),
            6 => out.extend_from_slice(&[0xc3, 0xa9]),
            7 => out.push(0xc3),
            8 => out.extend(std::iter::repeat_n(b'x', rng.below(300))),
            _ => out.push(b' ' + u8::try_from(rng.below(95)).unwrap()),
        }
    }
    out
}

fn feed_in_chunks(reader: &mut IcsReader, bytes: &[u8], rng: &mut Rng) -> Vec<IcsItem> {
    let mut items = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let size = 1 + rng.below(64);
        let end = (at + size).min(bytes.len());
        items.extend(reader.feed(&bytes[at..end]));
        at = end;
    }
    items
}

fn without_prompts(items: Vec<IcsItem>) -> Vec<IcsItem> {
    items
        .into_iter()
        .filter(|i| !matches!(i, IcsItem::Prompt(_)))
        .collect()
}

#[test]
fn hostile_bytes_never_panic_and_never_leak_control_characters() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..300 {
        let max = 1 + rng.below(200);
        let len = 1 + rng.below(4000);
        let bytes = hostile(&mut rng, len);
        let mut reader = IcsReader::new(max);
        for item in feed_in_chunks(&mut reader, &bytes, &mut rng) {
            match item {
                IcsItem::Line(text) | IcsItem::Prompt(text) => {
                    assert!(!text.trim().is_empty());
                    assert!(text.chars().count() <= max, "{} > {max}", text.len());
                    assert!(
                        text.chars().all(|c| c == '\t' || !c.is_control()),
                        "{text:?}"
                    );
                }
                IcsItem::LineTooLong { limit } => assert_eq!(limit, max),
            }
        }
    }
}

#[test]
fn how_the_bytes_are_split_does_not_change_the_lines() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for _ in 0..300 {
        let max = 1 + rng.below(200);
        let len = 1 + rng.below(3000);
        let bytes = hostile(&mut rng, len);
        let whole = without_prompts(IcsReader::new(max).feed(&bytes));
        let mut reader = IcsReader::new(max);
        let split = without_prompts(feed_in_chunks(&mut reader, &bytes, &mut rng));
        assert_eq!(whole, split);
    }
}

#[test]
fn clean_lines_come_back_exactly_with_the_fics_line_ending() {
    let mut rng = Rng(42);
    for _ in 0..200 {
        let lines: Vec<String> = (0..1 + rng.below(20))
            .map(|_| {
                (0..1 + rng.below(80))
                    .map(|_| char::from(b'!' + u8::try_from(rng.below(94)).unwrap()))
                    .collect()
            })
            .collect();
        let mut bytes = Vec::new();
        for line in &lines {
            bytes.extend_from_slice(line.as_bytes());
            bytes.extend_from_slice(b"\n\r");
        }
        let mut reader = IcsReader::new(1024);
        let got = without_prompts(feed_in_chunks(&mut reader, &bytes, &mut rng));
        let want: Vec<IcsItem> = lines.into_iter().map(IcsItem::Line).collect();
        assert_eq!(got, want);
    }
}

#[test]
fn telnet_negotiation_anywhere_is_removed_without_touching_the_text() {
    let mut rng = Rng(7);
    let text = b"<12> rnbqkbnr pppppppp -------- -------- -------- -------- PPPPPPPP RNBQKBNR W -1 1 1 1 1 0 7 GuestA GuestB 1 5 0 39 39 300 300 1 none (0:00) none 0 0 0";
    for _ in 0..200 {
        let mut bytes = Vec::new();
        for &b in text {
            if rng.below(10) == 0 {
                match rng.below(3) {
                    0 => bytes.extend_from_slice(&[0xff, 0xfb, 0x01]),
                    1 => bytes.extend_from_slice(&[0xff, 0xfa, 0x1f, 0x00, 0x50, 0xff, 0xf0]),
                    _ => bytes.extend_from_slice(&[0xff, 0xf1]),
                }
            }
            bytes.push(b);
        }
        bytes.extend_from_slice(b"\n\r");
        let mut reader = IcsReader::new(1024);
        let got = without_prompts(feed_in_chunks(&mut reader, &bytes, &mut rng));
        assert_eq!(
            got,
            vec![IcsItem::Line(String::from_utf8(text.to_vec()).unwrap())]
        );
    }
}

#[test]
fn a_giant_line_is_dropped_once_and_the_next_line_survives() {
    let mut reader = IcsReader::new(100);
    let mut bytes = vec![b'a'; 10_000];
    bytes.extend_from_slice(b"\n\rfics% \n\r");
    let items = reader.feed(&bytes);
    assert_eq!(
        items,
        vec![
            IcsItem::LineTooLong { limit: 100 },
            IcsItem::Line("fics% ".into())
        ]
    );
}
