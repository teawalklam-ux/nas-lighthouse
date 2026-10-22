use crate::collector;
use crate::config;
use crate::state::Reducer;
use anyhow::Result;

pub async fn run() -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    if !cfg.configured() {
        anyhow::bail!(
            "尚未配置 NAS。请先在应用里填写 SSH 别名和 NAS 地址，或直接编辑 {}",
            config::config_path().display()
        );
    }
    let mut reducer = Reducer::new();
    let mut last = None;
    collector::run_stream_once(
        &cfg.ssh_alias,
        &collector::patched_script(&cfg),
        &mut reducer,
        |snap| {
            last = Some(snap);
        },
        Some(2),
    )
    .await?;
    let snap = last.ok_or_else(|| anyhow::anyhow!("probe collected no frames"))?;
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}
