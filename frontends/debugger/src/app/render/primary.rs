use super::*;

impl Debugger {
    pub(super) fn build_primary_panels(
        &mut self,
        visible: &HashSet<String>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> (HashMap<String, AnyElement>, Vec<AnyElement>) {
        let this = cx.entity();
        let framebuffer = visible
            .contains(FramebufferWidget::ID)
            .then(|| {
                FramebufferWidget::new(self.framebuffer.clone())
                    .collapsed(self.is_collapsed(FramebufferWidget::ID))
                    .on_toggle(cx.listener(|this, _, _, cx| this.toggle(FramebufferWidget::ID, cx)))
            })
            .map(|widget| widget.into_any_element());

        let cartridge = visible.contains(CartridgeWidget::ID).then(|| {
            CartridgeWidget::new(self.cartridge.clone())
                .collapsed(self.is_collapsed(CartridgeWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(CartridgeWidget::ID, cx)))
        });

        let controls_view = ControlsView {
            running: self.execution.is_running(),
        };
        let controls = ControlsWidget::new(controls_view)
            .collapsed(self.is_collapsed(ControlsWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(ControlsWidget::ID, cx)))
            .on_run(cx.listener(|this, _, _, cx| this.set_running(true, cx)))
            .on_pause(cx.listener(|this, _, _, cx| this.set_running(false, cx)))
            .on_step(cx.listener(|this, _, _, cx| this.step(cx)))
            .on_reset(cx.listener(|this, _, _, cx| this.reset(cx)));

        let mmio = MmioWidget::new(self.mmio_view.clone())
            .expanded(self.expanded_set(MmioWidget::ID))
            .collapsed(self.is_collapsed(MmioWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(MmioWidget::ID, cx)))
            .on_toggle_register({
                let this = this.clone();
                move |id, _, cx| this.update(cx, |d, cx| d.toggle_expanded(MmioWidget::ID, id, cx))
            });

        let dma = visible.contains(DmaWidget::ID).then(|| {
            DmaWidget::new(self.dma_view.clone())
                .expanded(self.expanded_set(DmaWidget::ID))
                .collapsed(self.is_collapsed(DmaWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(DmaWidget::ID, cx)))
                .on_toggle_channel({
                    let this = this.clone();
                    move |id, _, cx| {
                        this.update(cx, |d, cx| d.toggle_expanded(DmaWidget::ID, id, cx))
                    }
                })
        });

        let palette = visible.contains(PaletteWidget::ID).then(|| {
            PaletteWidget::new(self.palette_view.clone())
                .selected(self.palette_selected)
                .collapsed(self.is_collapsed(PaletteWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(PaletteWidget::ID, cx)))
                .on_select({
                    let this = this.clone();
                    move |palette, color, _, cx| {
                        this.update(cx, |d, cx| d.select_palette_color(palette, color, cx))
                    }
                })
        });

        let ppu = visible.contains(PpuWidget::ID).then(|| {
            PpuWidget::new(self.ppu_view.clone())
                .expanded(self.expanded_set(PpuWidget::ID))
                .collapsed(self.is_collapsed(PpuWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(PpuWidget::ID, cx)))
                .on_toggle_group({
                    let this = this.clone();
                    move |id, _, cx| {
                        this.update(cx, |d, cx| d.toggle_expanded(PpuWidget::ID, id, cx))
                    }
                })
        });

        let tileset = visible.contains(TilesetWidget::ID).then(|| {
            self.wire_grid(
                TilesetWidget::panel(self.tileset.clone()).layers(self.tileset_layers.clone()),
                TilesetWidget::ID,
                &self.tileset_ui,
                |d| &mut d.tileset_ui,
                Self::refresh_tileset,
                cx,
            )
        });

        let tilemap = visible.contains(TilemapWidget::ID).then(|| {
            self.wire_grid(
                TilemapWidget::panel(self.tilemap.clone()).layers(self.tileset_layers.clone()),
                TilemapWidget::ID,
                &self.tilemap_ui,
                |d| &mut d.tilemap_ui,
                Self::refresh_tilemap,
                cx,
            )
        });

        let sprites = visible.contains(SpritesWidget::ID).then(|| {
            self.wire_grid(
                SpritesWidget::panel(self.sprites.clone()),
                SpritesWidget::ID,
                &self.sprites_ui,
                |d| &mut d.sprites_ui,
                Self::refresh_sprites,
                cx,
            )
        });

        let breakpoints =
            visible.contains(BreakpointsWidget::ID).then(|| {
                BreakpointsWidget::new(self.breakpoints.view(), self.breakpoints.draft_kind())
                    .collapsed(self.is_collapsed(BreakpointsWidget::ID))
                    .scrollable(true)
                    .address_input(self.bp_input.clone())
                    .on_toggle(cx.listener(|this, _, _, cx| this.toggle(BreakpointsWidget::ID, cx)))
                    .on_add(cx.listener(|this, _, _, cx| this.bp_add(cx)))
                    .on_kind_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.bp_open_combo(ev.position(), cx)
                    }))
                    .on_remove({
                        let this = this.clone();
                        move |id, _, cx| this.update(cx, |d, cx| d.bp_remove(id, cx))
                    })
                    .on_toggle_entry({
                        let this = this.clone();
                        move |id, _, cx| this.update(cx, |d, cx| d.bp_toggle(id, cx))
                    })
            });

        let mut cpu_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut call_stack_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_menus = Vec::new();

        for (pane_ix, pane) in self.panes.iter().enumerate() {
            if visible.contains(pane.id.as_str()) {
                let cpu_id = SharedString::from(pane.id.arc());
                let cpu_widget = CpuWidget::new(pane.cpu_view.clone())
                    .id(cpu_id.clone())
                    .title(pane.title.clone())
                    .collapsed(self.is_collapsed(&cpu_id))
                    .on_toggle({
                        let id = cpu_id.clone();
                        cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                    });
                cpu_panels.push((
                    pane.id.as_str().to_string(),
                    div().flex_none().child(cpu_widget).into_any_element(),
                ));
            }

            let disasm_panel_id = format!("{}.disasm", pane.id.as_str());
            if !visible.contains(disasm_panel_id.as_str()) {
                continue;
            }

            let disasm_id = SharedString::from(disasm_panel_id.clone());
            let disasm_title = SharedString::from(format!("{} Disassembly", pane.title));
            let disassembly =
                DisassemblyWidget::new(pane.disasm.view.clone(), pane.disasm.scroll.clone())
                    .id(disasm_id.clone())
                    .title(disasm_title)
                    .current(pane.disasm.current_ix())
                    .current_note(pane.disasm.current_note())
                    .selected(pane.disasm.selected_ix())
                    .breakpoints(pane.disasm.breakpoint_rows())
                    .layout_cache(pane.disasm.layout_cache.clone())
                    .collapsed(self.is_collapsed(&disasm_id))
                    .on_toggle({
                        let id = disasm_id.clone();
                        cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                    })
                    .on_select({
                        let this = this.clone();
                        move |ix, _, cx| this.update(cx, |d, cx| d.disasm_select(pane_ix, ix, cx))
                    })
                    .on_context({
                        let this = this.clone();
                        move |ix, pos, _, cx| {
                            this.update(cx, |d, cx| d.disasm_open_menu(pane_ix, ix, pos, cx))
                        }
                    })
                    .view_mode(pane.disasm.view_mode())
                    .graph_offset(pane.disasm.graph_offset())
                    .on_set_view({
                        let this = this.clone();
                        move |mode, _, cx| {
                            this.update(cx, |d, cx| d.disasm_set_view(pane_ix, mode, cx))
                        }
                    })
                    .on_pan_begin({
                        let this = this.clone();
                        move |pos, _, cx| this.update(cx, |d, _cx| d.disasm_pan_begin(pane_ix, pos))
                    })
                    .on_pan_drag({
                        let this = this.clone();
                        move |pos, _, cx| {
                            this.update(cx, |d, cx| d.disasm_pan_drag(pane_ix, pos, cx))
                        }
                    })
                    .follow(pane.disasm.follow_pc())
                    .graph_viewport(pane.disasm.graph_viewport())
                    .on_viewport({
                        let this = this.clone();
                        move |viewport, _, cx| {
                            this.update(cx, |d, cx| d.disasm_set_viewport(pane_ix, viewport, cx))
                        }
                    })
                    .hovered_edge(pane.disasm.hovered_edge())
                    .on_edge_hover({
                        let this = this.clone();
                        move |edge, _, cx| {
                            this.update(cx, |d, cx| d.disasm_hover_edge(pane_ix, edge, cx))
                        }
                    })
                    .on_jump({
                        let this = this.clone();
                        move |from, to, _, cx| {
                            this.update(cx, |d, cx| d.disasm_jump(pane_ix, from, to, cx))
                        }
                    });

            let cs_id = SharedString::from(format!("{}.callstack", pane.id.as_str()));
            let cs_title = SharedString::from(format!("{} Call Stack", pane.title));
            let callstack = CallStackWidget::new(pane.call_stack.clone())
                .id(cs_id.clone())
                .title(cs_title)
                .scrollable(true)
                .collapsed(self.is_collapsed(&cs_id))
                .on_toggle({
                    let id = cs_id.clone();
                    cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                });

            disasm_panels.push((
                disasm_panel_id,
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .child(disassembly)
                    .into_any_element(),
            ));
            call_stack_panels.push((cs_id.to_string(), callstack.into_any_element()));

            if let Some(menu) = pane.disasm.menu.as_ref() {
                let addr = menu.addr;
                let len = menu.len;

                let mut items = vec![
                    MenuItem::new(
                        "Copy Address",
                        super::clipboard_listener(cx, menu.address.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Bytes",
                        super::clipboard_listener(cx, menu.bytes.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Disassembly",
                        super::clipboard_listener(cx, menu.text.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Set PC to here",
                        cx.listener(move |this, _, _, cx| this.set_pc_to(pane_ix, addr, cx)),
                    ),
                    MenuItem::new(
                        "Add execute breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Execute, cx)
                        }),
                    ),
                ];

                if pane_ix == 0 {
                    items.push(MenuItem::new(
                        "Add read breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Read, cx)
                        }),
                    ));
                    items.push(MenuItem::new(
                        "Add write breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Write, cx)
                        }),
                    ));
                }

                disasm_menus.push(context_menu(
                    t,
                    menu.position,
                    items,
                    cx.listener(|this, _event: &MouseDownEvent, _window, cx| this.close_menu(cx)),
                ));
            }
        }

        let mut panels = HashMap::new();
        panels.insert(
            ControlsWidget::ID.into(),
            div().flex_none().child(controls).into_any_element(),
        );
        if let Some(breakpoints) = breakpoints {
            panels.insert(BreakpointsWidget::ID.into(), breakpoints.into_any_element());
        }
        if let Some(dma) = dma {
            panels.insert(DmaWidget::ID.into(), dma.into_any_element());
        }
        if let Some(palette) = palette {
            panels.insert(PaletteWidget::ID.into(), palette.into_any_element());
        }
        if let Some(ppu) = ppu {
            panels.insert(PpuWidget::ID.into(), ppu.into_any_element());
        }
        if let Some(tileset) = tileset {
            panels.insert(TilesetWidget::ID.into(), tileset.into_any_element());
        }
        if let Some(tilemap) = tilemap {
            panels.insert(TilemapWidget::ID.into(), tilemap.into_any_element());
        }
        if let Some(cartridge) = cartridge {
            panels.insert(
                CartridgeWidget::ID.into(),
                div().flex_none().child(cartridge).into_any_element(),
            );
        }
        panels.insert(MmioWidget::ID.into(), mmio.into_any_element());
        if let Some(sprites) = sprites {
            panels.insert(SpritesWidget::ID.into(), sprites.into_any_element());
        }
        if let Some(screen) = framebuffer {
            panels.insert(FramebufferWidget::ID.into(), screen);
        }
        panels.extend(cpu_panels);
        panels.extend(disasm_panels);
        panels.extend(call_stack_panels);

        let overlays = disasm_menus
            .into_iter()
            .map(IntoElement::into_any_element)
            .collect();
        (panels, overlays)
    }
}
