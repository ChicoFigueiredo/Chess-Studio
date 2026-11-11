#![allow(clippy::unwrap_used)]
//! Download em fluxo de corpo binário (#238): pelo mesmo gate, em pedaços, sem cache, com
//! progresso e encerrado cedo por quem lê. Servidor falso; nenhuma chamada real (CX-17).

mod common;

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{DownloadEvent, FakeClock, Net, NetError, ProviderId, Request};
use common::*;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

const PATH: &str = "/lichess_db_puzzle.csv.zst";
const WAIT: Duration = Duration::from_secs(10);

fn net_with_database(server: &FakeServer, clock: &Arc<FakeClock>) -> Net {
    let net = Net::open(config(server, clock).route("database.lichess.org", &server.base_url()))
        .expect("abre a rede");
    net.set_enabled(ProviderId::Lichess, true).unwrap();
    net
}

fn request() -> Request {
    Request::get(
        ProviderId::Lichess,
        format!("https://database.lichess.org{PATH}"),
    )
}

/// Corpo grande e sem padrão simples, para a ordem dos bytes importar.
fn body(len: usize) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15_u64;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

#[test]
fn o_corpo_chega_inteiro_e_em_ordem_com_o_tamanho_anunciado_e_sem_ir_para_o_cache() {
    let clock = clock();
    let server = server(&clock);
    let bytes = body(1_500_000);
    server.route(PATH, FakeResponse::ok(bytes.clone()));
    let net = net_with_database(&server, &clock);

    let download = net.download(request());
    let meter = download.meter();
    let mut reader = download.into_reader();
    let mut got = Vec::new();
    reader.read_to_end(&mut got).unwrap();

    assert_eq!(got, bytes);
    assert_eq!(meter.received(), bytes.len() as u64);
    assert_eq!(meter.total(), Some(bytes.len() as u64));
    // O corpo não é guardado no banco da rede.
    assert!(net.read_cache(&request()).unwrap().is_none());
}

#[test]
fn quem_le_devagar_segura_a_conexao_e_fechar_cedo_nao_baixa_o_resto() {
    let clock = clock();
    let server = server(&clock);
    let total = 24 * 1024 * 1024;
    server.route(PATH, FakeResponse::ok(body(total)));
    let net = net_with_database(&server, &clock);

    let download = net.download(request());
    let meter = download.meter();
    let first = loop {
        match download.next_timeout(WAIT).expect("evento") {
            DownloadEvent::Started { total: announced } => {
                assert_eq!(announced, Some(total as u64));
            }
            DownloadEvent::Chunk(bytes) => break bytes,
            other => panic!("inesperado: {other:?}"),
        }
    };
    assert!(!first.is_empty() && first.len() <= 64 * 1024);
    // Dá tempo de a thread de rede encher o que o canal comporta e parar de ler.
    std::thread::sleep(Duration::from_millis(400));
    drop(download);
    assert!(
        meter.received() < (total / 8) as u64,
        "baixou {} de {total} bytes",
        meter.received()
    );
}

#[test]
fn provedor_desligado_nao_abre_conexao_nenhuma() {
    let clock = clock();
    let server = server(&clock);
    server.route(PATH, FakeResponse::ok(body(1000)));
    let net = net_with_database(&server, &clock);
    net.set_enabled(ProviderId::Lichess, false).unwrap();

    let download = net.download(request());
    assert_eq!(
        download.next_timeout(WAIT),
        Some(DownloadEvent::Failed(NetError::ProviderDisabled {
            provider: ProviderId::Lichess
        }))
    );
    assert!(server.requests().is_empty());
}

#[test]
fn so_baixa_o_que_o_catalogo_marca_como_download() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/cloud-eval", FakeResponse::ok(body(10)));
    let net = net_with_database(&server, &clock);

    let download = net.download(Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/cloud-eval",
    ));
    assert!(matches!(
        download.next_timeout(WAIT),
        Some(DownloadEvent::Failed(NetError::EndpointNotAllowed { .. }))
    ));
    assert!(server.requests().is_empty());
}

#[test]
fn resposta_de_erro_vira_erro_tipado_pela_leitura() {
    let clock = clock();
    let server = server(&clock);
    server.route(PATH, FakeResponse::status(503));
    let net = net_with_database(&server, &clock);

    let mut reader = net.download(request()).into_reader();
    let error = reader.read(&mut [0u8; 16]).unwrap_err();
    let net_error = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<NetError>())
        .expect("carrega o erro da rede");
    assert!(matches!(
        net_error,
        NetError::Http { status: 503, .. } | NetError::Unreachable { .. }
    ));
}

fn redirect(to: &str) -> FakeResponse {
    FakeResponse::status(302).header("Location", to)
}

fn failure_of(net: &Net) -> NetError {
    match net.download(request()).next_timeout(WAIT).expect("evento") {
        DownloadEvent::Failed(err) => err,
        other => panic!("inesperado: {other:?}"),
    }
}

fn read_all(net: &Net) -> Vec<u8> {
    let mut got = Vec::new();
    net.download(request())
        .into_reader()
        .read_to_end(&mut got)
        .unwrap();
    got
}

#[test]
fn segue_redirecionamento_para_outro_endereco_de_download_do_catalogo() {
    let clock = clock();
    let server = server(&clock);
    let bytes = body(5000);
    server.route(PATH, redirect("https://database.lichess.org/outro.zst"));
    server.route("/outro.zst", FakeResponse::ok(bytes.clone()));
    let net = net_with_database(&server, &clock);
    assert_eq!(read_all(&net), bytes);
}

#[test]
fn segue_redirecionamento_relativo_no_mesmo_host() {
    let clock = clock();
    let server = server(&clock);
    let bytes = body(300);
    server.route(PATH, redirect("/espelho/pacote.zst"));
    server.route("/espelho/pacote.zst", FakeResponse::ok(bytes.clone()));
    let net = net_with_database(&server, &clock);
    assert_eq!(read_all(&net), bytes);
}

#[test]
fn recusa_redirecionamento_para_fora_do_catalogo_e_nao_pede_la() {
    let clock = clock();
    let server = server(&clock);
    server.route(PATH, redirect("https://evil.example.com/pacote.zst"));
    let net = net_with_database(&server, &clock);
    assert!(matches!(failure_of(&net), NetError::RedirectRefused { .. }));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn recusa_redirecionamento_para_outra_capacidade_sem_https_porta_ou_consulta() {
    for target in [
        "https://lichess.org/api/cloud-eval",
        "https://lichess.org/api/token",
        "http://database.lichess.org/outro.zst",
        "https://database.lichess.org:8443/outro.zst",
        "https://database.lichess.org/outro.zst?assinatura=abc",
        "//evil.example.com/pacote.zst",
        "",
    ] {
        let clock = clock();
        let server = server(&clock);
        server.route(PATH, redirect(target));
        let net = net_with_database(&server, &clock);
        assert!(
            matches!(failure_of(&net), NetError::RedirectRefused { .. }),
            "{target}"
        );
        assert_eq!(server.requests().len(), 1, "{target}");
    }
}

#[test]
fn laco_de_redirecionamentos_para_depois_do_limite_de_saltos() {
    let clock = clock();
    let server = server(&clock);
    server.route(PATH, redirect("https://database.lichess.org/laco.zst"));
    server.route(
        "/laco.zst",
        redirect("https://database.lichess.org/laco.zst"),
    );
    let net = net_with_database(&server, &clock);
    assert!(matches!(failure_of(&net), NetError::RedirectRefused { .. }));
    assert_eq!(server.requests().len(), 4);
}

#[test]
fn redirecionamento_nao_vale_para_consulta_comum() {
    let clock = clock();
    let server = server(&clock);
    server.route(PATH, redirect("https://database.lichess.org/outro.zst"));
    let net = net_with_database(&server, &clock);
    let err = net.fetch(request()).wait().unwrap_err();
    assert!(matches!(err, NetError::RedirectRefused { .. }), "{err:?}");
}
