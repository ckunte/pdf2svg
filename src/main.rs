//! pdf2svg — a small command-line tool that converts PDF pages into standalone
//! SVG files, built on top of the `hayro-svg` / `hayro-interpret` crates.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use hayro_interpret::InterpreterSettings;
use hayro_svg::hayro_syntax::Pdf;
use hayro_svg::{convert, RenderCache, SvgRenderSettings};

/// Convert PDF pages to standalone SVG files.
#[derive(Parser, Debug)]
#[command(name = "pdf2svg", version, about, long_about = None)]
struct Cli {
    /// Path to the input PDF file.
    input: PathBuf,

    /// Directory to write the SVG file(s) into. Defaults to the input
    /// file's own directory.
    #[arg(short = 'o', long = "output-dir", value_name = "DIR")]
    output_dir: Option<PathBuf>,

    /// Which pages to convert, e.g. "1,3,5-8". Defaults to all pages.
    /// Pages are numbered starting at 1.
    #[arg(short = 'p', long = "pages", value_name = "RANGE")]
    pages: Option<String>,

    /// Password for an encrypted PDF.
    #[arg(long, value_name = "PASSWORD", default_value = "")]
    password: String,

    /// Base name for the generated file(s), without extension. Defaults to
    /// the input file's stem. A single output page is written as
    /// "<prefix>.svg"; multiple pages are written as "<prefix>-<n>.svg".
    #[arg(long, value_name = "NAME")]
    prefix: Option<String>,

    /// Background color for the SVG root element, as "#rrggbb" or
    /// "#rrggbbaa". Defaults to transparent.
    #[arg(long, value_name = "COLOR")]
    background: Option<String>,

    /// Skip rendering annotations (form fields, comments, etc.).
    #[arg(long)]
    no_annotations: bool,

    /// Overwrite output files if they already exist.
    #[arg(short = 'f', long)]
    force: bool,

    /// Print a line for each page as it's converted.
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(count) => {
            println!(
                "Wrote {count} SVG file{}.",
                if count == 1 { "" } else { "s" }
            );
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<usize, String> {
    let started = Instant::now();

    let data = fs::read(&cli.input)
        .map_err(|e| format!("could not read '{}': {e}", cli.input.display()))?;

    let pdf = Pdf::new_with_password(data, &cli.password).map_err(|e| {
        format!(
            "could not parse '{}' as a PDF ({e:?}) — if it's encrypted, pass --password",
            cli.input.display()
        )
    })?;

    let total_pages = pdf.pages().len();
    if total_pages == 0 {
        return Err("the PDF has no pages".to_string());
    }

    let page_indices = match &cli.pages {
        Some(spec) => parse_page_ranges(spec, total_pages)?,
        None => (0..total_pages).collect(),
    };
    if page_indices.is_empty() {
        return Err("no pages selected".to_string());
    }

    let output_dir = match &cli.output_dir {
        Some(dir) => dir.clone(),
        None => cli
            .input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("could not create '{}': {e}", output_dir.display()))?;

    let prefix = cli.prefix.clone().unwrap_or_else(|| {
        cli.input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".to_string())
    });

    let bg_color = match &cli.background {
        Some(spec) => parse_color(spec)?,
        None => [0, 0, 0, 0],
    };

    let interpreter_settings = InterpreterSettings {
        render_annotations: !cli.no_annotations,
        ..InterpreterSettings::default()
    };
    let render_settings = SvgRenderSettings { bg_color };
    let cache = RenderCache::new();

    let single_page = page_indices.len() == 1;
    let mut written = 0usize;

    for &idx in &page_indices {
        let page = &pdf.pages()[idx];
        let svg = convert(page, &cache, &interpreter_settings, &render_settings);

        let file_name = if single_page {
            format!("{prefix}.svg")
        } else {
            format!("{prefix}-{}.svg", idx + 1)
        };
        let out_path = output_dir.join(&file_name);

        if out_path.exists() && !cli.force {
            return Err(format!(
                "'{}' already exists (pass --force to overwrite)",
                out_path.display()
            ));
        }

        fs::write(&out_path, svg)
            .map_err(|e| format!("could not write '{}': {e}", out_path.display()))?;

        if cli.verbose {
            println!("page {} -> {}", idx + 1, out_path.display());
        }
        written += 1;
    }

    if cli.verbose {
        println!("done in {:.2?}", started.elapsed());
    }

    Ok(written)
}

/// Parse a 1-based page range spec like "1,3,5-8,12" into 0-based, sorted,
/// de-duplicated page indices, validated against `total_pages`.
fn parse_page_ranges(spec: &str, total_pages: usize) -> Result<Vec<usize>, String> {
    let mut pages = Vec::new();

    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (start, end) = match part.split_once('-') {
            Some((a, b)) => (
                parse_page_number(a, total_pages)?,
                parse_page_number(b, total_pages)?,
            ),
            None => {
                let n = parse_page_number(part, total_pages)?;
                (n, n)
            }
        };

        if start > end {
            return Err(format!(
                "invalid page range '{part}': start ({start}) is after end ({end})"
            ));
        }

        for n in start..=end {
            pages.push(n - 1);
        }
    }

    pages.sort_unstable();
    pages.dedup();
    Ok(pages)
}

fn parse_page_number(s: &str, total_pages: usize) -> Result<usize, String> {
    let s = s.trim();
    let n: usize = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid page number"))?;
    if n == 0 || n > total_pages {
        return Err(format!(
            "page {n} is out of range (the PDF has {total_pages} page{})",
            if total_pages == 1 { "" } else { "s" }
        ));
    }
    Ok(n)
}

/// Parse "#rrggbb" or "#rrggbbaa" into an `[r, g, b, a]` array.
fn parse_color(spec: &str) -> Result<[u8; 4], String> {
    let hex = spec.strip_prefix('#').unwrap_or(spec);
    let bytes = match hex.len() {
        6 => {
            let rgb = u32::from_str_radix(hex, 16)
                .map_err(|_| format!("'{spec}' is not a valid hex color"))?;
            [
                ((rgb >> 16) & 0xff) as u8,
                ((rgb >> 8) & 0xff) as u8,
                (rgb & 0xff) as u8,
                255,
            ]
        }
        8 => {
            let rgba = u32::from_str_radix(hex, 16)
                .map_err(|_| format!("'{spec}' is not a valid hex color"))?;
            [
                ((rgba >> 24) & 0xff) as u8,
                ((rgba >> 16) & 0xff) as u8,
                ((rgba >> 8) & 0xff) as u8,
                (rgba & 0xff) as u8,
            ]
        }
        _ => return Err(format!("'{spec}' must be #rrggbb or #rrggbbaa")),
    };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_basic() {
        assert_eq!(
            parse_page_ranges("1,3,5-7", 10).unwrap(),
            vec![0, 2, 4, 5, 6]
        );
    }

    #[test]
    fn ranges_dedup_and_sort() {
        assert_eq!(
            parse_page_ranges("5,1,3-4,1", 10).unwrap(),
            vec![0, 2, 3, 4]
        );
    }

    #[test]
    fn ranges_out_of_bounds() {
        assert!(parse_page_ranges("11", 10).is_err());
        assert!(parse_page_ranges("0", 10).is_err());
    }

    #[test]
    fn ranges_backwards() {
        assert!(parse_page_ranges("8-3", 10).is_err());
    }

    #[test]
    fn color_rgb() {
        assert_eq!(parse_color("#ffffff").unwrap(), [255, 255, 255, 255]);
        assert_eq!(parse_color("000000").unwrap(), [0, 0, 0, 255]);
    }

    #[test]
    fn color_rgba() {
        assert_eq!(parse_color("#11223344").unwrap(), [0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn color_invalid() {
        assert!(parse_color("#zzz").is_err());
        assert!(parse_color("#12345").is_err());
    }
}
