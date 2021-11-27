use crate::worker::{Logs, RunningContainerStats};

use docker_api::api::{
    ContainerDetails, ContainerInfo, ContainerListOpts, DeleteStatus, DistributionInspectInfo,
    History, ImageDetails, ImageInfo, ImageListOpts,
};

#[derive(Debug)]
pub struct ImageInspectInfo {
    pub details: ImageDetails,
    pub distribution_info: Option<DistributionInspectInfo>,
    pub history: Vec<History>,
}

#[derive(Debug)]
pub enum EventRequest {
    ListContainers(Option<ContainerListOpts>),
    ListImages(Option<ImageListOpts>),
    InspectImage {
        id: String,
    },
    DeleteContainer {
        id: String,
    },
    DeleteImage {
        id: String,
    },
    ContainerStats,
    ContainerLogs,
    ContainerDetails,
    StopContainer {
        id: String,
    },
    UnpauseContainer {
        id: String,
    },
    PauseContainer {
        id: String,
    },
    StartContainer {
        id: String,
    },
    ContainerTraceStart {
        id: String,
    },
    SaveImage {
        id: String,
        output_path: std::path::PathBuf,
    },
}

#[derive(Debug)]
pub enum EventResponse {
    ListContainers(Vec<ContainerInfo>),
    ListImages(Vec<ImageInfo>),
    InspectImage(Box<ImageInspectInfo>),
    ContainerStats(Box<RunningContainerStats>),
    ContainerLogs(Box<Logs>),
    ContainerDetails(Box<ContainerDetails>),
    DeleteContainer(anyhow::Result<String>),
    DeleteImage(anyhow::Result<DeleteStatus>),
    StopContainer(anyhow::Result<()>),
    UnpauseContainer(anyhow::Result<()>),
    PauseContainer(anyhow::Result<()>),
    StartContainer(anyhow::Result<()>),
    InspectContainerNotFound,
    SaveImage(anyhow::Result<(String, std::path::PathBuf)>),
}
