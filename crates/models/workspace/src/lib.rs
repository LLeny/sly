use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockLocation {
    Left,
    Center,
    Right,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefreshPolicy {
    OnPause,
    Continuous,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelGroupLayout {
    EqualColumns,
    CompanionRail,
    CompactColumns,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PanelKind {
    Controls,
    Savestates,
    Breakpoints,
    Cartridge,
    Registers { cpu: Arc<str> },
    Disassembly { cpu: Arc<str> },
    CallStack { cpu: Arc<str> },
    Memory,
    Mmio,
    Framebuffer,
    Palette,
    Tileset,
    Tilemap,
    Sprites,
    Raster,
    Structured,
    Table,
    ChannelScopes,
    AudioScope,
    ScriptEditor,
    ScriptConsole,
    ScriptBuiltins,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceIcon {
    Cpu,
    Video,
    Sound,
    Chip,
    Script,
}

#[derive(Clone, Debug)]
pub struct WorkspaceDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub icon: WorkspaceIcon,
    pub mmio_group: Option<Arc<str>>,
}

impl WorkspaceDescriptor {
    pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>, icon: WorkspaceIcon) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon,
            mmio_group: None,
        }
    }

    pub fn mmio_group(mut self, group: impl Into<Arc<str>>) -> Self {
        self.mmio_group = Some(group.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct PanelPlacement {
    pub workspace: Arc<str>,
    pub location: DockLocation,
    pub after: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct PanelDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub kind: PanelKind,
    pub location: DockLocation,
    pub refresh: RefreshPolicy,
    pub workspaces: Vec<Arc<str>>,
    pub selection_from: Option<Arc<str>>,
    pub placements: Vec<PanelPlacement>,
}

impl PanelDescriptor {
    pub fn new(
        id: impl Into<Arc<str>>,
        title: impl Into<Arc<str>>,
        kind: PanelKind,
        location: DockLocation,
        refresh: RefreshPolicy,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind,
            location,
            refresh,
            workspaces: Vec::new(),
            selection_from: None,
            placements: Vec::new(),
        }
    }

    pub fn tag(mut self, workspace: impl Into<Arc<str>>) -> Self {
        self.workspaces.push(workspace.into());
        self
    }

    pub fn selection_from(mut self, table_id: impl Into<Arc<str>>) -> Self {
        self.selection_from = Some(table_id.into());
        self
    }

    /// A panel with no workspace tags is visible everywhere; otherwise it is
    /// visible only in the workspaces it is tagged for.
    pub fn visible_in(&self, workspace: &str) -> bool {
        self.workspaces.is_empty() || self.workspaces.iter().any(|tag| tag.as_ref() == workspace)
    }

    fn placement_in(&self, workspace: &str) -> Option<&PanelPlacement> {
        self.placements
            .iter()
            .find(|placement| placement.workspace.as_ref() == workspace)
    }

    pub fn location_in(&self, workspace: &str) -> DockLocation {
        self.placement_in(workspace)
            .map_or(self.location, |placement| placement.location)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelComposition {
    pub workspace: Arc<str>,
    pub host: Arc<str>,
    pub companion: Arc<str>,
    pub layout: PanelGroupLayout,
}

impl PanelComposition {
    pub fn new(
        workspace: impl Into<Arc<str>>,
        host: impl Into<Arc<str>>,
        companion: impl Into<Arc<str>>,
        layout: PanelGroupLayout,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            host: host.into(),
            companion: companion.into(),
            layout,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    message: String,
}

impl ManifestError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ManifestError {}

#[derive(Clone, Debug)]
pub struct ComponentManifest {
    pub system: Arc<str>,
    pub workspaces: Vec<WorkspaceDescriptor>,
    pub panels: Vec<PanelDescriptor>,
    pub compositions: Vec<PanelComposition>,
}

impl ComponentManifest {
    pub fn in_workspace<'a>(
        &'a self,
        workspace: &'a str,
    ) -> impl Iterator<Item = &'a PanelDescriptor> {
        self.panels.iter().filter(move |p| p.visible_in(workspace))
    }

    pub fn in_location<'a>(
        &'a self,
        workspace: &'a str,
        location: DockLocation,
    ) -> impl Iterator<Item = &'a PanelDescriptor> {
        let mut docked: Vec<&PanelDescriptor> = self
            .in_workspace(workspace)
            .filter(move |p| p.location_in(workspace) == location)
            .collect();

        let mut placed = Vec::new();
        docked.retain(|p| match p.placement_in(workspace) {
            Some(placement) => {
                placed.push((*p, placement.after.clone()));
                false
            }
            None => true,
        });

        let mut tails: std::collections::HashMap<Arc<str>, Arc<str>> =
            std::collections::HashMap::new();
        while !placed.is_empty() {
            let Some(index) = placed
                .iter()
                .position(|(_, anchor)| docked.iter().any(|p| p.id == *anchor))
            else {
                docked.extend(placed.into_iter().map(|(panel, _)| panel));
                break;
            };

            let (panel, anchor) = placed.remove(index);
            let after = tails.get(&anchor).unwrap_or(&anchor);
            let index = docked.iter().position(|p| p.id == *after).unwrap() + 1;
            tails.insert(anchor, panel.id.clone());
            docked.insert(index, panel);
        }

        docked.into_iter()
    }

    pub fn host_of<'a>(&'a self, workspace: &str, id: &str) -> Option<&'a str> {
        self.compositions
            .iter()
            .find(|c| c.workspace.as_ref() == workspace && c.companion.as_ref() == id)
            .map(|c| c.host.as_ref())
    }

    pub fn insert_after(
        &mut self,
        anchor: &str,
        panel: PanelDescriptor,
    ) -> Result<(), ManifestError> {
        let ix = self
            .panels
            .iter()
            .position(|p| p.id.as_ref() == anchor)
            .ok_or_else(|| ManifestError::new(format!("unknown panel anchor `{anchor}`")))?
            + 1;

        self.panels.insert(ix, panel);
        Ok(())
    }

    pub fn compose_in(
        &mut self,
        workspace: impl Into<Arc<str>>,
        host: impl Into<Arc<str>>,
        companion: impl Into<Arc<str>>,
        layout: PanelGroupLayout,
    ) {
        self.compositions
            .push(PanelComposition::new(workspace, host, companion, layout));
    }

    fn panel_mut(&mut self, id: &str) -> Result<&mut PanelDescriptor, ManifestError> {
        self.panels
            .iter_mut()
            .find(|p| p.id.as_ref() == id)
            .ok_or_else(|| ManifestError::new(format!("unknown panel `{id}`")))
    }

    pub fn tag(&mut self, id: &str, workspace: impl Into<Arc<str>>) -> Result<(), ManifestError> {
        let panel = self.panel_mut(id)?;
        let workspace = workspace.into();
        if !panel.workspaces.contains(&workspace) {
            panel.workspaces.push(workspace);
        }

        Ok(())
    }

    pub fn untag(&mut self, id: &str, workspace: &str) -> Result<(), ManifestError> {
        self.panel_mut(id)?
            .workspaces
            .retain(|w| w.as_ref() != workspace);
        self.compositions.retain(|c| {
            c.workspace.as_ref() != workspace
                || (c.host.as_ref() != id && c.companion.as_ref() != id)
        });

        Ok(())
    }

    fn check_anchor(&self, id: &str, anchor: &str) -> Result<(), ManifestError> {
        if id == anchor || !self.panels.iter().any(|p| p.id.as_ref() == anchor) {
            return Err(ManifestError::new(format!(
                "invalid panel anchor `{anchor}`"
            )));
        }

        Ok(())
    }

    pub fn relocate_after(
        &mut self,
        id: &str,
        anchor: &str,
        location: DockLocation,
    ) -> Result<(), ManifestError> {
        self.check_anchor(id, anchor)?;
        let pos = self
            .panels
            .iter()
            .position(|p| &*p.id == id)
            .ok_or_else(|| ManifestError::new(format!("unknown panel `{id}`")))?;

        let mut panel = self.panels.remove(pos);
        panel.location = location;
        self.insert_after(anchor, panel)
    }

    /// Like `relocate_after`, but only in one workspace.
    pub fn place_after(
        &mut self,
        id: &str,
        workspace: impl Into<Arc<str>>,
        anchor: &str,
        location: DockLocation,
    ) -> Result<(), ManifestError> {
        self.check_anchor(id, anchor)?;
        self.panel_mut(id)?.placements.push(PanelPlacement {
            workspace: workspace.into(),
            location,
            after: anchor.into(),
        });

        Ok(())
    }

    pub fn restrict(
        &mut self,
        id: &str,
        workspace: impl Into<Arc<str>>,
    ) -> Result<(), ManifestError> {
        let panel = self.panel_mut(id)?;
        panel.workspaces.clear();
        panel.workspaces.push(workspace.into());
        Ok(())
    }

    pub fn workspace_last(&mut self, id: &str) -> Result<(), ManifestError> {
        let index = self
            .workspaces
            .iter()
            .position(|w| w.id.as_ref() == id)
            .ok_or_else(|| ManifestError::new(format!("unknown workspace `{id}`")))?;

        let workspace = self.workspaces.remove(index);
        self.workspaces.push(workspace);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), ManifestError> {
        self.panel_mut(id)?;
        self.panels.retain(|p| &*p.id != id);
        self.compositions
            .retain(|c| &*c.host != id && &*c.companion != id);
        for panel in &mut self.panels {
            panel.placements.retain(|p| p.after.as_ref() != id);
            if panel.selection_from.as_deref() == Some(id) {
                panel.selection_from = None;
            }
        }

        Ok(())
    }

    pub fn insert_before(
        &mut self,
        anchor: &str,
        panel: PanelDescriptor,
    ) -> Result<(), ManifestError> {
        let ix = self
            .panels
            .iter()
            .position(|p| p.id.as_ref() == anchor)
            .ok_or_else(|| ManifestError::new(format!("unknown panel anchor `{anchor}`")))?;

        self.panels.insert(ix, panel);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut workspace_ids = HashSet::new();
        for workspace in &self.workspaces {
            if !workspace_ids.insert(workspace.id.as_ref()) {
                return Err(ManifestError::new(format!(
                    "duplicate workspace id `{}`",
                    workspace.id
                )));
            }
        }

        let mut panel_ids = HashSet::new();
        for panel in &self.panels {
            if !panel_ids.insert(panel.id.as_ref()) {
                return Err(ManifestError::new(format!(
                    "duplicate panel id `{}`",
                    panel.id
                )));
            }

            for tag in &panel.workspaces {
                if !workspace_ids.contains(tag.as_ref()) {
                    return Err(ManifestError::new(format!(
                        "panel `{}` tagged with undeclared workspace `{tag}`",
                        panel.id
                    )));
                }
            }

            let mut placement_workspaces = HashSet::new();
            for placement in &panel.placements {
                if !placement_workspaces.insert(placement.workspace.as_ref()) {
                    return Err(ManifestError::new(format!(
                        "duplicate placement for `{}`",
                        panel.id
                    )));
                }

                let workspace = placement.workspace.as_ref();
                if !workspace_ids.contains(workspace) {
                    return Err(ManifestError::new(format!(
                        "panel `{}` placed in undeclared workspace `{workspace}`",
                        panel.id
                    )));
                }
                if !panel.visible_in(workspace) {
                    return Err(ManifestError::new(format!(
                        "panel `{}` placed in workspace `{workspace}` it is hidden from",
                        panel.id
                    )));
                }
            }

            if let Some(link) = &panel.selection_from {
                let provider = self.panels.iter().find(|p| p.id == *link).ok_or_else(|| {
                    ManifestError::new(format!(
                        "panel `{}` links to unknown panel `{link}`",
                        panel.id
                    ))
                })?;

                if !matches!(
                    provider.kind,
                    PanelKind::Table
                        | PanelKind::Tileset
                        | PanelKind::Tilemap
                        | PanelKind::Sprites
                        | PanelKind::Raster
                ) {
                    return Err(ManifestError::new(format!(
                        "panel `{link}` cannot provide selection"
                    )));
                }
            }

            for placement in &panel.placements {
                let anchor = self
                    .panels
                    .iter()
                    .find(|p| p.id == placement.after)
                    .ok_or_else(|| {
                        ManifestError::new(format!(
                            "unknown placement anchor `{}`",
                            placement.after
                        ))
                    })?;

                if anchor.id == panel.id
                    || !anchor.visible_in(&placement.workspace)
                    || anchor.location_in(&placement.workspace) != placement.location
                {
                    return Err(ManifestError::new(format!(
                        "invalid placement anchor `{}` for `{}`",
                        anchor.id, panel.id
                    )));
                }

                let mut visited = HashSet::from([panel.id.as_ref()]);
                let mut next = Some(anchor);
                while let Some(current) = next {
                    if !visited.insert(current.id.as_ref()) {
                        return Err(ManifestError::new("cyclic panel placements"));
                    }

                    next = current
                        .placement_in(&placement.workspace)
                        .and_then(|p| self.panels.iter().find(|candidate| candidate.id == p.after));
                }
            }
        }

        let mut hosts = HashSet::new();
        let mut companions = HashSet::new();

        for composition in &self.compositions {
            let workspace = composition.workspace.as_ref();
            let host = composition.host.as_ref();
            let companion = composition.companion.as_ref();

            if !workspace_ids.contains(workspace) {
                return Err(ManifestError::new(format!(
                    "composition references undeclared workspace `{workspace}`"
                )));
            }
            if host == companion {
                return Err(ManifestError::new(format!(
                    "panel `{host}` cannot be its own companion in workspace `{workspace}`"
                )));
            }

            let panel_visible = |id: &str| {
                self.panels
                    .iter()
                    .find(|panel| panel.id.as_ref() == id)
                    .is_some_and(|panel| panel.visible_in(workspace))
            };
            if !panel_visible(host) || !panel_visible(companion) {
                return Err(ManifestError::new(format!(
                    "composition `{host}` + `{companion}` contains a missing or hidden panel in workspace `{workspace}`"
                )));
            }
            if companions.contains(&(workspace, host)) || hosts.contains(&(workspace, companion)) {
                return Err(ManifestError::new(format!(
                    "panel composition nesting or cycle in workspace `{workspace}`"
                )));
            }
            if !hosts.insert((workspace, host)) {
                return Err(ManifestError::new(format!(
                    "panel `{host}` has multiple companions in workspace `{workspace}`"
                )));
            }
            if !companions.insert((workspace, companion)) {
                return Err(ManifestError::new(format!(
                    "panel `{companion}` is claimed by multiple hosts in workspace `{workspace}`"
                )));
            }
        }

        Ok(())
    }
}
