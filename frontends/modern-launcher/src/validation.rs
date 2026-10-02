use crate::app::App;
use crate::input::Action;
use crate::renderer::Renderer;
use crate::settings::{Settings, ViewMode};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn capture(mut settings: Settings, directory: PathBuf, size: [u32; 2]) -> Result<()> {
    // Captures need the full animations. The effective settings are recorded.
    settings.reduced_motion = false;
    let effective_settings = settings.clone();

    let mut renderer = pollster::block_on(Renderer::new(
        None,
        size,
        settings.quality.texture_budget_bytes(),
    ))?;

    let mut app = App::new(settings, directory.join("qa-settings.toml"))?;
    let startup = Instant::now();
    let mut startup_upload_peak = 0;
    let mut startup_update_peak_ms = 0.0_f64;
    let start = Instant::now();

    while !app.discovery_finished() && start.elapsed() < Duration::from_secs(30) {
        let before = Instant::now();
        app.update(0.016, Instant::now(), &mut renderer);
        startup_update_peak_ms =
            startup_update_peak_ms.max(before.elapsed().as_secs_f64() * 1000.0);
        startup_upload_peak = startup_upload_peak.max(renderer.last_upload_bytes);
        std::thread::sleep(Duration::from_millis(10));
    }

    anyhow::ensure!(app.discovery_finished(), "Library discovery timed out");

    if let Some(index) = app.entries.iter().position(|e| e.title.contains("Zelda")) {
        app.select(index);
    }

    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(25) {
        let before = Instant::now();
        app.update(0.016, Instant::now(), &mut renderer);
        startup_update_peak_ms =
            startup_update_peak_ms.max(before.elapsed().as_secs_f64() * 1000.0);
        startup_upload_peak = startup_upload_peak.max(renderer.last_upload_bytes);

        if !app.pending() {
            break;
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    let library_ready_ms = startup.elapsed().as_secs_f64() * 1000.0;

    for _ in 0..60 {
        app.update(1.0 / 60.0, Instant::now(), &mut renderer);
    }

    if std::env::var("SLY_CAPTURE_CONSOLES_ONLY").is_ok_and(|value| value == "1") {
        capture_consoles(&mut app, &mut renderer, &directory, size)?;
        app.close();
        return Ok(());
    }

    let primary_theme = app.theme_accent();
    app.animation.advance(2.0);

    let render = |app: &mut App, renderer: &mut Renderer| -> Result<()> {
        renderer.render(&app.scene(size), &app.ui(size, 1.0))
    };

    for (name, view) in [
        ("row", ViewMode::Row),
        ("column", ViewMode::Column),
        ("ribbon", ViewMode::Ribbon),
    ] {
        app.settings.layout = view;
        app.animation.set_view(view);
        app.animation.advance(2.0);
        app.animation.set_ambient_phase(0.0);
        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join(format!("{name}.png")))?;
    }

    app.settings.layout = ViewMode::Row;
    app.animation.set_view(ViewMode::Row);
    app.animation.advance(2.0);
    app.action(Action::Flip);
    let flip_frames = [0, 4, 8, 12, 18, 30];

    for frame in 0..=30 {
        if frame > 0 {
            app.animation.advance(1.0 / 60.0);
        }

        if flip_frames.contains(&frame) {
            render(&mut app, &mut renderer)?;
            renderer.capture(&directory.join(format!("flip-{frame:03}.png")))?;
        }
    }

    app.animation.advance(1.5);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("back.png"))?;
    app.action(Action::Flip);
    app.animation.advance(2.0);
    render(&mut app, &mut renderer)?;
    let navigation_capture = capture_navigation(&mut app, &mut renderer, &directory, size)?;
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("effects-reference.png"))?;
    let mut effect_timings = serde_json::Map::new();

    effect_timings.insert(
        "all_enabled".into(),
        measure_composition(&app, &mut renderer, size)?,
    );

    for effect in ["contours", "lighting"] {
        if effect == "contours" {
            app.contours = false;
        } else {
            app.lighting = false;
        }

        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join(format!("without-{effect}.png")))?;

        effect_timings.insert(
            format!("without_{effect}"),
            measure_composition(&app, &mut renderer, size)?,
        );

        app.contours = true;
        app.lighting = true;
    }

    app.animation.set_ambient_phase(0.0);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("ambient-a.png"))?;
    app.animation.set_ambient_phase(std::f32::consts::FRAC_PI_2);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("ambient-b.png"))?;

    for (name, view) in [("column", ViewMode::Column), ("ribbon", ViewMode::Ribbon)] {
        app.settings.layout = view;
        app.animation.set_view(view);
        app.animation.advance(2.0);

        for (phase_name, phase) in [("a", 0.0), ("b", std::f32::consts::FRAC_PI_2)] {
            app.animation.set_ambient_phase(phase);
            render(&mut app, &mut renderer)?;
            renderer.capture(&directory.join(format!("ambient-{name}-{phase_name}.png")))?;
        }
    }

    app.settings.layout = ViewMode::Row;
    app.animation.set_view(ViewMode::Row);
    app.animation.advance(2.0);
    app.animation.set_reduced_motion(true);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("reduced-motion.png"))?;
    app.animation.set_ambient_phase(std::f32::consts::PI);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("reduced-motion-other-phase.png"))?;
    app.animation.set_reduced_motion(false);
    app.animation.set_ambient_phase(0.0);
    app.action(Action::Launch);
    app.animation.advance(0.64);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("launch-closeup.png"))?;
    app.animation.advance(0.46);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("launch-iris.png"))?;
    let wait = Instant::now();

    while !app.entries.is_empty()
        && app.session.is_none()
        && wait.elapsed() < Duration::from_secs(20)
    {
        app.update(0.016, Instant::now(), &mut renderer);
        std::thread::sleep(Duration::from_millis(5));
    }

    if !app.entries.is_empty() {
        anyhow::ensure!(
            app.session.is_some(),
            "Real core failed to launch: {}",
            app.status
        );

        let base = Instant::now();

        for frame in 0..240 {
            app.update(
                1.0 / 60.0,
                base + Duration::from_secs_f64(frame as f64 / 60.0),
                &mut renderer,
            );
        }

        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("gameplay.png"))?;
        app.action(Action::Pause);
        app.action(Action::QuickSave);
        anyhow::ensure!(app.status == "Quick state saved", "{}", app.status);
        app.action(Action::QuickLoad);
        anyhow::ensure!(app.status == "Quick state loaded", "{}", app.status);
        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("paused.png"))?;
        app.action(Action::Back);
        app.action(Action::Back);
        app.animation.advance(0.29);
        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("return-iris.png"))?;
        app.animation.advance(1.0);
    }

    let mut recovery_smoke = false;

    if !app.entries.is_empty() {
        let selected = app.selected;
        app.action(Action::Launch);
        app.animation.advance(0.30);
        app.action(Action::Back);

        for _ in 0..20 {
            app.update(0.05, Instant::now(), &mut renderer);
            std::thread::sleep(Duration::from_millis(5));
        }

        anyhow::ensure!(
            app.session.is_none()
                && app.selected == selected
                && app.context() == crate::input::InputContext::Browsing,
            "Cancelled load reactivated or lost selection"
        );

        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("cancelled-launch.png"))?;
        let original = app.entries[selected].path.clone();
        app.entries[selected].path = directory.join("missing-rom.sfc");
        app.action(Action::Launch);
        let wait = Instant::now();

        while app.animation.phase == crate::animation::Phase::Launching
            && wait.elapsed() < Duration::from_secs(5)
        {
            app.update(0.05, Instant::now(), &mut renderer);
            std::thread::sleep(Duration::from_millis(5));
        }

        app.entries[selected].path = original;
        app.animation.advance(1.0);

        anyhow::ensure!(
            app.session.is_none()
                && app.selected == selected
                && app.context() == crate::input::InputContext::Browsing
                && !app.status.is_empty(),
            "Failed load did not restore library"
        );

        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("failed-launch.png"))?;
        recovery_smoke = true;
    }

    app.action(Action::Settings);
    render(&mut app, &mut renderer)?;
    renderer.capture(&directory.join("settings.png"))?;
    app.action(Action::Back);
    let mut japan_theme = None;

    if let Some(index) = app
        .entries
        .iter()
        .position(|e| e.region == Some(core_model::Region::Japan))
    {
        app.select(index);
        app.update(2.0, Instant::now(), &mut renderer);
        let wait = Instant::now();

        while app.pending() && wait.elapsed() < Duration::from_secs(10) {
            app.update(0.016, Instant::now(), &mut renderer);
            std::thread::sleep(Duration::from_millis(10));
        }

        for _ in 0..60 {
            app.update(1.0 / 60.0, Instant::now(), &mut renderer);
        }

        japan_theme = Some(app.theme_accent());
        app.animation.set_ambient_phase(0.0);
        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("japan.png"))?;
    }

    let mut unknown_theme = None;

    if let Some(entry) = app.entries.get(app.selected) {
        let hash = entry.sha1.clone();

        let previous = app
            .settings
            .release_overrides
            .insert(hash.clone(), crate::models::ReleaseMarket::Unknown);

        app.invalidate_artwork();
        let wait = Instant::now();

        loop {
            app.update(1.0 / 60.0, Instant::now(), &mut renderer);

            if !app.pending() {
                break;
            }

            anyhow::ensure!(
                wait.elapsed() < Duration::from_secs(15),
                "Unknown-release artwork timed out"
            );

            std::thread::sleep(Duration::from_millis(10));
        }

        for _ in 0..90 {
            app.update(1.0 / 60.0, Instant::now(), &mut renderer);
        }

        unknown_theme = Some(app.theme_accent());
        app.animation.set_ambient_phase(0.0);
        render(&mut app, &mut renderer)?;
        renderer.capture(&directory.join("unknown-missing-art.png"))?;

        if let Some(previous) = previous {
            app.settings.release_overrides.insert(hash, previous);
        } else {
            app.settings.release_overrides.remove(&hash);
        }

        app.invalidate_artwork();

        for _ in 0..90 {
            app.update(1.0 / 60.0, Instant::now(), &mut renderer);
        }

        app.animation.set_ambient_phase(0.0);
    }

    renderer.render(&app.scene_with_scale(size, 2.0), &app.ui(size, 2.0))?;
    renderer.capture(&directory.join("dpi-200-percent.png"))?;
    let mut text_sample = crate::ui::UiFrame::new(size, 1.0);

    text_sample.rect(
        [0.0, 0.0, size[0] as f32, size[1] as f32],
        [247, 246, 240, 255],
        0.0,
    );

    text_sample.text(
        48.0,
        64.0,
        size[0] as f32 - 96.0,
        32.0,
        "日本語表示 / Japanese type specimen",
        [45, 49, 42, 255],
    );

    text_sample.text(
        48.0,
        125.0,
        size[0] as f32 - 96.0,
        28.0,
        "スーパーファミコン　マザー２　ゼルダの伝説",
        [45, 49, 42, 255],
    );

    renderer.render(&crate::renderer::Scene::default(), &text_sample)?;
    renderer.capture(&directory.join("japanese-text.png"))?;
    let mut frame_times = Vec::new();
    let mut upload_peak = 0;
    let mut update_peak_ms = 0.0_f64;
    let benchmark_start = Instant::now();

    for index in 0..360 {
        if index % 30 == 0 {
            app.action(Action::Navigate(if index < 180 { 1 } else { -1 }));
        }

        let before = Instant::now();

        app.update(
            1.0 / 60.0,
            benchmark_start + Duration::from_secs_f64(index as f64 / 60.0),
            &mut renderer,
        );

        update_peak_ms = update_peak_ms.max(before.elapsed().as_secs_f64() * 1000.0);
        upload_peak = upload_peak.max(renderer.last_upload_bytes);
        render(&mut app, &mut renderer)?;
        renderer.device.poll(wgpu::PollType::wait_indefinitely())?;
        frame_times.push(before.elapsed().as_secs_f64() * 1000.0);
    }

    frame_times.sort_by(f64::total_cmp);
    let cache = app.cache_stats();

    let report = serde_json::json!({
        "adapter": format!("{:?}", renderer.adapter_info),
        "size": size,
        "effective_settings": effective_settings,
        "models": renderer.model_diagnostics(),
        "effect_composition_timings_ms": effect_timings,
        "cover_themes_srgb": {
            "primary": primary_theme,
            "japan": japan_theme,
            "unknown": unknown_theme,
        },
        "presentation": renderer.presentation_diagnostics(),
        "navigation_capture": navigation_capture,
        "flip_capture": {
            "step_seconds": 1.0 / 60.0,
            "snapshot_frames": flip_frames,
            "spring_rate": crate::animation::PRESENTATION.flip_rate,
        },
        "sample_count": frame_times.len(),
        "measurement": "Warm library update, active navigation, pending upload, UI/encode/submit \
            plus serialized GPU completion; offscreen, excludes display vsync",
        "startup": {
            "library_ready_ms": library_ready_ms,
            "peak_upload_bytes_per_update": startup_upload_peak,
            "peak_update_ms": startup_update_peak_ms,
            "measurement": "Discovery, artwork readiness, decode and upload using the existing \
                shared artwork cache; includes worker polling sleeps",
        },
        "peak_upload_bytes_per_frame": upload_peak,
        "peak_update_ms": update_peak_ms,
        "process_memory": process_memory(),
        "milliseconds": {
            "p50": frame_times[180],
            "p95": frame_times[342],
            "p99": frame_times[356],
        },
        "cpu_artwork_bytes": app.cpu_artwork_bytes(),
        "gpu_artwork_bytes": renderer.artwork_bytes(),
        "cache": {
            "cpu_bytes": cache.cpu_bytes,
            "budget_bytes": cache.budget_bytes,
            "cached_items": cache.cached_items,
            "queued": cache.queued,
            "active": cache.active,
            "completed": cache.completed,
            "pending": cache.pending,
            "stale_discarded": cache.stale_discarded,
        },
        "last_draws": renderer.last_draws,
        "games": app.entries.len(),
        "roms": app
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "sha1": entry.sha1,
                    "title": entry.title,
                    "market": format!("{:?}", entry.region),
                })
            })
            .collect::<Vec<_>>(),
        "real_core_smoke": !app.entries.is_empty(),
        "cancel_and_failure_smoke": recovery_smoke,
    });

    std::fs::write(
        directory.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    app.close();
    std::fs::metadata(directory.join("row.png")).context("No capture written")?;
    Ok(())
}

fn capture_consoles(
    app: &mut App,
    renderer: &mut Renderer,
    directory: &std::path::Path,
    size: [u32; 2],
) -> Result<()> {
    let initial = app
        .entries
        .get(app.selected)
        .context("No ROMs discovered")?;

    let original_system = initial.system;
    let original_sha1 = initial.sha1.clone();
    app.action(Action::Consoles);
    app.animation.advance(2.0);
    renderer.render(&app.scene(size), &app.ui(size, 1.0))?;
    renderer.capture(&directory.join("console-picker.png"))?;
    app.action(Action::Back);

    let mut consoles = Vec::new();

    for _ in core_model::System::ALL {
        let system = app.entries[app.selected].system;

        anyhow::ensure!(
            app.entries.iter().all(|entry| entry.system == system),
            "Console shelf mixes systems"
        );

        if system == core_model::System::Gba
            && let Some(index) = app.entries.iter().position(|entry| {
                entry.title.contains("Zelda") && entry.region == Some(core_model::Region::Europe)
            })
        {
            app.select(index);
        }

        let wait = Instant::now();

        loop {
            app.update(1.0 / 60.0, Instant::now(), renderer);

            if !app.pending() {
                break;
            }

            anyhow::ensure!(
                wait.elapsed() < Duration::from_secs(40),
                "Console artwork timed out"
            );

            std::thread::sleep(Duration::from_millis(10));
        }

        for _ in 0..90 {
            app.update(1.0 / 60.0, Instant::now(), renderer);
        }

        app.animation.set_ambient_phase(0.0);
        let entry = &app.entries[app.selected];
        let scene = app.scene(size);

        let selected_objects = scene.objects.iter().filter(|object| {
            object.artwork.as_ref().is_some_and(|key| key.sha1 == entry.sha1)
        }).map(|object| serde_json::json!({
            "model": format!("{:?}", object.kind),
            "suppresses_artwork": object.placeholder,
            "gpu_resident": object.artwork.as_ref().is_some_and(|key| renderer.has_artwork(key)),
        })).collect::<Vec<_>>();

        if system == core_model::System::Gba {
            anyhow::ensure!(
                selected_objects
                    .iter()
                    .any(|object| object["gpu_resident"] == true
                        && object["suppresses_artwork"] == false),
                "GBA artwork is absent or suppressed by proxy geometry"
            );
        }

        consoles.push(serde_json::json!({
            "system": system.slug(), "games": app.entries.len(), "selected_title": entry.title,
            "selected_sha1": entry.sha1, "region": format!("{:?}", entry.region),
            "theme_srgb": app.theme_accent(), "selected_objects": selected_objects,
        }));

        renderer.render(&scene, &app.ui(size, 1.0))?;
        renderer.capture(&directory.join(format!("{}-front.png", system.slug())))?;
        app.action(Action::Flip);
        app.animation.advance(2.0);
        renderer.render(&app.scene(size), &app.ui(size, 1.0))?;
        renderer.capture(&directory.join(format!("{}-back.png", system.slug())))?;
        app.action(Action::Flip);
        app.animation.advance(2.0);

        if system == core_model::System::Gba {
            let identity = app.entries[app.selected].game_id();
            let original_finish = app.settings.cartridge_finish(&identity).map(str::to_owned);
            app.menu = Some(crate::app::Menu::Appearance);

            for finish in crate::materials::cartridge_finishes(system) {
                app.settings
                    .set_cartridge_finish(&identity, Some(finish.id));
                renderer.render(&app.scene(size), &app.ui(size, 1.0))?;
                renderer.capture(&directory.join(format!("gba-appearance-{}.png", finish.id)))?;
            }

            app.settings
                .set_cartridge_finish(&identity, original_finish.as_deref());
            app.menu = None;
            let previous_view = app.settings.layout;

            for (name, view) in [
                ("row", ViewMode::Row),
                ("column", ViewMode::Column),
                ("ribbon", ViewMode::Ribbon),
            ] {
                app.settings.layout = view;
                app.animation.set_view(view);
                app.animation.advance(2.0);
                renderer.render(&app.scene(size), &app.ui(size, 1.0))?;
                renderer.capture(&directory.join(format!("gba-{name}.png")))?;
            }

            app.settings.layout = previous_view;
            app.animation.set_view(previous_view);
            app.animation.advance(2.0);
        }

        app.action(Action::Console(1));

        if app.entries[app.selected].system == original_system {
            break;
        }
    }

    anyhow::ensure!(
        app.entries[app.selected].system == original_system
            && app.entries[app.selected].sha1 == original_sha1,
        "Console switch did not restore the original selection"
    );

    let report = serde_json::json!({
        "adapter": format!("{:?}", renderer.adapter_info), "size": size,
        "consoles": consoles, "selection_restored": true, "emulation_exercised": false,
        "cpu_artwork_bytes": app.cpu_artwork_bytes(), "gpu_artwork_bytes": renderer.artwork_bytes(),
    });

    std::fs::write(
        directory.join("console-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

/// Captures 60 Hz navigation frames. SLY_CAPTURE_NAVIGATION=1 saves every frame.
fn capture_navigation(
    app: &mut App,
    renderer: &mut Renderer,
    directory: &std::path::Path,
    size: [u32; 2],
) -> Result<serde_json::Value> {
    if app.entries.len() < 2 {
        return Ok(serde_json::json!({"skipped": "fewer than two entries"}));
    }

    let original = app.selected;

    let next = if original + 1 < app.entries.len() {
        original + 1
    } else {
        original - 1
    };

    let full = std::env::var("SLY_CAPTURE_NAVIGATION").is_ok_and(|value| value == "1");
    let snapshots = [0, 1, 4, 8, 12, 18, 30, 60];

    for (name, flipped) in [("navigation", false), ("navigation-flipped", true)] {
        app.select(original);
        app.update(2.0, Instant::now(), renderer);

        if flipped {
            app.action(Action::Flip);
            app.animation.advance(2.0);
        }

        app.animation.set_ambient_phase(0.0);
        let start = Instant::now();

        for frame in 0..=60 {
            if frame == 1 {
                app.select(next);
            }

            if frame > 0 {
                app.update(
                    1.0 / 60.0,
                    start + Duration::from_secs_f64(frame as f64 / 60.0),
                    renderer,
                );
            }

            renderer.render(&app.scene(size), &app.ui(size, 1.0))?;

            if full || snapshots.contains(&frame) {
                renderer.capture(&directory.join(format!("{name}-{frame:03}.png")))?;
            }
        }
    }

    app.select(original);

    for _ in 0..90 {
        app.update(1.0 / 60.0, Instant::now(), renderer);
    }

    app.animation.set_ambient_phase(0.0);

    Ok(serde_json::json!({
        "step_seconds": 1.0 / 60.0,
        "frames": 61,
        "all_frames_written": full,
        "snapshot_frames": snapshots,
        "sequences": ["navigation", "navigation-flipped"],
    }))
}

fn measure_composition(
    app: &App,
    renderer: &mut Renderer,
    size: [u32; 2],
) -> Result<serde_json::Value> {
    let scene = app.scene(size);
    let ui = app.ui(size, 1.0);
    let mut samples = Vec::with_capacity(120);

    for index in 0..130 {
        let start = Instant::now();
        renderer.render(&scene, &ui)?;
        renderer.device.poll(wgpu::PollType::wait_indefinitely())?;

        if index >= 10 {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }

    samples.sort_by(f64::total_cmp);

    Ok(serde_json::json!({
        "p50": samples[60],
        "p95": samples[114],
        "p99": samples[118],
        "samples": samples.len(),
    }))
}

fn process_memory() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .map(|text| {
                text.lines()
                    .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
