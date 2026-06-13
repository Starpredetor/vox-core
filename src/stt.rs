use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use std::path::Path;

pub struct SttEngine {
    ctx: WhisperContext,
}

impl SttEngine {
    pub fn new<P: AsRef<Path>>(model_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path_str = model_path
            .as_ref()
            .to_str()
            .ok_or("Invalid model path")?;

        let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())?;
        Ok(Self { ctx })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, Box<dyn std::error::Error>> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_n_threads(4);                
        params.set_language(Some("en"));       
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);       

        let mut state = self.ctx.create_state()?;
        state.full(params, samples)?;

        let mut text = String::new();
        for segment in state.as_iter() {
            if let Ok(segment_str) = segment.to_str() {
                text.push_str(&segment_str);
            }
        }

        Ok(text.trim().to_string())
    }
}

