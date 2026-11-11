//! Pacotes de Exercícios instalados no diretório de dados do Perfil local.
//!
//! Um pacote é um arquivo `<id>.json` no formato do catálogo (`format: 1`), com um
//! cabeçalho opcional `"package": {"name": "..."}`. O `id` é um slug seguro
//! (`a-z`, `0-9`, `-`, `_`), usado também como nome do arquivo. Os pacotes ficam
//! **separados** do catálogo de fábrica: cada Exercício carregado leva `pack = id`.

use crate::catalog::{Catalog, CatalogError, FORMAT_VERSION};
use crate::exercise::Exercise;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackError {
    /// O id não é um slug seguro.
    BadId(String),
    Io(String),
    Invalid(CatalogError),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadId(id) => write!(f, "identificador de pacote inválido: `{id}`"),
            Self::Io(e) => write!(f, "erro de arquivo no pacote: {e}"),
            Self::Invalid(e) => write!(f, "pacote inválido: {e}"),
        }
    }
}

impl std::error::Error for PackError {}

/// Resumo de um pacote instalado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub count: usize,
    /// Provedor e licença da Origem dos Exercícios (do primeiro que tiver).
    pub provider: Option<String>,
    pub license: Option<String>,
}

#[derive(Serialize)]
struct PackFileOut<'a> {
    format: u32,
    package: Meta<'a>,
    exercises: &'a [Exercise],
}

#[derive(Serialize, Deserialize)]
struct Meta<'a> {
    name: std::borrow::Cow<'a, str>,
}

#[derive(Deserialize)]
struct PackFileIn {
    package: Option<Meta<'static>>,
}

fn check_id(id: &str) -> Result<(), PackError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && !id.starts_with(['.', '-'])
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(PackError::BadId(id.to_owned()))
    }
}

fn path_of(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn io(e: std::io::Error) -> PackError {
    PackError::Io(e.to_string())
}

fn info_of(id: &str, name: String, catalog: &Catalog) -> PackInfo {
    let origin = catalog.iter().find_map(|e| e.provenance.as_ref());
    PackInfo {
        id: id.to_owned(),
        name,
        count: catalog.len(),
        provider: origin.map(|o| o.provider.clone()),
        license: origin.map(|o| o.license.clone()),
    }
}

fn write_atomic(path: &Path, text: &str) -> Result<(), PackError> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

/// Grava (ou substitui) o pacote `id` com estes Exercícios, que são validados antes.
pub fn install_pack(
    dir: &Path,
    id: &str,
    name: &str,
    exercises: &[Exercise],
) -> Result<PackInfo, PackError> {
    check_id(id)?;
    let text = serde_json::to_string(&PackFileOut {
        format: FORMAT_VERSION,
        package: Meta { name: name.into() },
        exercises,
    })
    .map_err(|e| PackError::Io(e.to_string()))?;
    let catalog = Catalog::from_pack_json(id, &text).map_err(PackError::Invalid)?;
    std::fs::create_dir_all(dir).map_err(io)?;
    write_atomic(&path_of(dir, id), &text)?;
    Ok(info_of(id, name.to_owned(), &catalog))
}

/// Instala um arquivo de pacote já em JSON (formato do catálogo); recusa o inválido sem
/// gravar nada.
pub fn install_pack_json(dir: &Path, id: &str, text: &str) -> Result<PackInfo, PackError> {
    check_id(id)?;
    let catalog = Catalog::from_pack_json(id, text).map_err(PackError::Invalid)?;
    let name = pack_name(id, text);
    std::fs::create_dir_all(dir).map_err(io)?;
    write_atomic(&path_of(dir, id), text)?;
    Ok(info_of(id, name, &catalog))
}

fn pack_name(id: &str, text: &str) -> String {
    serde_json::from_str::<PackFileIn>(text)
        .ok()
        .and_then(|f| f.package)
        .map_or_else(|| id.to_owned(), |m| m.name.into_owned())
}

/// Arquivos `*.json` do diretório, em ordem de nome, com o id (nome sem extensão).
fn pack_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let id = p.file_stem()?.to_str()?.to_owned();
            Some((id, p))
        })
        .collect();
    files.sort();
    files
}

/// Pacotes instalados e legíveis (os quebrados ficam de fora; veja [`load_packs`]).
pub fn list_packs(dir: &Path) -> Vec<PackInfo> {
    pack_files(dir)
        .into_iter()
        .filter_map(|(id, path)| {
            let text = std::fs::read_to_string(path).ok()?;
            let catalog = Catalog::from_pack_json(&id, &text).ok()?;
            let name = pack_name(&id, &text);
            Some(info_of(&id, name, &catalog))
        })
        .collect()
}

/// Remove o arquivo do pacote. Pacote ausente não é erro.
pub fn remove_pack(dir: &Path, id: &str) -> Result<(), PackError> {
    check_id(id)?;
    match std::fs::remove_file(path_of(dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(e)),
    }
}

/// Carrega todos os pacotes. O que é inválido ou repete id (do `base` ou de outro pacote)
/// vai para a lista de erros `(id do pacote, erro)` sem derrubar os demais.
pub fn load_packs(dir: &Path, base: &Catalog) -> (Catalog, Vec<(String, CatalogError)>) {
    let mut all = Catalog::default();
    let mut errors = Vec::new();
    for (id, path) in pack_files(dir) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| CatalogError::Io(e.to_string()))
            .and_then(|text| Catalog::from_pack_json(&id, &text))
            .and_then(
                |pack| match base.clash_with(&pack).or_else(|| all.clash_with(&pack)) {
                    Some(id) => Err(CatalogError::DuplicateId(id)),
                    None => Ok(pack),
                },
            );
        match loaded {
            // Sem repetição: já checado acima contra `base` e contra os pacotes anteriores.
            Ok(pack) => all.extend(pack).unwrap_or_default(),
            Err(err) => errors.push((id, err)),
        }
    }
    (all, errors)
}
