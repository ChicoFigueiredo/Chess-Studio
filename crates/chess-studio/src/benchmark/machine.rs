//! Carga da máquina durante o benchmark (lógica pura, sem Bevy): o marcador
//! `maquina_ocupada` diz se os números devem ser lidos com cautela (UI-12 ficou
//! UNVERIFIED no PR #122 por causa de uma máquina saturada).
//!
//! Critério (documentado em `docs/benchmark.md`): a máquina está **ocupada** quando, no
//! início ou no fim da medição,
//! - a média de carga de 1 min por núcleo lógico passa de [`LOAD_PER_CORE_LIMIT`] (Linux e
//!   macOS), ou
//! - o uso de CPU do sistema passa de [`SYSTEM_CPU_LIMIT_PERCENT`] (todas as plataformas; é o
//!   único sinal no Windows, que não tem load average).

use serde::Serialize;

/// Carga de 1 min por núcleo lógico acima da qual a máquina conta como ocupada.
pub const LOAD_PER_CORE_LIMIT: f64 = 0.70;
/// Uso de CPU do sistema (%) acima do qual a máquina conta como ocupada.
pub const SYSTEM_CPU_LIMIT_PERCENT: f64 = 70.0;

/// Leitura da carga num instante.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct LoadSample {
    /// Média de carga de 1 min (Linux/macOS); `None` onde não existe (Windows).
    pub load_average_1m: Option<f64>,
    /// Uso de CPU de todo o sistema (0-100), sobre a última janela de ~1 s.
    pub system_cpu_percent: Option<f64>,
}

/// Seção `machine` do relatório.
#[derive(Debug, Serialize)]
pub struct MachineLoad {
    pub start: LoadSample,
    pub end: LoadSample,
    pub cpu_logical_cores: usize,
    pub load_per_core_limit: f64,
    pub system_cpu_limit_percent: f64,
    /// Carga por núcleo mais alta entre início e fim, quando há load average.
    pub load_per_core_max: Option<f64>,
    pub maquina_ocupada: bool,
}

impl MachineLoad {
    pub fn new(start: LoadSample, end: LoadSample, cores: usize) -> Self {
        let cores = cores.max(1);
        Self {
            start,
            end,
            cpu_logical_cores: cores,
            load_per_core_limit: LOAD_PER_CORE_LIMIT,
            system_cpu_limit_percent: SYSTEM_CPU_LIMIT_PERCENT,
            load_per_core_max: load_per_core_max(&start, &end, cores),
            maquina_ocupada: is_busy(&start, &end, cores),
        }
    }

    /// Linha do resumo impresso.
    pub fn summary(&self) -> String {
        let f =
            |v: Option<f64>, unit: &str| v.map_or("n/d".to_owned(), |v| format!("{v:.2}{unit}"));
        format!(
            "Carga da máquina: load 1m {} -> {} | CPU do sistema {} -> {} | {} núcleos | {}",
            f(self.start.load_average_1m, ""),
            f(self.end.load_average_1m, ""),
            f(self.start.system_cpu_percent, "%"),
            f(self.end.system_cpu_percent, "%"),
            self.cpu_logical_cores,
            if self.maquina_ocupada {
                "MÁQUINA OCUPADA (números suspeitos)"
            } else {
                "máquina livre"
            }
        )
    }
}

fn load_per_core_max(start: &LoadSample, end: &LoadSample, cores: usize) -> Option<f64> {
    [start.load_average_1m, end.load_average_1m]
        .into_iter()
        .flatten()
        .map(|l| l / cores as f64)
        .reduce(f64::max)
}

/// Ocupada se qualquer leitura (início ou fim) passar de um dos limiares.
pub fn is_busy(start: &LoadSample, end: &LoadSample, cores: usize) -> bool {
    let load_busy =
        load_per_core_max(start, end, cores.max(1)).is_some_and(|l| l > LOAD_PER_CORE_LIMIT);
    let cpu_busy = [start.system_cpu_percent, end.system_cpu_percent]
        .into_iter()
        .flatten()
        .any(|c| c > SYSTEM_CPU_LIMIT_PERCENT);
    load_busy || cpu_busy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(load: Option<f64>, cpu: Option<f64>) -> LoadSample {
        LoadSample {
            load_average_1m: load,
            system_cpu_percent: cpu,
        }
    }

    #[test]
    fn idle_machine_is_not_busy() {
        let s = sample(Some(1.0), Some(10.0));
        assert!(!is_busy(&s, &s, 8));
    }

    #[test]
    fn high_load_per_core_at_start_or_end_is_busy() {
        let calm = sample(Some(1.0), Some(5.0));
        let hot = sample(Some(8.0), Some(5.0)); // 1,0 por núcleo
        assert!(is_busy(&hot, &calm, 8));
        assert!(is_busy(&calm, &hot, 8));
        // Exatamente no limiar não conta como ocupada.
        assert!(!is_busy(
            &sample(Some(5.6), None),
            &sample(Some(5.6), None),
            8
        ));
        assert!(is_busy(&sample(Some(5.7), None), &calm, 8));
    }

    #[test]
    fn windows_has_only_system_cpu() {
        let hot = sample(None, Some(95.0));
        let calm = sample(None, Some(20.0));
        assert!(is_busy(&calm, &hot, 4));
        assert!(!is_busy(&calm, &calm, 4));
        assert!(!is_busy(&sample(None, None), &sample(None, None), 4));
    }

    #[test]
    fn report_carries_the_flag_and_limits() {
        let m = MachineLoad::new(
            sample(Some(2.0), Some(10.0)),
            sample(Some(16.0), Some(40.0)),
            8,
        );
        assert!(m.maquina_ocupada);
        assert_eq!(m.load_per_core_max, Some(2.0));
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["maquina_ocupada"], true);
        assert_eq!(json["cpu_logical_cores"], 8);
        assert!(m.summary().contains("MÁQUINA OCUPADA"));
        let zero = MachineLoad::new(LoadSample::default(), LoadSample::default(), 0);
        assert_eq!(zero.cpu_logical_cores, 1);
        assert!(!zero.maquina_ocupada);
    }
}
