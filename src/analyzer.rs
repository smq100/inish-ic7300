use rustfft::{num_complex::Complex, FftPlanner};

#[derive(Clone, Copy)]
pub struct AnalysisConfig {
    pub low_hz: f64,
    pub high_hz: f64,
    pub window_seconds: usize,
    pub fft_size: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            low_hz: 300.0,
            high_hz: 2700.0,
            window_seconds: 5,
            fft_size: 4096,
        }
    }
}

#[allow(unused)]
pub struct NoiseMetrics {
    pub band_power_dbfs: f64,
    pub median_psd_dbfs_hz: f64,
    pub peak_dbfs: f64,
    pub frames: usize,
}

pub struct NoiseAnalyzer {
    pub config: AnalysisConfig,
}

impl NoiseAnalyzer {
    pub fn new(config: AnalysisConfig) -> Self {
        Self { config }
    }

    pub fn analyze(&self, samples: &[f32], sample_rate: u32) -> Option<NoiseMetrics> {
        let n = self.config.fft_size;
        if samples.len() < n || sample_rate == 0 {
            return None;
        }
        let fs = sample_rate as f64;
        let low = self.config.low_hz;
        let high = self.config.high_hz.min(fs / 2.0);
        if high <= low {
            return None;
        }
        let mut planner = FftPlanner::<f64>::new();
        let fft = planner.plan_fft_forward(n);
        let window: Vec<f64> = (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos())
            .collect();
        let window_energy: f64 = window.iter().map(|x| x * x).sum();
        let mut sum_psd = vec![0.0; n / 2 + 1];
        let mut frames = 0usize;
        let hop = n / 2;
        for start in (0..=samples.len() - n).step_by(hop) {
            let mut buf: Vec<Complex<f64>> = samples[start..start + n]
                .iter()
                .zip(window.iter())
                .map(|(&x, &w)| Complex::new(x as f64 * w, 0.0))
                .collect();
            fft.process(&mut buf);
            for k in 0..=n / 2 {
                let factor = if k == 0 || k == n / 2 { 1.0 } else { 2.0 };
                sum_psd[k] += factor * buf[k].norm_sqr() / (fs * window_energy);
            }
            frames += 1;
        }
        let mut selected = Vec::new();
        let mut band_power = 0.0;
        let bin_hz = fs / n as f64;
        for (k, power) in sum_psd.into_iter().enumerate() {
            let hz = k as f64 * bin_hz;
            if hz >= low && hz <= high {
                let psd = power / frames as f64;
                selected.push(psd);
                band_power += psd * bin_hz;
            }
        }
        if selected.is_empty() {
            return None;
        }
        selected.sort_by(|a, b| a.total_cmp(b));
        let median = if selected.len() % 2 == 0 {
            (selected[selected.len() / 2 - 1] + selected[selected.len() / 2]) / 2.0
        } else {
            selected[selected.len() / 2]
        };
        let peak = samples
            .iter()
            .map(|v| v.abs() as f64)
            .fold(0.0_f64, f64::max);
        Some(NoiseMetrics {
            band_power_dbfs: db10(band_power),
            median_psd_dbfs_hz: db10(median),
            peak_dbfs: 20.0 * peak.max(1e-15).log10(),
            frames,
        })
    }
}

fn db10(value: f64) -> f64 {
    10.0 * value.max(1e-30).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_is_low() {
        let a = NoiseAnalyzer::new(AnalysisConfig::default());
        let result = a.analyze(&vec![0.0; 48_000], 48_000).unwrap();
        assert!(result.band_power_dbfs < -200.0);
    }
    #[test]
    fn tone_has_power() {
        let fs = 48_000;
        let samples: Vec<f32> = (0..fs)
            .map(|i| {
                (0.5 * (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / fs as f64).sin()) as f32
            })
            .collect();
        let a = NoiseAnalyzer::new(AnalysisConfig::default());
        let result = a.analyze(&samples, fs as u32).unwrap();
        assert!((result.band_power_dbfs - (-9.03)).abs() < 0.5);
    }
}
