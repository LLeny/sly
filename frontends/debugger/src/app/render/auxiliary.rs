use super::*;

impl Debugger {
    pub(super) fn build_auxiliary_panels(
        &mut self,
        visible: &HashSet<String>,
        t: &Theme,
        panels: &mut HashMap<String, AnyElement>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let this = cx.entity();
        let settings_popup = self.settings.map(|pos| {
            let content = div()
                .flex()
                .flex_col()
                .gap_2()
                .min_w(px(150.))
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("DISASSEMBLY"),
                )
                .child(checkbox(
                    t,
                    "set-follow",
                    self.panes[0].disasm.follow_pc(),
                    "Follow PC",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_follow(cx)),
                ))
                .child(checkbox(
                    t,
                    "set-bytes",
                    self.panes[0].disasm.show_bytes(),
                    "Show bytes",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_bytes(cx)),
                ))
                .child(div().text_xs().text_color(t.text_muted).child("PANELS"))
                .child(checkbox(
                    t,
                    "set-live-refresh",
                    self.live_refresh,
                    "Live refresh while running",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_live_refresh(cx)),
                ))
                .child(div().text_xs().text_color(t.text_muted).child("LOG"))
                .child(checkbox(
                    t,
                    "set-log-show",
                    self.show_log,
                    "Show log panel",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_show_log(cx)),
                ))
                .child(checkbox(
                    t,
                    "set-log-dedup",
                    self.logs.dedup(),
                    "Drop repeated logs",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.log_toggle_dedup(cx)),
                ))
                .child(div().text_xs().text_color(t.text_muted).child("ACCENT"))
                .child({
                    let selected = self.accent;
                    (0..ACCENT_PRESETS.len())
                        .fold(div().flex().items_center().gap_2().py_1(), |row, i| {
                            row.child(super::accent_dot(t, i, i == selected, cx))
                        })
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("SPECIAL GUEST COLORS"),
                )
                .child(GUEST_COLORS.iter().enumerate().fold(
                    div().flex().flex_col().gap(px(3.)),
                    |col, (gi, &(name, _color))| {
                        let index = ACCENT_PRESETS.len() + gi;
                        col.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(super::accent_dot(t, index, index == self.accent, cx))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_secondary)
                                        .child(name.to_string()),
                                ),
                        )
                    },
                ));
            popover(
                t,
                pos,
                content,
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| this.close_settings(cx)),
            )
        });

        let row_provider: RowProvider = {
            let this = this.clone();
            Rc::new(move |range, cx| {
                let debugger = this.read(cx);
                let rows = debugger.memory.cached_rows(range.clone());
                let missing = debugger.memory.missing_pages(range);

                if !missing.is_empty() {
                    let this = this.clone();
                    cx.defer(move |cx| {
                        this.update(cx, |debugger, cx| {
                            for first_row in missing {
                                debugger
                                    .memory
                                    .load_page(first_row, debugger.target.as_ref());
                            }
                            cx.notify();
                        });
                    });
                }

                rows
            })
        };

        let jump_button = field_button(
            t,
            "mem-jump-go",
            icon::CHEVRON_RIGHT,
            cx.listener(|this, _, _, cx| this.jump_submit(cx)),
        );
        let jump_field = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .w(px(150.))
            .child(self.jump_input.clone())
            .child(jump_button);
        let dump_button = field_button(
            t,
            "mem-dump",
            icon::DOWNLOAD,
            cx.listener(|this, _, _, cx| this.memory_dump(cx)),
        );
        let follow_checkbox = checkbox(
            t,
            "mem-follow",
            self.memory.follow_enabled(),
            "Follow access",
            CheckboxSize::Normal,
            cx.listener(|this, _, _, cx| this.memory_toggle_follow(cx)),
        );
        let memory_header = div()
            .flex()
            .items_center()
            .gap_3()
            .child(dump_button)
            .child(follow_checkbox)
            .child(jump_field);
        let memory_widget = MemoryWidget::new(
            row_provider,
            self.memory.row_count(),
            memory::BYTES_PER_ROW,
            self.memory.scroll.clone(),
            self.memory.region_label(),
        )
        .collapsed(self.is_collapsed(MemoryWidget::ID))
        .selection(self.memory.selection())
        .breakpoints(self.memory.breakpoint_ranges())
        .header_extra(memory_header)
        .on_toggle(cx.listener(|this, _, _, cx| this.toggle(MemoryWidget::ID, cx)))
        .on_region_open(
            cx.listener(|this, ev: &ClickEvent, _, cx| this.memory_open_combo(ev.position(), cx)),
        )
        .on_byte_down({
            let this = this.clone();
            move |addr, shift, _, cx| this.update(cx, |d, cx| d.memory_byte_down(addr, shift, cx))
        })
        .on_byte_drag({
            let this = this.clone();
            move |addr, _, cx| this.update(cx, |d, cx| d.memory_byte_drag(addr, cx))
        })
        .on_context({
            let this = this.clone();
            move |addr, pos, _, cx| this.update(cx, |d, cx| d.memory_open_menu(addr, pos, cx))
        });

        let memory_combo = self.memory.combo_open.map(|pos| {
            let options = self.memory.options();
            combobox_menu(
                t,
                pos,
                &options,
                self.memory.region_id(),
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.memory_set_region(id, cx))
                },
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.memory_close_combo(cx)
                }),
            )
        });

        let bp_combo = self.breakpoints.combo_open.map(|pos| {
            let options = self.breakpoints.options();
            combobox_menu(
                t,
                pos,
                &options,
                self.breakpoints.draft_kind().id(),
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.bp_set_kind(id, cx))
                },
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| this.bp_close_combo(cx)),
            )
        });

        let memory_menu = self.memory.menu.as_ref().map(|menu| {
            let (bp_start, bp_end) = menu.range;
            let bp_label: SharedString = if bp_end > bp_start {
                format!("${}...{:04X}", menu.address, bp_end & 0xFFFF).into()
            } else {
                format!("${}", menu.address).into()
            };
            let items = vec![
                MenuItem::new(
                    "Copy Address",
                    super::clipboard_listener(
                        cx,
                        menu.address.clone(),
                        Debugger::memory_close_menu,
                    ),
                ),
                MenuItem::new(
                    "Copy Hex",
                    super::clipboard_listener(cx, menu.hex.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy Raw",
                    super::clipboard_listener(cx, menu.raw.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy ASCII",
                    super::clipboard_listener(cx, menu.ascii.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Dump to File",
                    cx.listener(|this, _, _, cx| {
                        this.memory_dump(cx);
                        this.memory_close_menu(cx);
                    }),
                ),
                MenuItem::new("Add read breakpoint", {
                    let label = bp_label.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.bp_add_mem(bp_start, bp_end, BreakpointKind::Read, label.clone(), cx)
                    })
                }),
                MenuItem::new("Add write breakpoint", {
                    let label = bp_label.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.bp_add_mem(bp_start, bp_end, BreakpointKind::Write, label.clone(), cx)
                    })
                }),
            ];
            context_menu(
                t,
                menu.position,
                items,
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.memory_close_menu(cx)
                }),
            )
        });

        let log_region = self.show_log.then(|| {
            let log_collapsed = self.is_collapsed(LogWidget::ID);
            let log = LogWidget::new(self.logs.view(), self.logs.scroll.clone())
                .collapsed(log_collapsed)
                .target_label(self.logs.target_label())
                .level_label(self.logs.level_label())
                .message_input(self.log_filter_input.clone())
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(LogWidget::ID, cx)))
                .on_target_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                    this.log_open_target(ev.position(), cx)
                }))
                .on_level_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                    this.log_open_level(ev.position(), cx)
                }))
                .on_clear(cx.listener(|this, _, _, cx| this.clear_logs(cx)));
            div()
                .flex()
                .flex_col()
                .when(!log_collapsed, |d| d.h(px(180.)).min_h(px(0.)))
                .child(log)
        });

        let log_target_combo = self.logs.target_combo.map(|pos| {
            let options = self.logs.target_options();
            let selected = self.logs.target_id();
            combobox_menu(
                t,
                pos,
                &options,
                &selected,
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.log_set_target(id, cx))
                },
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| this.log_close_target(cx)),
            )
        });

        let log_level_combo = self.logs.level_combo.map(|pos| {
            let options = self.logs.level_options();
            let selected = self.logs.level_id();
            combobox_menu(
                t,
                pos,
                &options,
                &selected,
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.log_set_level(id, cx))
                },
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| this.log_close_level(cx)),
            )
        });

        if visible.contains(MemoryWidget::ID) {
            panels.insert(MemoryWidget::ID.into(), memory_widget.into_any_element());
        }
        if let Some(log_region) = log_region {
            panels.insert(LogWidget::ID.into(), log_region.into_any_element());
        }

        let mut overlays = Vec::new();
        if let Some(element) = settings_popup {
            overlays.push(element.into_any_element());
        }
        if let Some(element) = bp_combo {
            overlays.push(element.into_any_element());
        }
        if let Some(element) = memory_combo {
            overlays.push(element.into_any_element());
        }
        if let Some(element) = memory_menu {
            overlays.push(element.into_any_element());
        }
        if let Some(element) = log_target_combo {
            overlays.push(element.into_any_element());
        }
        if let Some(element) = log_level_combo {
            overlays.push(element.into_any_element());
        }
        overlays
    }
}
