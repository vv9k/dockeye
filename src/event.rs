use crate::worker::{Logs, RunningContainerStats};

use docker_api::api::{
    ContainerCreateOpts, ContainerDetails, ContainerInfo, ContainerListOpts, DeleteStatus,
    DistributionInspectInfo, History, ImageBuildChunk, ImageDetails, ImageInfo, ImageListOpts,
    Info, PingInfo, RegistryAuth, Version,
};

#[derive(Debug)]
pub struct SystemInspectInfo {
    pub version: Version,
    pub info: Info,
    pub ping_info: PingInfo,
}

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
    PullImage {
        image: String,
        auth: Option<RegistryAuth>,
    },
    PullImageChunks,
    DockerUriChange {
        uri: String,
    },
    ContainerCreate(ContainerCreateOpts),
    SystemInspect,
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
    PullImage(anyhow::Result<String>),
    PullImageChunks(Vec<ImageBuildChunk>),
    DockerUriChange(anyhow::Result<()>),
    ContainerCreate(anyhow::Result<String>),
    SystemInspect(anyhow::Result<SystemInspectInfo>),
}
