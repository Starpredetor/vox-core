pub mod audio;
pub mod stt;

use ringbuf::{traits::*, HeapRb};
use std::time::Duration;
use std::io::{self, Write};
use audio::{AudioInput, AudioSource};
use stt::SttEngine;

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
    whisper_rs::install_logging_hooks();
    
    let source = select_audio_source();
    println!("\nLoading Whisper model...");

    let model_path = "models/ggml-tiny.en.bin"; 
    let stt_engine = SttEngine::new(model_path)?;
    println!("Model loaded successfully!");

    let rb = HeapRb::<f32>::new(64000);
    let (prod, mut cons) = rb.split();
    let _audio_input = AudioInput::new(source, prod)?;
    println!("\nSTT Pipeline Running! Speak or play audio (Ctrl+C to exit)...\n");

    let mut temp_buffer = vec![0.0f32; 1600]; 
    let mut audio_accumulator = Vec::<f32>::new(); 
    
    let mut silent_blocks_count = 0;
    let mut last_transcribe_len = 0;
    
    let silence_threshold = 0.008; 
    let silence_timeout_blocks = 12;
    loop {
        let read_count = cons.pop_slice(&mut temp_buffer);
        if read_count > 0 {
            let chunk = &temp_buffer[..read_count];
            
            
            let peak = chunk.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
            
            
            audio_accumulator.extend_from_slice(chunk);
            if peak < silence_threshold {
                silent_blocks_count += 1;
            } else {
                silent_blocks_count = 0; 
            }
            
            if silent_blocks_count >= silence_timeout_blocks {
                if !audio_accumulator.is_empty() {
                    
                    if let Ok(text) = stt_engine.transcribe(&audio_accumulator) {
                        if !text.is_empty() {
                            print!("\r[Final] {}\n", text);
                            io::stdout().flush().unwrap();
                        }
                    }
                    audio_accumulator.clear();
                    last_transcribe_len = 0;
                }
                silent_blocks_count = 0;
            } 
            else if audio_accumulator.len() - last_transcribe_len >= 12800 { 
                if let Ok(text) = stt_engine.transcribe(&audio_accumulator) {
                    if !text.is_empty() {
                        print!("\r[Transcribing...] {}\x1b[K", text);
                        io::stdout().flush().unwrap();
                    }
                }
                last_transcribe_len = audio_accumulator.len();
            }
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
}