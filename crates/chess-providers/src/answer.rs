//! O resultado com Origem ([`Answer`]) e o pedido pendente de uma consulta ([`Query`]).

use crate::error::{Missing, ProviderError};
use chess_net::{
    Clock, Fetched, Freshness, ImportMode, Net, Pending, Provenance, ProviderId, Request,
};
use std::fmt;
use std::mem;
use std::time::Duration;

/// Um resultado tipado com a sua Origem: de onde veio, quando, sob qual licença e quão
/// antigo é. Nenhum dado externo sai deste crate sem ela.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer<T> {
    pub value: T,
    pub provenance: Provenance,
    pub freshness: Freshness,
    /// Idade do conteúdo: agora menos `provenance.queried_at`.
    pub age: Duration,
}

impl<T> Answer<T> {
    pub(crate) fn from_fetched(value: T, fetched: &Fetched) -> Self {
        Self {
            value,
            provenance: fetched.provenance.clone(),
            freshness: fetched.freshness.clone(),
            age: fetched.age,
        }
    }

    /// Mesma Origem, outro valor (por exemplo, um trecho do resultado).
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        Answer {
            value: f(self.value),
            provenance: self.provenance,
            freshness: self.freshness,
            age: self.age,
        }
    }
}

type Decode<T> = Box<dyn FnOnce(&Fetched) -> Result<T, ProviderError> + Send>;
/// Resultado de um `404` que é válido (por exemplo, "sem avaliação").
type Absent<T> = Box<dyn FnOnce() -> Answer<T> + Send>;

/// O que falta para transformar a resposta da rede em [`Answer`].
struct Parts<T> {
    decode: Decode<T>,
    missing: Missing,
    absent: Option<Absent<T>>,
}

enum State<T> {
    Waiting { pending: Pending, parts: Parts<T> },
    Ready(Option<Result<Answer<T>, ProviderError>>),
    Taken,
}

/// Consulta a caminho. Consulte a cada quadro com [`Query::try_take`] (nunca bloqueia); o
/// resultado é entregue uma vez. Descartar a `Query` cancela o pedido.
pub struct Query<T> {
    state: State<T>,
}

impl<T> fmt::Debug for Query<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match &self.state {
            State::Waiting { .. } => "pendente",
            State::Ready(Some(_)) => "pronta",
            State::Ready(None) | State::Taken => "entregue",
        };
        f.debug_struct("Query").field("estado", &state).finish()
    }
}

impl<T> Query<T> {
    pub(crate) fn waiting(pending: Pending, missing: Missing, decode: Decode<T>) -> Self {
        Self::waiting_or_absent(pending, missing, decode, None)
    }

    /// Como [`Query::waiting`], mas um `404` vira o resultado `absent` em vez de erro.
    pub(crate) fn waiting_or_absent(
        pending: Pending,
        missing: Missing,
        decode: Decode<T>,
        absent: Option<Absent<T>>,
    ) -> Self {
        Self {
            state: State::Waiting {
                pending,
                parts: Parts {
                    decode,
                    missing,
                    absent,
                },
            },
        }
    }

    /// Consulta já resolvida (argumento inválido, cache negativo): nenhum pedido sai.
    pub(crate) fn ready(result: Result<Answer<T>, ProviderError>) -> Self {
        Self {
            state: State::Ready(Some(result)),
        }
    }

    pub(crate) fn failed(error: ProviderError) -> Self {
        Self::ready(Err(error))
    }

    fn finish(
        result: Result<Fetched, chess_net::NetError>,
        parts: Parts<T>,
    ) -> Result<Answer<T>, ProviderError> {
        match result {
            Ok(fetched) => {
                let value = (parts.decode)(&fetched)?;
                Ok(Answer::from_fetched(value, &fetched))
            }
            Err(chess_net::NetError::Http {
                status: 404 | 410, ..
            }) if parts.absent.is_some() => match parts.absent {
                Some(absent) => Ok(absent()),
                None => Err(ProviderError::NoData),
            },
            Err(error) => Err(parts.missing.translate(error)),
        }
    }

    /// O resultado, se já chegou (nunca bloqueia). Entregue uma vez.
    pub fn try_take(&mut self) -> Option<Result<Answer<T>, ProviderError>> {
        match mem::replace(&mut self.state, State::Taken) {
            State::Waiting { mut pending, parts } => match pending.try_take() {
                None => {
                    self.state = State::Waiting { pending, parts };
                    None
                }
                Some(result) => Some(Self::finish(result, parts)),
            },
            State::Ready(result) => result,
            State::Taken => None,
        }
    }

    /// Espera até `timeout` (tempo real). Para threads de fundo e testes, nunca para a
    /// interface.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<Result<Answer<T>, ProviderError>> {
        match mem::replace(&mut self.state, State::Taken) {
            State::Waiting { mut pending, parts } => match pending.wait_timeout(timeout) {
                None => {
                    self.state = State::Waiting { pending, parts };
                    None
                }
                Some(result) => Some(Self::finish(result, parts)),
            },
            State::Ready(result) => result,
            State::Taken => None,
        }
    }

    /// Bloqueia até o resultado. Para threads de fundo e testes, nunca para a interface.
    pub fn wait(mut self) -> Result<Answer<T>, ProviderError> {
        loop {
            if let Some(result) = self.wait_timeout(Duration::from_secs(3600)) {
                return result;
            }
            if matches!(self.state, State::Taken) {
                return Err(ProviderError::Cancelled);
            }
        }
    }

    /// Já entregou o resultado (ou nunca o terá).
    pub fn is_done(&self) -> bool {
        matches!(self.state, State::Taken | State::Ready(None))
    }

    /// Cancela: um pedido na fila nem sai; um em andamento é abandonado.
    pub fn cancel(&self) {
        if let State::Waiting { pending, .. } = &self.state {
            pending.cancel();
        }
    }
}

/// Envia `request` pelo gate e prepara a decodificação tipada do corpo.
pub(crate) fn fetch<T: 'static>(
    net: &Net,
    request: Request,
    missing: Missing,
    decode: impl FnOnce(&Fetched) -> Result<T, ProviderError> + Send + 'static,
) -> Query<T> {
    Query::waiting(net.fetch(request), missing, Box::new(decode))
}

/// Interpreta o corpo como JSON do tipo `W`; formato diferente do documentado vira
/// [`ProviderError::Malformed`].
pub(crate) fn json<W: serde::de::DeserializeOwned>(
    provider: ProviderId,
    fetched: &Fetched,
) -> Result<W, ProviderError> {
    serde_json::from_slice(&fetched.body).map_err(|e| ProviderError::malformed(provider, e))
}

/// Origem de uma resposta em fluxo (que não passa pelo cache): licença do catálogo para o
/// endereço, instante do relógio injetado.
pub(crate) fn stream_provenance(
    provider: ProviderId,
    host: &str,
    path: &str,
    clock: &dyn Clock,
) -> Provenance {
    Provenance {
        provider,
        queried_at: clock.now(),
        license: license_of(provider, host, path),
        mode: ImportMode::Api,
        source: format!("https://{host}{path}"),
    }
}

/// Licença do endereço no catálogo (prefixo mais longo).
pub(crate) fn license_of(provider: ProviderId, host: &str, path: &str) -> String {
    chess_net::provider(provider)
        .endpoints
        .iter()
        .filter(|e| e.host == host && path.starts_with(e.path_prefix))
        .max_by_key(|e| e.path_prefix.len())
        .map(|e| e.license.to_owned())
        .unwrap_or_default()
}

/// Nome de usuário seguro para ir no caminho do endereço: letras, números, `_` e `-`.
pub(crate) fn valid_name(name: &str) -> Result<&str, ProviderError> {
    let name = name.trim();
    let ok = !name.is_empty()
        && name.len() <= 40
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(name)
    } else {
        Err(ProviderError::invalid(
            "nome de usuário vazio ou com caracteres fora de letras, números, «_» e «-»",
        ))
    }
}
