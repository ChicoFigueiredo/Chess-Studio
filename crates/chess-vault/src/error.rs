//! Erros do Cofre, em português. Nenhuma variante carrega segredo, senha ou conteúdo do
//! arquivo: o texto pode ir para log e diagnóstico como está.

use std::fmt;

/// Tamanho mínimo da senha-mestra, em caracteres.
pub const MIN_PASSWORD_CHARS: usize = 8;

/// Por que o Cofre não fez o que se pediu.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VaultError {
    /// Não há cofre do sistema e o cofre próprio ainda não foi criado: nada foi gravado.
    Unavailable,
    /// O cofre próprio está bloqueado: desbloqueie com a senha-mestra.
    Locked,
    /// A operação é do cofre próprio, e o app usa o cofre do sistema (ou nenhum).
    NoOwnVault,
    /// O cofre próprio só é criado quando não há cofre do sistema e ainda não existe um.
    CannotCreateOwn,
    /// Senha-mestra errada.
    WrongPassword,
    /// Senha-mestra curta demais.
    WeakPassword { min_chars: usize },
    /// Segredo vazio.
    EmptySecret,
    /// Não há segredo guardado para a Conexão.
    NotFound,
    /// O arquivo do cofre próprio ou o índice foi alterado ou está corrompido. Nada é lido
    /// dele; os segredos que ele guardava não são recuperáveis sem a cópia íntegra.
    Corrupted { file: &'static str },
    /// Arquivo de uma versão mais nova do app.
    UnsupportedFormat { file: &'static str, version: u32 },
    /// O cofre do sistema falhou (mensagem fixa, sem dado do segredo).
    System { detail: &'static str },
    /// Falha de disco no diretório do Cofre.
    Io { detail: String },
}

impl VaultError {
    pub(crate) fn io(err: &std::io::Error) -> Self {
        Self::Io {
            detail: err.kind().to_string(),
        }
    }
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str(
                "Não há cofre do sistema disponível: crie o cofre próprio do Chess Studio, \
                 protegido por senha-mestra. Nada foi gravado.",
            ),
            Self::Locked => f.write_str(
                "O cofre do Chess Studio está bloqueado: desbloqueie-o com a senha-mestra.",
            ),
            Self::NoOwnVault => {
                f.write_str("O app usa o cofre do sistema: não há senha-mestra para isto.")
            }
            Self::CannotCreateOwn => f.write_str(
                "O cofre próprio só é criado quando o sistema não oferece cofre e ainda não \
                 existe um.",
            ),
            Self::WrongPassword => f.write_str("Senha-mestra errada."),
            Self::WeakPassword { min_chars } => write!(
                f,
                "A senha-mestra precisa ter pelo menos {min_chars} caracteres."
            ),
            Self::EmptySecret => f.write_str("O segredo está vazio."),
            Self::NotFound => f.write_str("Não há segredo guardado para esta Conexão."),
            Self::Corrupted { file } => write!(
                f,
                "O arquivo {file} do cofre foi alterado ou está corrompido; nada foi lido dele."
            ),
            Self::UnsupportedFormat { file, version } => write!(
                f,
                "O arquivo {file} do cofre é de uma versão mais nova do app (formato {version})."
            ),
            Self::System { detail } => write!(f, "Cofre do sistema: {detail}."),
            Self::Io { detail } => write!(f, "Falha de disco no cofre: {detail}."),
        }
    }
}

impl std::error::Error for VaultError {}
