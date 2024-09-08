use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use futures_util::stream::Stream;
use std::process::Stdio;
use tokio::{net::TcpListener, process::Command};
use tokio::{sync::broadcast, task};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt as _};
use tokio_util::io::ReaderStream;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let (sender, _) = broadcast::channel::<Event>(8);
    let app = Router::new()
        .route("/run", post(run))
        .route("/listen", get(listen))
        .nest_service("/", ServeDir::new("public"))
        .with_state(sender);
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn run(State(sender): State<broadcast::Sender<Event>>, cmd: String) -> impl IntoResponse {
    let mut command = Command::new("bash");
    command.arg("-c");
    command.arg(cmd);
    command.stdin(Stdio::null());
    command.stderr(Stdio::piped());
    command.stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let output = child.stdout.take().unwrap();
    let error = child.stderr.take().unwrap();
    task::spawn(async move {
        let stream_out = ReaderStream::new(output).filter_map(|data| {
            data.ok().and_then(|data| {
                String::from_utf8(data.to_vec())
                    .ok()
                    .and_then(|data| Event::default().event("stdout").json_data(data).ok())
            })
        });
        let stream_err = ReaderStream::new(error).filter_map(|data| {
            data.ok().and_then(|data| {
                String::from_utf8(data.to_vec())
                    .ok()
                    .and_then(|data| Event::default().event("stderr").json_data(data).ok())
            })
        });
        let mut stream = stream_out.merge(stream_err);
        while let Some(msg) = stream.next().await {
            sender.send(msg).ok();
        }
    });
    StatusCode::OK
}

async fn listen(
    State(sender): State<broadcast::Sender<Event>>,
) -> Sse<impl Stream<Item = Result<Event, BroadcastStreamRecvError>>> {
    let receiver = sender.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(receiver);
    Sse::new(stream).keep_alive(KeepAlive::default())
}
