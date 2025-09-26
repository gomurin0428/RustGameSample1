mod cli;
mod game;

use anyhow::{Context, Result};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<()> {
    let project_dir =
        std::env::current_dir().context("カレントディレクトリの取得に失敗しました")?;
    let config_path = project_dir.join("config").join("countries.json");

    if !config_path.exists() {
        anyhow::bail!("国設定ファイルが見つかりません: {}", config_path.display());
    }

    let file = File::open(&config_path)
        .with_context(|| format!("国設定ファイルを開けません: {}", config_path.display()))?;
    let reader = BufReader::new(file);

    let rng = StdRng::from_entropy();
    let mut game = game::GameState::from_reader(reader, rng).with_context(|| {
        format!(
            "国設定ファイルの読み込みに失敗しました: {}",
            config_path.display()
        )
    })?;

    cli::run(&mut game)
}
