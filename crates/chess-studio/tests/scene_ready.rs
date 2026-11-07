//! Não-regressão do B22: o modo captura só fotografa com a cena pronta, então nenhuma
//! imagem sai escura/vazia. Precisam de janela e GPU, por isso são `#[ignore]`:
//! `cargo test -p chess-studio --test scene_ready -- --ignored`.

use std::path::Path;
use std::process::Command;

/// Luminância média (0..255) e desvio padrão de um PNG.
fn luminance_stats(path: &Path) -> (f64, f64) {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).expect("PNG existe"),
    ));
    let mut reader = decoder.read_info().expect("PNG válido");
    let mut buf = vec![0; reader.output_buffer_size().expect("tamanho")];
    let info = reader.next_frame(&mut buf).expect("quadro");
    let channels = info.color_type.samples();
    let bytes = &buf[..info.buffer_size()];
    let lum: Vec<f64> = bytes
        .chunks(channels)
        .map(|p| 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]))
        .collect();
    let n = lum.len() as f64;
    let mean = lum.iter().sum::<f64>() / n;
    let var = lum.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

fn chess_studio() -> Command {
    Command::new(env!("CARGO_BIN_EXE_chess-studio"))
}

#[test]
#[ignore = "precisa de janela e GPU"]
fn first_capture_is_never_dark_or_flat() {
    let dir = std::env::temp_dir().join(format!("chess-studio-b22-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let status = chess_studio()
        .args(["--screenshot"])
        .arg(&dir)
        .args(["--size", "960x540"])
        .status()
        .expect("executa o app");
    assert!(status.success(), "captura terminou com erro: {status}");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("diretório de saída") {
        let path = entry.expect("entrada").path();
        if path.extension().is_some_and(|e| e == "png") {
            let (mean, std_dev) = luminance_stats(&path);
            eprintln!("{}: média {mean:.1}, desvio {std_dev:.1}", path.display());
            assert!(
                mean > 25.0 && std_dev > 15.0,
                "{} saiu escura ou lisa (média {mean:.1}, desvio {std_dev:.1})",
                path.display()
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 7, "uma captura por Apresentação e Vista");
    let report = std::fs::read_to_string(dir.join("desempenho.txt")).expect("relatório");
    assert!(report.contains("carregamento da cena"), "{report}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "precisa de janela e GPU"]
fn missing_assets_fail_with_a_clear_error_instead_of_a_dark_capture() {
    let dir = std::env::temp_dir().join(format!("chess-studio-b22-fail-{}", std::process::id()));
    let output = chess_studio()
        .args(["--screenshot"])
        .arg(&dir)
        .args(["--ready-timeout", "5"])
        .env("CHESS_STUDIO_ASSETS", "/nonexistent-assets")
        .output()
        .expect("executa o app");
    assert_eq!(output.status.code(), Some(2));
    let log = String::from_utf8_lossy(&output.stderr) + String::from_utf8_lossy(&output.stdout);
    assert!(log.contains("a cena não ficou pronta"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}
