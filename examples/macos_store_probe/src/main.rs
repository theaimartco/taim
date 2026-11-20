//! Packaged macOS acceptance fixture: container storage, persistence, input and rendering.
use taim::prelude::*;
use taim::storage::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = taim::platform::data_dir()?;
    let mut store = Store::open("macos-probe")?;
    let launches = store.get_as::<u32>("launches").unwrap_or(0) + 1;
    store.set("launches", launches)?;
    store.set("roundtrip", "taim-macos-sandbox")?;
    let readback = Store::open("macos-probe")?;
    if readback.get("roundtrip") != Some("taim-macos-sandbox")
        || readback.get_as::<u32>("launches") != Some(launches)
    {
        return Err("persistent storage roundtrip failed".into());
    }
    let in_container = directory
        .to_string_lossy()
        .contains("/Library/Containers/com.theaimartco.taim.storeprobe/Data/");
    let report = serde_json::json!({
        "storage_roundtrip": true,
        "launches": launches,
        "container_path": directory,
        "expected_container": in_container,
        "run_id": std::env::var("TAIM_MACOS_PROBE_RUN").ok(),
    });
    std::fs::write(
        directory.join("probe-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("TAIM_MACOS_STORAGE {report}");

    taim::launch("Taim Store Probe", move |cx| {
        let input = cx.signal(String::new());
        let count = cx.signal(0_u32);
        container(
            column(views![
                text("Taim macOS sandbox probe").size(28.0),
                text(format!("Storage roundtrip: PASS · launches: {launches}")),
                text(format!("Expected sandbox container: {in_container}")),
                text_field("Test keyboard / composition", input),
                text(move || format!("Typed: {}", input.get())),
                button(move || format!("Count: {}", count.get()))
                    .on_press(move || count.update(|value| *value += 1)),
            ])
            .gap(16.0),
        )
        .padding(32.0)
        .center()
        .fill()
    })?;
    Ok(())
}
