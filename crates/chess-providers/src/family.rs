//! Perfil de Provedor: o que muda entre os Provedores da família do Lichess (o PlayStrategy
//! é um servidor da mesma família, com a mesma forma de API). Hosts, caminhos, escopos e
//! variantes aceitas ficam aqui, em dados; os clientes ([`crate::Lichess`],
//! [`crate::LichessBoard`], [`crate::LichessBot`], [`crate::LichessAuth`]) leem o perfil em vez
//! de ter o Provedor escrito no código. Acrescentar um Provedor da família é acrescentar um
//! perfil (e a entrada dele no catálogo do `chess-net`), não copiar os clientes.
//!
//! O que o catálogo já diz (variantes aceitas, endereços permitidos) continua sendo a fonte
//! da verdade: o perfil só aponta para ele.

use chess_net::ProviderId;

/// Perfil de um Provedor da família do Lichess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderProfile {
    /// O Provedor no catálogo do `chess-net` (consentimento, limites, Origem).
    pub id: ProviderId,
    /// Nome para mostrar ao usuário.
    pub name: &'static str,
    /// Host da API de jogo, perfis e Partidas públicas (também do OAuth).
    pub api_host: &'static str,
    /// Host do Explorador de aberturas, se o Provedor tem.
    pub explorer_host: Option<&'static str>,
    /// Host dos finais exatos (tablebase), se o Provedor tem.
    pub tablebase_host: Option<&'static str>,
    /// Avaliação em cache (`/api/cloud-eval`).
    pub has_cloud_eval: bool,
    /// Ratings FIDE reproduzidos pelo Provedor.
    pub has_fide: bool,
    /// Caminho da autorização no navegador (OAuth2 PKCE).
    pub authorize_path: &'static str,
    /// Caminho da troca do código e da revogação do token.
    pub token_path: &'static str,
    /// Caminho da conta autenticada.
    pub account_path: &'static str,
    /// Escopo da Conexão de jogador (Board API).
    pub player_scope: &'static str,
    /// Escopo da Conexão de Engine (Bot API).
    pub engine_scope: &'static str,
    /// Como a interface chama a Engine do próprio Provedor.
    pub provider_engine_name: &'static str,
}

impl ProviderProfile {
    /// Lichess: o comportamento que o app sempre teve.
    pub const LICHESS: Self = Self {
        id: ProviderId::Lichess,
        name: "Lichess",
        api_host: "lichess.org",
        explorer_host: Some("explorer.lichess.org"),
        tablebase_host: Some("tablebase.lichess.ovh"),
        has_cloud_eval: true,
        has_fide: true,
        authorize_path: "/oauth",
        token_path: "/api/token",
        account_path: "/api/account",
        player_scope: "board:play",
        engine_scope: "bot:play",
        provider_engine_name: "Engine do Lichess",
    };

    /// PlayStrategy: mesma família de API; só jogo, perfil e Partidas públicas (sem
    /// Explorador, avaliação em cache, finais nem FIDE). Os formatos foram lidos na
    /// documentação pública e as respostas dos testes são sintéticas.
    pub const PLAYSTRATEGY: Self = Self {
        id: ProviderId::PlayStrategy,
        name: "PlayStrategy",
        api_host: "playstrategy.org",
        explorer_host: None,
        tablebase_host: None,
        has_cloud_eval: false,
        has_fide: false,
        authorize_path: "/oauth",
        token_path: "/api/token",
        account_path: "/api/account",
        player_scope: "board:play",
        engine_scope: "bot:play",
        provider_engine_name: "Engine do PlayStrategy",
    };

    /// O perfil de um Provedor da família, se `id` for um deles.
    pub fn of(id: ProviderId) -> Option<Self> {
        match id {
            ProviderId::Lichess => Some(Self::LICHESS),
            ProviderId::PlayStrategy => Some(Self::PLAYSTRATEGY),
            _ => None,
        }
    }

    /// Só xadrez padrão (as variantes que o catálogo aceita para o Provedor).
    pub fn accepts_variant(&self, key: &str) -> bool {
        chess_net::provider(self.id).accepts_variant(key)
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("https://{}{path}", self.api_host)
    }

    /// Chave gravada na Partida (`OnlineGame::provider`): a do catálogo para o jogador, com
    /// o sufixo de Engine para a Partida da Engine online.
    pub(crate) fn game_key(&self, engine: bool) -> String {
        if engine {
            format!("{}{}", self.id.key(), chess_game::ENGINE_GAME_SUFFIX)
        } else {
            self.id.key().to_owned()
        }
    }
}
