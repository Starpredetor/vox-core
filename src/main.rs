pub mod audio;

use ringbuf::{traits::*, HeapRb};
use std::time::Duration;
use std::io::{self, Write};
use audio::{AudioInput, AudioSource};

fn select_audio_source() -> AudioSource {
    println!("=== VOX-CORE AUDIO CAPTURE TEST ===");
    println!("1. Microphone (Input Device)");
    println!("2. System Audio (Output Loopback)");
    print!("Select source [1-2]: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    match input.trim() {
        "2" => {
            println!("\nCapturing System Audio...");
            AudioSource::SystemAudio
        }
        _ => {
            println!("\nCapturing Microphone...");
            AudioSource::Microphone
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Get user selection from the menu
    let source = select_audio_source();

    // 2. Create the ring buffer
    let rb = HeapRb::<f32>::new(64000);
    let (prod, mut cons) = rb.split();

    // 3. Initialize the selected audio input stream
    let _audio_input = AudioInput::new(source, prod)?;

    println!("Recording started! Speak or play audio (Ctrl+C to stop).");

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
            print!("\r[{: <50}] (read {:4} samples, Peak: {:.4}, RMS: {:.4})\x1b[K", meter, read_count, peak, rms);
            std::io::stdout().flush().unwrap();
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
}
