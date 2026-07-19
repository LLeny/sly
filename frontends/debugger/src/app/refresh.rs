use super::*;

impl Debugger {
    pub(super) fn refresh_pane_view(
        target: &dyn DebugCore,
        pane: &mut ProcessorPane,
        mode: CpuRefresh,
    ) {
        pane.cpu_view = Arc::new(
            Self::track_changes(target.cpu_view(&pane.id), &mut pane.reg_changes, mode).into(),
        );
        pane.disasm.recompute(target);
        pane.call_stack = Arc::new(target.call_stack(&pane.id).into());
    }

    pub(super) fn selected_color(&self) -> usize {
        self.palette_selected.map_or(0, |(palette, color)| {
            let row = self
                .palette_view
                .palettes
                .get(palette)
                .map_or(16, |row| row.len());
            palette * row + color
        })
    }

    pub(super) fn invalidate_video_keys(&mut self) {
        self.palette_key = None;
        self.tileset_key = None;
        self.tilemap_key = None;
        self.sprites_key = None;
    }

    pub(super) fn drop_grid_image(cx: &mut Context<Self>, image: Option<Arc<RenderImage>>) {
        if let Some(image) = image {
            cx.defer(move |cx| cx.drop_image(image, None));
        }
    }

    pub(super) fn refresh_tileset(&mut self, cx: &mut Context<Self>) {
        self.refresh_tileset_keyed(self.target.video_generations(), cx);
    }

    pub(super) fn refresh_tileset_keyed(
        &mut self,
        gens: Option<VideoGenerations>,
        cx: &mut Context<Self>,
    ) {
        let key = gens.map(|g| (g.tileset, self.tileset_ui.layer, self.selected_color()));
        if key.is_some() && key == self.tileset_key && self.tileset.is_some() {
            return;
        }
        self.tileset_key = key;

        Self::drop_grid_image(cx, self.tileset.take().and_then(|view| view.image));
        self.tileset = self
            .target
            .tileset(self.tileset_ui.layer, self.selected_color())
            .map(Into::into);
    }

    pub(super) fn refresh_tilemap(&mut self, cx: &mut Context<Self>) {
        self.refresh_tilemap_keyed(self.target.video_generations(), cx);
    }

    pub(super) fn refresh_tilemap_keyed(
        &mut self,
        gens: Option<VideoGenerations>,
        cx: &mut Context<Self>,
    ) {
        let key = gens.map(|g| (g.tilemap, self.tilemap_ui.layer));
        if key.is_some() && key == self.tilemap_key && self.tilemap.is_some() {
            let viewport = self.target.tilemap_viewport(self.tilemap_ui.layer);
            if let Some(view) = &mut self.tilemap {
                view.viewport = viewport.map(Into::into);
            }

            return;
        }
        self.tilemap_key = key;

        Self::drop_grid_image(cx, self.tilemap.take().and_then(|view| view.image));
        self.tilemap = self.target.tilemap(self.tilemap_ui.layer).map(Into::into);
    }

    pub(super) fn refresh_sprites(&mut self, cx: &mut Context<Self>) {
        self.refresh_sprites_keyed(self.target.video_generations(), cx);
    }

    pub(super) fn refresh_sprites_keyed(
        &mut self,
        gens: Option<VideoGenerations>,
        cx: &mut Context<Self>,
    ) {
        if self.is_collapsed(SpritesWidget::ID) {
            return;
        }

        let key = gens.map(|g| g.sprites);
        if key.is_some() && key == self.sprites_key && self.sprites.is_some() {
            return;
        }
        self.sprites_key = key;

        Self::drop_grid_image(cx, self.sprites.take().and_then(|view| view.image));
        self.sprites = self.target.sprites().map(Into::into);
    }

    pub(super) fn refresh_framebuffer(&mut self, cx: &mut Context<Self>) {
        if self.is_collapsed(FramebufferWidget::ID) {
            return;
        }

        let fb = self.target.software_frame();

        if let (Some(view), Some(fb)) = (&self.framebuffer, &fb)
            && view.matches(fb)
        {
            cirsium::widgets::stage_bgra(fb, &mut self.framebuffer_staged);
            self.framebuffer_dirty = true;
            return;
        }

        Self::drop_grid_image(cx, self.framebuffer.take().and_then(|view| view.image));
        self.framebuffer = fb.map(Into::into);
        self.framebuffer_dirty = false;
    }

    pub(super) fn should_refresh(&self, panel: &PanelDescriptor, live: bool) -> bool {
        !live || self.live_refresh || panel.refresh == RefreshPolicy::Continuous
    }

    pub(super) fn wants_refresh(&self, live: bool, kind: impl Fn(&PanelKind) -> bool) -> bool {
        self.registry
            .manifest()
            .in_workspace(&self.workspace)
            .any(|panel| kind(&panel.kind) && self.should_refresh(panel, live))
    }

    pub(super) fn refresh_panes(&mut self, mode: CpuRefresh, live: bool, cx: &mut Context<Self>) {
        for ix in 0..self.panes.len() {
            let id = self.panes[ix].id.clone();
            let visible = self.wants_refresh(live, |kind| {
                matches!(
                    kind,
                    PanelKind::Registers { cpu } | PanelKind::Disassembly { cpu }
                        if **cpu == *id.as_str()
                )
            });

            if visible {
                Self::refresh_pane_view(self.target.as_ref(), &mut self.panes[ix], mode);
            }
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Mmio)) {
            self.mmio_view = Arc::new(self.target.mmio_view().into());
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Dma)) {
            self.dma_view = Arc::new(self.target.dma_view().into());
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Framebuffer)) {
            self.refresh_framebuffer(cx);
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Memory)) {
            self.memory.refresh_cached(self.target.as_ref());
        }

        self.refresh_video_panes(live, cx);
        self.refresh_generic_panels(live);
        if self.show_log && self.wants_refresh(live, |kind| matches!(kind, PanelKind::Log)) {
            self.logs.refresh();
        }
        self.registry.refresh_visible(
            &self.workspace,
            cirsium::PanelRefresh { live },
            self.live_refresh,
            cx,
        );
    }

    pub(super) fn refresh_video_panes(&mut self, live: bool, cx: &mut Context<Self>) {
        let wants_palette = self.wants_refresh(live, |kind| matches!(kind, PanelKind::Palette));
        let wants_tileset = self.wants_refresh(live, |kind| matches!(kind, PanelKind::Tileset));
        let wants_tilemap = self.wants_refresh(live, |kind| matches!(kind, PanelKind::Tilemap));
        let wants_sprites = self.wants_refresh(live, |kind| matches!(kind, PanelKind::Sprites));

        let gens = (wants_palette || wants_tileset || wants_tilemap || wants_sprites)
            .then(|| self.target.video_generations())
            .flatten();

        if wants_palette {
            let key = gens.map(|g| g.palette);
            if key.is_none() || key != self.palette_key {
                self.palette_key = key;
                self.palette_view = Arc::new(self.target.palette_view().into());
            }
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Ppu)) {
            self.ppu_view = Arc::new(self.target.ppu_view().into());
        }

        if wants_tileset {
            self.tileset_layers = self
                .target
                .tileset_layers()
                .into_iter()
                .map(Into::into)
                .collect();
            self.refresh_tileset_keyed(gens, cx);
        }

        if wants_tilemap {
            self.refresh_tilemap_keyed(gens, cx);
        }

        if wants_sprites {
            self.refresh_sprites_keyed(gens, cx);
        }
    }

    pub(super) fn refresh_generic_panels(&mut self, live: bool) {
        let generic = self.visible_generic_panels(live);

        for id in generic.tables {
            let view = Arc::new(self.target.table_view(id.as_ref()));
            self.table_views.insert(id, view);
        }

        for (id, selection) in generic.structured {
            let view = Arc::new(self.target.structured_view(id.as_ref(), selection));
            self.structured_views.insert(id, view);
        }

        for id in generic.scopes {
            let view = self.target.scopes_view(id.as_ref());
            self.scopes_views.insert(id, view);
        }
    }

    pub(super) fn visible_generic_panels(&self, live: bool) -> GenericRefresh {
        let mut refresh = GenericRefresh::default();

        for panel in self.registry.manifest().in_workspace(&self.workspace) {
            if !self.should_refresh(panel, live) || self.is_collapsed(panel.id.as_ref()) {
                continue;
            }

            let id = SharedString::from(panel.id.clone());
            match panel.kind {
                PanelKind::Table => refresh.tables.push(id),
                PanelKind::Structured => {
                    let selection = panel
                        .selection_from
                        .as_ref()
                        .map(|table| self.table_selected(table.as_ref()));
                    refresh.structured.push((id, selection));
                }
                PanelKind::ChannelScopes => refresh.scopes.push(id),
                _ => {}
            }
        }

        refresh
    }

    pub(super) fn table_selected(&self, id: &str) -> usize {
        self.table_selection.get(id).copied().unwrap_or(0)
    }

    pub(super) fn push_scope(buffer: &mut Vec<f32>, samples: &[i16]) {
        const SCOPE_LEN: usize = 512;

        for pair in samples.chunks_exact(2) {
            let mono = (pair[0] as f32 + pair[1] as f32) * 0.5 / 32768.0;
            buffer.push(mono);
        }

        let len = buffer.len();
        if len > SCOPE_LEN {
            buffer.drain(..len - SCOPE_LEN);
        }
    }

    pub(super) fn track_changes(
        mut cpu: CpuView,
        changes: &mut ChangeTracker<Arc<str>>,
        mode: CpuRefresh,
    ) -> CpuView {
        if mode == CpuRefresh::Baseline {
            changes.clear();
        }

        for reg in &mut cpu.registers {
            let (key, value) = (reg.name.clone(), reg.value as u64);
            let highlight = changes.highlight(key, value);
            reg.highlight = match mode {
                CpuRefresh::Highlight => highlight,
                CpuRefresh::Baseline => Highlight::None,
            };
        }
        cpu
    }

    pub(super) fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed.contains(id)
            || self
                .band_host(id)
                .is_some_and(|h| self.collapsed.contains(h))
    }

    fn band_host(&self, id: &str) -> Option<&str> {
        self.registry
            .manifest()
            .host_of(self.workspace.as_ref(), id)
    }

    pub(super) fn toggle_in(set: &mut HashSet<SharedString>, id: SharedString) {
        if !set.remove(&id) {
            set.insert(id);
        }
    }

    pub(super) fn toggle(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        let host = self.band_host(&id).map(SharedString::from);
        let id = host.unwrap_or(id);
        Self::toggle_in(&mut self.collapsed, id.clone());

        if !self.collapsed.contains(&id) {
            self.refresh_framebuffer(cx);
            self.refresh_sprites(cx);
        }

        cx.notify();
    }

    pub(super) fn expanded_set(&self, widget: &str) -> HashSet<SharedString> {
        self.expanded.get(widget).cloned().unwrap_or_default()
    }

    pub(super) fn toggle_expanded(
        &mut self,
        widget: &'static str,
        id: SharedString,
        cx: &mut Context<Self>,
    ) {
        Self::toggle_in(self.expanded.entry(widget).or_default(), id);
        cx.notify();
    }

    pub(super) fn select_palette_color(
        &mut self,
        palette: usize,
        color: usize,
        cx: &mut Context<Self>,
    ) {
        let selected = Some((palette, color));
        self.palette_selected = (self.palette_selected != selected).then_some((palette, color));
        self.refresh_tileset(cx);
        cx.notify();
    }

    pub(super) fn select_table_row(
        &mut self,
        table_id: SharedString,
        row: usize,
        cx: &mut Context<Self>,
    ) {
        self.table_selection.insert(table_id.clone(), row);

        let linked: Vec<SharedString> = self
            .registry
            .manifest()
            .in_workspace(&self.workspace)
            .filter(|p| p.selection_from.as_deref() == Some(table_id.as_ref()))
            .map(|p| SharedString::from(p.id.clone()))
            .collect();
        for id in linked {
            let view = Arc::new(self.target.structured_view(id.as_ref(), Some(row)));
            self.structured_views.insert(id, view);
        }

        cx.notify();
    }

    pub(super) fn wire_grid(
        &self,
        panel: GridPanel,
        id: &'static str,
        state: &GridState,
        ui: fn(&mut Self) -> &mut GridState,
        refresh: fn(&mut Self, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) -> GridPanel {
        let this = cx.entity();
        panel
            .layer(state.layer)
            .selected(state.selected())
            .hovered(state.hovered)
            .viewport(state.viewport)
            .collapsed(self.is_collapsed(id))
            .on_toggle(cx.listener(move |this, _, _, cx| this.toggle(id, cx)))
            .on_layer_select({
                let this = this.clone();
                move |layer, _, cx| {
                    this.update(cx, |d, cx| {
                        ui(d).layer = layer;
                        refresh(d, cx);
                        cx.notify();
                    })
                }
            })
            .on_select({
                let this = this.clone();
                move |index, _, cx| {
                    this.update(cx, |d, cx| {
                        ui(d).toggle_selected(index);
                        cx.notify();
                    })
                }
            })
            .on_hover({
                let this = this.clone();
                move |hovered, _, cx| {
                    this.update(cx, |d, cx| {
                        if ui(d).set_hovered(hovered) {
                            cx.notify();
                        }
                    })
                }
            })
            .on_viewport({
                let this = this.clone();
                move |viewport, _, cx| {
                    this.update(cx, |d, cx| {
                        if ui(d).set_viewport(viewport) {
                            cx.notify();
                        }
                    })
                }
            })
    }
}
