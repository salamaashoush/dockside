use gpui::actions;

use crate::colima::ColimaStartOptions;

// Machine actions
actions!(
    machines,
    [
        RefreshMachines,
        ShowCreateMachineDialog,
        HideCreateMachineDialog,
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub struct CreateMachine {
    pub options: ColimaStartOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartMachine {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StopMachine {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestartMachine {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteMachine {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectMachine {
    pub name: String,
}

// Container actions
actions!(
    containers,
    [
        RefreshContainers,
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub struct StartContainer {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StopContainer {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestartContainer {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteContainer {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectContainer {
    pub id: String,
}

// Image actions
actions!(
    images,
    [
        RefreshImages,
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub struct PullImage {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteImage {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectImage {
    pub id: String,
}

// Volume actions
actions!(
    volumes,
    [
        RefreshVolumes,
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteVolume {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectVolume {
    pub name: String,
}

// Network actions
actions!(
    networks,
    [
        RefreshNetworks,
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteNetwork {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectNetwork {
    pub id: String,
}

// Navigation actions
actions!(
    navigation,
    [
        ShowContainers,
        ShowImages,
        ShowVolumes,
        ShowNetworks,
        ShowMachines,
        ShowPods,
        ShowServices,
        ShowActivityMonitor,
    ]
);
