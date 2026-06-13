pub mod audio;

use ringbuf::{traits::*, HeapRb};
use std::time::Duration;
use std::io::Write;
use audio::AudioInput;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rb = HeapRb::<f32>::new(64000);
    let (prod, mut cons) = rb.split();

    let _audio_input = AudioInput::new(prod)?;

    println!("Recording started! Speak into the mic (Ctrl+C to stop).");

    let mut temp_buffer = vec![0.0f32; 1600]; 

    loop {
        let read_count = cons.pop_slice(&mut temp_buffer);
        if read_count > 0 {
            let peak = temp_buffer[..read_count]
                .iter()
                .map(|&x| x.abs())
                .fold(0.0f32, f32::max);
            
            let sum_sq: f32 = temp_buffer[..read_count].iter().map(|&x| x * x).sum();
            let rms = (sum_sq / read_count as f32).sqrt();
            
            let meter_width = (peak * 100.0) as usize;
            let meter = "*".repeat(meter_width.min(50));
            print!("\r[{: <50}] (read {:4} samples, RMS: {:.4})\x1b[K", meter, read_count, rms);
            std::io::stdout().flush().unwrap();
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
}
