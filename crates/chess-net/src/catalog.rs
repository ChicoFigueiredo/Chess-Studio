//! Catálogo de Provedores: dados estáticos e versionados, fonte da verdade da Central de
//! Conexões e do gate de rede. Vem da matriz 2.1/4.2 da especificação de integrações
//! (`docs/integracoes/`), filtrada pelas decisões CX-01..CX-20.
//!
//! Três ideias guiam os dados:
//!
//! - **O que o Provedor libera** ([`Service`]): capacidade, como se identifica, custo e
//!   situação. "Planejado" quer dizer que o Provedor oferece pelo contrato e o app ainda não
//!   tem o adaptador (a etapa vem junto); "requer aprovação", "só importação manual" e "não
//!   confirmado" nunca saem para a rede.
//! - **Para onde se pode ir** ([`Endpoint`]): host + prefixo de caminho, cada um com a sua
//!   capacidade e a licença dos dados. Um pedido fora desta lista é recusado (allowlist
//!   contra SSRF, 9.1.3), e é daqui que o gate tira se o pedido calcula (Trava de fair play).
//! - **Só xadrez padrão** ([`Variants`]): o núcleo de regras só conhece o xadrez padrão; as
//!   variantes que o Provedor oferece ficam nomeadas como recusadas.
//!
//! Ao mudar dados que alteram o que é enviado ou liberado, suba [`CATALOG_VERSION`] e a data
//! de revisão do Provedor.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::time::Duration;

/// Versão dos dados do catálogo; gravada junto do consentimento.
pub const CATALOG_VERSION: u32 = 8;

/// Provedor do catálogo. A chave ([`ProviderId::key`]) é o que se grava no banco e na Origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProviderId {
    Lichess,
    PlayStrategy,
    Fics,
    ChessCom,
    ChessApi,
    StockfishApi,
    StockfishOnline,
    Chessigma,
    QueenAlice,
    ChessBase,
    Twic,
    PgnMentor,
    Fide,
    PgnBase,
    ChessTempo,
    Chessify,
    ChessUrge,
    PyChessVariants,
    Icc,
}

impl ProviderId {
    pub const ALL: [ProviderId; 19] = [
        Self::Lichess,
        Self::PlayStrategy,
        Self::Fics,
        Self::ChessCom,
        Self::ChessApi,
        Self::StockfishApi,
        Self::StockfishOnline,
        Self::Chessigma,
        Self::QueenAlice,
        Self::ChessBase,
        Self::Twic,
        Self::PgnMentor,
        Self::Fide,
        Self::PgnBase,
        Self::ChessTempo,
        Self::Chessify,
        Self::ChessUrge,
        Self::PyChessVariants,
        Self::Icc,
    ];

    /// Chave estável (banco, Origem, chaves Fluent da interface).
    pub const fn key(self) -> &'static str {
        match self {
            Self::Lichess => "lichess",
            Self::PlayStrategy => "playstrategy",
            Self::Fics => "fics",
            Self::ChessCom => "chesscom",
            Self::ChessApi => "chess-api",
            Self::StockfishApi => "stockfishapi",
            Self::StockfishOnline => "stockfish-online",
            Self::Chessigma => "chessigma",
            Self::QueenAlice => "queenalice",
            Self::ChessBase => "chessbase",
            Self::Twic => "twic",
            Self::PgnMentor => "pgnmentor",
            Self::Fide => "fide",
            Self::PgnBase => "pgnbase",
            Self::ChessTempo => "chesstempo",
            Self::Chessify => "chessify",
            Self::ChessUrge => "chessurge",
            Self::PyChessVariants => "pychess-variants",
            Self::Icc => "icc",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.key() == key)
    }

    pub fn info(self) -> &'static Provider {
        provider(self)
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(provider(*self).name)
    }
}

impl Serialize for ProviderId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        Self::from_key(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("Provedor desconhecido: {key}")))
    }
}

/// Categoria principal do Provedor na Central (4.1, item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Jogo online.
    OnlinePlay,
    /// Engine remota.
    Engine,
    /// Dados e consultas.
    Data,
    /// Ratings.
    Ratings,
    /// Importação de arquivos.
    FileImport,
}

/// O que um pedido faz. O gate deriva a capacidade do endereço (catálogo), nunca de quem
/// chama: não há como um pedido que calcula se declarar "só consulta".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Jogar (Lances, desafios, fluxo da Partida online).
    Play,
    /// Engine online: Engine local jogando por uma Conexão de Engine.
    OnlineEngine,
    /// Explorador: estatística de aberturas.
    Explore,
    /// Finais exatos (tablebase).
    Endgames,
    /// Avaliação em cache.
    CachedEval,
    /// Engine remota: análise calculada sob demanda.
    Analyze,
    /// Consultar perfis, ratings e Partidas públicas (inclui importar Partidas pela API).
    Query,
    /// Baixar pacotes e bases (problemas e Partidas CC0).
    Download,
    /// Importação manual de PGN que o usuário baixou (sem rede).
    ImportPgn,
    /// Só identidade: nome ou ID público vinculado ao Perfil local (sem rede).
    IdentityOnly,
}

impl Capability {
    /// Calcula algo sobre a Posição: bloqueado pela Trava de fair play (ADR-0007, CX-08).
    pub const fn computes(self) -> bool {
        matches!(
            self,
            Self::Explore | Self::Endgames | Self::CachedEval | Self::Analyze
        )
    }

    /// Envolve rede. Importação manual e identidade são locais.
    pub const fn uses_network(self) -> bool {
        !matches!(self, Self::ImportPgn | Self::IdentityOnly)
    }

    /// Rótulo em português (diagnóstico; a interface usa Fluent).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Play => "Jogar",
            Self::OnlineEngine => "Engine online",
            Self::Explore => "Explorador",
            Self::Endgames => "Finais",
            Self::CachedEval => "Avaliação pronta",
            Self::Analyze => "Analisar",
            Self::Query => "Consultar",
            Self::Download => "Baixar",
            Self::ImportPgn => "Importar PGN",
            Self::IdentityOnly => "Só identidade",
        }
    }
}

/// Método real de identificação (4.1, item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthMethod {
    /// Nenhum: API pública.
    None,
    /// Nome ou ID público.
    PublicId,
    /// Autorização pelo navegador (OAuth2 com PKCE S256).
    OAuth2Pkce,
    /// Token pessoal.
    PersonalToken,
    /// Chave de API.
    ApiKey,
    /// Usuário e senha, só em protocolo aprovado e transporte seguro comprovado (CX-18).
    UsernamePasswordApprovedProtocol,
    /// Só importação manual.
    ManualImportOnly,
    /// Requer aprovação/parceria do Provedor.
    PartnerApproval,
}

impl AuthMethod {
    pub const fn uses_password(self) -> bool {
        matches!(self, Self::UsernamePasswordApprovedProtocol)
    }

    /// Precisa de um segredo do Cofre (etapa 3).
    pub const fn needs_secret(self) -> bool {
        matches!(
            self,
            Self::OAuth2Pkce
                | Self::PersonalToken
                | Self::ApiKey
                | Self::UsernamePasswordApprovedProtocol
        )
    }
}

/// Custo e elegibilidade (4.1, item 4). A ordem é a da Central: gratuitos primeiro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cost {
    FreeNoSignup,
    FreeWithSignup,
    FreeLimited,
    PaidOptional,
    NeedsApproval,
    NotConfirmed,
}

/// Situação de um serviço do Provedor no app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceStatus {
    /// Pronto: o app já usa.
    Ready,
    /// O Provedor oferece pelo contrato; o adaptador chega na etapa indicada.
    Planned { stage: u8 },
    /// Requer aprovação do Provedor: nada sai para a rede.
    NeedsApproval,
    /// Só importação manual de arquivo.
    ManualImport,
    /// API ou termos não confirmados: nada sai para a rede.
    NotConfirmed,
    /// Pago, opcional: fora do modo padrão de custo zero.
    PaidOptional,
}

impl ServiceStatus {
    /// O gate só deixa sair pedidos de serviços prontos ou planejados.
    pub const fn allows_network(self) -> bool {
        matches!(self, Self::Ready | Self::Planned { .. })
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Ready => 0,
            Self::Planned { .. } => 1,
            Self::ManualImport => 2,
            Self::NeedsApproval => 3,
            Self::PaidOptional => 4,
            Self::NotConfirmed => 5,
        }
    }
}

/// Uma capacidade do Provedor: como se identifica, quanto custa e em que situação está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Service {
    pub capability: Capability,
    pub auth: &'static [AuthMethod],
    pub cost: Cost,
    pub status: ServiceStatus,
    /// Restrição ou detalhe específico, em português.
    pub note: &'static str,
}

impl Service {
    /// O serviço só funciona com uma Conexão autorizada (segredo do Cofre).
    pub fn requires_connection(&self) -> bool {
        !self.auth.is_empty() && self.auth.iter().all(|a| a.needs_secret())
    }
}

/// Endereço permitido: host exato + prefixo de caminho, com a capacidade e a licença dos
/// dados que vêm dali.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub host: &'static str,
    pub path_prefix: &'static str,
    pub capability: Capability,
    pub license: &'static str,
    /// Recebe o `Authorization` da Conexão (o token do Cofre). Só os endereços em que o
    /// Provedor documenta autenticação; os demais nunca veem o token, mesmo que o pedido
    /// nomeie uma Conexão.
    pub takes_token: bool,
}

/// Token bucket: rajada de até `burst` pedidos, depois um a cada `every`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub burst: u32,
    pub every: Duration,
}

impl RateLimit {
    pub const fn new(burst: u32, every: Duration) -> Self {
        Self { burst, every }
    }
}

/// Variantes nas chaves do próprio Provedor. Só as de `standard` são aceitas; `refused`
/// nomeia as que o Provedor oferece e o app recusa explicitamente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variants {
    pub standard: &'static [&'static str],
    pub refused: &'static [&'static str],
}

/// Ficha de um Provedor (`ProviderDefinition` da especificação 4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    pub id: ProviderId,
    pub name: &'static str,
    pub category: Category,
    /// O que o Provedor permite, por capacidade.
    pub services: &'static [Service],
    /// Para onde a rede pode ir (vazio: nada sai por HTTP).
    pub endpoints: &'static [Endpoint],
    pub docs_url: &'static str,
    pub terms_url: &'static str,
    /// Resumo da licença dos dados (a de cada endereço está no [`Endpoint`]).
    pub data_license: &'static str,
    pub restrictions: &'static [&'static str],
    pub variants: Variants,
    pub rate_limit: RateLimit,
    /// O que será enviado e para onde: o aviso do consentimento.
    pub sends: &'static str,
    /// Data da última revisão dos dados (AAAA-MM-DD).
    pub reviewed_on: &'static str,
}

impl Provider {
    pub fn service(&self, capability: Capability) -> Option<&'static Service> {
        let services: &'static [Service] = self.services;
        services.iter().find(|s| s.capability == capability)
    }

    /// O menor custo entre os serviços (para "gratuitos primeiro").
    pub fn cost(&self) -> Cost {
        self.services
            .iter()
            .map(|s| s.cost)
            .min()
            .unwrap_or(Cost::NotConfirmed)
    }

    /// A melhor situação entre os serviços.
    pub fn status(&self) -> ServiceStatus {
        self.services
            .iter()
            .map(|s| s.status)
            .min_by_key(|s| s.rank())
            .unwrap_or(ServiceStatus::NotConfirmed)
    }

    /// Algum serviço com rede que o Provedor libera: só esses Provedores se ligam.
    pub fn uses_network(&self) -> bool {
        self.services
            .iter()
            .any(|s| s.capability.uses_network() && s.status.allows_network())
    }

    pub fn auth_methods(&self) -> Vec<AuthMethod> {
        let mut out = Vec::new();
        for method in self.services.iter().flat_map(|s| s.auth.iter().copied()) {
            if !out.contains(&method) {
                out.push(method);
            }
        }
        out
    }

    /// Só xadrez padrão (o núcleo de regras não conhece variantes).
    pub fn accepts_variant(&self, key: &str) -> bool {
        self.variants.standard.contains(&key)
    }

    /// Endereço do catálogo para `host` + `path` (prefixo mais longo).
    pub(crate) fn endpoint_for(&self, host: &str, path: &str) -> Option<&'static Endpoint> {
        let endpoints: &'static [Endpoint] = self.endpoints;
        endpoints
            .iter()
            .filter(|e| e.host == host && path.starts_with(e.path_prefix))
            .max_by_key(|e| e.path_prefix.len())
    }
}

/// O catálogo completo, gratuitos primeiro.
pub fn catalog() -> &'static [Provider] {
    &CATALOG
}

pub fn provider(id: ProviderId) -> &'static Provider {
    // Todo `ProviderId` está no catálogo (teste `every_provider_of_the_matrix_is_in_the_catalog`).
    CATALOG.iter().find(|p| p.id == id).unwrap_or(&CATALOG[0])
}

// ---------------------------------------------------------------------------------------
// Dados (revisão de 11/11/2026; especificação seções 2.1, 3 e 4.2)
// ---------------------------------------------------------------------------------------

const REVIEWED: &str = "2026-11-11";
const SECOND: Duration = Duration::from_secs(1);
const STANDARD_ONLY: Variants = Variants {
    standard: &["standard"],
    refused: &[],
};
const DEFAULT_RATE: RateLimit = RateLimit::new(2, SECOND);

const fn svc(
    capability: Capability,
    auth: &'static [AuthMethod],
    cost: Cost,
    status: ServiceStatus,
    note: &'static str,
) -> Service {
    Service {
        capability,
        auth,
        cost,
        status,
        note,
    }
}

/// Endereço que nunca recebe o token da Conexão.
const fn at(
    host: &'static str,
    path_prefix: &'static str,
    capability: Capability,
    license: &'static str,
) -> Endpoint {
    Endpoint {
        host,
        path_prefix,
        capability,
        license,
        takes_token: false,
    }
}

/// Endereço autenticado: recebe o `Authorization` da Conexão do mesmo Provedor.
const fn authed(
    host: &'static str,
    path_prefix: &'static str,
    capability: Capability,
    license: &'static str,
) -> Endpoint {
    Endpoint {
        takes_token: true,
        ..at(host, path_prefix, capability, license)
    }
}

use AuthMethod as A;
use Capability as C;
use Cost as K;
use ServiceStatus as S;

const LICHESS_TERMS: &str = "Termos do Lichess";
const LICHESS_VARIANTS: &[&str] = &[
    "chess960",
    "crazyhouse",
    "antichess",
    "atomic",
    "horde",
    "kingOfTheHill",
    "racingKings",
    "threeCheck",
];

static CATALOG: [Provider; 19] = [
    // ---------------- Gratuitos sem cadastro ----------------
    Provider {
        id: ProviderId::Lichess,
        name: "Lichess",
        category: Category::OnlinePlay,
        services: &[
            svc(
                C::Explore,
                // Só com segredo do Cofre: sem Conexão, o gate recusa antes da rede (#224).
                &[A::OAuth2Pkce, A::PersonalToken],
                K::FreeWithSignup,
                S::Ready,
                "Gratuito com cadastro: requer Conexão autorizada do Lichess; sem ela o serviço recusa o pedido (medido em 2026-11-11). Mestres, Lichess e jogador",
            ),
            svc(
                C::CachedEval,
                &[A::None],
                K::FreeNoSignup,
                S::Ready,
                "Só o que já está em cache; \"sem avaliação\" é resultado válido, nunca cálculo sob demanda",
            ),
            svc(
                C::Endgames,
                &[A::None],
                K::FreeNoSignup,
                S::Ready,
                "Finais exatos Syzygy, até sete peças",
            ),
            svc(
                C::Query,
                &[A::None, A::PublicId],
                K::FreeNoSignup,
                S::Ready,
                "Partidas públicas, perfis e rating FIDE reproduzido pelo Lichess",
            ),
            svc(
                C::Download,
                &[A::None],
                K::FreeNoSignup,
                S::Ready,
                "Pacotes de problemas CC0, baixados só quando a pessoa pede; nunca a base inteira",
            ),
            svc(
                C::Play,
                &[A::OAuth2Pkce, A::PersonalToken],
                K::FreeWithSignup,
                S::Ready,
                "Partidas online pelo tabuleiro conectado; Engine proibida em jogo contra pessoa",
            ),
            svc(
                C::OnlineEngine,
                &[A::OAuth2Pkce, A::PersonalToken],
                K::FreeWithSignup,
                S::Ready,
                "Bot API (bot:play) com conta BOT separada e irreversível",
            ),
        ],
        endpoints: &[
            // Desde 2026-11-11 o Explorador responde 401 sem token: recebe o da Conexão.
            authed(
                "explorer.lichess.org",
                "/",
                C::Explore,
                "Termos do Lichess; corpus de mestres sem licença CC0 confirmada",
            ),
            at(
                "tablebase.lichess.org",
                "/",
                C::Endgames,
                "Termos do Lichess (tablebases Syzygy)",
            ),
            // O host real dos finais é `.ovh` (conferido por consulta manual em 2026-11-11);
            // o `.org` acima segue na lista por compatibilidade com a especificação.
            at(
                "tablebase.lichess.ovh",
                "/",
                C::Endgames,
                "Termos do Lichess (tablebases Syzygy)",
            ),
            at(
                "database.lichess.org",
                "/",
                C::Download,
                "CC0-1.0 (exportações gerais; conferir cada conjunto)",
            ),
            at(
                "lichess.org",
                "/api/cloud-eval",
                C::CachedEval,
                LICHESS_TERMS,
            ),
            at(
                "lichess.org",
                "/api/fide/",
                C::Query,
                "Dados da FIDE reproduzidos pelo Lichess; direitos da FIDE",
            ),
            at("lichess.org", "/api/games/user/", C::Query, LICHESS_TERMS),
            at("lichess.org", "/game/export/", C::Query, LICHESS_TERMS),
            at("lichess.org", "/api/user/", C::Query, LICHESS_TERMS),
            // Troca do código por token (vai sem token) e revogação (`DELETE`, com o token):
            // não calcula nem joga; o gate trata como consulta.
            authed("lichess.org", "/api/token", C::Query, LICHESS_TERMS),
            authed("lichess.org", "/api/account", C::Play, LICHESS_TERMS),
            authed("lichess.org", "/api/stream/event", C::Play, LICHESS_TERMS),
            authed("lichess.org", "/api/board/", C::Play, LICHESS_TERMS),
            authed("lichess.org", "/api/challenge/", C::Play, LICHESS_TERMS),
            authed("lichess.org", "/api/bot/", C::OnlineEngine, LICHESS_TERMS),
        ],
        docs_url: "https://lichess-org.github.io/api/",
        terms_url: "https://lichess.org/terms-of-service",
        data_license: "Base, problemas e aberturas CC0; Explorador e consultas sob os termos do Lichess",
        restrictions: &[
            "Um pedido por vez; quando o Lichess pedir uma pausa, esperar o prazo informado ou 60 s",
            "Engine proibida em Partida contra pessoa (inclui correspondência)",
            "Conta BOT separada da conta do jogador",
            "Nunca pedir a senha do Lichess no app",
        ],
        variants: Variants {
            standard: &["standard", "fromPosition"],
            refused: LICHESS_VARIANTS,
        },
        rate_limit: RateLimit::new(4, SECOND),
        sends: "As Posições e os Lances que você abre no Explorador (explorer.lichess.org), na \
                avaliação em cache e nas consultas de Partidas públicas (lichess.org), nos \
                finais (tablebase.lichess.ovh e tablebase.lichess.org) e nos pacotes de \
                problemas que você baixar (database.lichess.org); nomes e IDs FIDE públicos que \
                você pedir. Com uma Conexão autorizada, a chave dela vai só para lichess.org \
                (conta e jogo) e explorer.lichess.org, junto dos seus Lances e desafios. Nada do \
                Perfil local vai junto.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::ChessCom,
        name: "Chess.com",
        category: Category::Data,
        services: &[
            svc(
                C::Query,
                &[A::None, A::PublicId],
                K::FreeNoSignup,
                S::Ready,
                "Dados públicos somente leitura: perfil, arquivos mensais e PGN públicos",
            ),
            svc(
                C::IdentityOnly,
                &[A::PublicId],
                K::FreeNoSignup,
                S::Ready,
                "Nome de usuário público, opcional",
            ),
            svc(
                C::Play,
                &[A::PartnerApproval],
                K::NeedsApproval,
                S::NeedsApproval,
                "OAuth e tabuleiro conectado só com parceria aprovada; sem botão de jogar",
            ),
        ],
        endpoints: &[at(
            "api.chess.com",
            "/pub/",
            C::Query,
            "Dados públicos do Chess.com; uso pessoal, sem espelho ou mineração em massa",
        )],
        docs_url: "https://www.chess.com/news/view/published-data-api",
        terms_url: "https://support.chess.com/en/articles/9650547-what-is-the-pubapi-and-how-do-i-use-it",
        data_license: "Dados públicos do Chess.com sob os termos do serviço; sem redistribuição",
        restrictions: &[
            "Somente leitura: nunca enviar Lances, simular login ou automatizar o site",
            "Acesso serial; a CDN pode servir cache de até 12 h",
            "Exibir aviso de não afiliação quando exigido",
        ],
        variants: Variants {
            standard: &["chess"],
            refused: &[
                "chess960",
                "bughouse",
                "crazyhouse",
                "kingofthehill",
                "threecheck",
                "oddschess",
            ],
        },
        rate_limit: RateLimit::new(2, SECOND),
        sends: "Os nomes de usuário públicos que você consultar e os meses de Partidas pedidos, \
                para api.chess.com. Nenhuma senha.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::Fide,
        name: "FIDE",
        category: Category::Ratings,
        services: &[
            svc(
                C::IdentityOnly,
                &[A::PublicId],
                K::FreeNoSignup,
                S::Planned { stage: 2 },
                "FIDE ID público; o rating é consultado pelo Lichess (Provedor Lichess)",
            ),
            svc(
                C::Download,
                &[A::ManualImportOnly],
                K::FreeNoSignup,
                S::ManualImport,
                "Listas oficiais TXT/XML baixadas pelo usuário; não é base de Partidas",
            ),
        ],
        endpoints: &[],
        docs_url: "https://ratings.fide.com/download_lists.phtml",
        terms_url: "https://ratings.fide.com/download_lists.phtml",
        data_license: "Reprodução e armazenamento restritos pela FIDE; só uso pessoal compatível",
        restrictions: &["Nenhuma senha: só ID público", "Sem republicar as listas"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada vai para a FIDE: o FIDE ID é guardado no Perfil local e o rating vem pelo \
                Provedor Lichess, com o consentimento dele.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::Chessigma,
        name: "Chessigma",
        category: Category::Data,
        services: &[svc(
            C::Query,
            &[A::None],
            K::FreeNoSignup,
            S::Planned { stage: 2 },
            "Partidas, metadados e problemas; o cálculo roda no navegador, não no servidor",
        )],
        // Host da API a confirmar na documentação antes do adaptador; até lá nada sai.
        endpoints: &[],
        docs_url: "https://www.chessigma.com/developers",
        terms_url: "https://www.chessigma.com/developers",
        data_license: "Exige atribuição ao Chessigma",
        restrictions: &[
            "Limite global de 30 pedidos por minuto",
            "Atribuição obrigatória",
        ],
        variants: STANDARD_ONLY,
        rate_limit: RateLimit::new(3, Duration::from_secs(2)),
        sends: "As consultas de Partidas, metadados e problemas que você fizer, para o \
                Chessigma (endereço da API confirmado no adaptador).",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::Twic,
        name: "TWIC",
        category: Category::FileImport,
        services: &[svc(
            C::ImportPgn,
            &[A::ManualImportOnly],
            K::FreeNoSignup,
            S::ManualImport,
            "Arquivos PGN por edição, baixados pelo usuário",
        )],
        endpoints: &[],
        docs_url: "https://theweekinchess.com/twic",
        terms_url: "https://theweekinchess.com/twic",
        data_license: "Edição gratuita só para uso pessoal; publicar ou compartilhar exige autorização",
        restrictions: &[
            "Sem coleta automática",
            "Nenhuma sincronização para catálogo público",
        ],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: o arquivo PGN é baixado por você e importado do disco.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::PgnMentor,
        name: "PGN Mentor",
        category: Category::FileImport,
        services: &[svc(
            C::ImportPgn,
            &[A::ManualImportOnly],
            K::FreeNoSignup,
            S::ManualImport,
            "PGNs por jogador, evento e abertura, baixados pelo usuário",
        )],
        endpoints: &[],
        docs_url: "https://www.pgnmentor.com/files.html",
        terms_url: "https://www.pgnmentor.com/files.html",
        data_license: "Direitos de redistribuição e uso comercial não confirmados",
        restrictions: &["Sem coleta automática nem redistribuição em massa"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: o arquivo PGN é baixado por você e importado do disco.",
        reviewed_on: REVIEWED,
    },
    // ---------------- Gratuitos com cadastro ----------------
    Provider {
        id: ProviderId::PlayStrategy,
        name: "PlayStrategy",
        category: Category::OnlinePlay,
        services: &[
            svc(
                C::Play,
                &[A::OAuth2Pkce],
                K::FreeWithSignup,
                S::Planned { stage: 5 },
                "Board API; Engine proibida em jogo humano",
            ),
            svc(
                C::OnlineEngine,
                &[A::OAuth2Pkce],
                K::FreeWithSignup,
                S::Planned { stage: 5 },
                "Conta BOT separada",
            ),
            svc(
                C::Query,
                &[A::None, A::PublicId],
                K::FreeNoSignup,
                S::Ready,
                "Perfil e exportação de Partidas públicas (importação sob demanda; só xadrez padrão)",
            ),
        ],
        endpoints: &[
            authed(
                "playstrategy.org",
                "/api/stream/event",
                C::Play,
                "Termos do PlayStrategy",
            ),
            // Mesma família de API do Lichess (#234): conta, desafios, troca e revogação do
            // token. Não há Explorador, avaliação em cache nem finais no PlayStrategy.
            authed(
                "playstrategy.org",
                "/api/account",
                C::Play,
                "Termos do PlayStrategy",
            ),
            authed(
                "playstrategy.org",
                "/api/challenge/",
                C::Play,
                "Termos do PlayStrategy",
            ),
            authed(
                "playstrategy.org",
                "/api/token",
                C::Query,
                "Termos do PlayStrategy",
            ),
            at(
                "playstrategy.org",
                "/api/user/",
                C::Query,
                "Termos do PlayStrategy",
            ),
            authed(
                "playstrategy.org",
                "/api/board/",
                C::Play,
                "Termos do PlayStrategy",
            ),
            authed(
                "playstrategy.org",
                "/api/bot/",
                C::OnlineEngine,
                "Termos do PlayStrategy",
            ),
            at(
                "playstrategy.org",
                "/api/games/user/",
                C::Query,
                "Termos do PlayStrategy",
            ),
        ],
        docs_url: "https://playstrategy.org/api",
        terms_url: "https://playstrategy.org/developers",
        data_license: "Termos do PlayStrategy",
        restrictions: &[
            "Uma requisição por vez; depois de 429, esperar um minuto",
            "Só xadrez padrão: as demais variantes (damas, shogi, xiangqi…) são recusadas",
            "Perfil de Provedor sobre os clientes da família do Lichess, com testes de contrato próprios",
        ],
        variants: Variants {
            standard: &["standard", "fromPosition"],
            refused: &[
                "chess960",
                "crazyhouse",
                "antichess",
                "atomic",
                "horde",
                "kingOfTheHill",
                "racingKings",
                "threeCheck",
                "fiveCheck",
                "noCastling",
                "linesOfAction",
                "shogi",
                "xiangqi",
                "flipello",
                "oware",
                "draughts",
                "go",
                "backgammon",
            ],
        },
        rate_limit: RateLimit::new(1, SECOND),
        sends: "Com uma Conexão autorizada, os seus Lances, desafios e consultas de Partidas, \
                para playstrategy.org.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::Fics,
        name: "FICS",
        category: Category::OnlinePlay,
        // Pronto como visitante (#254): o app só entra como visitante, sem senha (FC-01); conta
        // registrada só com canal cifrado comprovado, que o FICS não tem (CX-18).
        services: &[svc(
            C::Play,
            &[A::None],
            K::FreeNoSignup,
            S::Ready,
            "Protocolo próprio do FICS (não é a web), em texto puro: só como visitante, sem senha",
        )],
        // ICS não passa pelo cliente HTTP: o endereço ICS está em `crate::ics_endpoints`.
        endpoints: &[],
        docs_url: "https://www.freechess.org/interfaces.html",
        terms_url: "https://www.freechess.org/",
        data_license: "Termos do FICS",
        restrictions: &[
            "Senha nunca trafega sem criptografia",
            "Segredo do FICS nunca vai para outro Provedor",
        ],
        variants: Variants {
            standard: &["standard", "blitz", "lightning", "untimed"],
            refused: &[
                "wild",
                "bughouse",
                "crazyhouse",
                "suicide",
                "losers",
                "atomic",
            ],
        },
        rate_limit: DEFAULT_RATE,
        sends: "Lances e comandos do protocolo do FICS para freechess.org, como visitante, em texto \
                puro (sem criptografia); nenhuma senha.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::QueenAlice,
        name: "Queen Alice",
        category: Category::FileImport,
        services: &[
            svc(
                C::ImportPgn,
                &[A::ManualImportOnly],
                K::FreeWithSignup,
                S::ManualImport,
                "Até 25 Partidas concluídas por exportação, feita pela página",
            ),
            svc(
                C::IdentityOnly,
                &[A::PublicId],
                K::FreeWithSignup,
                S::Planned { stage: 2 },
                "Nome ou ID público, opcional, para a Origem das Partidas",
            ),
        ],
        endpoints: &[],
        docs_url: "https://www.queenalice.com/",
        terms_url: "https://www.queenalice.com/about.php",
        data_license: "Partidas do próprio usuário",
        restrictions: &[
            "Sem API externa: nunca login, polling ou Lance automatizado",
            "Engine proibida em Partida em andamento",
        ],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: o PGN é exportado por você no site e importado do disco.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::ChessBase,
        name: "ChessBase",
        category: Category::FileImport,
        services: &[
            svc(
                C::ImportPgn,
                &[A::ManualImportOnly],
                K::FreeWithSignup,
                S::ManualImport,
                "PGN exportado legalmente pelo usuário",
            ),
            svc(
                C::IdentityOnly,
                &[A::PublicId],
                K::FreeWithSignup,
                S::Planned { stage: 2 },
                "Apelido opcional, só para a Origem",
            ),
            svc(
                C::Play,
                &[A::PartnerApproval],
                K::NeedsApproval,
                S::NeedsApproval,
                "PlayChess só no ecossistema próprio",
            ),
            svc(
                C::Analyze,
                &[A::PartnerApproval],
                K::NeedsApproval,
                S::NeedsApproval,
                "Engine Cloud só no ecossistema próprio",
            ),
            svc(
                C::Download,
                &[A::PartnerApproval],
                K::PaidOptional,
                S::PaidOptional,
                "Mega/Big/Corr Database: licença comercial; .cbh/.cbv fora do escopo",
            ),
        ],
        endpoints: &[],
        docs_url: "https://help.chessbase.com/apps/en/the_live_database.htm",
        terms_url: "https://support.chessbase.com/en/content/details/CBAccount/30983b7b-118e-4328-a87d-3e0854ce9fb0",
        data_license: "Conforme o plano do usuário; assinatura não é licença de integração",
        restrictions: &[
            "Sem API pública confirmada",
            "Nunca pedir senha da conta ChessBase",
        ],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: o PGN é exportado por você e importado do disco.",
        reviewed_on: REVIEWED,
    },
    // ---------------- Gratuitos com limite ----------------
    Provider {
        id: ProviderId::ChessApi,
        name: "Chess-API.com",
        category: Category::Engine,
        services: &[svc(
            C::Analyze,
            &[A::None],
            K::FreeLimited,
            S::Ready,
            "Stockfish remoto; profundidade 18 e 100 ms de pensar no acesso comum; sem SLA; \
             sem chave (conferido por consulta de leitura em 2026-11-11)",
        )],
        endpoints: &[at(
            "chess-api.com",
            "/v1",
            C::Analyze,
            "Resultado de Engine (Stockfish) sob os termos do Chess-API.com",
        )],
        docs_url: "https://chess-api.com/",
        terms_url: "https://chess-api.com/",
        data_license: "Resultado de Engine sob os termos do Chess-API.com",
        restrictions: &["Só por escolha explícita de Engine remota", "Sem SLA"],
        variants: STANDARD_ONLY,
        rate_limit: RateLimit::new(2, Duration::from_secs(2)),
        sends: "A Posição analisada e a profundidade pedida, para chess-api.com.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::StockfishApi,
        name: "StockfishAPI.com",
        category: Category::Engine,
        services: &[svc(
            C::Analyze,
            &[A::None, A::ApiKey],
            K::FreeLimited,
            S::NotConfirmed,
            "Limites e exigência de cartão divergem na documentação: validar antes; nenhuma cobrança implícita",
        )],
        endpoints: &[],
        docs_url: "https://stockfishapi.com/docs/limits",
        terms_url: "https://stockfishapi.com/pricing",
        data_license: "Resultado de Engine sob os termos do StockfishAPI.com",
        restrictions: &["Custo automático R$ 0: nada habilitado até validar o plano gratuito"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada por enquanto: o serviço aguarda validação de termos e limites.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::StockfishOnline,
        name: "Stockfish.online",
        category: Category::Engine,
        services: &[svc(
            C::Analyze,
            &[A::None],
            K::FreeLimited,
            S::NotConfirmed,
            "Termos, estabilidade, cotas e esquema a validar",
        )],
        endpoints: &[],
        docs_url: "https://stockfish.online/",
        terms_url: "https://stockfish.online/",
        data_license: "Resultado de Engine sob os termos do Stockfish.online",
        restrictions: &["Nada habilitado até validar termos e cotas"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada por enquanto: o serviço aguarda validação de termos e limites.",
        reviewed_on: REVIEWED,
    },
    // ---------------- Pagos ----------------
    Provider {
        id: ProviderId::Icc,
        name: "ICC",
        category: Category::OnlinePlay,
        services: &[svc(
            C::Play,
            &[A::PartnerApproval],
            K::PaidOptional,
            S::PaidOptional,
            "Serviço comercial; cliente externo só se documentado ou contratado",
        )],
        endpoints: &[],
        docs_url: "https://www.chessclub.com/",
        terms_url: "https://www.chessclub.com/",
        data_license: "Comercial",
        restrictions: &["Fora do modo padrão de custo zero"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
    // ---------------- Não confirmados (cartões informativos) ----------------
    Provider {
        id: ProviderId::PgnBase,
        name: "PGNBase",
        category: Category::Data,
        services: &[svc(
            C::Query,
            &[A::None],
            K::NotConfirmed,
            S::NotConfirmed,
            "API pública não confirmada; sem scraping",
        )],
        endpoints: &[],
        docs_url: "https://www.pgnbase.com/",
        terms_url: "https://www.pgnbase.com/",
        data_license: "Não confirmada",
        restrictions: &["Sem scraping"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::ChessTempo,
        name: "ChessTempo",
        category: Category::Data,
        services: &[svc(
            C::Query,
            &[A::None],
            K::NotConfirmed,
            S::NotConfirmed,
            "Base consultável no navegador; API de reutilização não confirmada",
        )],
        endpoints: &[],
        docs_url: "https://chesstempo.com/game-database/",
        terms_url: "https://chesstempo.com/game-database/",
        data_license: "Não confirmada",
        restrictions: &["Sem scraping"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::Chessify,
        name: "Chessify",
        category: Category::Engine,
        services: &[svc(
            C::Analyze,
            &[A::PartnerApproval],
            K::NotConfirmed,
            S::NotConfirmed,
            "Plano no site não é contrato de API",
        )],
        endpoints: &[],
        docs_url: "https://chessify.me/pricing",
        terms_url: "https://chessify.me/pricing",
        data_license: "Não confirmada",
        restrictions: &["Conta gratuita não é API"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::ChessUrge,
        name: "ChessUrge",
        category: Category::Engine,
        services: &[svc(
            C::Analyze,
            &[A::None],
            K::NotConfirmed,
            S::NotConfirmed,
            "Beta; serviço e cobrança a revalidar",
        )],
        endpoints: &[],
        docs_url: "https://chessurge.com/",
        terms_url: "https://chessurge.com/",
        data_license: "Não confirmada",
        restrictions: &["Não é dependência de produção"],
        variants: STANDARD_ONLY,
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
    Provider {
        id: ProviderId::PyChessVariants,
        name: "PyChess Variants",
        category: Category::OnlinePlay,
        services: &[svc(
            C::Play,
            &[A::OAuth2Pkce],
            K::NotConfirmed,
            S::NotConfirmed,
            "Plataforma de variantes; protocolo externo a estudar",
        )],
        endpoints: &[],
        docs_url: "https://github.com/pychess/pychess-variants",
        terms_url: "https://github.com/pychess/pychess-variants",
        data_license: "Servidor AGPL; reuso de código exige estudo separado",
        restrictions: &["Foco em variantes, que o núcleo não suporta"],
        variants: Variants {
            standard: &["chess"],
            refused: &[
                "chess960",
                "crazyhouse",
                "atomic",
                "makruk",
                "shogi",
                "xiangqi",
                "janggi",
                "capablanca",
                "seirawan",
                "shako",
                "orda",
                "synochess",
            ],
        },
        rate_limit: DEFAULT_RATE,
        sends: "Nada: cartão informativo.",
        reviewed_on: REVIEWED,
    },
];
