use super::*;
use debugger_model::panel;

pub(super) fn sa1_manifest(manifest: &mut ComponentManifest) {
    use sa1::{CPU, STATE_ID, WORKSPACE};

    manifest
        .workspaces
        .push(WorkspaceDescriptor::new(WORKSPACE, "SA-1", WorkspaceIcon::Chip).mmio_group("SA-1"));

    let disasm_id = format!("{CPU}.disasm");
    let callstack_id = format!("{CPU}.callstack");

    for id in [CPU, disasm_id.as_str(), callstack_id.as_str()] {
        manifest.untag(id, CPU_WORKSPACE).expect("shipped panel id");
    }

    manifest
        .insert_before(
            panel::MMIO,
            PanelDescriptor::new(
                STATE_ID,
                "SA-1 State",
                PanelKind::Structured,
                DockLocation::Right,
                RefreshPolicy::OnPause,
            )
            .tag(WORKSPACE),
        )
        .expect("shipped panel id");

    for id in [
        CPU,
        disasm_id.as_str(),
        callstack_id.as_str(),
        panel::BREAKPOINTS,
        panel::MEMORY,
        panel::LOG,
    ] {
        manifest.tag(id, WORKSPACE).expect("shipped panel id");
    }

    manifest.compose_in(
        WORKSPACE,
        disasm_id,
        callstack_id,
        PanelGroupLayout::CompanionRail,
    );

    manifest.compose_in(
        WORKSPACE,
        panel::MEMORY,
        panel::MMIO,
        PanelGroupLayout::CompanionRail,
    );
}

pub(super) fn superfx_manifest(manifest: &mut ComponentManifest) {
    use superfx::{CACHE_ID, CPU_GSU, STATE_ID, WORKSPACE};

    manifest.workspaces.push(
        WorkspaceDescriptor::new(WORKSPACE, "Super FX", WorkspaceIcon::Chip).mmio_group("GSU"),
    );

    manifest
        .remove(&format!("{CPU_GSU}.callstack"))
        .expect("shipped panel id");

    let disasm_id = format!("{CPU_GSU}.disasm");

    for id in [CPU_GSU, disasm_id.as_str()] {
        manifest.untag(id, CPU_WORKSPACE).expect("shipped panel id");
    }

    manifest
        .insert_before(
            panel::MMIO,
            PanelDescriptor::new(
                STATE_ID,
                "GSU State",
                PanelKind::Structured,
                DockLocation::Right,
                RefreshPolicy::OnPause,
            )
            .tag(WORKSPACE),
        )
        .expect("shipped panel id");

    manifest
        .insert_after(
            panel::MEMORY,
            PanelDescriptor::new(
                CACHE_ID,
                "Code Cache",
                PanelKind::Table,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(WORKSPACE),
        )
        .expect("shipped panel id");

    for id in [
        CPU_GSU,
        disasm_id.as_str(),
        panel::BREAKPOINTS,
        panel::MEMORY,
        panel::BITMAP,
        panel::LOG,
    ] {
        manifest.tag(id, WORKSPACE).expect("shipped panel id");
    }

    manifest.compose_in(
        WORKSPACE,
        disasm_id,
        CACHE_ID,
        PanelGroupLayout::CompanionRail,
    );

    manifest.compose_in(
        WORKSPACE,
        panel::MEMORY,
        panel::MMIO,
        PanelGroupLayout::CompanionRail,
    );
}

impl SnesDebug {
    pub(super) fn build_manifest(&self) -> ComponentManifest {
        let mut manifest = standard_manifest(&self.cpus());
        manifest.workspaces.push(
            WorkspaceDescriptor::new(VIDEO_WORKSPACE, "PPU", WorkspaceIcon::Video)
                .mmio_group("PPU"),
        );

        manifest
            .insert_after(
                panel::BREAKPOINTS,
                PanelDescriptor::new(
                    panel::PALETTE,
                    "Palettes",
                    PanelKind::Palette,
                    DockLocation::Left,
                    RefreshPolicy::OnPause,
                )
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_after(
                panel::MMIO,
                PanelDescriptor::new(
                    panel::DMA,
                    "DMA",
                    PanelKind::Structured,
                    DockLocation::Right,
                    RefreshPolicy::OnPause,
                )
                .tag(CPU_WORKSPACE)
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_before(
                panel::MMIO,
                PanelDescriptor::new(
                    panel::PPU,
                    "PPU",
                    PanelKind::Structured,
                    DockLocation::Right,
                    RefreshPolicy::OnPause,
                )
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_before(
                panel::MEMORY,
                PanelDescriptor::new(
                    panel::TILESET,
                    "Tileset",
                    PanelKind::Tileset,
                    DockLocation::Center,
                    RefreshPolicy::OnPause,
                )
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_after(
                panel::TILESET,
                PanelDescriptor::new(
                    panel::TILEMAP,
                    "Tilemap",
                    PanelKind::Tilemap,
                    DockLocation::Center,
                    RefreshPolicy::OnPause,
                )
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_after(
                panel::TILEMAP,
                PanelDescriptor::new(
                    panel::SPRITES,
                    "Sprites",
                    PanelKind::Sprites,
                    DockLocation::Center,
                    RefreshPolicy::OnPause,
                )
                .tag(VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest.workspaces.push(
            WorkspaceDescriptor::new(SOUND_WORKSPACE, "APU", WorkspaceIcon::Sound)
                .mmio_group("APU"),
        );

        manifest
            .insert_before(
                panel::MEMORY,
                PanelDescriptor::new(
                    panel::AUDIO_SCOPE,
                    "Output",
                    PanelKind::AudioScope,
                    DockLocation::Center,
                    RefreshPolicy::Continuous,
                )
                .tag(SOUND_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_after(
                panel::AUDIO_SCOPE,
                PanelDescriptor::new(
                    dsp::INSPECTOR_ID,
                    "Audio Inspector",
                    PanelKind::Structured,
                    DockLocation::Center,
                    RefreshPolicy::Continuous,
                )
                .selection_from(dsp::VOICES_ID)
                .tag(SOUND_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest.compose_in(
            SOUND_WORKSPACE,
            panel::AUDIO_SCOPE,
            dsp::INSPECTOR_ID,
            PanelGroupLayout::CompactColumns,
        );

        manifest
            .insert_after(
                panel::AUDIO_SCOPE,
                PanelDescriptor::new(
                    dsp::SCOPES_ID,
                    "Voice Scopes",
                    PanelKind::ChannelScopes,
                    DockLocation::Center,
                    RefreshPolicy::Continuous,
                )
                .tag(SOUND_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_before(
                panel::MMIO,
                PanelDescriptor::new(
                    dsp::VOICES_ID,
                    "DSP Voices",
                    PanelKind::Table,
                    DockLocation::Right,
                    RefreshPolicy::Continuous,
                )
                .tag(SOUND_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest
            .insert_after(
                panel::FRAMEBUFFER,
                PanelDescriptor::new(
                    dsp::REGISTERS_ID,
                    "DSP Registers",
                    PanelKind::Structured,
                    DockLocation::Left,
                    RefreshPolicy::Continuous,
                )
                .tag(SOUND_WORKSPACE),
            )
            .expect("shipped panel id");

        for id in [
            CPU_SPC,
            &format!("{CPU_SPC}.disasm"),
            &format!("{CPU_SPC}.callstack"),
            panel::LOG,
        ] {
            manifest.tag(id, SOUND_WORKSPACE).expect("shipped panel id");
        }

        manifest
            .place_after(
                panel::SAVESTATES,
                SOUND_WORKSPACE,
                panel::MMIO,
                DockLocation::Right,
            )
            .expect("shipped panel id");

        manifest.compose_in(
            SOUND_WORKSPACE,
            format!("{CPU_SPC}.disasm"),
            format!("{CPU_SPC}.callstack"),
            PanelGroupLayout::CompanionRail,
        );

        manifest.compose_in(
            CPU_WORKSPACE,
            panel::MEMORY,
            panel::FRAMEBUFFER,
            PanelGroupLayout::CompanionRail,
        );

        manifest.compose_in(
            VIDEO_WORKSPACE,
            panel::TILESET,
            panel::TILEMAP,
            PanelGroupLayout::EqualColumns,
        );

        manifest.compose_in(
            VIDEO_WORKSPACE,
            panel::SPRITES,
            panel::FRAMEBUFFER,
            PanelGroupLayout::EqualColumns,
        );

        if self.has_gsu() {
            manifest
                .insert_after(
                    panel::SPRITES,
                    PanelDescriptor::new(
                        panel::BITMAP,
                        "Bitmap",
                        PanelKind::Bitmap,
                        DockLocation::Center,
                        RefreshPolicy::OnPause,
                    )
                    .tag(VIDEO_WORKSPACE),
                )
                .expect("shipped panel id");

            self::superfx_manifest(&mut manifest);
        }

        if self.has_sa1() {
            self::sa1_manifest(&mut manifest);
        }

        manifest
    }
}
