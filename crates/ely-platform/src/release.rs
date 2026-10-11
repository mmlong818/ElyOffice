//! The newest release's files, as the release server lists them.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Deserialize)]
struct Latest {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Where the latest release at `api` keeps the file `name`.
pub fn asset_url(api: &str, name: &str) -> Result<String> {
    let text = crate::update::fetch(&crate::update::manifest_agent(), api)?
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_string()
        .context("cannot read the latest release")?;
    asset_in(&text, name)
}

fn asset_in(text: &str, name: &str) -> Result<String> {
    let latest: Latest = serde_json::from_str(text).context("the latest release is not valid")?;
    match latest.assets.into_iter().find(|asset| asset.name == name) {
        Some(asset) => Ok(asset.browser_download_url),
        None => bail!("release {} has no {name}", latest.tag_name),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_named_file_is_found_or_said_missing() {
        let text = r#"{"tag_name":"v0.1.2","assets":[{"name":"ely-doc.json","browser_download_url":"https://h/d.json"}]}"#;
        assert_eq!(super::asset_in(text, "ely-doc.json").unwrap(), "https://h/d.json");
        let missing = super::asset_in(text, "ely-tree.json").unwrap_err().to_string();
        assert_eq!(missing, "release v0.1.2 has no ely-tree.json");
    }
}
