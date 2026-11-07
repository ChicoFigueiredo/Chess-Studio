//! Backends do Lc0 (C10, UCI-06): quais existem por plataforma, qual escolher sozinho, como
//! explicar uma falha (sem driver CUDA, sem DirectML…) e para qual backend de CPU recuar.
//!
//! Nem todo backend neural funciona em toda máquina: o Lc0 anuncia no handshake o que **o
//! binário instalado** sabe fazer (`option name Backend type combo … var cuda-auto …`), e a
//! escolha automática só considera o que foi anunciado. Ter o backend no binário não prova
//! que ele sobe (o driver ou a biblioteca pode faltar), então a falha vira erro claro e o
//! chamador recua para a CPU.

use crate::{Control, EngineOption, parse_options};

/// Fabricante da GPU (só o que muda a escolha do backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Other,
}

/// O que se sabe da máquina: `gpu = None` quando nenhuma GPU utilizável foi achada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardware {
    pub gpu: Option<GpuVendor>,
}

/// Onde o backend calcula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendClass {
    Gpu,
    Cpu,
    /// Backends de teste/composição do Lc0 (`random`, `check`, `multiplexing`…).
    Other,
}

/// Um backend conhecido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendInfo {
    /// Nome exato da opção `Backend` do Lc0.
    pub id: &'static str,
    pub label: &'static str,
    pub class: BackendClass,
    /// Sistemas (`std::env::consts::OS`) em que o backend existe nas builds oficiais ou
    /// compiláveis.
    pub os: &'static [&'static str],
    /// O que o backend exige da máquina.
    pub requires: &'static str,
}

const ALL: &[&str] = &["windows", "linux", "macos"];

/// Mapa de backends por plataforma (documentado em `docs/lc0-gpu.md`).
pub const BACKENDS: &[BackendInfo] = &[
    BackendInfo {
        id: "cuda-auto",
        label: "CUDA (automático)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA com driver CUDA",
    },
    BackendInfo {
        id: "cuda-fp16",
        label: "CUDA (FP16)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA com Tensor Cores (RTX)",
    },
    BackendInfo {
        id: "cuda",
        label: "CUDA",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA com driver CUDA",
    },
    BackendInfo {
        id: "cudnn-auto",
        label: "cuDNN (automático)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA, driver CUDA e cuDNN",
    },
    BackendInfo {
        id: "cudnn-fp16",
        label: "cuDNN (FP16)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA, driver CUDA e cuDNN",
    },
    BackendInfo {
        id: "cudnn",
        label: "cuDNN",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA, driver CUDA e cuDNN",
    },
    BackendInfo {
        id: "onnx-cuda",
        label: "ONNX (CUDA)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA, driver CUDA e onnxruntime com CUDA",
    },
    BackendInfo {
        id: "onnx-trt",
        label: "ONNX (TensorRT)",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "GPU NVIDIA e TensorRT",
    },
    BackendInfo {
        id: "onnx-dml",
        label: "ONNX (DirectML)",
        class: BackendClass::Gpu,
        os: &["windows"],
        requires: "GPU com DirectX 12 (NVIDIA, AMD ou Intel) e directml.dll",
    },
    BackendInfo {
        id: "metal",
        label: "Metal",
        class: BackendClass::Gpu,
        os: &["macos"],
        requires: "Mac com GPU Apple Silicon",
    },
    BackendInfo {
        id: "opencl",
        label: "OpenCL",
        class: BackendClass::Gpu,
        os: &["windows", "linux"],
        requires: "driver OpenCL",
    },
    BackendInfo {
        id: "blas",
        label: "CPU (BLAS)",
        class: BackendClass::Cpu,
        os: ALL,
        requires: "nenhum (OpenBLAS, oneDNN ou Accelerate)",
    },
    BackendInfo {
        id: "eigen",
        label: "CPU (Eigen)",
        class: BackendClass::Cpu,
        os: ALL,
        requires: "nenhum",
    },
    BackendInfo {
        id: "onnx-cpu",
        label: "CPU (ONNX)",
        class: BackendClass::Cpu,
        os: ALL,
        requires: "onnxruntime",
    },
];

/// Dados de um backend pelo nome (sem diferenciar maiúsculas).
pub fn backend_info(id: &str) -> Option<&'static BackendInfo> {
    BACKENDS.iter().find(|b| b.id.eq_ignore_ascii_case(id))
}

/// O backend roda na GPU?
pub fn is_gpu_backend(id: &str) -> bool {
    backend_info(id).is_some_and(|b| b.class == BackendClass::Gpu)
}

/// O backend existe neste sistema operacional?
pub fn supported_on(id: &str, os: &str) -> bool {
    backend_info(id).is_some_and(|b| b.os.contains(&os))
}

/// Backends que a Engine anuncia (`option name Backend type combo … var …`); vazio se ela
/// não tem a opção (Stockfish) ou se a opção não é uma lista.
pub fn advertised_backends(options: &[EngineOption]) -> Vec<String> {
    parse_options(options)
        .into_iter()
        .find_map(|spec| match spec.control {
            Control::Combo { vars, .. } if spec.name.eq_ignore_ascii_case("backend") => Some(vars),
            _ => None,
        })
        .unwrap_or_default()
}

/// Ordem de preferência da escolha automática, do melhor para o pior, só com backends que
/// existem no sistema e que a máquina tem chance de rodar.
pub fn auto_order(os: &str, hardware: Hardware) -> Vec<&'static str> {
    const NVIDIA_ORDER: &[&str] = &[
        "cuda-auto",
        "cuda-fp16",
        "cudnn-auto",
        "cudnn-fp16",
        "cuda",
        "cudnn",
        "onnx-cuda",
        "onnx-trt",
    ];
    const CPU_ORDER: &[&str] = &["blas", "eigen", "onnx-cpu"];
    let mut order: Vec<&'static str> = Vec::new();
    match (os, hardware.gpu) {
        (_, Some(GpuVendor::Nvidia)) if os != "macos" => {
            order.extend(NVIDIA_ORDER);
            if os == "windows" {
                // Placa NVIDIA sem o pacote CUDA: o DirectML ainda usa a GPU.
                order.push("onnx-dml");
            }
        }
        ("windows", Some(_)) => order.push("onnx-dml"),
        ("macos", Some(GpuVendor::Apple)) => order.push("metal"),
        // Linux com AMD/Intel e Mac sem Apple Silicon: nenhum backend de GPU que valha o
        // risco como padrão (OpenCL só por escolha manual).
        _ => {}
    }
    order.extend(CPU_ORDER);
    order.retain(|id| supported_on(id, os));
    order
}

/// Backend recomendado: o primeiro da [`auto_order`] que a Engine anuncia. `None` se a
/// Engine não anuncia nenhum conhecido (deixe a escolha com ela).
pub fn recommended_backend(os: &str, hardware: Hardware, advertised: &[String]) -> Option<String> {
    auto_order(os, hardware)
        .into_iter()
        .find(|id| advertised.iter().any(|a| a.eq_ignore_ascii_case(id)))
        .map(str::to_owned)
}

/// Backend de CPU para recuar quando `failed` não subiu: o primeiro de CPU anunciado.
pub fn fallback_backend(os: &str, advertised: &[String], failed: &str) -> Option<String> {
    auto_order(os, Hardware { gpu: None })
        .into_iter()
        .filter(|id| !id.eq_ignore_ascii_case(failed))
        .find(|id| advertised.iter().any(|a| a.eq_ignore_ascii_case(id)))
        .map(str::to_owned)
}

/// Por que um backend não subiu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendProblem {
    /// Sem GPU compatível ou sem driver (CUDA/DirectX/Metal).
    NoDevice,
    /// Biblioteca ausente (`cudart`, `cublas`, `cudnn`, `onnxruntime`, `directml.dll`…).
    MissingLibrary,
    OutOfMemory,
    /// O binário não tem o backend ou a rede não é suportada por ele.
    Unsupported,
    Unknown,
}

/// Classifica a saída de erro (stderr/log) da Engine que não subiu.
pub fn classify_failure(text: &str) -> BackendProblem {
    let t = text.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    if has(&["out of memory", "outofmemory", "bad_alloc", "e_outofmemory"]) {
        BackendProblem::OutOfMemory
    } else if has(&[
        "shared librar",
        "cannot open shared object",
        "dll was not found",
        "dll not found",
        "could not load library",
        "failed to load library",
        "libcudart",
        "libcublas",
        "libcudnn",
        "onnxruntime.dll",
        "directml.dll",
    ]) {
        BackendProblem::MissingLibrary
    } else if has(&[
        "no cuda-capable",
        "driver version is insufficient",
        "cudagetdevicecount",
        "no cuda device",
        "no gpu",
        "d3d12",
        "no suitable device",
        "cuda driver",
    ]) {
        BackendProblem::NoDevice
    } else if has(&[
        "unknown backend",
        "unsupported",
        "not supported",
        "not compiled",
    ]) {
        BackendProblem::Unsupported
    } else {
        BackendProblem::Unknown
    }
}

/// Frase em pt-BR para o usuário: o backend, o motivo provável e o que acontece a seguir.
pub fn describe_failure(backend: &str, problem: BackendProblem, fallback: Option<&str>) -> String {
    let cause = match problem {
        BackendProblem::NoDevice => {
            "a máquina não tem GPU compatível ou o driver está ausente ou desatualizado"
        }
        BackendProblem::MissingLibrary => "falta uma biblioteca exigida (driver, CUDA ou DirectML)",
        BackendProblem::OutOfMemory => "a memória da GPU não foi suficiente para a rede",
        BackendProblem::Unsupported => "esta build do Lc0 não suporta o backend ou a rede",
        BackendProblem::Unknown => "a Engine encerrou ao carregar a rede",
    };
    match fallback {
        Some(cpu) => format!("O backend {backend} não subiu: {cause}. Usando {cpu} (CPU)."),
        None => format!(
            "O backend {backend} não subiu: {cause}. Escolha outro backend no gerenciador de Engines."
        ),
    }
}

/// Procura GPU na máquina (barato: no máximo uma chamada curta a `nvidia-smi`).
pub fn detect_hardware() -> Hardware {
    if cfg!(target_os = "macos") {
        let apple = cfg!(target_arch = "aarch64");
        return Hardware {
            gpu: apple.then_some(GpuVendor::Apple),
        };
    }
    let nvidia = std::process::Command::new("nvidia-smi")
        .arg("-L")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success() && !o.stdout.is_empty());
    if nvidia {
        return Hardware {
            gpu: Some(GpuVendor::Nvidia),
        };
    }
    // Windows: toda máquina com DirectX 12 serve ao DirectML; sem uma API do sistema não há
    // como saber o fabricante, então assume-se que há GPU (a falha recua para a CPU).
    Hardware {
        gpu: cfg!(windows).then_some(GpuVendor::Other),
    }
}

/// Como obter o Lc0 em cada sistema (o projeto só publica binários para Windows).
pub fn lc0_install_help(os: &str) -> &'static str {
    match os {
        "windows" => {
            "Windows: instale o Lc0 pelo Gerenciador de Engines (build oficial da release \
             v0.32.1: CPU, DirectML ou CUDA 12)."
        }
        "macos" => {
            "macOS: o projeto não publica binário. Instale com Homebrew (`brew install lc0`) ou \
             compile o código-fonte com o backend Metal; o app detecta lc0 em /opt/homebrew/bin."
        }
        _ => {
            "Linux: o projeto não publica binário. Instale o pacote da distribuição (lc0, quando \
             houver) ou compile o código-fonte (meson; -Dplain_cuda para NVIDIA, -Dblas para CPU) \
             e deixe o executável lc0 no PATH, em ~/.local/bin ou em /usr/local/bin."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adv(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    const NVIDIA: Hardware = Hardware {
        gpu: Some(GpuVendor::Nvidia),
    };
    const AMD: Hardware = Hardware {
        gpu: Some(GpuVendor::Amd),
    };
    const APPLE: Hardware = Hardware {
        gpu: Some(GpuVendor::Apple),
    };
    const NONE: Hardware = Hardware { gpu: None };

    #[test]
    fn the_map_knows_every_backend_of_the_issue_per_platform() {
        for id in [
            "cuda",
            "cudnn",
            "onnx-cuda",
            "onnx-dml",
            "onnx-trt",
            "metal",
            "blas",
            "eigen",
        ] {
            assert!(backend_info(id).is_some(), "{id}");
        }
        assert!(supported_on("onnx-dml", "windows"));
        assert!(!supported_on("onnx-dml", "linux"));
        assert!(supported_on("metal", "macos"));
        assert!(!supported_on("metal", "windows"));
        assert!(supported_on("cuda", "linux"));
        assert!(!supported_on("cuda", "macos"));
        assert!(is_gpu_backend("cuda-fp16") && !is_gpu_backend("blas"));
        assert!(!is_gpu_backend("desconhecido"));
    }

    #[test]
    fn backends_are_read_from_the_combo_the_engine_announces() {
        let options = vec![
            EngineOption {
                name: "Threads".into(),
                kind: "spin".into(),
                details: "default 2 min 1 max 128".into(),
            },
            EngineOption {
                name: "Backend".into(),
                kind: "combo".into(),
                details: "default <autodiscover> var <autodiscover> var cuda-auto var blas".into(),
            },
        ];
        assert_eq!(
            advertised_backends(&options),
            adv(&["<autodiscover>", "cuda-auto", "blas"])
        );
        assert!(advertised_backends(&options[..1]).is_empty(), "Stockfish");
    }

    #[test]
    fn nvidia_on_windows_prefers_cuda_then_directml_then_cpu() {
        let order = auto_order("windows", NVIDIA);
        let pos = |id: &str| order.iter().position(|b| *b == id).unwrap();
        assert!(pos("cuda-auto") < pos("onnx-dml"));
        assert!(pos("onnx-dml") < pos("blas"));
        assert!(!order.contains(&"metal"));
    }

    #[test]
    fn amd_on_windows_goes_to_directml_and_never_tries_cuda() {
        let order = auto_order("windows", AMD);
        assert_eq!(order[0], "onnx-dml");
        assert!(!order.iter().any(|b| b.starts_with("cuda")));
    }

    #[test]
    fn nvidia_on_linux_prefers_cuda_and_has_no_directml() {
        let order = auto_order("linux", NVIDIA);
        assert_eq!(order[0], "cuda-auto");
        assert!(!order.contains(&"onnx-dml") && !order.contains(&"metal"));
    }

    #[test]
    fn macos_prefers_metal_and_machines_without_gpu_only_get_cpu_backends() {
        assert_eq!(auto_order("macos", APPLE)[0], "metal");
        for os in ["windows", "linux", "macos"] {
            let order = auto_order(os, NONE);
            assert!(!order.is_empty());
            assert!(order.iter().all(|b| !is_gpu_backend(b)), "{os}: {order:?}");
        }
    }

    #[test]
    fn the_recommendation_only_picks_what_the_engine_announced() {
        let cuda_build = adv(&["<autodiscover>", "cuda", "cuda-fp16", "blas"]);
        assert_eq!(
            recommended_backend("linux", NVIDIA, &cuda_build).as_deref(),
            Some("cuda-fp16")
        );
        // Mesma máquina, build só de CPU (OpenBLAS do Windows): fica na CPU.
        let cpu_build = adv(&["<autodiscover>", "blas", "random"]);
        assert_eq!(
            recommended_backend("windows", NVIDIA, &cpu_build).as_deref(),
            Some("blas")
        );
        // Sem GPU, mesmo uma build com CUDA fica na CPU.
        assert_eq!(
            recommended_backend("linux", NONE, &cuda_build).as_deref(),
            Some("blas")
        );
        assert_eq!(
            recommended_backend("linux", NVIDIA, &adv(&["random"])),
            None
        );
        assert_eq!(recommended_backend("linux", NVIDIA, &[]), None);
    }

    #[test]
    fn fallback_is_the_first_announced_cpu_backend() {
        let a = adv(&["cuda-auto", "eigen", "blas"]);
        assert_eq!(
            fallback_backend("linux", &a, "cuda-auto").as_deref(),
            Some("blas")
        );
        assert_eq!(
            fallback_backend("linux", &adv(&["cuda-auto"]), "cuda-auto"),
            None,
            "sem CPU na build: nada a fazer além do erro"
        );
        assert_eq!(
            fallback_backend("linux", &adv(&["blas"]), "blas"),
            None,
            "o que falhou não é candidato"
        );
    }

    #[test]
    fn failures_are_classified_from_engine_output() {
        use BackendProblem::*;
        let cases = [
            ("CUDA error: no CUDA-capable device is detected", NoDevice),
            (
                "cudaGetDeviceCount failed: CUDA driver version is insufficient",
                NoDevice,
            ),
            ("Failed to create D3D12 device", NoDevice),
            (
                "error while loading shared libraries: libcudart.so.12: cannot open shared object file",
                MissingLibrary,
            ),
            (
                "The code execution cannot proceed because directml.dll was not found",
                MissingLibrary,
            ),
            ("cudaMalloc failed: out of memory", OutOfMemory),
            ("Unknown backend: metal", Unsupported),
            ("alguma coisa estranha", Unknown),
        ];
        for (text, expected) in cases {
            assert_eq!(classify_failure(text), expected, "{text}");
        }
    }

    #[test]
    fn the_failure_message_names_the_backend_the_cause_and_the_fallback() {
        let msg = describe_failure("cuda-auto", BackendProblem::NoDevice, Some("blas"));
        assert!(msg.contains("cuda-auto") && msg.contains("blas"), "{msg}");
        assert!(msg.to_lowercase().contains("driver"), "{msg}");
        let msg = describe_failure("cuda-auto", BackendProblem::MissingLibrary, None);
        assert!(msg.contains("cuda-auto") && !msg.contains("None"), "{msg}");
    }

    #[test]
    fn install_help_exists_for_each_system_and_mentions_the_official_route() {
        assert!(lc0_install_help("linux").contains("lc0"));
        assert!(lc0_install_help("macos").to_lowercase().contains("brew"));
        assert!(lc0_install_help("windows").contains("Gerenciador"));
    }
}
