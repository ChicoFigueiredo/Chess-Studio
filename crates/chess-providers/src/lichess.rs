//! Cliente do Lichess. Cada consulta fica num módulo: [`crate::explorer`] (Explorador),
//! [`crate::eval`] (avaliação em cache), [`crate::tablebase`] (finais), [`crate::games`]
//! (Partidas públicas) e [`crate::profile`] (perfil e FIDE). Aqui ficam o cliente e as
//! constantes de endereço.

use chess_net::{Clock, Net, ProviderId, Timestamp};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

pub(crate) const PROVIDER: ProviderId = ProviderId::Lichess;
pub(crate) const EXPLORER_HOST: &str = "explorer.lichess.org";
pub(crate) const API_HOST: &str = "lichess.org";
pub(crate) const TABLEBASE_HOST: &str = "tablebase.lichess.ovh";

/// "Sem avaliação" lembrado por pouco tempo, para a interface consultar por quadro sem
/// repetir o pedido (404 não entra no cache da camada de rede, que guarda só `200`).
#[derive(Debug, Clone)]
pub(crate) struct NoEval {
    pub(crate) until: Timestamp,
    pub(crate) provenance: chess_net::Provenance,
}

/// Cliente tipado do Lichess sobre `chess_net::Net`. Barato de clonar; os clones compartilham
/// a rede e a memória do "sem avaliação".
#[derive(Clone)]
pub struct Lichess {
    pub(crate) net: Net,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) no_eval: Arc<Mutex<HashMap<String, NoEval>>>,
}

impl fmt::Debug for Lichess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lichess").finish_non_exhaustive()
    }
}

impl Lichess {
    /// `clock` é o relógio injetado (o mesmo da `Net`): nele se baseiam a Origem das respostas
    /// em fluxo e o prazo do "sem avaliação".
    pub fn new(net: Net, clock: Arc<dyn Clock>) -> Self {
        Self {
            net,
            clock,
            no_eval: Arc::default(),
        }
    }
}
