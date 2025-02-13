#![feature(once_cell)]

mod models;
mod helpers;
mod templates;

use axum::{
    response::{Html, IntoResponse},
    routing::{get, post, get_service},
    http::StatusCode,
    extract::Path,
    body::Body,
    Router,
    Json,
};
use mongodb::{
    options::ClientOptions,
    bson::doc,
    Collection,
    Client,
};
use askama::Template;
use std::sync::OnceLock;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

static COLLECTION: OnceLock<Collection<models::PasteModel>> = OnceLock::new();


async fn post_upload(Json(payload): Json<models::FormPayload>) -> impl IntoResponse {
    if payload.content.len() > 1 {
        let id = helpers::generate_id(20);
        let collection = COLLECTION.get().unwrap();

        collection.insert_one(
            models::PasteModel { id: id.clone(), content: payload.content}, None
        ).await.unwrap();

        return Json(models::PasteJsonResponse { id: id }).into_response();
    } else {
        return StatusCode::BAD_REQUEST.into_response();
    }
}


async fn get_root() -> Html<String> {
    let template = templates::Index {};
    Html(template.render().unwrap_or("Woops something went wrong".to_string()))
}


async fn get_paste(Path(params): Path<String>) -> Html<String> {
    let collection = COLLECTION.get().unwrap();
    let paste = collection.find_one(
        doc! { "id": params }, None
    ).await.unwrap();

    if paste.is_none() {
        return Html(
            templates::NotFound {}
            .render()
            .unwrap_or_else(|_| "Woops something went wrong".to_string())
        )
    } else {
        return Html(
            templates::Paste { paste_content: &paste.unwrap().content.as_str() }
            .render()
            .unwrap_or_else(|_| "Woops something went wrong".to_string())
        )
    }
}


async fn init_mongo() -> mongodb::error::Result<()> {
    let config = helpers::get_config();
    let mongo_url = format!(
        "mongodb+srv://{}:{}@{}.efj2q.mongodb.net/?retryWrites=true&w=majority",
        config.mongo_username, config.mongo_password, config.mongo_cluster,
    );

    let client_options = ClientOptions::parse(mongo_url).await?;
    let client = Client::with_options(client_options)?;
    let database = client.database(config.database_name.as_str());

    COLLECTION.set(database.collection::<models::PasteModel>(config.collection_name.as_str())).unwrap();
    Ok(())
}


async fn run(app: Router<Body>) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8030));
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to await for SIGINT")
        });

    server.await.expect("Failed to start server");
}


#[tokio::main]
async fn main() {
    let app: Router<Body> = Router::new()
        .route("/", get(get_root))
        .route("/upload", post(post_upload))
        .route("/:paste_id", get(get_paste))
        .fallback(get_service(ServeDir::new("./static/"))
        .handle_error(|err| async move {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serve files: {err}"),
            )
        }));

    init_mongo().await.unwrap();

    run(app).await;
}
