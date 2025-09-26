mod cli;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::SeedableRng;
use rand::rngs::StdRng;
use realgeopolitics_core::{CountryDefinition, GameState};

fn main() -> Result<()> {
    let config_path = resolve_config_path()?;

    let file = File::open(&config_path)
        .with_context(|| format!("国設定ファイルを開けません: {}", config_path.display()))?;
    let reader = BufReader::new(file);
    let definitions: Vec<CountryDefinition> =
        serde_json::from_reader(reader).with_context(|| {
            format!(
                "国設定ファイルの解析に失敗しました: {}",
                config_path.display()
            )
        })?;

    let rng = StdRng::from_entropy();
    let mut game = GameState::from_definitions_with_rng(definitions, rng).with_context(|| {
        format!(
            "国設定ファイルの読み込みに失敗しました: {}",
            config_path.display()
        )
    })?;

    cli::run(&mut game)
}

fn resolve_config_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("カレントディレクトリの取得に失敗しました")?;
    let candidates = [
        cwd.join("config").join("countries.json"),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("config")
            .join("countries.json"),
    ];

    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }

    anyhow::bail!("国設定ファイルが見つかりません。config/countries.json を配置してください。")
}
