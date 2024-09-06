use axum::{
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use std::{convert::Infallible, process::Stdio};
use tokio::{net::TcpListener, process::Command};
use tokio_stream::StreamExt as _;
use tokio_util::io::ReaderStream;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/run", get(run))
        .nest_service("/", ServeDir::new("public"));
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn run() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut command = Command::new("bash");
    command.arg("-c");
    command.arg("while true\n do\n echo \"Hi!\"\n sleep 1\n done");
    command.stdin(Stdio::null());
    command.stderr(Stdio::inherit());
    command.stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let output = child.stdout.take().unwrap();
    let stream = ReaderStream::new(output)
        .filter_map(|data| {
            data.ok().and_then(|data| {
                String::from_utf8(data.to_vec())
                    .ok()
                    .and_then(|data| Event::default().json_data(data).ok())
            })
        })
        .map(Ok);
    Sse::new(stream).keep_alive(KeepAlive::default())
}
