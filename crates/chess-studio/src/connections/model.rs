//! O modelo da Central de Conexões, sem Bevy nem egui (testável): como os Provedores do
//! catálogo se ordenam e agrupam, em que estado cada cartão está, quais ações e campos ele
//! mostra e o que "permite agora" (sem prometer o futuro).
//!
//! Regras de produto (CX-05, CX-06, CX-09; especificação 4.1 e 4.2):
//!
//! - gratuitos primeiro, agrupados por categoria (jogar, dados, Engines, importação, ratings);
//! - Provedor sem API ou sem aprovação nunca mostra campo de segredo nem botão de autorização;
//! - Autorizar e Inserir chave aparecem desabilitados até a etapa 3 (Cofre);
//! - vincular nome ou ID público grava uma Conexão por Perfil local.

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
/// dentro de cada uma, **gratuitos primeiro** (custo crescente; a ordem do catálogo desempata).
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
            providers.sort_by_key(|p| p.cost());
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

/// O estado do cartão (a palavra do chip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardState {
    /// Provedor de rede, desligado.
    Off,
    /// Ligado e sem problema conhecido.
    Available,
    /// Ligado e com uma Conexão vinculada.
    Connected,
    /// O Provedor pediu uma pausa.
    Limited,
    /// A última tentativa falhou.
    Failing,
    /// Sem API: só importação de arquivo.
    ManualImport,
    /// Pago ou sob aprovação do Provedor: nada sai para a rede.
    NeedsApproval,
    /// O adaptador chega daqui a duas ou mais etapas.
    Planned,
    /// API ou termos ainda não confirmados.
    Unconfirmed,
}

impl CardState {
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "connections-state-off",
            Self::Available => "connections-state-available",
            Self::Connected => "connections-state-connected",
            Self::Limited => "connections-state-limited",
            Self::Failing => "connections-state-failing",
            Self::ManualImport => "connections-state-manual",
            Self::NeedsApproval => "connections-state-approval",
            Self::Planned => "connections-state-planned",
            Self::Unconfirmed => "connections-state-unconfirmed",
        }
    }

    /// Cartões que nunca falam com a rede.
    pub fn is_informational(self) -> bool {
        matches!(
            self,
            Self::ManualImport | Self::NeedsApproval | Self::Unconfirmed
        )
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

/// A partir desta etapa o Provedor aparece como "Planejado" e não se liga ainda.
const PLANNED_FROM_STAGE: u8 = 3;

/// O estado do cartão a partir do catálogo, do estado da rede e de haver Conexão vinculada.
pub fn card_state(provider: &Provider, net: &ProviderState, has_connection: bool) -> CardState {
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
        ProviderState::Available if has_connection => CardState::Connected,
        ProviderState::Available => CardState::Available,
        ProviderState::Disabled => {
            if earliest_stage(provider).is_some_and(|stage| stage >= PLANNED_FROM_STAGE) {
                CardState::Planned
            } else {
                CardState::Off
            }
        }
    }
}

/// Como o Switch "Ligado" aparece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchMode {
    /// Sem serviço de rede: não há o que ligar.
    Hidden,
    /// Ainda não: o adaptador chega na etapa indicada.
    Locked {
        stage: u8,
    },
    Usable,
}

pub fn switch_mode(provider: &Provider, state: CardState) -> SwitchMode {
    if !provider.uses_network() {
        return SwitchMode::Hidden;
    }
    match (state, earliest_stage(provider)) {
        (CardState::Planned, Some(stage)) => SwitchMode::Locked { stage },
        _ => SwitchMode::Usable,
    }
}

/// Ações de um cartão, na ordem em que aparecem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
    /// Vincular nome ou ID público (uma Conexão do Perfil local).
    Link,
    Test,
    ClearCache,
    /// Autorizar pelo navegador (etapa 3).
    Authorize,
    /// Inserir chave ou token (etapa 3).
    EnterKey,
    OpenDocs,
    OpenTerms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionSpec {
    pub action: CardAction,
    pub enabled: bool,
    /// Desabilitada porque chega numa etapa futura.
    pub soon_stage: Option<u8>,
    /// Id Fluent do motivo de estar desabilitada agora (tooltip).
    pub reason_key: Option<&'static str>,
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

/// As ações do cartão conforme o método de identificação e o estado.
pub fn card_actions(
    provider: &Provider,
    state: CardState,
    has_connection: bool,
) -> Vec<ActionSpec> {
    let ready = |action| ActionSpec {
        action,
        enabled: true,
        soon_stage: None,
        reason_key: None,
    };
    let soon = |action, stage| ActionSpec {
        action,
        enabled: false,
        soon_stage: Some(stage),
        reason_key: None,
    };
    let mut out = Vec::new();
    if can_link(provider, state) {
        out.push(ready(CardAction::Link));
    }
    if can_test(provider.id) && can_link(provider, state) {
        out.push(ActionSpec {
            action: CardAction::Test,
            enabled: has_connection,
            soon_stage: None,
            reason_key: (!has_connection).then_some("connections-test-needs-connection"),
        });
    }
    if provider.uses_network() {
        out.push(ready(CardAction::ClearCache));
    }
    if !state.is_informational() {
        if has_auth(provider, &[AuthMethod::OAuth2Pkce]) {
            out.push(soon(CardAction::Authorize, 3));
        }
        if has_auth(provider, &[AuthMethod::PersonalToken, AuthMethod::ApiKey]) {
            out.push(soon(CardAction::EnterKey, 3));
        }
    }
    if !provider.docs_url.is_empty() {
        out.push(ready(CardAction::OpenDocs));
    }
    if !provider.terms_url.is_empty() {
        out.push(ready(CardAction::OpenTerms));
    }
    out
}

/// Um campo de entrada que o cartão pode mostrar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    PublicName,
    PublicId,
    Nickname,
    /// Campo de segredo (chave, token, senha): só existe a partir da etapa 3, e nunca em
    /// Provedor sem API ou sob aprovação. Hoje nenhum cartão o mostra.
    #[allow(dead_code)] // reservado à etapa 3 (Cofre)
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

/// Todos os campos de entrada do cartão (o formulário de vincular). Nunca `Secret` na etapa 1.
pub fn card_fields(provider: &Provider, state: CardState) -> Vec<FieldKind> {
    if !can_link(provider, state) {
        return Vec::new();
    }
    vec![
        match connection_kind(provider) {
            ConnectionKind::PublicId => FieldKind::PublicId,
            ConnectionKind::PublicName => FieldKind::PublicName,
        },
        FieldKind::Nickname,
    ]
}

/// Id Fluent do nome da capacidade.
pub fn capability_key(capability: Capability) -> &'static str {
    match capability {
        Capability::Play => "connections-cap-play",
        Capability::OnlineEngine => "connections-cap-online-engine",
        Capability::Explore => "connections-cap-explore",
        Capability::Endgames => "connections-cap-endgames",
        Capability::CachedEval => "connections-cap-cached-eval",
        Capability::Analyze => "connections-cap-analyze",
        Capability::Query => "connections-cap-query",
        Capability::Download => "connections-cap-download",
        Capability::ImportPgn => "connections-cap-import-pgn",
        Capability::IdentityOnly => "connections-cap-identity-only",
    }
}

/// O que o Provedor permite **agora**: o serviço pronto, a importação manual e vincular nome.
/// O que ainda não existe fica em [`planned`].
pub fn now_keys(provider: &Provider, state: CardState) -> Vec<&'static str> {
    let mut out = Vec::new();
    if can_link(provider, state) {
        out.push(capability_key(Capability::IdentityOnly));
    }
    for service in provider.services {
        let key = match (service.capability, service.status) {
            (Capability::IdentityOnly, _) => continue,
            (Capability::Download, ServiceStatus::ManualImport) => {
                "connections-cap-manual-download"
            }
            (capability, ServiceStatus::Ready | ServiceStatus::ManualImport) => {
                capability_key(capability)
            }
            _ => continue,
        };
        if !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

/// O que chega depois, por etapa (sem a identidade, que a Central já oferece): não é promessa
/// de data, só a etapa do programa.
pub fn planned(provider: &Provider) -> Vec<(u8, Vec<&'static str>)> {
    let mut out: Vec<(u8, Vec<&'static str>)> = Vec::new();
    for service in provider.services {
        let ServiceStatus::Planned { stage } = service.status else {
            continue;
        };
        if service.capability == Capability::IdentityOnly {
            continue;
        }
        let key = capability_key(service.capability);
        match out.iter_mut().find(|(s, _)| *s == stage) {
            Some((_, list)) => list.push(key),
            None => out.push((stage, vec![key])),
        }
    }
    out.sort_by_key(|(stage, _)| *stage);
    out
}

/// Serviços que custam mais que o Provedor e dizem que exigem uma Conexão autorizada (o
/// Explorador do Lichess): o nome da capacidade e a frase do catálogo.
pub fn requirements(provider: &Provider) -> Vec<(&'static str, &'static str)> {
    provider
        .services
        .iter()
        .filter(|s| s.cost > provider.cost() && s.note.contains("Conexão autorizada"))
        .map(|s| (capability_key(s.capability), s.note))
        .collect()
}

/// Ids Fluent dos métodos de identificação do Provedor.
pub fn auth_keys(provider: &Provider) -> Vec<&'static str> {
    provider
        .auth_methods()
        .into_iter()
        .map(|method| match method {
            AuthMethod::None => "connections-auth-none",
            AuthMethod::PublicId => "connections-auth-public-id",
            AuthMethod::OAuth2Pkce => "connections-auth-oauth",
            AuthMethod::PersonalToken => "connections-auth-token",
            AuthMethod::ApiKey => "connections-auth-key",
            AuthMethod::UsernamePasswordApprovedProtocol => "connections-auth-protocol",
            AuthMethod::ManualImportOnly => "connections-auth-manual",
            AuthMethod::PartnerApproval => "connections-auth-partner",
        })
        .collect()
}

/// Id Fluent do custo do Provedor.
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

/// Há quanto tempo foi a última consulta.
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
        card_state(provider(id), &ProviderState::Disabled, false)
    }

    #[test]
    fn free_providers_come_first_inside_every_category() {
        let all = groups(CostFilter::All, "");
        assert_eq!(
            all.iter().map(|g| g.category).collect::<Vec<_>>(),
            CATEGORY_ORDER
        );
        for group in &all {
            let costs: Vec<Cost> = group.providers.iter().map(|p| p.cost()).collect();
            let mut sorted = costs.clone();
            sorted.sort();
            assert_eq!(costs, sorted, "{:?}", group.category);
        }
        // Nos Provedores de jogo, os pagos (ICC) e os não confirmados ficam depois dos gratuitos.
        let play = &all[0];
        assert_eq!(
            play.providers.first().map(|p| p.id),
            Some(ProviderId::Lichess)
        );
        assert_eq!(
            play.providers.last().map(|p| p.id),
            Some(ProviderId::PyChessVariants)
        );
        let icc = play
            .providers
            .iter()
            .position(|p| p.id == ProviderId::Icc)
            .unwrap();
        let fics = play
            .providers
            .iter()
            .position(|p| p.id == ProviderId::Fics)
            .unwrap();
        assert!(fics < icc);
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
        // Adaptadores distantes aparecem como planejados.
        assert_eq!(state_of(ProviderId::Fics), CardState::Planned);
        assert_eq!(state_of(ProviderId::PlayStrategy), CardState::Planned);
        assert_eq!(state_of(ProviderId::ChessApi), CardState::Planned);

        let lichess = provider(ProviderId::Lichess);
        assert_eq!(
            card_state(lichess, &ProviderState::Available, false),
            CardState::Available
        );
        assert_eq!(
            card_state(lichess, &ProviderState::Available, true),
            CardState::Connected
        );
        assert_eq!(
            card_state(
                lichess,
                &ProviderState::Limited {
                    until: Timestamp::from_millis(1)
                },
                true
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
                false
            ),
            CardState::Failing
        );
    }

    #[test]
    fn manual_and_approval_cards_never_show_a_secret_field_or_an_authorize_action() {
        for p in catalog() {
            for net in [ProviderState::Disabled, ProviderState::Available] {
                let state = card_state(p, &net, false);
                let fields = card_fields(p, state);
                assert!(!fields.contains(&FieldKind::Secret), "{}", p.name);
                if state.is_informational() {
                    let actions = card_actions(p, state, true);
                    for spec in &actions {
                        assert!(
                            !matches!(spec.action, CardAction::Authorize | CardAction::EnterKey),
                            "{}: {:?}",
                            p.name,
                            spec.action
                        );
                    }
                    // E nenhuma chamada própria de rede: só o FIDE testa (pelo Lichess).
                    assert_eq!(
                        actions.iter().any(|a| matches!(a.action, CardAction::Test)),
                        p.id == ProviderId::Fide,
                        "{}",
                        p.name
                    );
                    assert!(
                        !actions
                            .iter()
                            .any(|a| matches!(a.action, CardAction::ClearCache))
                    );
                    assert_eq!(switch_mode(p, state), SwitchMode::Hidden, "{}", p.name);
                }
            }
        }
    }

    #[test]
    fn authorize_and_enter_key_are_disabled_until_stage_three() {
        let lichess = provider(ProviderId::Lichess);
        let actions = card_actions(lichess, CardState::Off, false);
        for wanted in [CardAction::Authorize, CardAction::EnterKey] {
            let spec = actions.iter().find(|a| a.action == wanted).unwrap();
            assert!(!spec.enabled);
            assert_eq!(spec.soon_stage, Some(3));
        }
        // Nenhum cartão do catálogo oferece um desses dois habilitado.
        for p in catalog() {
            for spec in card_actions(p, card_state(p, &ProviderState::Available, false), true) {
                if matches!(spec.action, CardAction::Authorize | CardAction::EnterKey) {
                    assert!(!spec.enabled, "{}", p.name);
                }
            }
        }
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
        let find = |has| {
            card_actions(lichess, CardState::Off, has)
                .into_iter()
                .find(|a| a.action == CardAction::Test)
                .unwrap()
        };
        assert!(!find(false).enabled);
        assert_eq!(
            find(false).reason_key,
            Some("connections-test-needs-connection")
        );
        assert!(find(true).enabled);
        for id in [
            ProviderId::PlayStrategy,
            ProviderId::Fics,
            ProviderId::Chessigma,
        ] {
            let p = provider(id);
            let state = state_of(id);
            assert!(
                !card_actions(p, state, true)
                    .iter()
                    .any(|a| a.action == CardAction::Test)
            );
        }
        // O FIDE testa pelo Lichess.
        assert_eq!(test_via(ProviderId::Fide), ProviderId::Lichess);
        assert_eq!(test_via(ProviderId::ChessCom), ProviderId::ChessCom);
    }

    #[test]
    fn the_lichess_explorer_requirement_is_shown_from_the_catalog() {
        let found = requirements(provider(ProviderId::Lichess));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "connections-cap-explore");
        assert!(found[0].1.contains("Gratuito com cadastro"));
        assert!(
            found[0]
                .1
                .contains("Conexão autorizada do Lichess (etapa 3)")
        );
        assert!(requirements(provider(ProviderId::ChessCom)).is_empty());
    }

    #[test]
    fn the_switch_is_hidden_without_network_and_locked_for_distant_stages() {
        assert_eq!(
            switch_mode(provider(ProviderId::Lichess), CardState::Off),
            SwitchMode::Usable
        );
        assert_eq!(
            switch_mode(provider(ProviderId::Fics), CardState::Planned),
            SwitchMode::Locked { stage: 5 }
        );
        assert_eq!(
            switch_mode(provider(ProviderId::Twic), CardState::ManualImport),
            SwitchMode::Hidden
        );
    }

    #[test]
    fn now_lists_only_what_exists_and_planned_lists_the_rest_by_stage() {
        let lichess = provider(ProviderId::Lichess);
        assert_eq!(
            now_keys(lichess, CardState::Off),
            ["connections-cap-identity-only"]
        );
        let planned = planned(lichess);
        assert_eq!(
            planned.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            [2, 4, 5]
        );
        assert!(planned[0].1.contains(&"connections-cap-explore"));
        let twic = provider(ProviderId::Twic);
        assert_eq!(
            now_keys(twic, CardState::ManualImport),
            ["connections-cap-import-pgn"]
        );
        assert!(now_keys(provider(ProviderId::Chessify), CardState::Unconfirmed).is_empty());
    }

    #[test]
    fn age_buckets_and_last_query() {
        let t = |s: i64| Timestamp::from_millis(s * 1000);
        assert_eq!(age(t(100), t(60)), Age::JustNow);
        assert_eq!(age(t(1000), t(1000 - 125)), Age::Minutes(2));
        assert_eq!(age(t(100_000), t(100_000 - 7300)), Age::Hours(2));
        assert_eq!(age(t(1_000_000), t(1_000_000 - 200_000)), Age::Days(2));
        assert_eq!(age(t(10), t(50)), Age::JustNow, "relógio que voltou");

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

    #[test]
    fn every_text_of_every_card_exists_and_avoids_the_banned_words() {
        let i18n = I18n::pt_br();
        let mut texts = Vec::new();
        for p in catalog() {
            let state = state_of(p.id);
            let mut keys = vec![state.key(), category_key(p.category), cost_key(p.cost())];
            keys.extend(now_keys(p, state));
            keys.extend(auth_keys(p));
            for (_, list) in planned(p) {
                keys.extend(list);
            }
            for key in keys {
                let text = i18n.t(key);
                assert_ne!(text, key, "{key}");
                texts.push(text);
            }
        }
        for filter in CostFilter::ALL {
            texts.push(i18n.t(filter.key()));
        }
        let all = texts.join(" ").to_lowercase();
        for banned in ["conta", "login", "credencial"] {
            assert!(!all.contains(banned), "{banned}");
        }
    }
}
