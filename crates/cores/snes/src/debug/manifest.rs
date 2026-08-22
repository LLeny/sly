use super::*;

pub(super) fn sa1_manifest(manifest: &mut ComponentManifest) {
    use sa1::{CPU, STATE_ID, WORKSPACE};

    manifest
        .workspaces
        .push(WorkspaceDescriptor::new(WORKSPACE, "SA-1", WorkspaceIcon::Chip).mmio_group("SA-1"));

    let disasm_id = format!("{CPU}.disasm");
    let callstack_id = format!("{CPU}.callstack");

    for id in [CPU, disasm_id.as_str(), callstack_id.as_str()] {
        manifest.untag(id, CPU_WORKSPACE);
    }

    manifest.insert_before(
        PanelKind::Mmio,
        PanelDescriptor::new(
            STATE_ID,
            "SA-1 State",
            PanelKind::Structured,
            DockLocation::Right,
            RefreshPolicy::OnPause,
        )
        .tag(WORKSPACE),
    );

    for id in [
        CPU,
        disasm_id.as_str(),
        callstack_id.as_str(),
        "breakpoints",
        "memory",
        "log",
    ] {
        manifest.tag(id, WORKSPACE);
    }

    manifest.compose_in(
        WORKSPACE,
        disasm_id,
        callstack_id,
        PanelGroupLayout::CompanionRail,
    );
    manifest.compose_in(WORKSPACE, "memory", "mmio", PanelGroupLayout::CompanionRail);
}

pub(super) fn superfx_manifest(manifest: &mut ComponentManifest) {
    use superfx::{CACHE_ID, CPU_GSU, STATE_ID, WORKSPACE};

    manifest.workspaces.push(
        WorkspaceDescriptor::new(WORKSPACE, "Super FX", WorkspaceIcon::Chip).mmio_group("GSU"),
    );

    manifest.remove(&format!("{CPU_GSU}.callstack"));

    let disasm_id = format!("{CPU_GSU}.disasm");

    for id in [CPU_GSU, disasm_id.as_str()] {
        manifest.untag(id, CPU_WORKSPACE);
    }

    manifest.insert_before(
        PanelKind::Mmio,
        PanelDescriptor::new(
            STATE_ID,
            "GSU State",
            PanelKind::Structured,
            DockLocation::Right,
            RefreshPolicy::OnPause,
        )
        .tag(WORKSPACE),
    );
    manifest.insert_after(
        PanelKind::Memory,
        PanelDescriptor::new(
            CACHE_ID,
            "Code Cache",
            PanelKind::Table,
            DockLocation::Center,
            RefreshPolicy::OnPause,
        )
        .tag(WORKSPACE),
    );

    for id in [
        CPU_GSU,
        disasm_id.as_str(),
        "breakpoints",
        "memory",
        "tilemap",
        "log",
    ] {
        manifest.tag(id, WORKSPACE);
    }

    manifest.compose_in(
        WORKSPACE,
        disasm_id,
        CACHE_ID,
        PanelGroupLayout::CompanionRail,
    );
    manifest.compose_in(WORKSPACE, "memory", "mmio", PanelGroupLayout::CompanionRail);
}

impl SnesDebug {
    pub(super) fn build_manifest(&self) -> ComponentManifest {
        let mut manifest = standard_manifest(&self.cpus());
        manifest.workspaces.push(
            WorkspaceDescriptor::new(VIDEO_WORKSPACE, "PPU", WorkspaceIcon::Video)
                .mmio_group("PPU"),
        );

        manifest.insert_after(
            PanelKind::Memory,
            PanelDescriptor::new(
                "framebuffer",
                "Screen",
                PanelKind::Framebuffer,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            ),
        );
        manifest.insert_after(
            PanelKind::Breakpoints,
            PanelDescriptor::new(
                "palette",
                "Palettes",
                PanelKind::Palette,
                DockLocation::Left,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Mmio,
            PanelDescriptor::new(
                "dma",
                "DMA",
                PanelKind::Dma,
                DockLocation::Right,
                RefreshPolicy::OnPause,
            )
            .tag(CPU_WORKSPACE)
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_before(
            PanelKind::Mmio,
            PanelDescriptor::new(
                "ppu",
                "PPU",
                PanelKind::Ppu,
                DockLocation::Right,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_before(
            PanelKind::Memory,
            PanelDescriptor::new(
                "tileset",
                "Tileset",
                PanelKind::Tileset,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Tileset,
            PanelDescriptor::new(
                "tilemap",
                "Tilemap",
                PanelKind::Tilemap,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Tilemap,
            PanelDescriptor::new(
                "sprites",
                "Sprites",
                PanelKind::Sprites,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );

        // The sprites panel takes the memory viewer's place in the PPU workspace.
        manifest.restrict(PanelKind::Memory, CPU_WORKSPACE);

        manifest.workspaces.push(
            WorkspaceDescriptor::new(SOUND_WORKSPACE, "APU", WorkspaceIcon::Sound)
                .mmio_group("APU"),
        );

        manifest.insert_before(
            PanelKind::Memory,
            PanelDescriptor::new(
                "audio_scope",
                "Output",
                PanelKind::AudioScope,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::AudioScope,
            PanelDescriptor::new(
                dsp::INSPECTOR_ID,
                "Audio Inspector",
                PanelKind::Structured,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .selection_from(dsp::VOICES_ID)
            .tag(SOUND_WORKSPACE),
        );
        manifest.compose_in(
            SOUND_WORKSPACE,
            "audio_scope",
            dsp::INSPECTOR_ID,
            PanelGroupLayout::CompactColumns,
        );
        manifest.insert_after(
            PanelKind::AudioScope,
            PanelDescriptor::new(
                dsp::SCOPES_ID,
                "Voice Scopes",
                PanelKind::ChannelScopes,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        manifest.insert_before(
            PanelKind::Mmio,
            PanelDescriptor::new(
                dsp::VOICES_ID,
                "DSP Voices",
                PanelKind::Table,
                DockLocation::Right,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        manifest.relocate_after("framebuffer", PanelKind::Controls, DockLocation::Left);
        manifest.insert_after(
            PanelKind::Framebuffer,
            PanelDescriptor::new(
                dsp::REGISTERS_ID,
                "DSP Registers",
                PanelKind::Structured,
                DockLocation::Left,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        for id in [
            CPU_SPC,
            &format!("{CPU_SPC}.disasm"),
            &format!("{CPU_SPC}.callstack"),
            "log",
        ] {
            manifest.tag(id, SOUND_WORKSPACE);
        }
        manifest.place_after(
            "savestates",
            SOUND_WORKSPACE,
            PanelKind::Mmio,
            DockLocation::Right,
        );
        manifest.compose_in(
            SOUND_WORKSPACE,
            format!("{CPU_SPC}.disasm"),
            format!("{CPU_SPC}.callstack"),
            PanelGroupLayout::CompanionRail,
        );
        manifest.compose_in(
            CPU_WORKSPACE,
            "memory",
            "framebuffer",
            PanelGroupLayout::CompanionRail,
        );
        manifest.compose_in(
            VIDEO_WORKSPACE,
            "tileset",
            "tilemap",
            PanelGroupLayout::EqualColumns,
        );
        manifest.compose_in(
            VIDEO_WORKSPACE,
            "sprites",
            "framebuffer",
            PanelGroupLayout::EqualColumns,
        );

        if self.has_gsu() {
            self::superfx_manifest(&mut manifest);
        }
        if self.has_sa1() {
            self::sa1_manifest(&mut manifest);
        }
        manifest.workspace_last(debugger_model::SCRIPT_WORKSPACE);

        manifest
    }
}
