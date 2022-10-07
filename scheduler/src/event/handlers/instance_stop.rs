use std::sync::Arc;

use crate::{orchestrator::Orchestrator, InstanceIdentifier};
use anyhow::Result;
use tokio::sync::{oneshot, Mutex};
use tonic::Response;

pub struct InstanceStopHandler {}

impl InstanceStopHandler {
    pub async fn handle(
        orchestrator: Arc<Mutex<Orchestrator>>,
        id: InstanceIdentifier,
        tx: oneshot::Sender<Result<Response<()>, tonic::Status>>,
    ) {
        match orchestrator.lock().await.stop_instance(id.clone()).await {
            Ok(_) => {
                log::info!("stopped instance : {:?}", id);

                tx.send(Ok(Response::new(()))).unwrap();
            }
            Err(err) => {
                log::error!("error while stopping instance : {:?} ({:?})", id, err);

                tx.send(Err(tonic::Status::internal(format!(
                    "Error thrown by the orchestrator: {:?}",
                    err
                ))))
                .unwrap();
            }
        };
    }
}