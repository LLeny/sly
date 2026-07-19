use super::*;

impl Debugger {
    pub(super) fn set_pc_to(&mut self, pane: usize, addr: u32, cx: &mut Context<Self>) {
        let id = self.panes[pane].id.clone();
        self.panes[pane].disasm.close_menu();
        self.target.set_pc(&id, addr);
        Self::refresh_pane_view(
            self.target.as_ref(),
            &mut self.panes[pane],
            CpuRefresh::Highlight,
        );
        self.set_state("pc set");
        cx.notify();
    }

    pub(super) fn disasm_select(&mut self, pane: usize, ix: usize, cx: &mut Context<Self>) {
        self.panes[pane].disasm.select(ix);
        cx.notify();
    }

    pub(super) fn disasm_open_menu(
        &mut self,
        pane: usize,
        ix: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.panes[pane].disasm.open_menu(ix, position);
        cx.notify();
    }

    pub(super) fn disasm_set_view(&mut self, pane: usize, mode: ViewMode, cx: &mut Context<Self>) {
        self.panes[pane]
            .disasm
            .set_view_mode(mode, self.target.as_ref());
        cx.notify();
    }

    pub(super) fn disasm_pan_begin(&mut self, pane: usize, pos: Point<Pixels>) {
        self.panes[pane].disasm.pan_begin(pos);
    }

    pub(super) fn disasm_pan_drag(
        &mut self,
        pane: usize,
        pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.panes[pane].disasm.pan_drag(pos);
        cx.notify();
    }

    pub(super) fn disasm_set_viewport(
        &mut self,
        pane: usize,
        viewport: Size<f32>,
        cx: &mut Context<Self>,
    ) {
        if self.panes[pane].disasm.set_graph_viewport(viewport) {
            cx.notify();
        }
    }

    pub(super) fn disasm_hover_edge(
        &mut self,
        pane: usize,
        edge: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if self.panes[pane].disasm.set_hovered_edge(edge) {
            cx.notify();
        }
    }

    pub(super) fn disasm_jump(
        &mut self,
        pane: usize,
        from: Point<Pixels>,
        to: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.panes[pane].disasm.jump(from, to);
        cx.notify();
    }

    pub(super) fn close_menu(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for pane in &mut self.panes {
            changed |= pane.disasm.close_menu();
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn memory_set_region(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.memory.set_region(&id);
        self.memory.load_page(0, self.target.as_ref());
        cx.notify();
    }

    pub(super) fn memory_open_combo(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.memory.open_combo(position);
        cx.notify();
    }

    pub(super) fn memory_close_combo(&mut self, cx: &mut Context<Self>) {
        if self.memory.close_combo() {
            cx.notify();
        }
    }

    pub(super) fn memory_byte_down(&mut self, addr: u32, shift: bool, cx: &mut Context<Self>) {
        self.memory.begin(addr, shift);
        cx.notify();
    }

    pub(super) fn memory_byte_drag(&mut self, addr: u32, cx: &mut Context<Self>) {
        if self.memory.drag(addr) {
            cx.notify();
        }
    }

    pub(super) fn memory_open_menu(
        &mut self,
        addr: u32,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.memory.open_menu(addr, position, self.target.as_ref());
        cx.notify();
    }

    pub(super) fn memory_close_menu(&mut self, cx: &mut Context<Self>) {
        if self.memory.close_menu() {
            cx.notify();
        }
    }

    pub(super) fn memory_dump(&mut self, cx: &mut Context<Self>) {
        if let Some((name, bytes)) = self.memory.dump(self.target.as_ref()) {
            crate::save::save_bin(name, bytes, cx);
        }
    }

    pub(super) fn memory_toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.memory.toggle_follow();
        self.follow_memory();
        cx.notify();
    }

    pub(super) fn follow_memory(&mut self) {
        if self.memory.follow_enabled()
            && let Some((region, addr)) = self.target.latest_access()
        {
            self.memory.follow_to(&region, addr);
        }
    }

    pub(super) fn on_jump_event(
        &mut self,
        _input: Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if let TextInputEvent::Submit = event {
            self.jump_submit(cx);
        }
    }

    pub(super) fn jump_submit(&mut self, cx: &mut Context<Self>) {
        let text = self.jump_input.read(cx).content().trim().to_string();
        if let Ok(addr) = u32::from_str_radix(&text, 16) {
            self.memory.jump_to(addr);
        }
        self.jump_input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    pub(super) fn on_bp_event(
        &mut self,
        _input: Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if let TextInputEvent::Submit = event {
            self.bp_add(cx);
        }
    }

    pub(super) fn bp_add(&mut self, cx: &mut Context<Self>) {
        let text = self.bp_input.read(cx).content().trim().to_string();
        if let Ok(addr) = u32::from_str_radix(&text, 16) {
            let (cpu, region) = self.pane_space(0);
            self.breakpoints
                .add(addr, cpu, region, self.target.as_ref());
            self.bp_input.update(cx, |input, cx| input.clear(cx));
        }
        self.sync_bp_markers();
        cx.notify();
    }

    pub(super) fn bp_add_at(
        &mut self,
        pane: usize,
        addr: u32,
        len: u32,
        kind: BreakpointKind,
        cx: &mut Context<Self>,
    ) {
        let (cpu, region) = self.pane_space(pane);
        self.panes[pane].disasm.close_menu();
        let end = addr.saturating_add(len.saturating_sub(1));
        self.breakpoints
            .add_kind(addr, end, kind, cpu, region, self.target.as_ref());
        self.sync_bp_markers();
        cx.notify();
    }

    pub(super) fn bp_add_mem(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        label: SharedString,
        cx: &mut Context<Self>,
    ) {
        self.memory.close_menu();
        let cpu = self.panes[0].id.clone();
        let region: Arc<str> = self.memory.region_id().into();
        let note = self.target.annotate(start).into();
        self.breakpoints
            .add_labeled(start, end, kind, cpu, region, label, note);
        self.sync_bp_markers();
        cx.notify();
    }

    /// A processor's id and address space, used to tag breakpoints raised from
    /// that pane (or pane 0 for manually-typed and memory-panel breakpoints).
    pub(super) fn pane_space(&self, pane: usize) -> (CpuId, Arc<str>) {
        (self.panes[pane].id.clone(), self.panes[pane].space.clone())
    }

    pub(super) fn bp_remove(&mut self, id: usize, cx: &mut Context<Self>) {
        self.breakpoints.remove(id);
        self.sync_bp_markers();
        cx.notify();
    }

    pub(super) fn bp_toggle(&mut self, id: usize, cx: &mut Context<Self>) {
        self.breakpoints.toggle(id);
        self.sync_bp_markers();
        cx.notify();
    }

    pub(super) fn sync_bp_markers(&mut self) {
        let marks = self.breakpoints.marks();
        for pane in &mut self.panes {
            pane.disasm.set_breakpoints(marks);
        }
        let primary = &self.panes[0].id;
        let mem_marks = marks
            .iter()
            .filter(|m| &m.cpu == primary)
            .cloned()
            .collect();
        self.memory.set_breakpoints(mem_marks);
    }

    pub(super) fn bp_set_kind(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if let Some(kind) = BreakpointKind::from_id(&id) {
            self.breakpoints.set_draft_kind(kind);
        }
        cx.notify();
    }

    pub(super) fn bp_open_combo(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.breakpoints.open_combo(position);
        cx.notify();
    }

    pub(super) fn bp_close_combo(&mut self, cx: &mut Context<Self>) {
        if self.breakpoints.close_combo() {
            cx.notify();
        }
    }

    pub(super) fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        for pane in &mut self.panes {
            pane.disasm.toggle_follow(self.target.as_ref());
        }
        cx.notify();
    }

    pub(super) fn toggle_bytes(&mut self, cx: &mut Context<Self>) {
        for pane in &mut self.panes {
            pane.disasm.toggle_bytes();
        }
        cx.notify();
    }

    pub(super) fn open_settings(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.settings = Some(position);
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn clear_logs(&mut self, cx: &mut Context<Self>) {
        self.logs.clear();
        cx.notify();
    }

    pub(super) fn log_toggle_dedup(&mut self, cx: &mut Context<Self>) {
        self.logs.toggle_dedup();
        cx.notify();
    }

    pub(super) fn toggle_show_log(&mut self, cx: &mut Context<Self>) {
        self.show_log = !self.show_log;
        cx.notify();
    }

    pub(super) fn toggle_live_refresh(&mut self, cx: &mut Context<Self>) {
        self.live_refresh = !self.live_refresh;
        cx.notify();
    }

    pub(super) fn on_log_filter_event(
        &mut self,
        input: Entity<TextInput>,
        _event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        let text = input.read(cx).content().to_string();
        self.logs.set_message(text);
        cx.notify();
    }

    pub(super) fn log_open_target(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.logs.open_target_combo(position);
        cx.notify();
    }

    pub(super) fn log_close_target(&mut self, cx: &mut Context<Self>) {
        if self.logs.close_target_combo() {
            cx.notify();
        }
    }

    pub(super) fn log_set_target(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.logs.set_target(&id);
        cx.notify();
    }

    pub(super) fn log_open_level(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.logs.open_level_combo(position);
        cx.notify();
    }

    pub(super) fn log_close_level(&mut self, cx: &mut Context<Self>) {
        if self.logs.close_level_combo() {
            cx.notify();
        }
    }

    pub(super) fn log_set_level(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.logs.set_level(&id);
        cx.notify();
    }
}
