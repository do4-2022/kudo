use std::sync::Arc;

use super::super::workload::model::Workload;
use super::super::workload::service::WorkloadService;
use super::service;
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse, Responder, Scope};
use tokio::sync::Mutex;
struct InstanceController {}

impl InstanceController {
    // pub async get_instance(instance_id: web::Path<String>) -> impl Responder {
    //   let mut instance_service = service::InstanceService::new().await;
    // }

    pub async fn put_instance(
        namespace: web::Path<String>,
        workload_id: web::Path<String>,
    ) -> impl Responder {
        let instance_service = service::InstanceService::new("0.0.0.0:50051").await;
        let mut workload_service = WorkloadService::new().await;
        match workload_service
            .get_workload(&workload_id, &namespace)
            .await
        {
            Ok(workload_str) => {
                let workload = serde_json::from_str::<Workload>(&workload_str);
                match workload {
                    Ok(_) => {
                        match super::service::InstanceService::retrieve_and_start_instance(
                            Arc::new(Mutex::new(instance_service)),
                            &workload_id,
                        )
                        .await
                        {
                            Ok(_) => HttpResponse::build(StatusCode::CREATED)
                                .body("Instance creating and starting..."),
                            Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                                .body("Internal Server Error"),
                        }
                    }
                    Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                        .body("Internal Server Error"),
                }
            }
            Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Workload not found"),
        }
    }
    pub async fn delete_instance(
        namespace: web::Path<String>,
        workload_id: web::Path<String>,
    ) -> impl Responder {
        let mut instance_service = service::InstanceService::new("0.0.0.0:50051").await;
        let mut workload_service = WorkloadService::new().await;
        match workload_service
            .get_workload(&workload_id, &namespace)
            .await
        {
            Ok(_) => match instance_service.get_instance(&workload_id).await {
                Ok(instance) => match instance_service.delete_instance(instance).await {
                    Ok(_) => HttpResponse::build(StatusCode::OK).body("Instance deleted"),
                    Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                        .body("Internal Server Error"),
                },
                Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Instance not found"),
            },
            Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Workload not found"),
        }
    }

    pub async fn patch_instance(
        namespace: web::Path<String>,
        workload_id: web::Path<String>,
    ) -> impl Responder {
        let mut instance_service = service::InstanceService::new("0.0.0.0:50051").await;
        let mut workload_service = WorkloadService::new().await;
        match workload_service
            .get_workload(&workload_id, &namespace)
            .await
        {
            Ok(_) => match instance_service.get_instance(&workload_id).await {
                Ok(instance) => match instance_service.delete_instance(instance).await {
                    Ok(_) => {
                        match super::service::InstanceService::retrieve_and_start_instance(
                            Arc::new(Mutex::new(instance_service)),
                            &workload_id,
                        )
                        .await
                        {
                            Ok(_) => HttpResponse::build(StatusCode::CREATED)
                                .body("Instance creating and starting..."),
                            Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                                .body("Internal Server Error"),
                        }
                    }
                    Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                        .body("Internal Server Error"),
                },
                Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Instance not found"),
            },
            Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Workload not found"),
        }
    }

    pub async fn get_instance(
        namespace: web::Path<String>,
        workload_id: web::Path<String>,
    ) -> impl Responder {
        let mut instance_service = service::InstanceService::new("0.0.0.0:20051").await;
        let mut workload_service = WorkloadService::new().await;
        match workload_service
            .get_workload(&workload_id, &namespace)
            .await
        {
            Ok(_) => match instance_service.get_instance(&workload_id).await {
                Ok(instance) => match serde_json::to_string(&instance) {
                    Ok(instance_str) => HttpResponse::build(StatusCode::OK).body(instance_str),
                    Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                        .body("Internal Server Error"),
                },
                Err(_) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Internal Server Error"),
            },
            Err(_) => HttpResponse::build(StatusCode::NOT_FOUND).body("Instance not found"),
        }
    }
}

pub fn get_services() -> Scope {
    web::scope("/instance")
        .service(
            web::resource("/{namespace}/{instance_id}")
                .route(web::delete().to(InstanceController::delete_instance))
                .route(web::get().to(InstanceController::get_instance))
                .route(web::patch().to(InstanceController::patch_instance)),
        )
        .service(
            web::resource("/{namespace}").route(web::put().to(InstanceController::put_instance)), // .route(web::get().to(WorkloadController::get_all_instances)),
        )
}
