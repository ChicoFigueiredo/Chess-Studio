//! O modelo da Central de Conexões, sem Bevy nem egui (testável): como os Provedores do
//! catálogo se ordenam e agrupam, em que estado cada cartão está, quais ações e campos ele
//! mostra e o que "permite agora" (sem prometer o futuro).
//!
//! Regras de produto (CX-05, CX-06, CX-09; especificação 4.1 e 4.2; #225):
//!
//! - gratuitos primeiro, agrupados por categoria (jogar, dados, Engines, importação, ratings);
//!   dentro de cada categoria, o que já funciona, depois "Em breve", por último "Requer
//!   aprovação";
//! - "Autorizado" só existe com uma Conexão autorizada de verdade (segredo no Cofre, aberto, e
//!   não recusada pelo Provedor); "Ligado" nunca se apresenta como conectado;
//! - Provedor sem API ou sem aprovação nunca mostra campo de segredo nem botão de autorização;
//! - o cartão mostra uma ação principal; as demais vão no menu "⋯";
//! - nenhum texto da tela vem do catálogo: as notas técnicas de `chess-net` são para quem
//!   desenvolve, e a interface usa os textos próprios em Fluent.

use crate::secrets::Standing;
use chess_net::{
    AuthMethod, Capability, Category, Cost, Operation, Provider, ProviderId, ProviderState,
    ServiceStatus, Timestamp, catalog,
};
use chess_store::ConnectionKind;
use std::collections::HashMap;

/// A ordem das categorias na Central.
pub const CATEGORY_ORDER: [Category; 5] = [
    Category::OnlinePlay,
    Category::Data,
    Category::Engine,
    Category::FileImport,
    Category::Ratings,
];

/// Id Fluent do título da categoria.
pub fn category_key(category: Category) -> &'static str {
    match category {
        Category::OnlinePlay => "connections-cat-play",
        Category::Data => "connections-cat-data",
        Category::Engine => "connections-cat-engine",
        Category::FileImport => "connections-cat-import",
        Category::Ratings => "connections-cat-ratings",
    }
}

/// Filtro por custo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CostFilter {
    #[default]
    All,
    /// Gratuitos (com ou sem cadastro, com ou sem limites).
    Free,
    /// Pagos, sob aprovação ou com custo não confirmado.
    Paid,
}

impl CostFilter {
    pub const ALL: [CostFilter; 3] = [CostFilter::All, CostFilter::Free, CostFilter::Paid];

    pub fn matches(self, cost: Cost) -> bool {
        let free = cost <= Cost::FreeLimited;
        match self {
            Self::All => true,
            Self::Free => free,
            Self::Paid => !free,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::All => "connections-filter-all",
            Self::Free => "connections-filter-free",
            Self::Paid => "connections-filter-paid",
        }
    }
}

/// Uma categoria com os Provedores que passaram nos filtros.
#[derive(Clone, Debug)]
pub struct Group {
    pub category: Category,
    pub providers: Vec<&'static Provider>,
}

/// O Provedor responde à busca (nome, chave e nome das capacidades; sem distinguir maiúsculas
/// nem acentos simples).
pub fn matches_query(provider: &Provider, query: &str) -> bool {
    let needle = fold(query.trim());
    if needle.is_empty() {
        return true;
    }
    fold(provider.name).contains(&needle)
        || provider.id.key().contains(&needle)
        || provider
            .services
            .iter()
            .any(|s| fold(s.capability.label()).contains(&needle))
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'õ' | 'ô' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Os Provedores do catálogo que passam nos filtros, por categoria na ordem da Central e,
/// dentro de cada uma, o que funciona antes do que vem em breve e do que requer aprovação e, em cada faixa,
/// **gratuitos primeiro** (custo crescente; a ordem do catálogo desempata).
pub fn groups(filter: CostFilter, query: &str) -> Vec<Group> {
    CATEGORY_ORDER
        .into_iter()
        .filter_map(|category| {
            let mut providers: Vec<&'static Provider> = catalog()
                .iter()
                .filter(|p| p.category == category)
                .filter(|p| filter.matches(p.cost()) && matches_query(p, query))
                .collect();
            // Estável: o desempate é a ordem do catálogo.
            providers.sort_by_key(|p| (tier_of(p), p.cost()));
            (!providers.is_empty()).then_some(Group {
                category,
                providers,
            })
        })
        .collect()
}

/// Quantos Provedores cada filtro mostraria (sem a busca): o detalhe dos cartões de filtro.
pub fn count(filter: CostFilter) -> usize {
    catalog()
        .iter()
        .filter(|p| filter.matches(p.cost()))
        .count()
}

/// O estado do cartão (a palavra do selo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardState {
    /// Provedor de rede, desligado.
    Off,
    /// Ligado, sem nome vinculado.
    On,
    /// Ligado e com um nome ou ID público vinculado (não é uma Conexão autorizada).
    Linked,
    /// Ligado e com uma Conexão **autorizada**: o único estado que pode dizer isso.
    Authorized,
    /// A autorização existe, mas o Provedor a recusou (expirada ou revogada) ou o segredo
    /// sumiu: reautorizar. Os dados locais ficam.
    Expired,
    /// A autorização existe, mas o cofre do Chess Studio está bloqueado.
    Locked,
    /// O Provedor pediu uma pausa.
    Limited,
    /// A última tentativa falhou.
    Failing,
    /// Sem API: só importação de arquivo.
    ManualImport,
    /// Pago ou sob aprovação do Provedor: nada sai para a rede.
    NeedsApproval,
    /// O adaptador ainda não existe.
    Planned,
    /// API ou termos ainda não confirmados.
    Unconfirmed,
}

/// A faixa do cartão dentro da categoria.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// O que já funciona: cartão completo.
    Working,
    /// Em breve: cartão menor.
    Soon,
    /// Requer aprovação: cartão menor.
    Approval,
}

impl Tier {
    /// Id Fluent do título da faixa (só as faixas menores têm título).
    pub fn key(self) -> Option<&'static str> {
        match self {
            Self::Working => None,
            Self::Soon => Some("connections-tier-soon"),
            Self::Approval => Some("connections-tier-approval"),
        }
    }
}

impl CardState {
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "connections-state-off",
            Self::On => "connections-state-on",
            Self::Linked => "connections-state-linked",
            Self::Authorized => "connections-state-authorized",
            Self::Expired => "connections-state-expired",
            Self::Locked => "connections-state-locked",
            Self::Limited => "connections-state-limited",
            Self::Failing => "connections-state-failing",
            Self::ManualImport => "connections-state-manual",
            Self::NeedsApproval => "connections-state-approval",
            Self::Planned => "connections-state-soon",
            Self::Unconfirmed => "connections-state-evaluating",
        }
    }

    /// Id Fluent da explicação do estado (tooltip do selo).
    pub fn hint_key(self) -> &'static str {
        match self {
            Self::Off => "connections-hint-off",
            Self::On => "connections-hint-on",
            Self::Linked => "connections-hint-linked",
            Self::Authorized => "connections-hint-authorized",
            Self::Expired => "connections-hint-expired",
            Self::Locked => "connections-hint-locked",
            Self::Limited => "connections-hint-limited",
            Self::Failing => "connections-hint-failing",
            Self::ManualImport => "connections-hint-manual",
            Self::NeedsApproval => "connections-hint-approval",
            Self::Planned => "connections-hint-planned",
            Self::Unconfirmed => "connections-hint-unconfirmed",
        }
    }

    pub fn tier(self) -> Tier {
        match self {
            Self::Planned | Self::Unconfirmed => Tier::Soon,
            Self::NeedsApproval => Tier::Approval,
            _ => Tier::Working,
        }
    }

    /// Cartões que nunca falam com a rede.
    pub fn is_informational(self) -> bool {
        matches!(
            self,
            Self::ManualImport | Self::NeedsApproval | Self::Unconfirmed
        )
    }

    /// Cartão menor, sem Switch, sem detalhes e sem ação principal.
    pub fn is_compact(self) -> bool {
        self.tier() != Tier::Working
    }
}

/// A primeira etapa em que o Provedor ganha um serviço de rede (só os "planejados").
pub fn earliest_stage(provider: &Provider) -> Option<u8> {
    provider
        .services
        .iter()
        .filter(|s| s.capability.uses_network())
        .filter_map(|s| match s.status {
            ServiceStatus::Planned { stage } => Some(stage),
            _ => None,
        })
        .min()
}

/// A partir desta etapa o Provedor aparece como "Em breve" e não se liga ainda.
const PLANNED_FROM_STAGE: u8 = 3;

/// O Provedor tem algo a fazer hoje: vincular nome ou ID público, ou um serviço pronto. Sem
/// isso o Switch não leva a nada e o cartão fica em "Em breve".
fn has_use_today(provider: &Provider) -> bool {
    has_auth(provider, &[AuthMethod::PublicId])
        || provider
            .services
            .iter()
            .any(|s| s.status == ServiceStatus::Ready)
}

/// O estado do cartão a partir do catálogo, do estado da rede, de haver nome vinculado e de
/// haver Conexão autorizada. `authorized` só vale com o Provedor ligado.
pub fn card_state(
    provider: &Provider,
    net: &ProviderState,
    has_connection: bool,
    authorized: bool,
) -> CardState {
    card_state_with(
        provider,
        net,
        has_connection,
        authorized.then_some(Standing::Active),
    )
}

/// Como [`card_state`], com a situação da melhor autorização do Provedor (`None`: nenhuma
/// Conexão autorizada). Só o Provedor ligado mostra "Autorizado"; a autorização recusada ou com
/// o cofre bloqueado aparece como tal.
pub fn card_state_with(
    provider: &Provider,
    net: &ProviderState,
    has_connection: bool,
    auth: Option<Standing>,
) -> CardState {
    if !provider.uses_network() {
        let services = provider.services;
        if services
            .iter()
            .any(|s| s.status == ServiceStatus::ManualImport)
        {
            return CardState::ManualImport;
        }
        if services.iter().any(|s| {
            matches!(
                s.status,
                ServiceStatus::NeedsApproval | ServiceStatus::PaidOptional
            )
        }) {
            return CardState::NeedsApproval;
        }
        return CardState::Unconfirmed;
    }
    match net {
        ProviderState::Limited { .. } => CardState::Limited,
        ProviderState::Failing { .. } => CardState::Failing,
        ProviderState::Available => match auth {
            Some(Standing::Active) => CardState::Authorized,
            Some(Standing::Locked) => CardState::Locked,
            Some(Standing::Expired | Standing::Unusable) => CardState::Expired,
            None if has_connection => CardState::Linked,
            None => CardState::On,
        },
        ProviderState::Disabled => {
            if has_use_today(provider)
                && earliest_stage(provider).is_none_or(|stage| stage < PLANNED_FROM_STAGE)
            {
                CardState::Off
            } else {
                CardState::Planned
            }
        }
    }
}

/// A faixa do Provedor, independente do estado da rede.
pub fn tier_of(provider: &Provider) -> Tier {
    card_state(provider, &ProviderState::Disabled, false, false).tier()
}

/// O Switch "Ligado" existe só nos cartões completos de Provedor de rede.
pub fn has_switch(provider: &Provider, state: CardState) -> bool {
    provider.uses_network() && !state.is_compact()
}

/// Ações de um cartão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
    /// Vincular nome ou ID público (uma Conexão do Perfil local).
    Link,
    Test,
    ClearCache,
    /// Autorizar pelo navegador.
    Authorize,
    /// Autorizar de novo a Conexão expirada ou recusada pelo Provedor.
    Reauthorize,
    /// Desbloquear o cofre do Chess Studio (as autorizações voltam).
    Unlock,
    /// Inserir uma chave pessoal (fluxo avançado).
    EnterKey,
    /// Abrir a entrada de Engine online, separada da de jogador.
    EngineOnline,
    OpenDocs,
    OpenTerms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionSpec {
    pub action: CardAction,
    pub enabled: bool,
    /// Id Fluent do motivo de estar desabilitada agora (tooltip, em linguagem de usuário).
    pub reason_key: Option<&'static str>,
}

/// A ação principal do cartão e as demais, que vão para o menu "⋯".
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Actions {
    pub primary: Option<ActionSpec>,
    pub menu: Vec<ActionSpec>,
}

/// O Provedor cujo adaptador responde ao "Testar" da Conexão por nome ou ID público. O FIDE
/// não tem API própria: o dado vem do Lichess (ver [`test_via`]).
pub fn can_test(provider: ProviderId) -> bool {
    matches!(
        provider,
        ProviderId::Lichess | ProviderId::ChessCom | ProviderId::Fide
    )
}

/// O Provedor que de fato recebe o pedido do teste (e que precisa estar ligado).
pub fn test_via(provider: ProviderId) -> ProviderId {
    match provider {
        ProviderId::Fide => ProviderId::Lichess,
        other => other,
    }
}

fn has_auth(provider: &Provider, wanted: &[AuthMethod]) -> bool {
    provider
        .auth_methods()
        .iter()
        .any(|method| wanted.contains(method))
}

/// A Conexão por nome ou ID público está disponível neste cartão.
pub fn can_link(provider: &Provider, state: CardState) -> bool {
    has_auth(provider, &[AuthMethod::PublicId])
        && !matches!(
            state,
            CardState::Planned | CardState::NeedsApproval | CardState::Unconfirmed
        )
}

/// O Provedor tem autorização pelo navegador e chave pessoal que o app sabe fazer (hoje só o
/// Lichess; os outros mostram as ações desabilitadas, com o motivo).
pub fn can_authorize(provider: &Provider) -> bool {
    provider.id == ProviderId::Lichess
}

/// O Provedor tem Engine online com autorização própria, separada da do jogador.
pub fn has_online_engine(provider: &Provider) -> bool {
    can_authorize(provider)
        && provider
            .services
            .iter()
            .any(|s| s.capability == Capability::OnlineEngine)
}

/// As ações do cartão: uma principal (a que a pessoa mais provavelmente quer agora) e o resto no
/// menu. Cartões menores só levam instruções e termos no menu.
pub fn card_actions(provider: &Provider, state: CardState, has_connection: bool) -> Actions {
    let ready = |action| ActionSpec {
        action,
        enabled: true,
        reason_key: None,
    };
    let soon = |action, reason_key| ActionSpec {
        action,
        enabled: false,
        reason_key: Some(reason_key),
    };
    let docs = !provider.docs_url.is_empty();
    let terms = !provider.terms_url.is_empty();
    let mut out = Actions::default();
    if state.is_compact() {
        if docs {
            out.menu.push(ready(CardAction::OpenDocs));
        }
        if terms {
            out.menu.push(ready(CardAction::OpenTerms));
        }
        return out;
    }
    let linkable = can_link(provider, state);
    let authorizes = can_authorize(provider) && !state.is_informational();
    let testable = linkable && has_connection && can_test(provider.id);
    out.primary = match state {
        // A autorização é o que o Lichess mais precisa: ela vem antes de vincular um nome.
        CardState::Authorized if testable => Some(ready(CardAction::Test)),
        CardState::Expired if authorizes => Some(ready(CardAction::Reauthorize)),
        CardState::Locked if authorizes => Some(ready(CardAction::Unlock)),
        CardState::Authorized | CardState::Expired | CardState::Locked => None,
        _ if authorizes => Some(ready(CardAction::Authorize)),
        _ if testable => Some(ready(CardAction::Test)),
        _ if linkable && !has_connection => Some(ready(CardAction::Link)),
        _ if !provider.uses_network() && docs => Some(ready(CardAction::OpenDocs)),
        _ => None,
    };
    let primary = out.primary.map(|spec| spec.action);
    if testable && primary != Some(CardAction::Test) {
        out.menu.push(ready(CardAction::Test));
    }
    if linkable && primary != Some(CardAction::Link) {
        out.menu.push(ready(CardAction::Link));
    }
    if provider.uses_network() {
        out.menu.push(ready(CardAction::ClearCache));
    }
    if !state.is_informational() {
        if has_auth(provider, &[AuthMethod::PersonalToken, AuthMethod::ApiKey]) {
            out.menu.push(if can_authorize(provider) {
                ready(CardAction::EnterKey)
            } else {
                soon(CardAction::EnterKey, "connections-reason-key")
            });
        }
        if has_auth(provider, &[AuthMethod::OAuth2Pkce]) && !can_authorize(provider) {
            out.menu
                .push(soon(CardAction::Authorize, "connections-reason-authorize"));
        }
        if has_online_engine(provider) {
            out.menu.push(ready(CardAction::EngineOnline));
        }
    }
    if docs && primary != Some(CardAction::OpenDocs) {
        out.menu.push(ready(CardAction::OpenDocs));
    }
    if terms {
        out.menu.push(ready(CardAction::OpenTerms));
    }
    out
}

/// Um campo de entrada que o cartão pode mostrar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    PublicName,
    PublicId,
    Nickname,
    /// Campo de segredo (chave pessoal): mascarado, nunca em log nem em área de transferência.
    /// Só no formulário da chave pessoal do Lichess (nunca no de vincular nome), e nunca em
    /// Provedor sem API ou sob aprovação.
    Secret,
}

/// Como a Conexão deste Provedor se identifica: FIDE por ID, os demais por nome.
pub fn connection_kind(provider: &Provider) -> ConnectionKind {
    if provider.id == ProviderId::Fide {
        ConnectionKind::PublicId
    } else {
        ConnectionKind::PublicName
    }
}

/// Os campos do formulário de vincular nome ou ID público. Nunca `Secret`: a chave pessoal tem
/// o próprio formulário ([`key_fields`]).
pub fn card_fields(provider: &Provider, state: CardState) -> Vec<FieldKind> {
    if !can_link(provider, state) {
        return Vec::new();
    }
    vec![
        match connection_kind(provider) {
            ConnectionKind::PublicId => FieldKind::PublicId,
            _ => FieldKind::PublicName,
        },
        FieldKind::Nickname,
    ]
}

/// Os campos do formulário da chave pessoal: só o campo de segredo, mascarado.
pub fn key_fields(provider: &Provider, state: CardState) -> Vec<FieldKind> {
    if can_authorize(provider) && !state.is_informational() && !state.is_compact() {
        vec![FieldKind::Secret]
    } else {
        Vec::new()
    }
}

/// Id Fluent do nome do serviço, como substantivo ("Explorador", "Finais exatos").
pub fn service_key(capability: Capability) -> &'static str {
    match capability {
        Capability::Play => "connections-svc-play",
        Capability::OnlineEngine => "connections-svc-online-engine",
        Capability::Explore => "connections-svc-explore",
        Capability::Endgames => "connections-svc-endgames",
        Capability::CachedEval => "connections-svc-cached-eval",
        Capability::Analyze => "connections-svc-analyze",
        Capability::Query => "connections-svc-query",
        Capability::Download => "connections-svc-download",
        Capability::ImportPgn => "connections-svc-import-pgn",
        Capability::IdentityOnly => "connections-svc-identity",
    }
}

/// Id Fluent do nome do serviço, com a situação: o que se importa de arquivo baixado é
/// "Arquivos baixados por você", não "Pacotes e bases".
pub fn service_label_key(service: &chess_net::Service) -> &'static str {
    match (service.capability, service.status) {
        (Capability::Download | Capability::ImportPgn, ServiceStatus::ManualImport) => {
            "connections-svc-manual-file"
        }
        (capability, _) => service_key(capability),
    }
}

/// Id Fluent do verbo do serviço, para "o que você pode fazer" (infinitivo, sem maiúscula).
pub fn verb_key(capability: Capability, status: ServiceStatus) -> &'static str {
    match (capability, status) {
        (Capability::Download | Capability::ImportPgn, ServiceStatus::ManualImport) => {
            "connections-do-import-file"
        }
        (Capability::Play, _) => "connections-do-play",
        (Capability::OnlineEngine, _) => "connections-do-online-engine",
        (Capability::Explore, _) => "connections-do-explore",
        (Capability::Endgames, _) => "connections-do-endgames",
        (Capability::CachedEval, _) => "connections-do-cached-eval",
        (Capability::Analyze, _) => "connections-do-analyze",
        (Capability::Query, _) => "connections-do-query",
        (Capability::Download, _) => "connections-do-download",
        (Capability::ImportPgn, _) => "connections-do-import-pgn",
        (Capability::IdentityOnly, _) => "connections-do-link",
    }
}

/// O que o cartão diz em "O que você pode fazer agora".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Face {
    /// Verbos (ids Fluent), numa frase curta.
    Can(Vec<&'static str>),
    /// Nada ainda: o texto próprio do estado (id Fluent).
    Note(&'static str),
}

/// O que o Provedor permite **agora**: vincular nome, testar e os serviços prontos ou de
/// importação manual (o Explorador do Lichess, só depois de autorizar). O que ainda não existe fica em [`later`].
pub fn face(provider: &Provider, state: CardState) -> Face {
    match state {
        CardState::Planned => return Face::Note("connections-face-planned"),
        CardState::Unconfirmed => return Face::Note("connections-face-unconfirmed"),
        CardState::NeedsApproval => return Face::Note("connections-face-approval"),
        _ => {}
    }
    let mut out: Vec<&'static str> = Vec::new();
    if can_link(provider, state) {
        out.push(match connection_kind(provider) {
            ConnectionKind::PublicId => "connections-do-link-id",
            _ => "connections-do-link",
        });
        if can_test(provider.id) {
            out.push("connections-do-test");
        }
    }
    for service in provider.services {
        let key = match (service.capability, service.status) {
            (Capability::IdentityOnly, _) => continue,
            // O Explorador do Lichess só responde a quem autorizou a conexão.
            (Capability::Explore, ServiceStatus::Ready) if state != CardState::Authorized => {
                "connections-do-explore-after"
            }
            (capability, ServiceStatus::Ready | ServiceStatus::ManualImport) => {
                verb_key(capability, service.status)
            }
            _ => continue,
        };
        if !out.contains(&key) {
            out.push(key);
        }
    }
    if out.is_empty() {
        Face::Note("connections-face-planned")
    } else {
        Face::Can(out)
    }
}

/// O que chega depois (verbos, sem a identidade, que a Central já oferece): sem etapas nem
/// datas, só o que o Provedor vai permitir.
pub fn later(provider: &Provider) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for service in provider.services {
        if !matches!(service.status, ServiceStatus::Planned { .. })
            || service.capability == Capability::IdentityOnly
        {
            continue;
        }
        let key = verb_key(service.capability, service.status);
        if !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

/// Custo por serviço, do mais barato ao mais caro: cada custo com os serviços que o têm. Um
/// Provedor de custos diferentes mostra cada grupo (Lichess: consultas sem cadastro; Explorador
/// com cadastro), nunca um custo só que contradiga o outro.
pub fn cost_groups(provider: &Provider) -> Vec<(Cost, Vec<&'static str>)> {
    let mut out: Vec<(Cost, Vec<&'static str>)> = Vec::new();
    for service in provider.services {
        if service.capability == Capability::IdentityOnly {
            continue;
        }
        let key = service_label_key(service);
        match out.iter_mut().find(|(cost, _)| *cost == service.cost) {
            Some((_, list)) => list.push(key),
            None => out.push((service.cost, vec![key])),
        }
    }
    if out.is_empty() {
        // Só identidade: o custo é o do Provedor.
        out.push((provider.cost(), Vec::new()));
    }
    out.sort_by_key(|(cost, _)| *cost);
    out
}

/// Serviços que exigem cadastro e custam mais que o resto do Provedor (o Explorador do
/// Lichess): a interface diz "Requer autorizar sua conexão do Lichess".
pub fn requirements(provider: &Provider) -> Vec<&'static str> {
    provider
        .services
        .iter()
        .filter(|s| s.cost > provider.cost() && s.cost <= Cost::FreeWithSignup)
        .map(|s| service_key(s.capability))
        .collect()
}

/// Id Fluent do texto "O que é enviado" do cartão: o do Provedor, ou o do tipo dele.
pub fn sends_key(provider: &Provider, state: CardState) -> &'static str {
    match provider.id {
        ProviderId::Lichess => "connections-sends-lichess",
        ProviderId::ChessCom => "connections-sends-chesscom",
        ProviderId::Fide => "connections-sends-fide",
        _ => match state {
            CardState::ManualImport => "connections-sends-manual",
            CardState::NeedsApproval | CardState::Unconfirmed => "connections-sends-info",
            CardState::Planned => "connections-sends-soon",
            _ => "connections-sends-generic",
        },
    }
}

/// Os endereços para onde o Provedor pode receber pedidos (sem repetir), para o diálogo de
/// consentimento: aparecem como lista curta, nunca dentro de uma frase.
pub fn hosts(provider: &Provider) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for endpoint in provider.endpoints {
        if !out.contains(&endpoint.host) {
            out.push(endpoint.host);
        }
    }
    out
}

/// Ids Fluent dos métodos de identificação do Provedor.
pub fn auth_keys(provider: &Provider) -> Vec<&'static str> {
    let ready = can_authorize(provider);
    provider
        .auth_methods()
        .into_iter()
        .map(|method| match method {
            AuthMethod::None => "connections-auth-none",
            AuthMethod::PublicId => "connections-auth-public-id",
            AuthMethod::OAuth2Pkce if ready => "connections-auth-oauth",
            AuthMethod::OAuth2Pkce => "connections-auth-oauth-soon",
            AuthMethod::PersonalToken if ready => "connections-auth-key-personal",
            AuthMethod::PersonalToken => "connections-auth-key-personal-soon",
            AuthMethod::ApiKey if ready => "connections-auth-key",
            AuthMethod::ApiKey => "connections-auth-key-soon",
            AuthMethod::UsernamePasswordApprovedProtocol => "connections-auth-protocol",
            AuthMethod::ManualImportOnly => "connections-auth-manual",
            AuthMethod::PartnerApproval => "connections-auth-partner",
        })
        .collect()
}

/// Id Fluent do custo.
pub fn cost_key(cost: Cost) -> &'static str {
    match cost {
        Cost::FreeNoSignup => "connections-cost-free-nosignup",
        Cost::FreeWithSignup => "connections-cost-free-signup",
        Cost::FreeLimited => "connections-cost-free-limited",
        Cost::PaidOptional => "connections-cost-paid",
        Cost::NeedsApproval => "connections-cost-approval",
        Cost::NotConfirmed => "connections-cost-unconfirmed",
    }
}

/// Id Fluent da frase curta que explica a falha do Provedor.
pub fn error_key(error: &chess_net::NetError) -> &'static str {
    use chess_net::NetError;
    match error {
        NetError::Timeout { .. } => "connections-err-timeout",
        NetError::Unreachable { .. } => "connections-err-unreachable",
        NetError::Http { .. } => "connections-err-http",
        _ => "connections-err-other",
    }
}

/// Há quanto tempo foi a última consulta. O app não lê o fuso do sistema (sem `unsafe`, e sem
/// uma dependência de fuso), então a interface mostra "há N min" em vez de uma hora local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Age {
    JustNow,
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

pub fn age(now: Timestamp, then: Timestamp) -> Age {
    let secs = now.since(then).as_secs();
    match secs {
        0..=59 => Age::JustNow,
        60..=3599 => Age::Minutes(secs / 60),
        3600..=86_399 => Age::Hours(secs / 3600),
        _ => Age::Days(secs / 86_400),
    }
}

/// Quanto falta para o Provedor sair da pausa, em minutos inteiros (para cima; no mínimo 1).
pub fn minutes_left(now: Timestamp, until: Timestamp) -> u64 {
    until.since(now).as_secs().div_ceil(60).max(1)
}

/// O instante da última consulta que deu certo de cada Provedor (das operações da sessão).
pub fn last_queries(operations: &[Operation]) -> HashMap<ProviderId, Timestamp> {
    let mut out: HashMap<ProviderId, Timestamp> = HashMap::new();
    for op in operations.iter().filter(|op| op.outcome.is_ok()) {
        let slot = out.entry(op.provider).or_insert(op.finished_at);
        if op.finished_at > *slot {
            *slot = op.finished_at;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;

    fn provider(id: ProviderId) -> &'static Provider {
        id.info()
    }

    fn state_of(id: ProviderId) -> CardState {
        card_state(provider(id), &ProviderState::Disabled, false, false)
    }

    #[test]
    fn free_providers_come_first_inside_every_tier_of_every_category() {
        let all = groups(CostFilter::All, "");
        assert_eq!(
            all.iter().map(|g| g.category).collect::<Vec<_>>(),
            CATEGORY_ORDER
        );
        for group in &all {
            let keys: Vec<(Tier, Cost)> = group
                .providers
                .iter()
                .map(|p| (tier_of(p), p.cost()))
                .collect();
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys, sorted, "{:?}", group.category);
        }
        // Em Jogar online: o que funciona, depois "Em breve", por último "Requer aprovação".
        let play = &all[0];
        assert_eq!(
            play.providers.first().map(|p| p.id),
            Some(ProviderId::Lichess)
        );
        assert_eq!(
            play.providers.last().map(|p| p.id),
            Some(ProviderId::Icc),
            "o que requer aprovação vem por último"
        );
        // Nenhum Provedor some nem se repete.
        let total: usize = all.iter().map(|g| g.providers.len()).sum();
        assert_eq!(total, catalog().len());
    }

    #[test]
    fn the_cost_filter_and_the_search_narrow_the_list() {
        let free = groups(CostFilter::Free, "");
        assert!(
            free.iter()
                .flat_map(|g| &g.providers)
                .all(|p| p.cost() <= Cost::FreeLimited)
        );
        let paid = groups(CostFilter::Paid, "");
        assert!(
            paid.iter()
                .flat_map(|g| &g.providers)
                .all(|p| p.cost() > Cost::FreeLimited)
        );
        assert_eq!(
            count(CostFilter::Free) + count(CostFilter::Paid),
            count(CostFilter::All)
        );
        let found = groups(CostFilter::All, "  LICHESS ");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].providers[0].id, ProviderId::Lichess);
        // Acentos e capacidades entram na busca.
        assert!(!groups(CostFilter::All, "avaliacao").is_empty());
        assert!(groups(CostFilter::All, "zzzz").is_empty());
    }

    #[test]
    fn card_states_follow_the_catalog_and_the_network_state() {
        assert_eq!(state_of(ProviderId::Lichess), CardState::Off);
        assert_eq!(state_of(ProviderId::ChessCom), CardState::Off);
        for manual in [
            ProviderId::Twic,
            ProviderId::PgnMentor,
            ProviderId::QueenAlice,
            ProviderId::ChessBase,
            ProviderId::Fide,
        ] {
            assert_eq!(state_of(manual), CardState::ManualImport, "{manual:?}");
        }
        assert_eq!(state_of(ProviderId::Icc), CardState::NeedsApproval);
        assert_eq!(state_of(ProviderId::Chessify), CardState::Unconfirmed);
        assert_eq!(state_of(ProviderId::StockfishApi), CardState::Unconfirmed);
        // Adaptadores distantes aparecem como "Em breve".
        assert_eq!(state_of(ProviderId::Fics), CardState::Planned);
        assert_eq!(state_of(ProviderId::PlayStrategy), CardState::Planned);
        assert_eq!(state_of(ProviderId::ChessApi), CardState::Planned);
        // Sem nada para fazer hoje, não há Switch: o Chessigma também espera.
        assert_eq!(state_of(ProviderId::Chessigma), CardState::Planned);

        let lichess = provider(ProviderId::Lichess);
        let on =
            |linked, authorized| card_state(lichess, &ProviderState::Available, linked, authorized);
        assert_eq!(on(false, false), CardState::On);
        assert_eq!(on(true, false), CardState::Linked);
        assert_eq!(on(true, true), CardState::Authorized);
        assert_eq!(
            card_state(
                lichess,
                &ProviderState::Limited {
                    until: Timestamp::from_millis(1)
                },
                true,
                false
            ),
            CardState::Limited
        );
        assert_eq!(
            card_state(
                lichess,
                &ProviderState::Failing {
                    since: Timestamp::from_millis(1),
                    error: chess_net::NetError::Timeout {
                        provider: ProviderId::Lichess
                    }
                },
                false,
                false
            ),
            CardState::Failing
        );
    }

    #[test]
    fn authorized_is_impossible_without_an_authorized_connection() {
        let nets = [
            ProviderState::Disabled,
            ProviderState::Available,
            ProviderState::Limited {
                until: Timestamp::from_millis(1),
            },
            ProviderState::Failing {
                since: Timestamp::from_millis(1),
                error: chess_net::NetError::Timeout {
                    provider: ProviderId::Lichess,
                },
            },
        ];
        for p in catalog() {
            for net in &nets {
                for linked in [false, true] {
                    // Sem Conexão autorizada, nunca "Autorizado", com ou sem nome vinculado.
                    let state = card_state(p, net, linked, false);
                    assert_ne!(state, CardState::Authorized, "{} {net:?}", p.name);
                    // E um nome vinculado nunca vira "Autorizado" nem "Conectado".
                    assert!(!matches!(state.key(), "connections-state-connected"));
                }
            }
        }
        // "Ligado · nome vinculado" é o que o nome vinculado vale.
        let lichess = provider(ProviderId::Lichess);
        assert_eq!(
            card_state(lichess, &ProviderState::Available, true, false),
            CardState::Linked
        );
        assert!(!CardState::Linked.key().ends_with("authorized"));
    }

    #[test]
    fn manual_and_approval_cards_never_show_a_secret_field_or_an_authorize_action() {
        for p in catalog() {
            for net in [ProviderState::Disabled, ProviderState::Available] {
                let state = card_state(p, &net, false, false);
                let fields = card_fields(p, state);
                assert!(!fields.contains(&FieldKind::Secret), "{}", p.name);
                if state.is_informational() {
                    let actions = card_actions(p, state, true);
                    let all: Vec<ActionSpec> =
                        actions.primary.into_iter().chain(actions.menu).collect();
                    for spec in &all {
                        assert!(
                            !matches!(spec.action, CardAction::Authorize | CardAction::EnterKey),
                            "{}: {:?}",
                            p.name,
                            spec.action
                        );
                        assert!(
                            !matches!(spec.action, CardAction::ClearCache),
                            "{}: não há cache de rede",
                            p.name
                        );
                    }
                    // E nenhuma chamada própria de rede: só o FIDE testa (pelo Lichess).
                    assert_eq!(
                        all.iter().any(|a| a.action == CardAction::Test),
                        p.id == ProviderId::Fide,
                        "{}",
                        p.name
                    );
                    assert!(!has_switch(p, state), "{}", p.name);
                }
            }
        }
    }

    #[test]
    fn authorizing_is_the_lichess_primary_action_and_other_providers_keep_it_disabled() {
        let lichess = provider(ProviderId::Lichess);
        for state in [CardState::Off, CardState::On, CardState::Linked] {
            let actions = card_actions(lichess, state, state == CardState::Linked);
            assert_eq!(
                actions.primary.map(|s| s.action),
                Some(CardAction::Authorize),
                "{state:?}"
            );
            for wanted in [CardAction::EnterKey, CardAction::EngineOnline] {
                let spec = actions.menu.iter().find(|a| a.action == wanted).unwrap();
                assert!(spec.enabled, "{wanted:?}");
            }
        }
        // Expirada: reautorizar. Bloqueada: desbloquear. Autorizada: testar.
        let primary = |state, linked| {
            card_actions(lichess, state, linked)
                .primary
                .map(|s| s.action)
        };
        assert_eq!(
            primary(CardState::Expired, true),
            Some(CardAction::Reauthorize)
        );
        assert_eq!(primary(CardState::Locked, true), Some(CardAction::Unlock));
        assert_eq!(primary(CardState::Authorized, true), Some(CardAction::Test));
        // Nenhum outro Provedor do catálogo oferece autorizar ou chave habilitados, e todo item
        // desabilitado explica o motivo.
        for p in catalog() {
            let state = card_state(p, &ProviderState::Available, false, false);
            let actions = card_actions(p, state, true);
            for spec in actions.primary.iter().chain(&actions.menu) {
                if p.id != ProviderId::Lichess
                    && matches!(
                        spec.action,
                        CardAction::Authorize
                            | CardAction::Reauthorize
                            | CardAction::EnterKey
                            | CardAction::EngineOnline
                    )
                {
                    assert!(!spec.enabled, "{}", p.name);
                }
                assert!(spec.enabled || spec.reason_key.is_some(), "{}", p.name);
            }
        }
    }

    #[test]
    fn the_secret_field_exists_only_in_the_lichess_key_form() {
        for p in catalog() {
            for state in [CardState::Off, CardState::On, CardState::Authorized] {
                let fields = key_fields(p, state);
                assert_eq!(
                    fields.contains(&FieldKind::Secret),
                    p.id == ProviderId::Lichess && !state.is_informational(),
                    "{} {state:?}",
                    p.name
                );
            }
        }
    }

    #[test]
    fn a_card_has_one_primary_action_and_the_rest_in_the_menu() {
        let lichess = provider(ProviderId::Lichess);
        let none = card_actions(lichess, CardState::On, false);
        assert_eq!(none.primary.map(|s| s.action), Some(CardAction::Authorize));
        assert!(none.menu.iter().any(|a| a.action == CardAction::Link));
        assert!(none.menu.iter().any(|a| a.action == CardAction::ClearCache));
        let linked = card_actions(lichess, CardState::Linked, true);
        assert_eq!(
            linked.primary.map(|s| s.action),
            Some(CardAction::Authorize)
        );
        assert!(linked.menu.iter().any(|a| a.action == CardAction::Test));
        assert!(
            linked.menu.iter().any(|a| a.action == CardAction::Link),
            "vincular outro nome fica no menu"
        );
        // Sem autorização no Provedor, a ação principal é vincular, e depois testar.
        let chesscom = provider(ProviderId::ChessCom);
        let first = card_actions(chesscom, CardState::On, false);
        assert_eq!(first.primary.map(|s| s.action), Some(CardAction::Link));
        let linked = card_actions(chesscom, CardState::Linked, true);
        assert_eq!(linked.primary.map(|s| s.action), Some(CardAction::Test));
        // Provedor só de arquivo: a ação principal abre as instruções.
        let twic = card_actions(provider(ProviderId::Twic), CardState::ManualImport, false);
        assert_eq!(twic.primary.map(|s| s.action), Some(CardAction::OpenDocs));
        // Cartão menor: nenhuma ação principal, só instruções e termos no menu.
        let fics = card_actions(provider(ProviderId::Fics), CardState::Planned, false);
        assert!(fics.primary.is_none());
        assert!(
            fics.menu
                .iter()
                .all(|a| matches!(a.action, CardAction::OpenDocs | CardAction::OpenTerms))
        );
    }

    #[test]
    fn linking_a_public_name_is_offered_exactly_where_the_issue_lists_it() {
        let mut linkable: Vec<ProviderId> = catalog()
            .iter()
            .filter(|p| can_link(p, state_of(p.id)))
            .map(|p| p.id)
            .collect();
        linkable.sort();
        let mut expected = vec![
            ProviderId::Lichess,
            ProviderId::ChessCom,
            ProviderId::Fide,
            ProviderId::QueenAlice,
            ProviderId::ChessBase,
        ];
        expected.sort();
        assert_eq!(linkable, expected);
        assert_eq!(
            connection_kind(provider(ProviderId::Fide)),
            ConnectionKind::PublicId
        );
        assert_eq!(
            connection_kind(provider(ProviderId::Lichess)),
            ConnectionKind::PublicName
        );
        assert_eq!(
            card_fields(provider(ProviderId::Fide), CardState::ManualImport),
            [FieldKind::PublicId, FieldKind::Nickname]
        );
    }

    #[test]
    fn test_exists_only_for_the_adapters_and_needs_a_linked_connection() {
        let lichess = provider(ProviderId::Lichess);
        let has_test = |linked| {
            let actions = card_actions(lichess, CardState::On, linked);
            actions
                .primary
                .into_iter()
                .chain(actions.menu)
                .any(|a| a.action == CardAction::Test)
        };
        assert!(!has_test(false), "sem nome vinculado não há o que testar");
        assert!(has_test(true));
        for id in [
            ProviderId::PlayStrategy,
            ProviderId::Fics,
            ProviderId::Chessigma,
        ] {
            let p = provider(id);
            let actions = card_actions(p, state_of(id), true);
            assert!(
                !actions
                    .primary
                    .into_iter()
                    .chain(actions.menu)
                    .any(|a| a.action == CardAction::Test)
            );
        }
        // O FIDE testa pelo Lichess.
        assert_eq!(test_via(ProviderId::Fide), ProviderId::Lichess);
        assert_eq!(test_via(ProviderId::ChessCom), ProviderId::ChessCom);
    }

    #[test]
    fn a_provider_with_services_of_different_costs_shows_each_cost_never_a_contradiction() {
        let groups = cost_groups(provider(ProviderId::Lichess));
        assert!(groups.len() >= 2, "{groups:?}");
        let costs: Vec<Cost> = groups.iter().map(|(c, _)| *c).collect();
        assert_eq!(costs.first(), Some(&Cost::FreeNoSignup));
        assert!(costs.contains(&Cost::FreeWithSignup));
        let mut sorted = costs.clone();
        sorted.sort();
        assert_eq!(costs, sorted);
        // O Explorador está só no grupo "com cadastro"; nenhum serviço aparece em dois grupos.
        let with_signup = groups
            .iter()
            .find(|(c, _)| *c == Cost::FreeWithSignup)
            .unwrap();
        assert!(with_signup.1.contains(&"connections-svc-explore"));
        let mut seen = Vec::new();
        for (_, services) in &groups {
            for key in services {
                assert!(!seen.contains(key), "{key} em dois custos");
                seen.push(*key);
            }
        }
        assert!(
            groups
                .iter()
                .filter(|(c, _)| *c == Cost::FreeNoSignup)
                .all(|(_, s)| !s.contains(&"connections-svc-explore"))
        );
        // Um Provedor de custo único tem um grupo só.
        assert_eq!(cost_groups(provider(ProviderId::Twic)).len(), 1);
        // O requisito do Explorador sai do custo (e do jogo), sem texto do catálogo.
        assert!(requirements(provider(ProviderId::Lichess)).contains(&"connections-svc-explore"));
        assert!(requirements(provider(ProviderId::ChessCom)).is_empty());
    }

    #[test]
    fn the_switch_exists_only_on_full_cards_of_network_providers() {
        assert!(has_switch(provider(ProviderId::Lichess), CardState::Off));
        assert!(!has_switch(provider(ProviderId::Fics), CardState::Planned));
        assert!(!has_switch(
            provider(ProviderId::Twic),
            CardState::ManualImport
        ));
    }

    #[test]
    fn face_lists_only_what_exists_and_later_lists_the_rest() {
        let lichess = provider(ProviderId::Lichess);
        let Face::Can(now) = face(lichess, CardState::Off) else {
            panic!("o Lichess permite algo agora");
        };
        // O que já funciona: o Explorador só depois de autorizar, o resto já.
        assert_eq!(
            now,
            [
                "connections-do-link",
                "connections-do-test",
                "connections-do-explore-after",
                "connections-do-cached-eval",
                "connections-do-endgames",
                "connections-do-query",
            ]
        );
        let Face::Can(authorized) = face(lichess, CardState::Authorized) else {
            panic!("o Lichess autorizado permite explorar");
        };
        assert!(authorized.contains(&"connections-do-explore"));
        assert!(!authorized.contains(&"connections-do-explore-after"));
        // O FIDE vincula um ID, não um nome.
        let Face::Can(fide) = face(provider(ProviderId::Fide), CardState::ManualImport) else {
            panic!("o FIDE permite vincular e testar");
        };
        assert_eq!(fide[0], "connections-do-link-id");
        let later = later(lichess);
        assert!(!later.contains(&"connections-do-explore"), "já funciona");
        assert!(later.contains(&"connections-do-play"));
        assert!(!later.contains(&"connections-do-link"));
        let Face::Can(twic) = face(provider(ProviderId::Twic), CardState::ManualImport) else {
            panic!("importar arquivo é o que o TWIC permite");
        };
        assert_eq!(twic, ["connections-do-import-file"]);
        assert_eq!(
            face(provider(ProviderId::Fics), CardState::Planned),
            Face::Note("connections-face-planned")
        );
        assert_eq!(
            face(provider(ProviderId::Icc), CardState::NeedsApproval),
            Face::Note("connections-face-approval")
        );
    }

    #[test]
    fn age_buckets_remaining_minutes_and_last_query() {
        let t = |s: i64| Timestamp::from_millis(s * 1000);
        assert_eq!(age(t(100), t(60)), Age::JustNow);
        assert_eq!(age(t(1000), t(1000 - 125)), Age::Minutes(2));
        assert_eq!(age(t(100_000), t(100_000 - 7300)), Age::Hours(2));
        assert_eq!(age(t(1_000_000), t(1_000_000 - 200_000)), Age::Days(2));
        assert_eq!(age(t(10), t(50)), Age::JustNow, "relógio que voltou");
        assert_eq!(minutes_left(t(0), t(1)), 1);
        assert_eq!(minutes_left(t(0), t(61)), 2);
        assert_eq!(minutes_left(t(100), t(40)), 1, "já passou: no mínimo 1");

        let op = |provider, at: i64, ok: bool| Operation {
            provider,
            capability: None,
            method: chess_net::Method::Get,
            path: "/x".into(),
            started_at: t(at),
            finished_at: t(at),
            http_status: None,
            outcome: if ok {
                Ok(chess_net::Freshness::Network)
            } else {
                Err(chess_net::NetError::Timeout { provider })
            },
        };
        let last = last_queries(&[
            op(ProviderId::Lichess, 10, true),
            op(ProviderId::Lichess, 30, true),
            op(ProviderId::Lichess, 50, false),
            op(ProviderId::ChessCom, 5, false),
        ]);
        assert_eq!(last.get(&ProviderId::Lichess), Some(&t(30)));
        assert!(!last.contains_key(&ProviderId::ChessCom));
    }

    /// Todo texto fixo da Central e do diálogo (o arquivo Fluent inteiro) existe em pt-BR e
    /// não usa jargão interno nem as palavras proibidas do glossário.
    #[test]
    fn no_interface_text_uses_stage_jargon_or_the_banned_words() {
        let files = [
            include_str!("../../assets/i18n/pt-BR/connections.ftl"),
            include_str!("../../assets/i18n/pt-BR/authorization.ftl"),
        ];
        let mut checked = 0;
        for line in files.iter().flat_map(|source| source.lines()) {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let text = trimmed.split_once('=').map_or(trimmed, |(_, value)| value);
            let lower = text.to_lowercase();
            for banned in [
                "etapa",
                "401",
                "token",
                "conta",
                "login",
                "credencial",
                "pkce",
                "oauth",
                "escopo",
                "planejad",
            ] {
                assert!(!lower.contains(banned), "{banned:?} em: {line}");
            }
            checked += 1;
        }
        assert!(checked > 200, "os arquivos foram lidos ({checked} linhas)");
    }

    #[test]
    fn every_text_key_of_every_card_exists() {
        let i18n = I18n::pt_br();
        for p in catalog() {
            for state in [
                state_of(p.id),
                CardState::On,
                CardState::Linked,
                CardState::Authorized,
                CardState::Limited,
                CardState::Failing,
            ] {
                let mut keys = vec![
                    state.key(),
                    state.hint_key(),
                    category_key(p.category),
                    sends_key(p, state),
                ];
                if let Face::Note(key) = face(p, state) {
                    keys.push(key);
                }
                if let Face::Can(list) = face(p, state) {
                    keys.extend(list);
                }
                keys.extend(later(p));
                keys.extend(auth_keys(p));
                keys.extend(requirements(p));
                keys.push("connections-svc-manual-file");
                for (cost, services) in cost_groups(p) {
                    keys.push(cost_key(cost));
                    keys.extend(services);
                }
                let actions = card_actions(p, state, true);
                keys.extend(
                    actions
                        .primary
                        .into_iter()
                        .chain(actions.menu)
                        .filter_map(|a| a.reason_key),
                );
                for key in keys {
                    assert_ne!(i18n.t(key), key, "{key} ({})", p.name);
                }
            }
        }
        for filter in CostFilter::ALL {
            assert_ne!(i18n.t(filter.key()), filter.key());
        }
        for tier in [Tier::Soon, Tier::Approval] {
            let key = tier.key().unwrap();
            assert_ne!(i18n.t(key), key);
        }
    }
}
