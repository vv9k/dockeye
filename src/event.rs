use crate::worker::{Logs, RunningContainerStats};

use docker_api::api::{
    Change, ClearCacheInfo, ContainerCreateOpts, ContainerDetails, ContainerId, ContainerInfo,
    ContainerListOpts, ContainersPruneInfo, DataUsage, DeleteStatus, DistributionInspectInfo,
    Event, History, ImageBuildChunk, ImageDetails, ImageId, ImageInfo, ImageListOpts,
    ImagesPruneInfo, Info, NetworkId, NetworkInfo, NetworkListOpts, NetworksPruneInfo,
    RegistryAuth, SearchResult, TagOpts, Top, Version,
};
use docker_api::Error;
use std::path::PathBuf;

#[derive(Debug)]
pub struct SystemInspectInfo {
    pub version: Version,
    pub info: Info,
}

#[derive(Debug)]
pub struct ImageInspectInfo {
    pub details: ImageDetails,
    pub distribution_info: Option<DistributionInspectInfo>,
    pub history: Vec<History>,
}

#[derive(Debug)]
pub enum ContainerEvent {
    List(Option<ContainerListOpts>),
    Delete { id: String },
    Stats,
    Logs,
    Details,
    Stop { id: String },
    Unpause { id: String },
    Pause { id: String },
    Start { id: String },
    TraceStart { id: String },
    Create(ContainerCreateOpts),
    Rename { id: String, name: String },
    ForceDelete { id: String },
    Prune,
    Restart { id: String },
    ProcessList,
    Changes,
}

#[derive(Debug)]
pub enum ImageEvent {
    List(Option<ImageListOpts>),
    Inspect {
        id: String,
    },
    Delete {
        id: String,
    },
    Save {
        id: String,
        output_path: PathBuf,
    },
    Pull {
        image: String,
        auth: Option<RegistryAuth>,
    },
    Search {
        image: String,
    },
    ForceDelete {
        id: String,
    },
    Import {
        path: PathBuf,
    },
    PullChunks,
    Prune,
    ClearCache,
    Tag {
        id: String,
        opts: TagOpts,
    },
}

#[derive(Debug)]
pub enum NetworkEvent {
    List(Option<NetworkListOpts>),
    Delete { id: NetworkId },
    Prune,
}
#[derive(Debug)]
pub enum NetworkEventResponse {
    List(Vec<NetworkInfo>),
    Delete(anyhow::Result<NetworkId>),
    Prune(anyhow::Result<NetworksPruneInfo>),
}

#[derive(Debug)]
pub enum EventRequest {
    Container(ContainerEvent),
    Image(ImageEvent),
    DockerUriChange { uri: String },
    SystemInspect,
    SystemDataUsage,
    SystemEvents,
    Network(NetworkEvent),
    NotifyGui(GuiEvent),
}

#[derive(Debug)]
pub enum ContainerEventResponse {
    List(Vec<ContainerInfo>),
    Stats(Box<RunningContainerStats>),
    Logs(Box<Logs>),
    Details(Box<ContainerDetails>),
    Delete(Result<ContainerId, (ContainerId, Error)>),
    Stop(anyhow::Result<()>),
    Unpause(anyhow::Result<()>),
    Pause(anyhow::Result<()>),
    Start(anyhow::Result<()>),
    InspectNotFound,
    Create(anyhow::Result<ContainerId>),
    Rename(anyhow::Result<()>),
    ForceDelete(anyhow::Result<ContainerId>),
    Prune(anyhow::Result<ContainersPruneInfo>),
    Restart(anyhow::Result<ContainerId>),
    RestartInProgress { id: String },
    ProcessList(anyhow::Result<Top>),
    Changes(anyhow::Result<Option<Vec<Change>>>),
}

#[derive(Debug)]
pub enum ImageEventResponse {
    List(Vec<ImageInfo>),
    Inspect(Box<ImageInspectInfo>),
    Delete(Result<DeleteStatus, (ImageId, Error)>),
    Save(anyhow::Result<(ImageId, PathBuf)>),
    Pull(anyhow::Result<ImageId>),
    PullChunks(Vec<ImageBuildChunk>),
    Search(anyhow::Result<Vec<SearchResult>>),
    ForceDelete(anyhow::Result<DeleteStatus>),
    Import(anyhow::Result<String>),
    Prune(anyhow::Result<ImagesPruneInfo>),
    ClearCache(anyhow::Result<ClearCacheInfo>),
    Tag(anyhow::Result<()>),
}

#[derive(Debug)]
pub enum EventResponse {
    Container(ContainerEventResponse),
    Image(ImageEventResponse),
    DockerUriChange(anyhow::Result<()>),
    SystemInspect(anyhow::Result<Box<SystemInspectInfo>>),
    SystemDataUsage(anyhow::Result<Box<DataUsage>>),
    SystemEvents(Vec<Event>),
    NotifyGui(GuiEventResponse),
    Network(NetworkEventResponse),
}

#[derive(Debug)]
pub enum GuiEvent {
    SetTab(crate::app::Tab),
}

#[derive(Debug)]
pub enum GuiEventResponse {
    SetTab(crate::app::Tab),
}

impl From<GuiEvent> for GuiEventResponse {
    fn from(event: GuiEvent) -> Self {
        match event {
            GuiEvent::SetTab(tab) => Self::SetTab(tab),
        }
    }
}
