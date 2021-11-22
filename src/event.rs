use crate::logs::Logs;
use crate::stats::RunningContainerStats;

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
    InspectContainer { id: String },
    InspectImage { id: String },
    DeleteContainer { id: String },
    DeleteImage { id: String },
    ContainerStatsStart { id: String },
    ContainerStats,
    ContainerLogs,
    StopContainer { id: String },
    UnpauseContainer { id: String },
    PauseContainer { id: String },
    StartContainer { id: String },
}

#[derive(Debug)]
pub enum EventResponse {
    ListContainers(Vec<ContainerInfo>),
    ListImages(Vec<ImageInfo>),
    InspectContainer(Box<ContainerDetails>),
    InspectImage(Box<ImageInspectInfo>),
    ContainerStats(Box<RunningContainerStats>),
    ContainerLogs(Box<Logs>),
    DeleteContainer(String),
    DeleteImage(DeleteStatus),
    StopContainer(docker_api::Result<()>),
    UnpauseContainer(docker_api::Result<()>),
    PauseContainer(docker_api::Result<()>),
    StartContainer(docker_api::Result<()>),
}
