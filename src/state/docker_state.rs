use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::ColimaVm;
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};

use super::app_state::{CurrentView, MachineTabState, SelectedItem};

use crate::docker::VolumeFileEntry;

/// Image inspect data for detailed view
#[derive(Clone, Debug, Default)]
pub struct ImageInspectData {
    pub config_cmd: Option<Vec<String>>,
    pub config_workdir: Option<String>,
    pub config_env: Vec<(String, String)>,
    pub config_entrypoint: Option<Vec<String>>,
    pub config_exposed_ports: Vec<String>,
    pub used_by: Vec<String>,
}

/// Event emitted when docker state changes
#[derive(Clone, Debug)]
pub enum StateChanged {
    MachinesUpdated,
    ContainersUpdated,
    ImagesUpdated,
    VolumesUpdated,
    NetworksUpdated,
    SelectionChanged,
    ViewChanged,
    Loading(bool),
    VolumeFilesLoaded {
        volume_name: String,
        path: String,
        files: Vec<VolumeFileEntry>,
    },
    VolumeFilesError {
        volume_name: String,
        error: String,
    },
    ImageInspectLoaded {
        image_id: String,
        data: ImageInspectData,
    },
}

/// Global docker state - all views subscribe to this
pub struct DockerState {
    // Data
    pub colima_vms: Vec<ColimaVm>,
    pub containers: Vec<ContainerInfo>,
    pub images: Vec<ImageInfo>,
    pub volumes: Vec<VolumeInfo>,
    pub networks: Vec<NetworkInfo>,

    // UI state
    pub current_view: CurrentView,
    pub selected_item: Option<SelectedItem>,
    pub active_detail_tab: usize,
    pub machine_tab_state: MachineTabState,

    // Loading states
    pub is_loading: bool,
}

impl DockerState {
    pub fn new() -> Self {
        Self {
            colima_vms: Vec::new(),
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
            current_view: CurrentView::default(),
            selected_item: None,
            active_detail_tab: 0,
            machine_tab_state: MachineTabState::new(),
            is_loading: true,
        }
    }

    // Machines
    pub fn set_machines(&mut self, vms: Vec<ColimaVm>) {
        self.colima_vms = vms;
    }

    pub fn get_machine(&self, name: &str) -> Option<&ColimaVm> {
        self.colima_vms.iter().find(|vm| vm.name == name)
    }

    // Containers
    pub fn set_containers(&mut self, containers: Vec<ContainerInfo>) {
        self.containers = containers;
    }

    pub fn get_container(&self, id: &str) -> Option<&ContainerInfo> {
        self.containers.iter().find(|c| c.id == id)
    }

    // Images
    pub fn set_images(&mut self, images: Vec<ImageInfo>) {
        self.images = images;
    }

    pub fn get_image(&self, id: &str) -> Option<&ImageInfo> {
        self.images.iter().find(|i| i.id == id)
    }

    // Volumes
    pub fn set_volumes(&mut self, volumes: Vec<VolumeInfo>) {
        self.volumes = volumes;
    }

    pub fn get_volume(&self, name: &str) -> Option<&VolumeInfo> {
        self.volumes.iter().find(|v| v.name == name)
    }

    // Networks
    pub fn set_networks(&mut self, networks: Vec<NetworkInfo>) {
        self.networks = networks;
    }

    pub fn get_network(&self, id: &str) -> Option<&NetworkInfo> {
        self.networks.iter().find(|n| n.id == id)
    }

    // Selection
    pub fn select_machine(&mut self, name: &str) {
        if let Some(vm) = self.get_machine(name).cloned() {
            self.selected_item = Some(SelectedItem::Machine(vm));
            self.active_detail_tab = 0;
            self.machine_tab_state = MachineTabState::new();
        }
    }

    pub fn select_container(&mut self, id: &str) {
        if let Some(container) = self.get_container(id).cloned() {
            self.selected_item = Some(SelectedItem::Container(container));
            self.active_detail_tab = 0;
        }
    }

    pub fn select_image(&mut self, id: &str) {
        if let Some(image) = self.get_image(id).cloned() {
            self.selected_item = Some(SelectedItem::Image(image));
            self.active_detail_tab = 0;
        }
    }

    pub fn select_volume(&mut self, name: &str) {
        if let Some(volume) = self.get_volume(name).cloned() {
            self.selected_item = Some(SelectedItem::Volume(volume));
            self.active_detail_tab = 0;
        }
    }

    pub fn select_network(&mut self, id: &str) {
        if let Some(network) = self.get_network(id).cloned() {
            self.selected_item = Some(SelectedItem::Network(network));
            self.active_detail_tab = 0;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_item = None;
        self.active_detail_tab = 0;
    }

    // Navigation
    pub fn set_view(&mut self, view: CurrentView) {
        self.current_view = view;
        self.selected_item = None;
        self.active_detail_tab = 0;
    }

    pub fn set_active_tab(&mut self, tab: usize) {
        self.active_detail_tab = tab;
    }
}

impl Default for DockerState {
    fn default() -> Self {
        Self::new()
    }
}

// Enable event emission for reactive updates
impl EventEmitter<StateChanged> for DockerState {}

/// Global wrapper for DockerState
pub struct GlobalDockerState(pub Entity<DockerState>);

impl Global for GlobalDockerState {}

/// Initialize the global docker state
pub fn init_docker_state(cx: &mut App) -> Entity<DockerState> {
    let state = cx.new(|_cx| DockerState::new());
    cx.set_global(GlobalDockerState(state.clone()));
    state
}

/// Get the global docker state entity
pub fn docker_state(cx: &App) -> Entity<DockerState> {
    cx.global::<GlobalDockerState>().0.clone()
}
