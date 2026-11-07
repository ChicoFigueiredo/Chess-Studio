//! Opções UCI anunciadas no handshake (`option name … type …`) como controles tipados, e a
//! validação dos valores que o usuário escolhe (PEND-08, T6–T8): só opções anunciadas, no
//! tipo e na faixa certos, sem caracteres de controle (injeção de comandos UCI) e sem
//! opções que apontam para arquivos arbitrários.

use std::collections::BTreeMap;
use std::fmt;

use crate::protocol::EngineOption;

/// Tipo e domínio de uma opção.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Check {
        default: bool,
    },
    Spin {
        default: i64,
        min: i64,
        max: i64,
    },
    Combo {
        default: String,
        vars: Vec<String>,
    },
    Text {
        default: String,
    },
    /// Ação sem valor (ex.: `Clear Hash`); não é configuração persistida.
    Button,
}

/// Uma opção anunciada pela Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionSpec {
    pub name: String,
    pub control: Control,
}

/// Onde a opção aparece no formulário.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionGroup {
    /// Threads e Hash.
    Resources,
    /// Força: Elo, limitação de força, nível de habilidade.
    Level,
    Advanced,
}

/// Grupo de uma opção pelo nome.
pub fn group_of(name: &str) -> OptionGroup {
    match name.to_ascii_lowercase().as_str() {
        "threads" | "hash" => OptionGroup::Resources,
        "uci_elo" | "uci_limitstrength" | "skill level" => OptionGroup::Level,
        _ => OptionGroup::Advanced,
    }
}

/// Teto de recursos que o app aceita, qualquer que seja o máximo anunciado pela Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_threads: u32,
    pub max_hash_mb: u32,
}

impl Limits {
    /// Teto de Hash do app: 16 GiB.
    pub const HASH_CEILING_MB: u32 = 16 * 1024;

    /// Threads até o número de núcleos lógicos; Hash até [`Self::HASH_CEILING_MB`].
    pub fn for_host() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            max_threads: u32::try_from(cores).unwrap_or(1).max(1),
            max_hash_mb: Self::HASH_CEILING_MB,
        }
    }
}

/// Por que um valor foi recusado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingError {
    /// A Engine não anunciou essa opção.
    Unknown(String),
    /// Opção que o app não deixa editar (caminhos de arquivo).
    Restricted(String),
    /// Valor no formato errado (número, booleano, item da lista) ou com caracteres de controle.
    Invalid { name: String, reason: String },
    /// Número fora da faixa anunciada ou do limite do app.
    OutOfRange {
        name: String,
        value: i64,
        min: i64,
        max: i64,
    },
}

impl fmt::Display for SettingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(name) => write!(f, "a Engine não tem a opção {name:?}"),
            Self::Restricted(name) => {
                write!(f, "a opção {name:?} aponta para arquivos e não é editável")
            }
            Self::Invalid { name, reason } => write!(f, "{name}: {reason}"),
            Self::OutOfRange {
                name,
                value,
                min,
                max,
            } => write!(f, "{name}: {value} fora da faixa permitida ({min} a {max})"),
        }
    }
}

impl std::error::Error for SettingError {}

/// Palavras-chave dentro de `details`.
const KEYWORDS: &[&str] = &["default", "min", "max", "var"];

/// Interpreta uma opção anunciada; `None` se o tipo não for reconhecido.
pub fn parse_option(option: &EngineOption) -> Option<OptionSpec> {
    let mut default = String::new();
    let mut min = None;
    let mut max = None;
    let mut vars: Vec<String> = Vec::new();
    // Cada palavra-chave abre um campo; as palavras seguintes (até a próxima) formam o valor.
    let mut current: Option<&str> = None;
    let mut buffer: Vec<&str> = Vec::new();
    let mut flush = |field: Option<&str>, words: &mut Vec<&str>| {
        let text = words.join(" ");
        words.clear();
        match field {
            Some("default") => default = text,
            Some("min") => min = text.parse::<i64>().ok(),
            Some("max") => max = text.parse::<i64>().ok(),
            Some("var") => vars.push(text),
            _ => {}
        }
    };
    for word in option.details.split_whitespace() {
        if KEYWORDS.contains(&word) {
            flush(current, &mut buffer);
            current = Some(word);
        } else {
            buffer.push(word);
        }
    }
    flush(current, &mut buffer);

    let control = match option.kind.as_str() {
        "check" => Control::Check {
            default: default == "true",
        },
        "spin" => {
            let min = min?;
            let max = max?;
            Control::Spin {
                default: default.parse().unwrap_or(min).clamp(min, max.max(min)),
                min,
                max,
            }
        }
        "combo" => Control::Combo { default, vars },
        "string" => Control::Text {
            default: if default == "<empty>" {
                String::new()
            } else {
                default
            },
        },
        "button" => Control::Button,
        _ => return None,
    };
    Some(OptionSpec {
        name: option.name.clone(),
        control,
    })
}

/// Opções de texto que apontam para arquivos ou pastas (log, tablebases, redes, livros).
pub fn is_path_like(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["file", "path", "dir", "folder", "book", "weights"]
        .iter()
        .any(|k| lower.contains(k))
}

/// Faixa efetiva de um número: a anunciada, reduzida pelos limites do app em Threads/Hash.
pub fn effective_bounds(spec: &OptionSpec, limits: &Limits) -> Option<(i64, i64)> {
    let Control::Spin { min, max, .. } = spec.control else {
        return None;
    };
    let cap = match spec.name.to_ascii_lowercase().as_str() {
        "threads" => i64::from(limits.max_threads),
        "hash" => i64::from(limits.max_hash_mb),
        _ => i64::MAX,
    };
    Some((min, max.min(cap).max(min)))
}

/// Valida `value` para a opção `name` e devolve o texto normalizado a enviar.
pub fn validate_option(
    specs: &[OptionSpec],
    limits: &Limits,
    name: &str,
    value: &str,
) -> Result<String, SettingError> {
    let spec = specs
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| SettingError::Unknown(name.to_owned()))?;
    let invalid = |reason: &str| SettingError::Invalid {
        name: name.to_owned(),
        reason: reason.to_owned(),
    };
    if value.chars().any(char::is_control) {
        return Err(invalid("o valor tem caracteres de controle"));
    }
    match &spec.control {
        Control::Button => Err(invalid("botões não têm valor")),
        Control::Check { .. } => match value {
            "true" | "false" => Ok(value.to_owned()),
            _ => Err(invalid("esperava true ou false")),
        },
        Control::Spin { .. } => {
            let number: i64 = value
                .trim()
                .parse()
                .map_err(|_| invalid("esperava um número inteiro"))?;
            let (min, max) = effective_bounds(spec, limits).unwrap_or((i64::MIN, i64::MAX));
            if number < min || number > max {
                return Err(SettingError::OutOfRange {
                    name: name.to_owned(),
                    value: number,
                    min,
                    max,
                });
            }
            Ok(number.to_string())
        }
        Control::Combo { vars, .. } => {
            if vars.iter().any(|v| v == value) {
                Ok(value.to_owned())
            } else {
                Err(invalid("valor fora da lista da Engine"))
            }
        }
        Control::Text { .. } => {
            if is_path_like(name) {
                return Err(SettingError::Restricted(name.to_owned()));
            }
            if value.chars().count() > 256 {
                return Err(invalid("texto longo demais (máximo 256 caracteres)"));
            }
            Ok(value.to_owned())
        }
    }
}

/// Valida todas as escolhas e as devolve na ordem de envio (Threads, Hash, demais).
pub fn resolve_settings(
    specs: &[OptionSpec],
    limits: &Limits,
    settings: &BTreeMap<String, String>,
) -> Result<Vec<(String, String)>, SettingError> {
    let mut out = Vec::with_capacity(settings.len());
    for (name, value) in settings {
        out.push((name.clone(), validate_option(specs, limits, name, value)?));
    }
    let rank = |name: &str| match name.to_ascii_lowercase().as_str() {
        "threads" => 0,
        "hash" => 1,
        _ => 2,
    };
    out.sort_by_key(|(name, _)| rank(name));
    Ok(out)
}

/// Opções anunciadas já interpretadas (tipos desconhecidos são descartados).
pub fn parse_options(options: &[EngineOption]) -> Vec<OptionSpec> {
    options.iter().filter_map(parse_option).collect()
}
