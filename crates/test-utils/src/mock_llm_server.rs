use zenon_core::error::{AgenticError, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Minimal HTTP server for exercising LLM providers in integration tests.
pub struct MockLlmServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl MockLlmServer {
    pub async fn start(
        expected_path: &'static str,
        response_body: &'static str,
        content_type: &'static str,
    ) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| AgenticError::Io(std::io::Error::other(format!("bind mock server: {e}"))))?;
        let addr = listener
            .local_addr()
            .map_err(|e| AgenticError::Io(std::io::Error::other(format!("mock addr: {e}"))))?;
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accept = listener.accept() => {
                        let Ok((mut stream, _peer)) = accept else {
                            break;
                        };
                        let _ = handle_connection(&mut stream, addr, expected_path, response_body, content_type).await;
                    }
                }
            }
        });

        Ok(Self {
            base_url: format!("http://{}", addr),
            shutdown: Some(shutdown_tx),
            task: Some(task),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn start_openai(response_body: &'static str) -> Result<Self> {
        Self::start("/v1/chat/completions", response_body, "application/json").await
    }

    pub async fn start_anthropic(response_body: &'static str) -> Result<Self> {
        Self::start("/v1/messages", response_body, "application/json").await
    }
}

impl Drop for MockLlmServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn handle_connection(
    stream: &mut tokio::net::TcpStream,
    _addr: SocketAddr,
    expected_path: &str,
    response_body: &str,
    content_type: &str,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    let read = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..read]);
    let first_line = request.lines().next().unwrap_or_default();
    let path = first_line.split_whitespace().nth(1).unwrap_or_default();

    let (status_line, body) = if path == expected_path {
        ("HTTP/1.1 200 OK", response_body)
    } else {
        ("HTTP/1.1 404 Not Found", "{\"error\":\"unexpected path\"}")
    };

    let response = format!(
        "{status_line}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}
