use std::env;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use log::{debug, info, trace};
use tokio::sync::mpsc::channel;
use tokio::sync::Mutex;
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

mod config;

use config::{GrpcServerConfig, NodeAgentConfig};
use node_manager::NodeSystem;
use workload_manager::workload_manager::WorkloadManager;

use proto::agent::{
    instance_service_server::InstanceService, instance_service_server::InstanceServiceServer,
    Instance, InstanceStatus, SignalInstruction,
};
use proto::scheduler::{
    node_service_client::NodeServiceClient, NodeRegisterRequest, NodeRegisterResponse, NodeStatus,
    Resource, ResourceSummary, Status as SchedulerStatus,
};

const NUMBER_OF_CONNECTION_ATTEMPTS: u16 = 10;

///
/// This Struct implement the Instance service from Node Agent proto file
pub struct InstanceServiceController {
    workload_manager: Arc<Mutex<WorkloadManager>>,
}

impl InstanceServiceController {
    pub fn new(node_id: String) -> Self {
        Self {
            workload_manager: Arc::new(Mutex::new(WorkloadManager::new(node_id))),
        }
    }
}

#[tonic::async_trait]
impl InstanceService for InstanceServiceController {
    type createStream = ReceiverStream<Result<InstanceStatus, Status>>;

    async fn create(
        &self,
        request: Request<Instance>,
    ) -> Result<Response<Self::createStream>, Status> {
        let instance = request.into_inner();
        let channel = channel(1024);

        // call workload_manager create function in an other thread
        let workload_manager = self.workload_manager.clone();

        tokio::spawn(async move {
            workload_manager
                .clone()
                .lock()
                .await
                .create(instance, channel.0.clone())
                .await
                .ok();
        });

        // send receiver to scheduler
        Ok(Response::new(ReceiverStream::new(channel.1)))
    }

    async fn signal(&self, request: Request<SignalInstruction>) -> Result<Response<()>, Status> {
        let signal_instruction = request.into_inner();

        // call workload_manager signal function in an other thread
        let workload_manager = self.workload_manager.clone();

        tokio::spawn(async move {
            workload_manager
                .clone()
                .lock()
                .await
                .signal(signal_instruction)
                .await
                .map_err(|_| Status::internal("Cannot send signal to the workload"))
                .unwrap();
        });

        Ok(Response::new(()))
    }
}

///
/// This function starts the grpc server of the Node Agent.
/// The server listens and responds to requests from the Scheduler.
/// The default port is 50053.
fn create_grpc_server(config: GrpcServerConfig, node_id: String) -> tokio::task::JoinHandle<()> {
    let addr = format!("{}:{}", config.host, config.port).parse().unwrap();
    let instance_service_controller = InstanceServiceController::new(node_id);

    info!("Node Agent server listening on {}", addr);

    tokio::spawn(async move {
        Server::builder()
            .add_service(InstanceServiceServer::new(instance_service_controller))
            .serve(addr)
            .await
            .unwrap()
    })
}

///
/// This function allows you to connect to the scheduler's grpc server.
async fn connect_to_scheduler(
    addr: String,
) -> Option<NodeServiceClient<tonic::transport::Channel>> {
    NodeServiceClient::connect(addr.clone()).await.ok()
}

///
/// This function allows you to register to the scheduler's grpc server.
async fn register_to_scheduler(
    client: &mut NodeServiceClient<tonic::transport::Channel>,
    certificate: String,
) -> Option<tonic::Response<NodeRegisterResponse>> {
    let register_request = tonic::Request::new(NodeRegisterRequest { certificate });

    client.register(register_request).await.ok()
}

///
/// This function allows you to send node status to the scheduler's grpc server.
async fn send_node_status_to_scheduler(
    client: &mut NodeServiceClient<tonic::transport::Channel>,
    node_system_arc: Arc<Mutex<NodeSystem>>,
    node_id: String,
) -> Option<tonic::Response<()>> {
    let node_status_stream = async_stream::stream! {
        let mut interval = time::interval(Duration::from_secs(1));

        let cpu_limit = node_system_arc.lock().await.total_cpu();
        let memory_limit = node_system_arc.lock().await.total_memory();
        let disk_limit = node_system_arc.lock().await.total_disk();

        loop {
            interval.tick().await;

            let cpu_usage = node_system_arc.lock().await.used_cpu();
            let memory_usage = node_system_arc.lock().await.used_memory();
            let disk_usage = node_system_arc.lock().await.used_disk();

            let node_status = NodeStatus {
                id: node_id.clone(),
                status: SchedulerStatus::Running as i32,
                status_description: "".into(),
                resource: Some(Resource {
                    limit: Some(ResourceSummary {
                        cpu: cpu_limit,
                        memory: memory_limit,
                        disk: disk_limit,
                    }),
                    usage: Some(ResourceSummary {
                        cpu: cpu_usage,
                        memory: memory_usage,
                        disk: disk_usage,
                    }),
                }),
            };

            debug!("Node resources sent to the Scheduler");

            yield node_status;
        }
    };

    client.status(Request::new(node_status_stream)).await.ok()
}

///
/// This function launch the Node Agent grpc client.
/// First, the client registered to the Scheduler.
/// Secondaly, once connected to it, it's send node resources to the Scheduler.
fn create_grpc_client(config: GrpcServerConfig, node_id: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        //  Connection to the Scheduler's grpc server

        let addr = format!("http://{}:{}", config.host, config.port);
        let mut connection = connect_to_scheduler(addr.clone()).await;

        let mut attempts: u16 = 0;
        while connection.is_none() {
            if attempts <= NUMBER_OF_CONNECTION_ATTEMPTS {
                sleep(Duration::from_secs(1));

                debug!("Connection to grpc scheduler server failed, retrying...");
                connection = connect_to_scheduler(addr.clone()).await;

                attempts += 1;
            } else {
                panic!("Error, unable to connect to the Scheduler server.");
            }
        }

        let mut client = connection.unwrap();

        info!("Node agent connected to the Scheduler at {}", addr);

        // Registration with the Scheduler

        let certificate = node_id.clone();
        let mut registration = register_to_scheduler(&mut client, certificate.clone()).await;

        // setup node network

        // let node_ip = registration.unwrap().into_inner().ip;
        // let node_ip_addr = Ipv4Addr::from_str(&node_ip).unwrap();
        // let node_ip_cidr = Ipv4Inet::new(node_ip_addr, 24).unwrap();

        // let request = SetupNodeRequest::new(node_id.to_string(), node_ip_cidr);
        // let response = setup_node(request).unwrap();

        attempts = 0;
        while registration.is_none() {
            if attempts <= NUMBER_OF_CONNECTION_ATTEMPTS {
                sleep(Duration::from_secs(1));

                debug!("Registration to the Scheduler failed, retrying...");
                registration = register_to_scheduler(&mut client, certificate.clone()).await;

                attempts += 1;
            } else {
                panic!("Error, unable to register to the Scheduler.");
            }
        }

        info!("Node agent registered to the Scheduler");

        // Send Node status to the Scheduler

        let node_system = NodeSystem::new();
        let arc_node_system = Arc::new(Mutex::new(node_system));

        let mut send_node_resources_to_scheduler = send_node_status_to_scheduler(
            &mut client,
            Arc::clone(&arc_node_system),
            node_id.clone(),
        )
        .await;

        attempts = 0;
        while send_node_resources_to_scheduler.is_none() {
            if attempts <= NUMBER_OF_CONNECTION_ATTEMPTS {
                sleep(Duration::from_secs(1));

                debug!("Sending node status to the Scheduler failed, retrying...");
                send_node_resources_to_scheduler = send_node_status_to_scheduler(
                    &mut client,
                    Arc::clone(&arc_node_system),
                    node_id.clone(),
                )
                .await;

                attempts += 1;
            } else {
                panic!("Error, unable to send node status to the Scheduler.");
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Starting up node agent");

    info!("Loading config");
    let mut dir = env::current_exe()?; // get executable path
    dir.pop(); // remove executable name
    dir.push("agent.conf"); // add config file name

    trace!("Node Agent config at: {:?}", dir);

    // load config from path
    let config: NodeAgentConfig = confy::load_path(dir.as_path())?;
    debug!("config: {:?}", config);

    // generate node id
    let node_id = Uuid::new_v4().to_string();

    // start grpc server and client
    let client_handler = create_grpc_client(config.client, node_id.clone());
    let server_handler = create_grpc_server(config.server, node_id.clone());

    client_handler.await?;
    server_handler.await?;

    info!("Shutting down node agent");

    Ok(())
}
