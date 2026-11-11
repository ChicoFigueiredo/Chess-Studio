//! Catálogo de Provedores (matriz 2.1 e 4.2 da especificação): campos obrigatórios, nenhum
//! pedido de senha onde não há protocolo aprovado, gratuitos primeiro e variantes recusadas.

use chess_net::{
    AuthMethod, CATALOG_VERSION, Capability, ProviderId, ServiceStatus, catalog, provider,
};
use std::collections::HashSet;

#[test]
fn every_provider_of_the_matrix_is_in_the_catalog() {
    let ids: HashSet<ProviderId> = catalog().iter().map(|p| p.id).collect();
    for id in ProviderId::ALL {
        assert!(ids.contains(&id), "{id:?} fora do catálogo");
    }
    assert_eq!(catalog().len(), ProviderId::ALL.len(), "Provedor repetido");
    let names: Vec<&str> = catalog().iter().map(|p| p.name).collect();
    for expected in [
        "Lichess",
        "PlayStrategy",
        "FICS",
        "Chess.com",
        "Chess-API.com",
        "StockfishAPI.com",
        "Stockfish.online",
        "Chessigma",
        "Queen Alice",
        "ChessBase",
        "TWIC",
        "PGN Mentor",
        "FIDE",
    ] {
        assert!(names.contains(&expected), "{expected} fora do catálogo");
    }
    const { assert!(CATALOG_VERSION >= 1) };
}

#[test]
fn every_provider_has_the_mandatory_fields() {
    for p in catalog() {
        assert!(!p.name.is_empty());
        assert_eq!(ProviderId::from_key(p.id.key()), Some(p.id));
        assert!(p.docs_url.starts_with("https://"), "{}: docs", p.name);
        assert!(p.terms_url.starts_with("https://"), "{}: termos", p.name);
        assert!(!p.data_license.is_empty(), "{}: licença", p.name);
        assert!(!p.sends.is_empty(), "{}: aviso", p.name);
        assert!(!p.services.is_empty(), "{}: o que permite", p.name);
        let date = p.reviewed_on.as_bytes();
        assert!(
            date.len() == 10 && date[4] == b'-' && date[7] == b'-',
            "{}: revisão {}",
            p.name,
            p.reviewed_on
        );
        for s in p.services {
            assert!(
                !s.auth.is_empty(),
                "{}: {:?} sem identificação",
                p.name,
                s.capability
            );
        }
        assert!(p.rate_limit.burst >= 1);
    }
}

#[test]
fn manual_import_or_approval_never_declares_a_password() {
    for p in catalog() {
        let gated = matches!(
            p.status(),
            ServiceStatus::ManualImport | ServiceStatus::NeedsApproval
        );
        for s in p.services {
            let password = s.auth.iter().any(|a| a.uses_password());
            if gated
                || matches!(
                    s.status,
                    ServiceStatus::ManualImport | ServiceStatus::NeedsApproval
                )
            {
                assert!(!password, "{}: {:?} pede senha", p.name, s.capability);
            }
            if password {
                assert_eq!(
                    p.id,
                    ProviderId::Fics,
                    "senha só em protocolo aprovado (FICS)"
                );
            }
        }
    }
}

#[test]
fn manual_import_providers_have_no_network_endpoint() {
    for p in catalog() {
        for s in p.services {
            if matches!(
                s.status,
                ServiceStatus::ManualImport
                    | ServiceStatus::NeedsApproval
                    | ServiceStatus::NotConfirmed
                    | ServiceStatus::PaidOptional
            ) {
                assert!(
                    !p.endpoints.iter().any(|e| e.capability == s.capability),
                    "{}: endereço para {:?}, que o Provedor não libera",
                    p.name,
                    s.capability
                );
            }
        }
    }
}

#[test]
fn every_endpoint_belongs_to_one_provider_and_to_a_declared_service() {
    let mut hosts = std::collections::HashMap::new();
    for p in catalog() {
        for e in p.endpoints {
            let owner = hosts.entry(e.host).or_insert(p.id);
            assert_eq!(*owner, p.id, "{} em dois Provedores", e.host);
            assert!(e.path_prefix.starts_with('/'));
            assert!(!e.license.is_empty());
            assert!(e.capability.uses_network());
            let service = p
                .service(e.capability)
                .unwrap_or_else(|| panic!("{}: {:?} sem serviço", p.name, e.capability));
            assert!(
                service.status.allows_network(),
                "{}: {:?}",
                p.name,
                e.capability
            );
        }
    }
}

#[test]
fn free_providers_come_first() {
    let costs: Vec<_> = catalog().iter().map(|p| p.cost()).collect();
    let mut sorted = costs.clone();
    sorted.sort();
    assert_eq!(costs, sorted);
    assert_eq!(catalog()[0].id, ProviderId::Lichess);
}

#[test]
fn chess_com_reads_but_playing_needs_approval() {
    let chess_com = provider(ProviderId::ChessCom);
    assert!(
        chess_com
            .service(Capability::Query)
            .expect("leitura")
            .status
            .allows_network()
    );
    let play = chess_com.service(Capability::Play).expect("jogo listado");
    assert_eq!(play.status, ServiceStatus::NeedsApproval);
    assert_eq!(play.auth, &[AuthMethod::PartnerApproval]);
}

#[test]
fn computing_capabilities_are_the_ones_the_fair_play_guard_blocks() {
    let computes: Vec<_> = [
        Capability::Play,
        Capability::OnlineEngine,
        Capability::Explore,
        Capability::Endgames,
        Capability::CachedEval,
        Capability::Analyze,
        Capability::Query,
        Capability::Download,
        Capability::ImportPgn,
        Capability::IdentityOnly,
    ]
    .into_iter()
    .filter(|c| c.computes())
    .collect();
    assert_eq!(
        computes,
        vec![
            Capability::Explore,
            Capability::Endgames,
            Capability::CachedEval,
            Capability::Analyze
        ]
    );
}

#[test]
fn only_standard_chess_is_accepted_and_offered_variants_are_refused_by_name() {
    for p in catalog() {
        assert!(
            !p.variants.standard.is_empty(),
            "{}: sem xadrez padrão",
            p.name
        );
        for v in p.variants.refused {
            assert!(!p.variants.standard.contains(v), "{}: {v}", p.name);
            assert!(!p.accepts_variant(v), "{}: aceitaria {v}", p.name);
        }
        for v in p.variants.standard {
            assert!(p.accepts_variant(v));
        }
        assert!(!p.accepts_variant("variante-desconhecida"));
        let plays = p
            .services
            .iter()
            .any(|s| matches!(s.capability, Capability::Play | Capability::OnlineEngine));
        if plays && p.uses_network() {
            assert!(
                !p.variants.refused.is_empty(),
                "{}: joga e não lista as variantes recusadas",
                p.name
            );
        }
    }
    assert!(provider(ProviderId::Lichess).accepts_variant("standard"));
    assert!(!provider(ProviderId::Lichess).accepts_variant("chess960"));
}

#[test]
fn the_real_tablebase_host_is_in_the_catalog_as_exact_endgames() {
    let request = chess_net::Request::get(
        ProviderId::Lichess,
        "https://tablebase.lichess.ovh/standard",
    );
    let capability = request.capability().expect("no catálogo");
    assert_eq!(capability, Capability::Endgames);
    assert!(
        capability.computes(),
        "finais calculam: a Trava os bloqueia"
    );
    let endpoint = provider(ProviderId::Lichess)
        .endpoints
        .iter()
        .find(|e| e.host == "tablebase.lichess.ovh")
        .expect("endpoint");
    assert!(endpoint.license.contains("Lichess"));
}
