//! Consentimento para executar uma Engine (B25; ameaças T5 e T11 de
//! `docs/decisoes/seguranca-engines.md`, ADR-0003).
//!
//! Nenhum processo de Engine sobe sem uma destas autorizações: o caminho está registrado
//! e validado no banco (gerenciador de Engines), foi escolhido no assistente/gerenciador
//! (que já fazem o handshake), ou o usuário confirmou o aviso de execução e a sonda UCI
//! passou. Esta lógica não depende do Bevy: a interface só chama e desenha o estado.

use std::path::{Path, PathBuf};

/// O que fazer com a Engine quando o caminho for liberado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Começar uma Partida nova contra a Engine. A Nova Partida só aceita Engine já liberada
    /// (assistente ou gerenciador); a intenção segue no modelo, coberta pelos testes.
    #[cfg_attr(not(test), allow(dead_code))]
    NewGame,
    /// Religar a Engine de uma Partida reaberta.
    Resume,
    /// Trocar a Engine de uma Partida em curso (após falha).
    Replace,
}

/// Etapa da liberação de um caminho ainda não validado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Mostrando o aviso de que o arquivo será executado.
    Confirming,
    /// Confirmado; a sonda UCI roda numa thread.
    Probing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub path: PathBuf,
    pub intent: Intent,
    pub stage: Stage,
}

/// Resultado de [`Consent::request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Nenhum caminho informado: abrir o gerenciador/assistente.
    Unconfigured,
    /// Já liberado: pode subir o processo.
    Ready(PathBuf, Intent),
    /// Falta o aviso de execução e a sonda (veja [`Consent::launch`]).
    Pending,
}

/// Estado do consentimento da sessão do app.
#[derive(Debug, Default)]
pub struct Consent {
    cleared: Option<PathBuf>,
    launch: Option<Launch>,
    error: Option<String>,
}

impl Consent {
    /// Marca um caminho como liberado (escolha no assistente ou no gerenciador, que já
    /// validaram por handshake).
    pub fn clear(&mut self, path: &Path) {
        self.cleared = Some(path.to_path_buf());
        self.launch = None;
        self.error = None;
    }

    /// O caminho foi liberado por uma ação do usuário nesta sessão?
    pub fn is_cleared(&self, path: &Path) -> bool {
        self.cleared.as_deref() == Some(path)
    }

    /// Liberação em andamento (aviso ou sonda), se houver.
    pub fn launch(&self) -> Option<&Launch> {
        self.launch.as_ref()
    }

    /// Motivo da última recusa da sonda.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Pedido do usuário para usar a Engine em `text`. `is_registered` diz se o caminho é
    /// de uma Engine registrada e pronta no banco.
    pub fn request(
        &mut self,
        text: &str,
        intent: Intent,
        is_registered: impl Fn(&Path) -> bool,
    ) -> Request {
        let text = text.trim();
        if text.is_empty() {
            return Request::Unconfigured;
        }
        let path = PathBuf::from(text);
        self.error = None;
        if self.is_cleared(&path) || is_registered(&path) {
            self.launch = None;
            return Request::Ready(path, intent);
        }
        self.launch = Some(Launch {
            path,
            intent,
            stage: Stage::Confirming,
        });
        Request::Pending
    }

    /// O usuário aceitou o aviso: devolve o caminho a sondar (nada executou antes disto).
    pub fn confirm(&mut self) -> Option<PathBuf> {
        let launch = self.launch.as_mut()?;
        if launch.stage != Stage::Confirming {
            return None;
        }
        launch.stage = Stage::Probing;
        Some(launch.path.clone())
    }

    /// O usuário desistiu do aviso.
    pub fn cancel(&mut self) {
        if self.launch.as_ref().map(|l| l.stage) == Some(Stage::Confirming) {
            self.launch = None;
        }
    }

    /// Resultado da sonda. Com sucesso o caminho é liberado e a ação pendente devolvida.
    pub fn probed(&mut self, result: Result<(), String>) -> Option<(PathBuf, Intent)> {
        let launch = self.launch.take()?;
        match result {
            Ok(()) => {
                self.cleared = Some(launch.path.clone());
                Some((launch.path, launch.intent))
            }
            Err(message) => {
                self.error = Some(message);
                None
            }
        }
    }
}

/// Engine que a análise pode subir. A escolhida entre as do gerenciador vale se está na
/// lista de registradas e prontas (`registered`) ou é o caminho já liberado nesta sessão;
/// sem escolha, só o caminho liberado (assistente, gerenciador ou aviso + sonda). Um
/// caminho vindo do ambiente (`CHESS_STUDIO_ENGINE`/`PATH`) não entra por aqui: só depois
/// de liberado pelo fluxo de consentimento.
pub fn analysis_engine(
    chosen: Option<&Path>,
    cleared: Option<&Path>,
    registered: &[PathBuf],
) -> Option<PathBuf> {
    match chosen {
        Some(path) => (cleared == Some(path) || registered.iter().any(|r| r == path))
            .then(|| path.to_path_buf()),
        None => cleared.map(Path::to_path_buf),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nobody(_: &Path) -> bool {
        false
    }

    #[test]
    fn empty_path_is_unconfigured_and_never_launches() {
        let mut c = Consent::default();
        assert_eq!(
            c.request("  ", Intent::NewGame, nobody),
            Request::Unconfigured
        );
        assert!(c.launch().is_none());
    }

    #[test]
    fn typed_path_asks_for_confirmation_before_anything_runs() {
        let mut c = Consent::default();
        assert_eq!(
            c.request("/x/eng", Intent::NewGame, nobody),
            Request::Pending
        );
        let launch = c.launch().unwrap();
        assert_eq!(launch.stage, Stage::Confirming);
        assert_eq!(launch.path, PathBuf::from("/x/eng"));
        assert!(!c.is_cleared(Path::new("/x/eng")));
    }

    #[test]
    fn declining_the_warning_drops_the_launch() {
        let mut c = Consent::default();
        c.request("/x/eng", Intent::Resume, nobody);
        c.cancel();
        assert!(c.launch().is_none());
        assert!(!c.is_cleared(Path::new("/x/eng")));
    }

    #[test]
    fn confirmed_and_probed_path_is_released_with_its_intent() {
        let mut c = Consent::default();
        c.request("/x/eng", Intent::Resume, nobody);
        assert_eq!(c.confirm(), Some(PathBuf::from("/x/eng")));
        assert_eq!(c.launch().unwrap().stage, Stage::Probing);
        assert_eq!(c.confirm(), None, "a sonda não repete");
        assert_eq!(
            c.probed(Ok(())),
            Some((PathBuf::from("/x/eng"), Intent::Resume))
        );
        // Depois de liberado, o mesmo caminho não pergunta de novo.
        assert_eq!(
            c.request("/x/eng", Intent::Replace, nobody),
            Request::Ready(PathBuf::from("/x/eng"), Intent::Replace)
        );
    }

    #[test]
    fn failed_probe_keeps_the_path_blocked() {
        let mut c = Consent::default();
        c.request("/x/eng", Intent::NewGame, nobody);
        c.confirm();
        assert_eq!(c.probed(Err("sem UCI".into())), None);
        assert_eq!(c.error(), Some("sem UCI"));
        assert!(!c.is_cleared(Path::new("/x/eng")));
        assert_eq!(
            c.request("/x/eng", Intent::NewGame, nobody),
            Request::Pending
        );
        assert_eq!(c.error(), None);
    }

    #[test]
    fn registered_engine_needs_no_confirmation() {
        let mut c = Consent::default();
        let registered = |p: &Path| p == Path::new("/db/stockfish");
        assert_eq!(
            c.request("/db/stockfish", Intent::NewGame, registered),
            Request::Ready(PathBuf::from("/db/stockfish"), Intent::NewGame)
        );
        // Outro caminho, mesmo parecido, continua exigindo o aviso.
        assert_eq!(
            c.request("/db/stockfish2", Intent::NewGame, registered),
            Request::Pending
        );
    }

    #[test]
    fn choosing_in_the_wizard_or_manager_clears_the_path() {
        let mut c = Consent::default();
        c.clear(Path::new("/w/eng"));
        assert_eq!(
            c.request("/w/eng", Intent::NewGame, nobody),
            Request::Ready(PathBuf::from("/w/eng"), Intent::NewGame)
        );
    }

    #[test]
    fn analysis_ignores_an_engine_that_was_never_cleared() {
        assert_eq!(analysis_engine(None, None, &[]), None);
        let registered = [PathBuf::from("/db/sf")];
        assert_eq!(analysis_engine(None, None, &registered), None);
        assert_eq!(
            analysis_engine(Some(Path::new("/env/sf")), None, &registered),
            None
        );
    }

    #[test]
    fn analysis_uses_cleared_or_registered_paths() {
        let registered = [PathBuf::from("/db/sf")];
        assert_eq!(
            analysis_engine(None, Some(Path::new("/w/eng")), &registered),
            Some(PathBuf::from("/w/eng"))
        );
        assert_eq!(
            analysis_engine(Some(Path::new("/db/sf")), None, &registered),
            Some(PathBuf::from("/db/sf"))
        );
        assert_eq!(
            analysis_engine(Some(Path::new("/w/eng")), Some(Path::new("/w/eng")), &[]),
            Some(PathBuf::from("/w/eng"))
        );
    }
}
