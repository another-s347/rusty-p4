use std::collections::HashMap;

use futures::{SinkExt, StreamExt};
use rusty_p4_core::service::{DefaultRequest, ServiceBus};
use warp::{path::Tail, ws::WebSocket, Filter, Rejection};

/// A web backend server for northbound
/// Regular:
/// GET /target/{target name}/{paths}{params}
/// POST /target/target name}/{paths} BODY {params}
/// Specify action mannully:
/// GET /action/{target name}/{action name}/{paths}{params}
/// POST /action/{target name}/{action name}/{paths} BODY {params}
/// Streaming via websocket:
/// /ws/{target name}/{action name}/{paths}{params}
pub struct WebServer {}

impl WebServer {
    pub async fn run(&self, service_bus: &ServiceBus) {
        let regular_path = warp::get()
            .and(warp::path("target"))
            .and(warp::path::param())
            .and(warp::path::tail())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_bus(service_bus.clone()))
            .and_then(
                |target: String,
                 path: Tail,
                 params: HashMap<String, String>,
                 service_bus: ServiceBus| async move {
                    send(target, "get".to_owned(), path, params, service_bus).await
                },
            );

        let regular_post_path = warp::post()
            .and(warp::path("target"))
            .and(warp::path::param())
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_bus(service_bus.clone()))
            .and_then(
                |target: String,
                 path: Tail,
                 params: HashMap<String, String>,
                 service_bus: ServiceBus| async move {
                    send(target, "set".to_owned(), path, params, service_bus).await
                },
            );

        let action_path = warp::get()
            .and(warp::path("action"))
            .and(warp::path::param())
            .and(warp::path::param())
            .and(warp::path::tail())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_bus(service_bus.clone()))
            .and_then(send);

        let action_post_path = warp::post()
            .and(warp::path("action"))
            .and(warp::path::param())
            .and(warp::path::param())
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_bus(service_bus.clone()))
            .and_then(send);

        let ws_path = warp::path("ws")
            .and(warp::path::param())
            .and(warp::path::param())
            .and(warp::path::tail())
            .and(warp::query::<HashMap<String, String>>())
            .and(warp::ws())
            .and(with_bus(service_bus.clone()))
            .map(
                |target: String,
                 action: String,
                 path,
                 params,
                 ws: warp::ws::Ws,
                 service_bus: ServiceBus| {
                    ws.on_upgrade(move |socket| {
                        stream(target, action, path, params, service_bus, socket)
                    })
                },
            );

        warp::serve(
            regular_path
                .or(regular_post_path)
                .or(action_path)
                .or(action_post_path)
                .or(ws_path),
        )
        .run(([127, 0, 0, 1], 3030))
        .await;
    }
}

impl rusty_p4_core::service::Server for WebServer {
    type EncodeTarget = serde_json::Value;

    const NAME: &'static str = "Web";

    fn encode<T>(response: T) -> Self::EncodeTarget
    where
        T: serde::Serialize,
    {
        serde_json::to_value(response).unwrap()
    }
}

fn with_bus(
    db: ServiceBus,
) -> impl Filter<Extract = (ServiceBus,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn map_err(_err: rusty_p4_core::error::MyError) -> warp::Rejection {
    warp::reject()
}

async fn send(
    target: String,
    action: String,
    path: Tail,
    params: HashMap<String, String>,
    service_bus: ServiceBus,
) -> Result<impl warp::Reply, Rejection> {
    let response = service_bus
        .send::<WebServer>(
            &target,
            DefaultRequest {
                path: path.as_str().split("\\").map(|x| x.to_owned()).collect(),
                action,
                params,
            },
            Default::default(),
        )
        .await
        .map_err(map_err)?;

    let response: Vec<serde_json::Value> = response.collect().await;

    let response = serde_json::json!({
        "len": response.len(),
        "data": response
    });

    Ok::<_, Rejection>(serde_json::to_string_pretty(&response).unwrap())
}

async fn stream(
    target: String,
    action: String,
    path: Tail,
    params: HashMap<String, String>,
    service_bus: ServiceBus,
    mut socket: WebSocket,
) {
    if let Ok(response) = service_bus
        .send::<WebServer>(
            &target,
            DefaultRequest {
                path: path.as_str().split("\\").map(|x| x.to_owned()).collect(),
                action,
                params,
            },
            Default::default(),
        )
        .await
        .map_err(map_err)
    {
        let _ = response
            .map(|x| {
                serde_json::to_string_pretty(&x)
                    .map(|x| warp::ws::Message::text(x))
                    .or_else(|_| Ok(warp::ws::Message::close()))
            })
            .forward(socket)
            .await;
    } else {
        let _ = socket.send(warp::ws::Message::close()).await;
    }
}

#[cfg(test)]
mod test {
    use super::WebServer;
    use rusty_p4_core::service::dummy::DummyService;

    #[tokio::test]
    async fn test_run() {
        let service_bus = rusty_p4_core::service::ServiceBus::new();
        let service = DummyService { size: 3 };
        service_bus.install_service(service);
        WebServer {}.run(&service_bus).await;
    }
}
