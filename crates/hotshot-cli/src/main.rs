use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use hotshot_core::capture::{self, CaptureMode};
use hotshot_core::config::{Config, ImageFormat};
use hotshot_core::storage::Storage;

#[derive(Parser)]
#[command(name = "hotshot", about = "Screenshot tool with organization", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot
    #[command(subcommand)]
    Capture(CaptureCommand),

    /// List recent screenshots
    List {
        /// Maximum number of screenshots to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Open a screenshot in the default viewer
    Open {
        /// Screenshot ID (or prefix)
        id: String,
    },

    /// Add tags to a screenshot
    Tag {
        /// Screenshot ID (or prefix)
        id: String,
        /// Tags to add
        tags: Vec<String>,
    },

    /// Search screenshots by tag, note, or id
    Search {
        /// Search query
        query: String,
    },

    /// Delete a screenshot (move to trash)
    Delete {
        /// Screenshot ID (or prefix)
        id: String,
    },

    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Args, Clone)]
struct CaptureOpts {
    /// Image format (png, jpeg, webp — overrides config)
    #[arg(short, long)]
    format: Option<ImageFormat>,

    /// Copy to clipboard
    #[arg(short, long)]
    clipboard: bool,

    /// Save to specific path instead of default storage
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Subcommand)]
enum CaptureCommand {
    /// Capture full screen
    Fullscreen {
        #[command(flatten)]
        opts: CaptureOpts,
    },
    /// Capture a region (interactive selection, or pass --geometry)
    Region {
        /// Explicit region: X,Y,W,H or WxH+X+Y (omit for interactive)
        #[arg(short, long)]
        geometry: Option<String>,
        #[command(flatten)]
        opts: CaptureOpts,
    },
    /// Capture the active window
    Window {
        #[command(flatten)]
        opts: CaptureOpts,
    },
}

impl CaptureCommand {
    fn opts(&self) -> &CaptureOpts {
        match self {
            CaptureCommand::Fullscreen { opts } => opts,
            CaptureCommand::Region { opts, .. } => opts,
            CaptureCommand::Window { opts } => opts,
        }
    }

    fn to_capture_mode(&self) -> Result<CaptureMode> {
        Ok(match self {
            CaptureCommand::Fullscreen { .. } => CaptureMode::Fullscreen,
            CaptureCommand::Region { geometry, .. } => match geometry {
                Some(g) => {
                    let region =
                        capture::parse_region(g).map_err(|e| anyhow::anyhow!(e))?;
                    CaptureMode::Region(region)
                }
                None => CaptureMode::RegionInteractive,
            },
            CaptureCommand::Window { .. } => CaptureMode::ActiveWindow,
        })
    }
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Open config file in $EDITOR
    Edit,
    /// Set a config value
    Set {
        /// Key=value pair (e.g. format=webp)
        pair: String,
    },
    /// Reset config to defaults
    Reset,
    /// Show config file path
    Path,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load_or_create().context("failed to load config")?;

    match cli.command {
        Commands::Capture(cmd) => cmd_capture(config, cmd),
        Commands::List { limit, tag } => cmd_list(config, limit, tag),
        Commands::Open { id } => cmd_open(config, id),
        Commands::Tag { id, tags } => cmd_tag(config, id, tags),
        Commands::Search { query } => cmd_search(config, query),
        Commands::Delete { id } => cmd_delete(config, id),
        Commands::Config { action } => cmd_config(config, action),
    }
}

fn cmd_capture(config: Config, cmd: CaptureCommand) -> Result<()> {
    let capture_mode = cmd.to_capture_mode()?;
    let opts = cmd.opts().clone();

    let display_server = capture::detect_display_server()?;
    eprintln!("capturing ({display_server})...");

    let image = capture::capture(&capture_mode)?;
    eprintln!("captured {}x{}", image.width(), image.height());

    // Save to custom output or default storage
    if let Some(output_path) = &opts.output {
        let path = std::path::Path::new(output_path);
        let dynamic = hotshot_core::image::DynamicImage::ImageRgba8(image.clone());
        dynamic.save(path).context("failed to save image")?;
        eprintln!("saved: {output_path}");
    } else {
        let storage = Storage::new(config.clone());
        let entry = storage
            .save(
                &image,
                &capture_mode,
                display_server,
                opts.format.as_ref(),
            )
            .context("failed to save screenshot")?;
        eprintln!("saved: {}", entry.image_path.display());
        eprintln!("id:    {}", entry.metadata.id);
    }

    // Copy to clipboard if requested
    if opts.clipboard || config.copy_to_clipboard {
        hotshot_core::clipboard::copy_image(&image).context("failed to copy to clipboard")?;
        eprintln!("copied to clipboard");
    }

    Ok(())
}

fn cmd_list(config: Config, limit: usize, tag: Option<String>) -> Result<()> {
    let storage = Storage::new(config);
    let entries = storage.list(Some(limit))?;

    if entries.is_empty() {
        eprintln!("no screenshots found");
        return Ok(());
    }

    println!(
        "{:<24} {:<20} {:>10} {}",
        "ID", "Date", "Size", "Tags"
    );
    println!("{}", "-".repeat(80));

    for entry in &entries {
        let m = &entry.metadata;

        if let Some(ref tag_filter) = tag {
            if !m.tags.iter().any(|t| t.contains(tag_filter)) {
                continue;
            }
        }

        let date = m.timestamp.format("%Y-%m-%d %H:%M:%S");
        let size = format!("{}x{}", m.width, m.height);
        let tags = if m.tags.is_empty() {
            String::new()
        } else {
            format!("[{}]", m.tags.join(", "))
        };
        println!("{:<24} {:<20} {:>10} {}", m.id, date, size, tags);
    }

    Ok(())
}

fn cmd_open(config: Config, id: String) -> Result<()> {
    let storage = Storage::new(config);
    let entry = storage.find_by_id(&id)?;

    std::process::Command::new("xdg-open")
        .arg(&entry.image_path)
        .spawn()
        .context("failed to open screenshot (is xdg-open installed?)")?;

    eprintln!("opening: {}", entry.image_path.display());
    Ok(())
}

fn cmd_tag(config: Config, id: String, tags: Vec<String>) -> Result<()> {
    let storage = Storage::new(config);
    let entry = storage.tag(&id, &tags)?;
    eprintln!(
        "tagged {} with: [{}]",
        entry.metadata.id,
        entry.metadata.tags.join(", ")
    );
    Ok(())
}

fn cmd_search(config: Config, query: String) -> Result<()> {
    let storage = Storage::new(config);
    let results = storage.search(&query)?;

    if results.is_empty() {
        eprintln!("no matches for: {query}");
        return Ok(());
    }

    println!(
        "{:<24} {:<20} {:>10} {}",
        "ID", "Date", "Size", "Tags"
    );
    println!("{}", "-".repeat(80));

    for entry in &results {
        let m = &entry.metadata;
        let date = m.timestamp.format("%Y-%m-%d %H:%M:%S");
        let size = format!("{}x{}", m.width, m.height);
        let tags = if m.tags.is_empty() {
            String::new()
        } else {
            format!("[{}]", m.tags.join(", "))
        };
        println!("{:<24} {:<20} {:>10} {}", m.id, date, size, tags);
    }

    eprintln!("{} result(s)", results.len());
    Ok(())
}

fn cmd_delete(config: Config, id: String) -> Result<()> {
    let storage = Storage::new(config);
    let entry = storage.delete(&id)?;
    eprintln!("deleted: {} (moved to trash)", entry.metadata.id);
    Ok(())
}

fn cmd_config(mut config: Config, action: Option<ConfigAction>) -> Result<()> {
    let action = action.unwrap_or(ConfigAction::Show);
    match action {
        ConfigAction::Show => {
            println!("{}", config.display());
        }
        ConfigAction::Edit => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let config_path = Config::config_path();
            if !config_path.exists() {
                config.save()?;
            }
            std::process::Command::new(&editor)
                .arg(&config_path)
                .status()
                .context(format!("failed to open editor: {editor}"))?;
        }
        ConfigAction::Set { pair } => {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid format. use: key=value"))?;
            config
                .set_value(key.trim(), value.trim())
                .map_err(|e| anyhow::anyhow!(e))?;
            config.save()?;
            eprintln!("set {key} = {value}");
        }
        ConfigAction::Reset => {
            let config = Config::default();
            config.save()?;
            eprintln!("config reset to defaults");
        }
        ConfigAction::Path => {
            println!("{}", Config::config_path().display());
        }
    }
    Ok(())
}
