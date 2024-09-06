use axum::{
    extract::Query,
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

async fn run(Query(cmd): Query<Cmd>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut command = Command::new("bash");
    command.arg("-c");
    command.arg(cmd.command);
    command.stdin(Stdio::null());
    command.stderr(Stdio::piped());
    command.stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let output = child.stdout.take().unwrap();
    let error = child.stderr.take().unwrap();
    let stream_out = ReaderStream::new(output)
        .filter_map(|data| {
            data.ok().and_then(|data| {
                String::from_utf8(data.to_vec())
                    .ok()
                    .and_then(|data| Event::default().event("stdout").json_data(data).ok())
            })
        })
        .map(Ok);
    let stream_err = ReaderStream::new(error)
        .filter_map(|data| {
            data.ok().and_then(|data| {
                String::from_utf8(data.to_vec())
                    .ok()
                    .and_then(|data| Event::default().event("stderr").json_data(data).ok())
            })
        })
        .map(Ok);
    let stream = stream_out.merge(stream_err);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(serde::Deserialize)]
struct Cmd {
    command: String,
}
