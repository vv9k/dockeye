use crate::stats::StatsWrapper;

use docker_api::api::{
    ContainerDetails, ContainerInfo, ContainerListOpts, DeleteStatus, DistributionInspectInfo,
    History, ImageDetails, ImageInfo, ImageListOpts,
};
use std::time::Duration;

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
    ContainerStats(Vec<(Duration, StatsWrapper)>),
    DeleteContainer(String),
    DeleteImage(DeleteStatus),
    StopContainer(docker_api::Result<()>),
    UnpauseContainer(docker_api::Result<()>),
    PauseContainer(docker_api::Result<()>),
    StartContainer(docker_api::Result<()>),
}
