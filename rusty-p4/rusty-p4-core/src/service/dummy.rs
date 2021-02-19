use std::time::Duration;

use futures::{Stream, StreamExt};

use super::{DefaultRequest, ParseRequest, RequestOption, Service, request, server::Server};

pub struct DummyService {
    pub size: usize
}

pub struct DummyRequest {
    action: String
}

impl ParseRequest for DummyRequest {
    fn parse(req: super::DefaultRequest) -> crate::error::Result<Self> {
        Ok(DummyRequest {
            action: req.action,
        })
    }
}

impl super::Service for DummyService {
    type Request = DummyRequest;

    const NAME: &'static str = "Dummy";

    fn process(&mut self, request: super::Request<Self::Request>) -> crate::error::Result<Option<usize>> {
        match request.inner.action.as_ref() {
            "set" => {
                self.size = 5;
                Ok(Some(0))
            }
            "get" => {
                let s = self.size;
                tokio::spawn(async move {
                    for i in 0..s {
                        request.respond(i).await;
                    }
                });
                Ok(Some(self.size))
            }
            other => {
                Err(crate::error::ServiceError::ActionNotFound(other.to_owned()).into())
            }
        }
    }
}

pub struct DummyServer {

}

impl Server for DummyServer {
    type EncodeTarget = String;

    const NAME: &'static str = "DummyServer";

    fn encode<T>(response: T) -> Self::EncodeTarget where T: serde::Serialize {
        serde_json::to_string(&response).unwrap()
    }
}

impl DummyServer {
    pub async fn run(&self, service_bus: &super::ServiceBus, request: DefaultRequest) -> crate::error::Result<Vec<String>> {
        let mut response = service_bus.send::<Self>(DummyService::NAME, request, RequestOption::default()).await?;

        Ok(response.collect::<Vec<String>>().await)
    }
}

#[tokio::test]
async fn test_service_dummy() {
    let service_bus = crate::service::ServiceBus::new();
    let service = DummyService {
        size: 3,
    };
    service_bus.install_service(service);
    let backend = DummyServer {

    };
    let expected:Vec<String> = vec!["0".to_owned(), "1".to_owned(), "2".to_owned()];
    assert!(backend.run(&service_bus, DefaultRequest {
        path: vec![],
        action: "get".to_owned(),
        params: Default::default(),
    }).await.unwrap()==expected);
}

#[tokio::test]
async fn test_service_dummy_set() {
    let service_bus = crate::service::ServiceBus::new();
    let service = DummyService {
        size: 3,
    };
    service_bus.install_service(service);
    let backend = DummyServer {

    };
    backend.run(&service_bus, DefaultRequest {
        path: vec![],
        action: "set".to_owned(),
        params: Default::default(),
    }).await;
    let expected:Vec<String> = vec!["0".to_owned(), "1".to_owned(), "2".to_owned(),"3".to_owned(),"4".to_owned()];
    assert!(backend.run(&service_bus, DefaultRequest {
        path: vec![],
        action: "get".to_owned(),
        params: Default::default(),
    }).await.unwrap()==expected);
}

#[tokio::test]
async fn test_service_error() {
    let service_bus = crate::service::ServiceBus::new();
    let service = DummyService {
        size: 3,
    };
    service_bus.install_service(service);
    let backend = DummyServer {

    };
    let expected:Vec<String> = vec!["0".to_owned(), "1".to_owned(), "2".to_owned()];
    assert!(backend.run(&service_bus,DefaultRequest {
        path: vec![],
        action: "whatever".to_owned(),
        params: Default::default(),
    }).await.err().is_some());
}