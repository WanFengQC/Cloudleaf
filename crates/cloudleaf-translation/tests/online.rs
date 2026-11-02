use cloudleaf_translation::{
    Language, OpenAiCompatibleConfig, OpenAiCompatibleProvider, OutputKind, ProviderIdentity,
    ProviderRegistry, QualityStatus, Secret, SecretStore, ServiceLimits, SessionSecretStore,
    TranslationCancellation, TranslationContext, TranslationDirection, TranslationError,
    TranslationKind, TranslationProvider, TranslationProviderConfig, TranslationRequest,
    TranslationService,
};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TEST_SECRET: &str = "fixture-only-not-a-real-credential";

struct Reply {
    status: u16,
    first_status: Option<u16>,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    delay: Duration,
    delayed_body: bool,
    chunked: bool,
    disconnect: bool,
}

impl Reply {
    fn json(value: Value) -> Self {
        Self {
            status: 200,
            first_status: None,
            body: serde_json::to_vec(&value).unwrap(),
            headers: Vec::new(),
            delay: Duration::ZERO,
            delayed_body: false,
            chunked: false,
            disconnect: false,
        }
    }
}

struct LocalServer {
    endpoint: String,
    stop: Arc<AtomicBool>,
    connections: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    thread: Option<JoinHandle<()>>,
}

impl LocalServer {
    fn new(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (thread_stop, thread_connections, thread_requests) =
            (stop.clone(), connections.clone(), requests.clone());
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let number = thread_connections.fetch_add(1, Ordering::AcqRel);
                        // Windows接受的socket可能继承listener模式，正文读取明确使用阻塞流。
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        if let Some(request) = receive_request(&mut stream) {
                            thread_requests.lock().unwrap().push(request);
                            if !reply.disconnect {
                                respond(&mut stream, &reply, &thread_stop, number);
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            endpoint,
            stop,
            connections,
            requests,
            thread: Some(thread),
        }
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::Acquire)
    }

    fn wait_for_request(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.requests.lock().unwrap().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !self.requests.lock().unwrap().is_empty(),
            "本地服务器未收到测试请求"
        );
    }

    fn body(&self, index: usize) -> Value {
        let requests = self.requests.lock().unwrap();
        let bytes = &requests[index];
        let offset = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        serde_json::from_slice(&bytes[offset..]).unwrap()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn receive_request(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer).ok()?;
        if count == 0 || bytes.len() + count > 2 * 1024 * 1024 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header = std::str::from_utf8(&bytes[..offset]).ok()?;
            let length = header.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })?;
            if bytes.len() >= offset + 4 + length {
                return Some(bytes);
            }
        }
    }
}

fn wait_delay(delay: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + delay;
    while !stop.load(Ordering::Acquire) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
}

fn respond(stream: &mut TcpStream, reply: &Reply, stop: &AtomicBool, number: usize) {
    if !reply.delayed_body {
        wait_delay(reply.delay, stop);
    }
    let status = if number == 0 {
        reply.first_status.unwrap_or(reply.status)
    } else {
        reply.status
    };
    let mut headers = format!("HTTP/1.1 {status} fixture\r\nConnection: close\r\n");
    if reply.chunked {
        headers.push_str("Transfer-Encoding: chunked\r\n");
    } else {
        headers.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
    }
    for (name, value) in &reply.headers {
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    headers.push_str("\r\n");
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }
    if reply.delayed_body {
        wait_delay(reply.delay, stop);
    }
    if !stop.load(Ordering::Acquire) {
        if reply.chunked {
            let _ = stream.write_all(format!("{:x}\r\n", reply.body.len()).as_bytes());
            let _ = stream.write_all(&reply.body);
            let _ = stream.write_all(b"\r\n0\r\n\r\n");
        } else {
            let _ = stream.write_all(&reply.body);
        }
    }
}

fn answer(text: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": text}}]})
}

fn config(endpoint: &str) -> OpenAiCompatibleConfig {
    OpenAiCompatibleConfig {
        identity: ProviderIdentity {
            id: "fixture-api".into(),
            version: "fixture-v1".into(),
        },
        endpoint: endpoint.into(),
        model: "fixture-model-not-a-real-service".into(),
        credential_reference: "fixture-key-reference".into(),
        enabled: true,
        allow_loopback_http: true,
        ..OpenAiCompatibleConfig::default()
    }
}

fn store() -> Arc<SessionSecretStore> {
    let store = Arc::new(SessionSecretStore::new());
    store
        .insert(
            "fixture-key-reference".into(),
            Secret::new(TEST_SECRET.into()).unwrap(),
        )
        .unwrap();
    store
}

fn request(provider: &OpenAiCompatibleProvider) -> TranslationRequest {
    TranslationRequest {
        text: "你好，世界🙂\n数量：12".into(),
        direction: TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        kind: TranslationKind::Sentence,
        context: TranslationContext {
            session: 1,
            epoch: 2,
            input_revision: 3,
        },
        send_authorized_to: Some(provider.authorization()),
        use_cache: false,
    }
}

fn token() -> TranslationCancellation {
    TranslationCancellation::new(Duration::from_secs(5))
}

#[test]
fn explicit_user_endpoint_handles_both_directions_auto_and_unicode() {
    let server = LocalServer::new(Reply::json(answer("fixture译文🙂\n12")));
    let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    for (index, source) in [Language::Chinese, Language::English, Language::Auto]
        .into_iter()
        .enumerate()
    {
        let mut request = request(&provider);
        request.kind = [
            TranslationKind::Word,
            TranslationKind::Phrase,
            TranslationKind::Sentence,
        ][index];
        request.direction.source = source;
        request.direction.target = if source == Language::English {
            Language::Chinese
        } else {
            Language::English
        };
        let output = provider.translate(&request, &token()).unwrap();
        assert_eq!(output.translated_text, "fixture译文🙂\n12");
        assert_eq!(output.output_kind, OutputKind::Translation);
        assert_eq!(output.quality, QualityStatus::Generated);
        assert_eq!(output.resolved_source, source);
        let body = server.body(index);
        assert_eq!(body["model"], "fixture-model-not-a-real-service");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][1]["content"], request.text);
    }
    assert_eq!(server.connections(), 3);
}

#[test]
fn a_failed_call_can_be_explicitly_requested_again_without_poisoning_the_provider() {
    let mut reply = Reply::json(answer("explicit-recovery-fixture"));
    reply.first_status = Some(503);
    let server = LocalServer::new(reply);
    let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::ProviderUnavailable)
    );
    assert_eq!(server.connections(), 1);
    assert_eq!(
        provider
            .translate(&request(&provider), &token())
            .unwrap()
            .translated_text,
        "explicit-recovery-fixture"
    );
    assert_eq!(server.connections(), 2);
}

#[test]
fn disabled_no_consent_missing_secret_and_offline_mode_make_zero_connections() {
    let server = LocalServer::new(Reply::json(answer("fixture")));
    let mut disabled = config(&server.endpoint);
    disabled.enabled = false;
    let provider = OpenAiCompatibleProvider::new(disabled, store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::ProviderUnavailable)
    );
    let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    let mut no_consent = request(&provider);
    no_consent.send_authorized_to = None;
    assert_eq!(
        provider.translate(&no_consent, &token()),
        Err(TranslationError::OnlineNotAuthorized)
    );
    let missing = OpenAiCompatibleProvider::new(
        config(&server.endpoint),
        Arc::new(SessionSecretStore::new()),
    )
    .unwrap();
    assert_eq!(
        missing.translate(&request(&missing), &token()),
        Err(TranslationError::MissingSecret)
    );
    let mut registry = ProviderRegistry::new();
    let request = request(&provider);
    registry
        .register(
            Arc::new(provider),
            TranslationProviderConfig {
                allow_network: true,
                ..TranslationProviderConfig::default()
            },
        )
        .unwrap();
    let mut service = TranslationService::new(registry, ServiceLimits::default()).unwrap();
    assert_eq!(
        service.submit("fixture-api", request),
        Err(TranslationError::OnlineNotAuthorized)
    );
    service.shutdown().unwrap();
    assert_eq!(server.connections(), 0);
}

#[test]
fn configuration_is_explicit_and_stale_consent_does_not_follow_an_endpoint_or_model_change() {
    assert!(!OpenAiCompatibleConfig::default().enabled);
    let server = LocalServer::new(Reply::json(answer("fixture")));
    let original = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    for changed in [
        "model",
        "endpoint",
        "credential",
        "version",
        "response-limit",
    ] {
        let mut config = config(&server.endpoint);
        match changed {
            "model" => config.model = "different-fixture-model".into(),
            "endpoint" => config.endpoint.push_str("/different-explicit-path"),
            "credential" => config.credential_reference = "other-ref".into(),
            "version" => config.config_version += 1,
            _ => config.max_response_bytes += 1,
        }
        let provider = OpenAiCompatibleProvider::new(config, store()).unwrap();
        assert_ne!(provider.identity(), original.identity());
        assert_eq!(
            provider.translate(&request(&original), &token()),
            Err(TranslationError::OnlineNotAuthorized)
        );
    }
    for endpoint in [
        "",
        "file:///tmp/request",
        "http://example.invalid/v1/chat/completions",
        "https://user:pass@example.invalid/v1/chat/completions",
        "https://example.invalid/v1/chat/completions?key=secret",
        "https://example.invalid/v1/chat/completions#fragment",
    ] {
        assert!(matches!(
            OpenAiCompatibleProvider::new(config(endpoint), store()),
            Err(TranslationError::InvalidConfig)
        ));
    }
    let mut config = config(&server.endpoint);
    config.allow_loopback_http = false;
    assert!(matches!(
        OpenAiCompatibleProvider::new(config, store()),
        Err(TranslationError::InvalidConfig)
    ));
    assert_eq!(server.connections(), 0);
}

#[test]
fn status_errors_have_no_response_body_and_do_not_automatically_retry() {
    for (status, retry, expected) in [
        (401, None, TranslationError::AuthenticationFailed),
        (403, None, TranslationError::AuthenticationFailed),
        (
            429,
            Some("7"),
            TranslationError::RateLimited {
                retry_after_seconds: Some(7),
            },
        ),
        (
            429,
            Some("999999999"),
            TranslationError::RateLimited {
                retry_after_seconds: None,
            },
        ),
        (
            429,
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            TranslationError::RateLimited {
                retry_after_seconds: None,
            },
        ),
        (503, None, TranslationError::ProviderUnavailable),
        (408, None, TranslationError::Timeout),
        (504, None, TranslationError::Timeout),
    ] {
        let mut reply = Reply::json(json!({"error": TEST_SECRET}));
        reply.status = status;
        if let Some(retry) = retry {
            reply.headers.push(("Retry-After".into(), retry.into()));
        }
        let server = LocalServer::new(reply);
        let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
        let result = provider.translate(&request(&provider), &token());
        assert_eq!(result, Err(expected));
        assert!(!format!("{result:?}").contains(TEST_SECRET));
        assert_eq!(server.connections(), 1);
    }
}

#[test]
fn redirect_is_not_followed_and_disconnect_is_classified() {
    let target = LocalServer::new(Reply::json(answer("must not be requested")));
    let mut reply = Reply::json(json!({}));
    reply.status = 302;
    reply
        .headers
        .push(("Location".into(), target.endpoint.clone()));
    let redirect = LocalServer::new(reply);
    let provider = OpenAiCompatibleProvider::new(config(&redirect.endpoint), store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::InvalidResponse)
    );
    assert_eq!(target.connections(), 0);
    let mut reply = Reply::json(json!({}));
    reply.disconnect = true;
    let disconnect = LocalServer::new(reply);
    let provider = OpenAiCompatibleProvider::new(config(&disconnect.endpoint), store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::NetworkUnavailable)
    );
    assert_eq!(disconnect.connections(), 1);
}

#[test]
fn timeout_and_cancellation_cover_headers_and_response_body() {
    for delayed_body in [false, true] {
        let mut reply = Reply::json(answer("late fixture"));
        reply.delay = Duration::from_secs(2);
        reply.delayed_body = delayed_body;
        let server = LocalServer::new(reply);
        let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
        let short = TranslationCancellation::new(Duration::from_secs(1));
        let request = request(&provider);
        let worker = thread::spawn(move || provider.translate(&request, &short));
        server.wait_for_request();
        assert_eq!(worker.join().unwrap(), Err(TranslationError::Timeout));
    }
    let mut reply = Reply::json(answer("late fixture"));
    reply.delay = Duration::from_secs(2);
    reply.delayed_body = true;
    let server = LocalServer::new(reply);
    let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    let cancellation = token();
    let worker_token = cancellation.clone();
    let request = request(&provider);
    let worker = thread::spawn(move || provider.translate(&request, &worker_token));
    server.wait_for_request();
    cancellation.cancel();
    assert_eq!(worker.join().unwrap(), Err(TranslationError::Cancelled));
}

#[test]
fn invalid_truncated_refused_and_tool_responses_never_become_translation() {
    let mut responses = vec![
        json!({}),
        json!({"choices": []}),
        json!({"choices": [{"finish_reason": "length", "message": {"role": "assistant", "content": "truncated"}}]}),
        json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": "text", "refusal": "refused"}}]}),
        json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": "text", "tool_calls": [{"function": "not executable"}]}}]}),
        answer("  "),
        answer("\0"),
    ];
    responses.push(json!({"choices": [{"finish_reason": "stop", "message": {"role": "user", "content": "not assistant"}}]}));
    for response in responses {
        let server = LocalServer::new(Reply::json(response));
        let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
        assert_eq!(
            provider.translate(&request(&provider), &token()),
            Err(TranslationError::InvalidResponse)
        );
    }
    let mut reply = Reply::json(json!({}));
    reply.body = b"not valid JSON".to_vec();
    let server = LocalServer::new(reply);
    let provider = OpenAiCompatibleProvider::new(config(&server.endpoint), store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::InvalidResponse)
    );
}

#[test]
fn response_caps_cover_both_content_length_and_chunked_transfer() {
    for chunked in [false, true] {
        let mut reply = Reply::json(answer(&"x".repeat(500)));
        reply.chunked = chunked;
        let server = LocalServer::new(reply);
        let mut config = config(&server.endpoint);
        config.max_response_bytes = 128;
        let provider = OpenAiCompatibleProvider::new(config, store()).unwrap();
        assert_eq!(
            provider.translate(&request(&provider), &token()),
            Err(TranslationError::ResponseTooLarge)
        );
    }
}

#[test]
fn input_and_wire_body_limits_reject_before_network() {
    let server = LocalServer::new(Reply::json(answer("fixture")));
    let mut limited = config(&server.endpoint);
    limited.max_input_bytes = 16;
    let provider = OpenAiCompatibleProvider::new(limited, store()).unwrap();
    let mut value = request(&provider);
    for (text, expected) in [
        (" ", TranslationError::InvalidInput),
        ("\0", TranslationError::InvalidInput),
        ("a very long request text", TranslationError::InputTooLong),
    ] {
        value.text = text.into();
        assert_eq!(provider.translate(&value, &token()), Err(expected));
    }
    let mut config = config(&server.endpoint);
    config.max_request_bytes = 10;
    let provider = OpenAiCompatibleProvider::new(config, store()).unwrap();
    assert_eq!(
        provider.translate(&request(&provider), &token()),
        Err(TranslationError::InputTooLong)
    );
    assert_eq!(server.connections(), 0);
}

#[test]
fn secrets_configuration_and_results_are_redacted_and_store_is_session_only() {
    let secret = Secret::new(TEST_SECRET.into()).unwrap();
    assert!(!format!("{secret:?}").contains(TEST_SECRET));
    let session_store = store();
    assert!(!format!("{session_store:?}").contains(TEST_SECRET));
    assert!(session_store.remove("fixture-key-reference").unwrap());
    assert!(matches!(
        session_store.get("fixture-key-reference"),
        Err(TranslationError::MissingSecret)
    ));
    for index in 0..32 {
        session_store
            .insert(
                format!("fixture-{index}"),
                Secret::new(TEST_SECRET.into()).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(
        session_store.insert(
            "over-budget".into(),
            Secret::new(TEST_SECRET.into()).unwrap()
        ),
        Err(TranslationError::Busy)
    );
    session_store.clear().unwrap();
    let server = LocalServer::new(Reply::json(answer("private-fixture-output")));
    let config = config(&server.endpoint);
    let debug = format!("{config:?}");
    assert!(!debug.contains(&server.endpoint));
    assert!(!debug.contains("fixture-key-reference"));
    let provider = OpenAiCompatibleProvider::new(config, store()).unwrap();
    let value = request(&provider);
    assert!(!format!("{value:?}").contains(&value.text));
    let output = provider.translate(&value, &token()).unwrap();
    assert!(!format!("{output:?}").contains("private-fixture-output"));
}
