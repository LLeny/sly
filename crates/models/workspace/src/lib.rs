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
    Breakpoints,
    Cartridge,
    Registers { cpu: Arc<str> },
    Disassembly { cpu: Arc<str> },
    CallStack { cpu: Arc<str> },
    Memory,
    Mmio,
    Dma,
    Framebuffer,
    Palette,
    Ppu,
    Tileset,
    Tilemap,
    Sprites,
    Structured,
    Table,
    ChannelScopes,
    AudioScope,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceIcon {
    Cpu,
    Video,
    Sound,
}

#[derive(Clone, Debug)]
pub struct WorkspaceDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub icon: WorkspaceIcon,
}

impl WorkspaceDescriptor {
    pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>, icon: WorkspaceIcon) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon,
        }
    }
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
        self.in_workspace(workspace)
            .filter(move |p| p.location == location)
    }

    pub fn host_of<'a>(&'a self, workspace: &str, id: &str) -> Option<&'a str> {
        self.compositions
            .iter()
            .find(|c| c.workspace.as_ref() == workspace && c.companion.as_ref() == id)
            .map(|c| c.host.as_ref())
    }

    pub fn insert_after(&mut self, anchor: PanelKind, panel: PanelDescriptor) {
        let ix = self
            .panels
            .iter()
            .position(|p| p.kind == anchor)
            .map_or(self.panels.len(), |ix| ix + 1);
        self.panels.insert(ix, panel);
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

    /// Tag the panel with the given id for a workspace, if not already tagged.
    pub fn tag(&mut self, id: &str, workspace: impl Into<Arc<str>>) {
        if let Some(panel) = self.panels.iter_mut().find(|p| &*p.id == id) {
            let workspace = workspace.into();
            if !panel.workspaces.contains(&workspace) {
                panel.workspaces.push(workspace);
            }
        }
    }

    /// Move the panel with the given id to `location`, re-inserting it right
    /// after the first panel of `anchor` kind (or at the end if absent).
    pub fn relocate_after(&mut self, id: &str, anchor: PanelKind, location: DockLocation) {
        let Some(pos) = self.panels.iter().position(|p| &*p.id == id) else {
            return;
        };

        let mut panel = self.panels.remove(pos);
        panel.location = location;
        self.insert_after(anchor, panel);
    }

    pub fn restrict(&mut self, kind: PanelKind, workspace: impl Into<Arc<str>>) {
        if let Some(panel) = self.panels.iter_mut().find(|p| p.kind == kind) {
            debug_assert!(
                panel.workspaces.is_empty(),
                "restrict() on already-tagged panel `{}`",
                panel.id,
            );
            panel.workspaces.push(workspace.into());
        }
    }

    pub fn insert_before(&mut self, anchor: PanelKind, panel: PanelDescriptor) {
        let ix = self
            .panels
            .iter()
            .position(|p| p.kind == anchor)
            .unwrap_or(self.panels.len());
        self.panels.insert(ix, panel);
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

            if let Some(link) = &panel.selection_from
                && !self.panels.iter().any(|p| p.id == *link)
            {
                return Err(ManifestError::new(format!(
                    "panel `{}` links to unknown panel `{link}`",
                    panel.id
                )));
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
