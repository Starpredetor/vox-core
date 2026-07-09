pub mod audio;
pub mod stt;

use ringbuf::{traits::*, HeapRb};
use std::time::Duration;
use std::io::{self, Write};
use audio::{AudioInput, AudioSource};
use stt::SttEngine;

const MAX_LINE_CHARS: usize = 50;
const MAX_LINES: usize = 5;

fn clean_transcription(text: &str) -> String {
    text.replace("[BLANK_AUDIO]", "")
        .replace("[blank_audio]", "")
        .replace("(laughter)", "")
        .replace("(sighs)", "")
        .replace("(music)", "")
        .replace("Thank you.", "")
        .replace("Thank you for watching.", "")
        .trim()
        .to_string()
}

fn format_into_lines(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

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
            
            let is_silence_timeout = silent_blocks_count >= silence_timeout_blocks;
            let is_time_to_transcribe = audio_accumulator.len() - last_transcribe_len >= 12800;
            
            if is_silence_timeout || is_time_to_transcribe {
                if let Ok(raw_text) = stt_engine.transcribe(&audio_accumulator) {
                    let cleaned = clean_transcription(&raw_text);
                    let lines = format_into_lines(&cleaned, MAX_LINE_CHARS);
                    let should_commit = (is_silence_timeout && !cleaned.is_empty()) || (lines.len() >= MAX_LINES);

                    if should_commit {
                        print!("\r\x1b[K");
                        for line in &lines {
                            println!("[Final] {}", line);
                        }
                        io::stdout().flush().unwrap();

                        let overlap_samples = 24000;
                        if audio_accumulator.len() > overlap_samples {
                            audio_accumulator = audio_accumulator[audio_accumulator.len() - overlap_samples..].to_vec();
                        } else {
                            audio_accumulator.clear();
                        }
                        
                        last_transcribe_len = audio_accumulator.len();
                        silent_blocks_count = 0;
                    } else if is_time_to_transcribe {
                        if !cleaned.is_empty() {
                            let max_display_len = 60;
                            let display_text = if cleaned.len() > max_display_len {
                                format!("...{}", &cleaned[cleaned.len() - max_display_len..])
                            } else {
                                cleaned.clone()
                            };
                            print!("\r[Transcribing...] {}\x1b[K", display_text);
                        } else {
                            print!("\r\x1b[K");
                        }
                        io::stdout().flush().unwrap();
                        last_transcribe_len = audio_accumulator.len();
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
