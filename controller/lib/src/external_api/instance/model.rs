use std::net::Ipv4Addr;

use proto::controller::{InstanceState, Type};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum InstanceError {
    InstanceNotFound,
    OutOfRange,
    Etcd(String),
    Grpc(String),
    SerdeError(serde_json::Error),
    GenerateIp(String),
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InstanceError::InstanceNotFound => write!(f, "Instance not found"),
            InstanceError::OutOfRange => write!(f, "Instance out of range"),
            InstanceError::Etcd(err) => write!(f, "Etcd error: {}", err),
            InstanceError::Grpc(err) => write!(f, "Grpc error: {}", err),
            InstanceError::SerdeError(err) => write!(f, "Serde error: {}", err),
            InstanceError::GenerateIp(err) => write!(f, "Generate ip error: {}", err),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub r#type: Type,
    pub state: InstanceState,
    pub status_description: String,
    pub num_restarts: i32,
    pub uri: String,
    pub environment: Vec<String>,
    pub resource: Option<Resource>,
    pub ports: Vec<Port>,
    pub ip: Ipv4Addr,
    pub namespace: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Resource {
    pub limit: Option<ResourceSummary>,
    pub usage: Option<ResourceSummary>,
}

#[derive(Deserialize, Serialize)]
pub struct InstanceDTO {
    pub workload_name: String,
}

#[derive(Deserialize, Serialize)]

pub struct Pagination {
    pub limit: u32,
    pub offset: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ResourceSummary {
    pub cpu: u64,
    pub memory: u64,
    pub disk: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Port {
    pub source: i32,
    pub dest: i32,
}

impl Instance {
    pub fn update_instance(&mut self, instance_status: proto::scheduler::InstanceStatus) {
        self.state =
            InstanceState::from_i32(instance_status.status).unwrap_or(InstanceState::Scheduling);
        self.status_description = instance_status.status_description;
        self.resource = instance_status.resource.map(|resource| Resource {
            limit: resource.limit.map(|resource_summary| ResourceSummary {
                cpu: resource_summary.cpu,
                memory: resource_summary.memory,
                disk: resource_summary.disk,
            }),
            usage: resource.usage.map(|resource_summary| ResourceSummary {
                cpu: resource_summary.cpu,
                memory: resource_summary.memory,
                disk: resource_summary.disk,
            }),
        });
    }
}

// Because we don't want to add a From<> to the ::InstanceState enum
#[allow(clippy::from_over_into)]
impl Into<proto::scheduler::Instance> for Instance {
    fn into(self) -> proto::scheduler::Instance {
        proto::scheduler::Instance {
            id: self.id,
            name: self.name,
            r#type: self.r#type as i32,
            status: self.state as i32,
            environnement: self.environment,
            ip: self.ip.to_string(),
            ports: self
                .ports
                .into_iter()
                .map(|port| proto::scheduler::Port {
                    source: port.source,
                    destination: port.dest,
                })
                .collect(),
            resource: self.resource.map(|resource| proto::scheduler::Resource {
                limit: resource
                    .limit
                    .map(|resource_summary| proto::scheduler::ResourceSummary {
                        cpu: resource_summary.cpu,
                        memory: resource_summary.memory,
                        disk: resource_summary.disk,
                    }),
                usage: resource
                    .usage
                    .map(|resource_summary| proto::scheduler::ResourceSummary {
                        cpu: resource_summary.cpu,
                        memory: resource_summary.memory,
                        disk: resource_summary.disk,
                    }),
            }),
            uri: self.uri,
        }
    }
}

impl From<super::super::workload::model::Workload> for Instance {
    fn from(workload: super::super::workload::model::Workload) -> Self {
        let random_id: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(5)
            .map(char::from)
            .collect();
        Self {
            id: format!("{}-{}", workload.id, random_id),
            name: format!("{}-{}", workload.name, random_id),
            r#type: Type::Container,
            state: InstanceState::Scheduling,
            status_description: "".to_string(),
            num_restarts: 0,
            uri: workload.uri,
            environment: workload.environment,
            namespace: workload.namespace,
            resource: Some(Resource {
                limit: Some(ResourceSummary {
                    cpu: workload.resources.cpu,
                    memory: workload.resources.memory,
                    disk: workload.resources.disk,
                }),
                usage: None,
            }),
            ports: workload
                .ports
                .into_iter()
                .map(|port| Port {
                    source: port.source,
                    dest: port.destination,
                })
                .collect(),
            ip: Ipv4Addr::new(10, 0, 0, 1),
        }
    }
}