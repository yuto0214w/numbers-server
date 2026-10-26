use std::{error::Error, io, net::SocketAddr, process};

use axum::{body::Bytes, http::header, middleware, response::Html, routing::get};
use hyper::{Request, body::Incoming};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server,
};
use tokio::runtime::Builder as TokioRuntimeBuilder;
use tower::{Service as _, util::ServiceExt as _};
use utoipa::OpenApi as _;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    config::{Config, Listener},
    util::{log_error, unwrap_infallible},
};

mod config;
mod game;
mod openapi;
mod types;
mod util;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    util::windows_setup();

    let config = match Config::load_or_create() {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("Edit the configuration file and/or restart the program.");
            process::exit(2);
        }
        Err(error) => {
            log_error!("config", error);
            process::exit(1);
        }
    };

    let rt = TokioRuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        game::spawn_player_eviction();

        let app = {
            let (mut router, api) = OpenApiRouter::with_openapi(openapi::ApiDoc::openapi())
                .routes(routes!(game::http::version))
                .nest(
                    "/rooms",
                    OpenApiRouter::new()
                        .routes(routes!(game::http::room_list))
                        .routes(routes!(game::http::new_room))
                        .nest(
                            "/{room_id}",
                            OpenApiRouter::new()
                                .routes(routes!(game::http::room_info))
                                .routes(routes!(game::http::serve_ws))
                                .layer(middleware::from_fn(game::http::room_existence_check)),
                        ),
                )
                .split_for_parts();

            router = router.route(
                "/rule.txt",
                get(|| async { include_str!("../res/rule_v1.txt") }),
            );

            if config.listener.serve_docs {
                let openapi_json = Bytes::from(api.to_json()?);
                let openapi_yaml = Bytes::from(api.to_yaml()?);
                router =
                    router
                        .route(
                            "/",
                            get(|| async { Html(include_str!("../res/scalar.html")) }),
                        )
                        .route(
                            "/openapi.json",
                            get(move || {
                                let openapi_json = openapi_json.clone();
                                async move {
                                    ([(header::CONTENT_TYPE, "application/json")], openapi_json)
                                }
                            }),
                        )
                        .route(
                            "/procedure.svg",
                            get(|| async {
                                (
                                    [(header::CONTENT_TYPE, "image/svg+xml")],
                                    include_str!(concat!(env!("OUT_DIR"), "/procedure.svg")),
                                )
                            }),
                        )
                        .route(
                            "/openapi.yaml",
                            get(move || {
                                let openapi_yaml = openapi_yaml.clone();
                                async move {
                                    ([(header::CONTENT_TYPE, "application/yaml")], openapi_yaml)
                                }
                            }),
                        );
            }

            router
                .layer(config.listener.cors.to_layer()?)
                .layer(middleware::from_fn(util::log_http_middleware))
        };

        let Listener { http, https, .. } = config.listener;
        if !http.enabled && !https.enabled {
            return Err("no listener is enabled".into());
        }
        // TODO: implement https listener
        if !http.enabled || https.enabled {
            todo!();
        }

        #[cfg(debug_assertions)]
        println!("This is a debug build.");
        #[cfg(not(debug_assertions))]
        println!("This is a release build.");

        let mut make_service = app.into_make_service_with_connect_info::<SocketAddr>();
        let listener = tokio::net::TcpListener::bind((http.address.as_str(), http.port)).await?;
        println!("Listening at http://{}/", listener.local_addr()?);

        loop {
            let (socket, remote_addr) = listener.accept().await?;
            let tower_service = unwrap_infallible(make_service.call(remote_addr).await);

            tokio::spawn(async move {
                let socket = TokioIo::new(socket);
                let hyper_service =
                    hyper::service::service_fn(move |request: Request<Incoming>| {
                        tower_service.clone().oneshot(request)
                    });
                let mut hyper_server = server::conn::auto::Builder::new(TokioExecutor::new());
                hyper_server.http1().title_case_headers(true);
                if let Err(error) = hyper_server
                    .serve_connection_with_upgrades(socket, hyper_service)
                    .await
                {
                    if !error
                        .downcast_ref::<io::Error>()
                        .is_some_and(|e| e.kind() == io::ErrorKind::UnexpectedEof)
                    {
                        log_error!("serve", error);
                    }
                }
            });
        }
    })
}
