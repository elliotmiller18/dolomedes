/// NOTE: this file is vibe coded.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::{io, io::Write};

use crate::proto::FileId;

pub enum Command {
    RequestFile(FileId),
    CreateFile(PathBuf),
    Exit,
}

pub fn run() -> Result<Command> {
    loop {
        draw_menu()?;

        let selection = read_line("Select an action [1-3]: ")?;
        let selection = selection.trim().to_ascii_lowercase();

        match selection.as_str() {
            "1" | "request" | "request file" => {
                return prompt_request_file();
            }
            "2" | "create" | "create file" => {
                return prompt_create_file();
            }
            "3" | "exit" | "quit" => {
                return Ok(Command::Exit);
            }
            _ => pause_with_message("Unrecognized action. Press enter to try again.")?,
        }
    }
}

fn prompt_request_file() -> Result<Command> {
    loop {
        draw_menu()?;
        println!("Request File");
        println!("Enter a 32-byte file id as 64 hex characters.");
        println!();

        let raw = read_line("File id: ")?;
        match parse_file_id(&raw) {
            Ok(file_id) => return Ok(Command::RequestFile(file_id)),
            Err(error) => pause_with_message(&format!("{error}\nPress enter to try again."))?,
        }
    }
}

fn prompt_create_file() -> Result<Command> {
    loop {
        draw_menu()?;
        println!("Create File");
        println!("Enter the path to the local file you want to publish.");
        println!();

        let raw = read_line("Path: ")?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            pause_with_message("Path cannot be empty. Press enter to try again.")?;
            continue;
        }

        return Ok(Command::CreateFile(PathBuf::from(trimmed)));
    }
}

fn draw_menu() -> Result<()> {
    let mut stdout = io::stdout();
    write!(stdout, "\x1B[2J\x1B[H")?;
    writeln!(stdout, "Dolomedes")?;
    writeln!(stdout, "=========")?;
    writeln!(stdout)?;
    writeln!(stdout, "1. Request file")?;
    writeln!(stdout, "2. Create file")?;
    writeln!(stdout, "3. Exit")?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

fn read_line(prompt: &str) -> Result<String> {
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read terminal input")?;

    Ok(input.trim().to_string())
}

fn pause_with_message(message: &str) -> Result<()> {
    println!("{message}");
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read terminal input")?;
    Ok(())
}

fn parse_file_id(raw: &str) -> Result<FileId> {
    let hex = raw.trim().strip_prefix("0x").unwrap_or(raw.trim());
    if hex.len() != 64 {
        bail!("file id must be exactly 64 hex characters");
    }

    let mut file_id = [0_u8; 32];
    hex::decode_to_slice(hex, &mut file_id).context("file id must be valid hex")?;
    Ok(file_id)
}
