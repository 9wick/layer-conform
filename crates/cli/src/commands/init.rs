//! `layer-conform init`: write a starter `.layer-conform.json`.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

const TEMPLATE: &str = r#"{
  "version": 1,
  "rules": [
    {
      "id": "example",
      "golden": "src/example/golden.ts:goldenFn",
      "applyTo": "src/example/**/*.ts",
      "threshold": 0.7
    }
  ]
}
"#;

pub struct InitOpts {
    pub force: bool,
}

pub fn run(opts: InitOpts) -> Result<i32> {
    let path = Path::new(crate::loader::CONFIG_FILE);
    if path.exists() && !opts.force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            path.display()
        ));
    }
    fs::write(path, TEMPLATE)?;
    println!(
        "Wrote {}. Edit `rules` to point at your golden(s).",
        path.display()
    );
    Ok(0)
}
