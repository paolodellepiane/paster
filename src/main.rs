use anyhow::{bail, Context, Result};
use arboard::{Clipboard, ImageData};
use chrono::{Datelike};
use image::{ImageFormat, RgbaImage};
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// paste clipboard content to a directory
    Paste {
        /// destination directory
        dest_dir: PathBuf,
        /// working directory
        #[arg(long, env = "PASTER_WORK_DIR")]
        cd: Option<PathBuf>,
    },
    /// print a date
    Date {
        /// when
        #[arg(value_parser)]
        when: WhatTypes,
        /// format
        #[arg(short, long, env = "PASTER_DATE_FORMAT", default_value = "%d/%m/%y")]
        format: String,
    },
}

#[derive(Clone, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum WhatTypes {
    Yesterday,
    Today,
    Tomorrow,
    NextWeek,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Paste { dest_dir, cd } => paste(dest_dir, cd)?,
        Commands::Date {   when, format } => date(when, &format)?,
    }
    
    Ok(())
}

fn date(when: WhatTypes, format: &str) -> Result<()> {
    match when {
        WhatTypes::Yesterday => {
            let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
            println!("{}", yesterday.format(format));
        }
        WhatTypes::Today => {
            println!("{}", chrono::Utc::now().format(format));
        }
        WhatTypes::Tomorrow => {
            println!("{}", (chrono::Utc::now() + chrono::Duration::days(1)).format(format));
        }
        WhatTypes::NextWeek => {
            // get next week monday
            let today = chrono::Local::now().date_naive();
            let days_since_monday = today.weekday().num_days_from_monday() as i64;
            let next_monday = today + chrono::Duration::days(7 - days_since_monday);

            println!("{}", next_monday.format(format));
        }
    }
    Ok(())
}

fn timestamp() -> String {
    chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string()
}

fn is_image_file<P: AsRef<Path>>(file_path: P) -> bool {
    if let Some(extension) = file_path.as_ref().extension() {
        if let Some(ext_str) = extension.to_str() {
            let ext_lower = ext_str.to_lowercase();
            return matches!(
                ext_lower.as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp"
            );
        }
    }

    false
}

fn handle_file_list(file_list: Vec<PathBuf>, dest_dir: impl AsRef<Path>) -> Result<()> {
    for file in file_list.iter() {
        let emark = if is_image_file(&file) { "!" } else { "" };
        let filename = file
            .file_stem()
            .with_context(|| "Could not determine filename")?
            .to_string_lossy()
            .replace(" ", "_");
        let extension = file
            .extension()
            .with_context(|| "Could not determine extension")?
            .to_string_lossy();
        let new_filename = format!("{}_{}.{}", filename, timestamp(), extension);
        std::fs::create_dir_all(&dest_dir)?;
        let dest_path = dest_dir.as_ref().join(&new_filename);

        std::fs::copy(file, &dest_path).with_context(|| "can't copy file")?;

        println!("{emark}[{filename}]({})", dest_path.to_string_lossy());
    }

    Ok(())
}

fn handle_image_data(image_data: ImageData, dest_dir: impl AsRef<Path>) -> Result<()> {
    let width = image_data.width as u32;
    let height = image_data.height as u32;

    if image_data.bytes.len() != (width * height * 4) as usize {
        // Ensure the data length matches what we expect for RGBA
        bail!("Error: Invalid image data length");
    }

    // Create RgbaImage from the raw bytes
    let img = match RgbaImage::from_raw(width, height, image_data.bytes.to_vec()) {
        Some(img) => img,
        None => {
            bail!("Error: Could not create image from raw data");
        }
    };

    let new_filename = format!("img_{}.png", timestamp());
    std::fs::create_dir_all(&dest_dir)?;
    let dest_path = dest_dir.as_ref().join(&new_filename);

    img.save_with_format(&dest_path, ImageFormat::Png)?;

    println!("![]({})", dest_path.to_string_lossy());

    Ok(())
}

fn handle_text(content: String) {
    println!("```");
    println!("{content}");
    println!("```");
}

fn paste(dest_dir: impl AsRef<Path>, work_dir: Option<impl AsRef<Path>>) -> Result<()> {
    if let Some(work_dir) = work_dir {
        std::env::set_current_dir(&work_dir)?;
    }

    let mut ctx = Clipboard::new()?;

    if let Ok(file_list) = ctx.get().file_list() {
        handle_file_list(file_list, dest_dir)?;
        return Ok(());
    }

    if let Ok(image) = ctx.get_image() {
        handle_image_data(image, dest_dir)?;
        return Ok(());
    }

    let content = ctx.get_text()?;
    if content.trim().is_empty() {
        return Ok(());
    }

    handle_text(content);

    Ok(())
}