#![allow(clippy::unwrap_used)]
//! Pacotes de Exercícios instalados num diretório de dados.

use chess_training::{
    Catalog, PuzzleFilter, import_lichess_csv, install_pack, install_pack_json, list_packs,
    load_packs, remove_pack,
};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

const BEFORE: &str = "6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cs-packs-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn exercises(n: usize) -> Vec<chess_training::Exercise> {
    let mut text = String::new();
    for i in 0..n {
        text += &format!("q{i},{BEFORE},b7b6 a1a8,1500,75,90,10,mate,https://lichess.org/q{i},\n");
    }
    let cancel = AtomicBool::new(false);
    import_lichess_csv(
        Cursor::new(text),
        &PuzzleFilter::default(),
        &cancel,
        &mut |_| {},
    )
    .unwrap()
    .0
}

#[test]
fn install_list_load_and_remove() {
    let d = dir("cycle");
    let info = install_pack(&d, "lichess-mate", "Lichess: mates", &exercises(3)).unwrap();
    assert_eq!((info.id.as_str(), info.count), ("lichess-mate", 3));
    assert_eq!(info.name, "Lichess: mates");
    assert_eq!(info.provider.as_deref(), Some("lichess"));
    assert_eq!(info.license.as_deref(), Some("CC0-1.0"));

    let listed = list_packs(&d);
    assert_eq!(listed, vec![info]);

    let (catalog, errors) = load_packs(&d, &Catalog::bundled().unwrap());
    assert!(errors.is_empty());
    assert_eq!(catalog.len(), 3);
    assert!(
        catalog
            .iter()
            .all(|e| e.pack.as_deref() == Some("lichess-mate"))
    );

    remove_pack(&d, "lichess-mate").unwrap();
    assert!(list_packs(&d).is_empty());
    assert!(load_packs(&d, &Catalog::default()).0.is_empty());
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn reinstalling_replaces_and_unsafe_ids_are_refused() {
    let d = dir("replace");
    install_pack(&d, "p", "P", &exercises(2)).unwrap();
    install_pack(&d, "p", "P", &exercises(5)).unwrap();
    assert_eq!(list_packs(&d)[0].count, 5);
    for bad in ["", "../x", "a/b", "A B", ".hidden"] {
        assert!(
            install_pack(&d, bad, "x", &exercises(1)).is_err(),
            "{bad:?}"
        );
        assert!(remove_pack(&d, bad).is_err(), "{bad:?}");
    }
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_broken_pack_is_reported_and_does_not_hide_the_others() {
    let d = dir("broken");
    install_pack(&d, "good", "Good", &exercises(2)).unwrap();
    std::fs::write(d.join("bad.json"), "{ nao e json").unwrap();
    let (catalog, errors) = load_packs(&d, &Catalog::default());
    assert_eq!(catalog.len(), 2);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, "bad");
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn ids_that_clash_with_the_bundled_catalog_are_refused() {
    let d = dir("clash");
    let bundled = Catalog::bundled().unwrap();
    let mut clash = exercises(1);
    clash[0].id = bundled.iter().next().unwrap().id.clone();
    install_pack(&d, "clash", "Clash", &clash).unwrap();
    let (catalog, errors) = load_packs(&d, &bundled);
    assert!(catalog.is_empty());
    assert_eq!(errors.len(), 1);
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_json_pack_file_can_be_installed_and_is_validated() {
    let d = dir("json");
    let json = serde_json::json!({
        "format": 1,
        "exercises": exercises(2),
    })
    .to_string();
    let info = install_pack_json(&d, "externo", &json).unwrap();
    assert_eq!(info.count, 2);
    assert!(install_pack_json(&d, "ruim", "{}").is_err());
    assert!(!d.join("ruim.json").exists());
    std::fs::remove_dir_all(d).ok();
}
