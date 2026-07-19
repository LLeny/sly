use super::*;

impl Debugger {
    pub(super) fn insert_generic_panels(
        &mut self,
        active_workspace: &str,
        visible: &HashSet<String>,
        panels: &mut HashMap<String, AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity();
        let manifest = self.registry.manifest();
        let panel_glyph: Option<SharedString> = manifest
            .workspaces
            .iter()
            .find(|w| w.id.as_ref() == active_workspace)
            .map(|w| SharedString::from(super::workspace_icon(w.icon)));
        for panel in manifest.in_workspace(active_workspace) {
            let id = panel.id.as_ref();
            if !visible.contains(id)
                || !matches!(
                    panel.kind,
                    PanelKind::Table
                        | PanelKind::Structured
                        | PanelKind::ChannelScopes
                        | PanelKind::AudioScope
                )
            {
                continue;
            }
            let pid = SharedString::from(panel.id.clone());
            let title = SharedString::from(panel.title.clone());
            let on_toggle = {
                let id = pid.clone();
                cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
            };
            let chrome = PanelChrome::new(pid.clone(), title)
                .collapsed(self.is_collapsed(id))
                .on_toggle(on_toggle)
                .glyph_opt(panel_glyph.clone());
            let element = match panel.kind {
                PanelKind::Table => {
                    let view = self.table_views.get(id).cloned().unwrap_or_default();
                    TablePanel::new(chrome, view)
                        .selected(self.table_selected(id))
                        .on_select({
                            let this = this.clone();
                            let id = pid.clone();
                            move |row, _, cx| {
                                let id = id.clone();
                                this.update(cx, |d, cx| d.select_table_row(id, row, cx))
                            }
                        })
                        .into_any_element()
                }
                PanelKind::Structured => {
                    let view = self.structured_views.get(id).cloned().unwrap_or_default();
                    StructuredPanel::new(chrome, view).into_any_element()
                }
                PanelKind::ChannelScopes => {
                    let view = self.scopes_views.get(id).cloned().unwrap_or_default();
                    ChannelScopesWidget::new(chrome, view).into_any_element()
                }
                PanelKind::AudioScope => {
                    AudioScopeWidget::new(chrome, self.scope_view.clone()).into_any_element()
                }
                _ => unreachable!(),
            };
            panels.insert(id.to_string(), element);
        }
    }
}
