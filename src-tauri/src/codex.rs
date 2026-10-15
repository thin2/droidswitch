//! App-owned CPA process. OAuth credentials never cross the Tauri boundary.
mod callback;
mod network;
pub mod quota;
use crate::{
    model::{
        entry::{ModelEntry, Protocol},
        ids::{ApiModelId, InternalId},
    },
    provider::{Credential, CredentialMode, ProviderConnection},
    workspace::Store,
};
use reqwest::{blocking::Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;

pub const PORT: u16 = 18317;
const BASE: &str = "http://127.0.0.1:18317";

// Platform-specific binary SHA256 hashes
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const BINARY_SHA: &str = "23427a5c7907005377e74161db94f7effda67d857ffc05708365a8e20d37f6f0";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const BINARY_SHA: &str = "";

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const BINARY_SHA: &str = "a3620a7e8d54d0f4257b493ea5fc01b3c59d581fb8c3596df5880976596da162";

#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
const BINARY_SHA: &str = "57e6bbb0edbf396c53abf4d46e00bf500dbb581efa426b6b99072c2f799fc1c9";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const BINARY_SHA: &str = "";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const BINARY_SHA: &str = "";
#[derive(Serialize, Deserialize)]
struct Keys {
    client: String,
    management: String,
    provider_id: InternalId,
}
struct Session {
    state: String,
    started: Instant,
    manual: bool,
    submitted: bool,
}
pub struct CodexService {
    inner: Mutex<Runtime>,
}
struct Runtime {
    root: PathBuf,
    binary: PathBuf,
    child: Option<Child>,
    session: Option<Session>,
    callback: Option<callback::Callback>,
    connect_after_login: bool,
    connection_error: Option<String>,
}
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    running: bool,
    login_pending: bool,
    manual_callback: bool,
    callback_submitted: bool,
    saved_login: bool,
    account: Option<String>,
    models: Vec<String>,
    endpoint: String,
    version: String,
    connected: bool,
    connection_error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Status,
    Start,
    Stop,
    Login,
    LoginManual,
    SubmitCallback,
    Cancel,
    Logout,
    AddModel,
}
impl CodexService {
    pub fn new(root: PathBuf, binary: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Runtime {
                root,
                binary,
                child: None,
                session: None,
                callback: None,
                connect_after_login: false,
                connection_error: None,
            }),
        }
    }
    pub fn shutdown(&self) {
        if let Ok(mut r) = self.inner.lock() {
            let _ = r.stop();
        }
    }
}
impl Drop for CodexService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| "无法创建 Codex 私有目录")?;
    crate::permissions::restrict_directory(path).map_err(|_| "无法保护 Codex 私有目录".into())
}
fn private_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("目录无效")?)
        .map_err(|_| "无法创建私有配置")?;
    crate::permissions::restrict_file(temp.path()).map_err(|_| "无法保护私有配置")?;
    temp.write_all(&serde_json::to_vec_pretty(value).map_err(|_| "配置编码失败")?)
        .map_err(|_| "写入私有配置失败")?;
    temp.persist(path).map_err(|_| "保存私有配置失败")?;
    Ok(())
}
fn proxy_config(root: &Path, keys: &Keys) -> Value {
    json!({"config-version":8,
        "server":{"host":"127.0.0.1","port":PORT,"commercial-mode":true,"discovery":{"enabled":false}},
        "management":{"allow-remote":false,"secret-key":keys.management,"disable-control-panel":true,"disable-auto-update-panel":true},
        "access":{"api-keys":[keys.client]},
        "oauth":{"auth-dir":root.join("auth"),"auth-auto-refresh-workers":1},
        "routing":{"strategy":"fill-first","retry":{"request-retry":0,"max-retry-credentials":1}},
        "quota-exceeded":{"switch-project":false,"switch-preview-model":false},
        "observability":{"logs":{"debug":false,"logging-to-file":false,"request-log":false},"usage":{"usage-statistics-enabled":false},"pprof":{"enable":false}}
    })
}
fn port_busy(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(250),
    )
    .is_ok()
}
fn client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(300))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "无法初始化本机连接".into())
}
fn api(keys: &Keys, method: Method, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let mut url =
        reqwest::Url::parse(&format!("{BASE}/v8/management/{path}")).map_err(|_| "本地地址无效")?;
    url.query_pairs_mut().extend_pairs(query.iter().copied());
    let response = client()?
        .request(method, url)
        .bearer_auth(&keys.management)
        .send()
        .map_err(|_| "本地代理未响应，请停止后重新启动")?;
    if !response.status().is_success() {
        return Err(format!(
            "本地代理操作失败（HTTP {}），请重试",
            response.status().as_u16()
        ));
    }
    response.json().map_err(|_| "代理响应格式不兼容".into())
}
fn auth_url(raw: &str, state: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "授权地址无效")?;
    if url.scheme() != "https"
        || url.host_str() != Some("auth.openai.com")
        || url.path() != "/oauth/authorize"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || state.is_empty()
    {
        return Err("已拒绝非预期的 OpenAI 授权地址".into());
    }
    let states: Vec<_> = url.query_pairs().filter(|(k, _)| k == "state").collect();
    if states.len() != 1 || states[0].1 != state {
        return Err("授权状态校验失败".into());
    }
    Ok(url)
}

// Return fixed messages only: upstream errors may contain codes, URLs or tokens.
fn oauth_failure(raw: &str) -> String {
    let message = if raw.contains("Failed to exchange authorization code for tokens") {
        let detail = raw.to_ascii_lowercase();
        if detail.contains("invalid_grant") {
            "授权码已失效或已被使用，请重新登录并提交新回调地址"
        } else if detail.contains("status 403") {
            "令牌交换被拒绝（HTTP 403），请检查代理线路及服务访问限制后重新登录"
        } else if detail.contains("status 401") {
            "令牌交换认证失败（HTTP 401），请重新登录"
        } else if detail.contains("status 429") {
            "令牌交换请求过于频繁（HTTP 429），请稍后重新登录"
        } else if detail.contains("connection refused") || detail.contains("proxyconnect") {
            "无法连接网络代理，请确认代理软件已运行、系统代理端口正确，再重新登录"
        } else if detail.contains("timeout") || detail.contains("deadline exceeded") {
            "连接授权服务器超时，请检查系统代理及线路后重新登录"
        } else if detail.contains("tls") || detail.contains("certificate") {
            "授权服务器 TLS 连接失败，请检查系统时间、代理线路与证书设置后重新登录"
        } else {
            "浏览器授权已返回，但代理换取登录令牌失败。请检查本地代理的网络连接后重新登录"
        }
    } else if raw.contains("Failed to save authentication tokens") {
        "已取得登录令牌，但保存失败。请检查应用私有目录的访问权限后重新登录"
    } else if raw.contains("Timeout") || raw.contains("expired") {
        "本次授权已超时或失效，请重新登录；旧回调地址不能用于新登录"
    } else if raw.contains("State") {
        "回调与本次登录不匹配，请使用当前登录窗口的回调地址"
    } else {
        "本次授权未完成，代理未保存登录凭据。请重新登录"
    };
    message.into()
}

fn callback_payload(raw: &str, state: &str) -> Result<Value, String> {
    if raw.len() > 16384 {
        return Err("回调地址过长".into());
    }
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| "请粘贴完整的本机回调地址")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        || !matches!(
            (url.port(), url.path()),
            (Some(1455), "/auth/callback") | (Some(PORT), "/codex/callback")
        )
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("只接受本次登录生成的 localhost 或 127.0.0.1 回调地址".into());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let one = |name: &str| -> Result<Option<String>, String> {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == name).collect();
        if values.len() > 1 {
            return Err("回调地址包含重复参数".into());
        }
        Ok(values.first().map(|(_, v)| v.to_string()))
    };
    if state.is_empty() || one("state")?.as_deref() != Some(state) {
        return Err("回调不属于本次登录；请勿使用之前的回调地址".into());
    }
    let code = one("code")?;
    let error = one("error")?;
    if error.is_some() || one("error_description")?.is_some() {
        return Err("浏览器返回授权拒绝或错误，请重新登录".into());
    }
    let code = code
        .filter(|s| !s.trim().is_empty())
        .ok_or("回调地址缺少授权码")?;
    Ok(json!({"provider":"codex", "state":state, "code":code}))
}
fn open_login(url: &reqwest::Url) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let program = "xdg-open";
    Command::new(program)
        .arg(url.as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法打开系统浏览器，请检查默认浏览器")?;
    Ok(())
}
fn accounts(value: &Value) -> Result<Vec<&Value>, String> {
    let files = value["files"].as_array().ok_or("代理账号目录格式不兼容")?;
    if files.len() > 1 || files.iter().any(|f| f["provider"] != "codex") {
        return Err("私有代理目录包含多个账号或非 Codex 凭据，已停止服务以避免自动切换".into());
    }
    Ok(files.iter().collect())
}
impl Runtime {
    fn keys(&self) -> Result<Keys, String> {
        private_dir(&self.root)?;
        let path = self.root.join("keys.json");
        if path.exists() {
            crate::permissions::restrict_file(&path).map_err(|_| "无法保护本地密钥")?;
            return serde_json::from_slice(&fs::read(path).map_err(|_| "无法读取本地密钥")?)
                .map_err(|_| "本地密钥文件损坏，请保留目录并修复".into());
        }
        let keys = Keys {
            client: uuid::Uuid::new_v4().to_string(),
            management: format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4()),
            provider_id: InternalId::new(),
        };
        private_json(&path, &keys)?;
        Ok(keys)
    }
    fn alive(&mut self) -> Result<bool, String> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().map_err(|_| "无法检查本地服务")?.is_none() {
                return Ok(true);
            }
            self.child = None;
            self.session = None;
        }
        Ok(false)
    }
    fn stop(&mut self) -> Result<(), String> {
        self.callback = None;
        self.connect_after_login = false;
        let pending = self.session.is_some() || self.root.join("login-pending").exists();
        if let Some(mut child) = self.child.take() {
            if child.try_wait().map_err(|_| "无法检查代理进程")?.is_none() {
                if child.kill().is_err() {
                    self.child = Some(child);
                    return Err("无法停止本地代理，请重试".into());
                }
                child.wait().map_err(|_| "无法确认代理退出")?;
            }
        }
        self.session = None;
        if pending {
            self.clear_auth()?;
        }
        Ok(())
    }
    fn clear_auth(&self) -> Result<(), String> {
        let path = self.root.join("auth");
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|_| "无法清理本次授权，请退出账号后重试")?;
        }
        let marker = self.root.join("login-pending");
        if marker.exists() {
            fs::remove_file(marker).map_err(|_| "无法清理授权状态")?;
        }
        private_dir(&path)
    }
    fn saved_login(&self) -> bool {
        fs::read_dir(self.root.join("auth"))
            .ok()
            .is_some_and(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|e| e.path().extension().is_some_and(|x| x == "json"))
            })
    }
    fn start(&mut self, keys: &Keys) -> Result<(), String> {
        if self.alive()? {
            return Ok(());
        }
        // Check if binary exists
        if !self.binary.exists() {
            return Err("缺少内置 Codex 代理组件，请重新安装完整应用或运行平台对应的准备脚本".into());
        }
        let bytes =
            fs::read(&self.binary).map_err(|_| "无法读取 Codex 代理组件")?;
        
        // Verify binary checksum if available for this platform
        if !BINARY_SHA.is_empty() {
            let hash = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            if hash != BINARY_SHA {
                return Err("内置代理校验失败，请重新安装应用".into());
            }
        }
        
        if port_busy(PORT) {
            return Err("端口 18317 已被占用；请停止冲突程序后重试".into());
        }
        if self.root.join("login-pending").exists() {
            self.clear_auth()?;
        }
        private_dir(&self.root.join("auth"))?;
        // Refuse pre-existing multi-account pools before listening.
        let auth_count = fs::read_dir(self.root.join("auth"))
            .map_err(|_| "无法读取授权目录")?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .count();
        if auth_count > 1 {
            return Err("授权目录中存在多个账号，请先退出本功能账号".into());
        }
        for entry in fs::read_dir(self.root.join("auth")).map_err(|_| "无法读取授权目录")? {
            let path = entry.map_err(|_| "无法读取授权目录")?.path();
            if path.extension().is_some_and(|x| x == "json") {
                crate::permissions::restrict_file(&path).map_err(|_| "无法保护授权文件")?;
                let data: Value =
                    serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取授权文件")?)
                        .map_err(|_| "授权文件损坏，请退出账号后重新登录")?;
                if data["type"] != "codex" {
                    return Err("授权目录包含非 Codex 凭据，请先退出本功能账号".into());
                }
            }
        }
        let mut config = proxy_config(&self.root, keys);
        if let Some(proxy) = network::system_proxy()? {
            config["requests"] = json!({"proxy-url":proxy});
        }
        private_json(&self.root.join("config.yaml"), &config)?;
        let mut command = Command::new(&self.binary);
        command
            .args(["--config"])
            .arg(self.root.join("config.yaml"))
            .arg("--local-model")
            .current_dir(&self.root)
            .env_clear();
        // Never inherit cloud stores, API keys, dotenv paths or management passwords.
        for key in [
            "PATH",
            "HOME",
            "USERPROFILE",
            "TMPDIR",
            "TEMP",
            "TMP",
            "SYSTEMROOT",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.child = Some(command.spawn().map_err(|_| "无法启动内置代理")?);
        for _ in 0..30 {
            if !self.alive()? {
                return Err("本地代理启动失败，请检查端口或组件版本".into());
            }
            if api(keys, Method::GET, "credentials", &[]).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.stop()?;
        Err("本地代理启动超时".into())
    }
    fn status(&mut self, keys: &Keys) -> Result<Status, String> {
        let result = self.read_status(keys);
        if let Err(error) = &result {
            self.connection_error = Some(error.clone());
        }
        result
    }
    fn authorization_failed(&mut self, message: String) -> String {
        // Keep the outcome even when the background monitor observes it first.
        self.connection_error = Some(message.clone());
        match self.stop() {
            Ok(()) => message,
            Err(_) => format!("{message}；停止本地服务失败，请退出应用后重试"),
        }
    }
    fn read_status(&mut self, keys: &Keys) -> Result<Status, String> {
        let mut view = Status {
            endpoint: format!("{BASE}/v1"),
            version: "CLIProxyAPI 8.0.27".into(),
            saved_login: self.saved_login(),
            connection_error: self.connection_error.clone(),
            ..Default::default()
        };
        if !self.alive()? {
            return Ok(view);
        }
        view.running = true;
        if let Some(s) = &self.session {
            if s.started.elapsed() > Duration::from_secs(300) {
                self.cancel(keys)?;
                return Err("授权已超时，请重新登录".into());
            }
            let poll = api(keys, Method::GET, "oauth/status", &[("state", &s.state)])?;
            match poll["status"].as_str() {
                Some("ok") => {
                    let marker = self.root.join("login-pending");
                    if marker.exists() {
                        fs::remove_file(marker).map_err(|_| "无法保存授权完成状态")?;
                    }
                    self.session = None;
                    self.callback = None;
                }
                Some("wait") => {
                    view.login_pending = true;
                    view.manual_callback = s.manual;
                    view.callback_submitted = s.submitted;
                }
                _ => {
                    let error = oauth_failure(poll["error"].as_str().unwrap_or(""));
                    return Err(self.authorization_failed(error));
                }
            }
        }
        let data = api(keys, Method::GET, "credentials", &[])?;
        let entries = match accounts(&data) {
            Ok(a) => a,
            Err(e) => {
                self.stop()?;
                return Err(e);
            }
        };
        if let Some(account) = entries.first() {
            view.account = Some(
                account["email"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("ChatGPT 账号")
                    .chars()
                    .take(160)
                    .collect(),
            );
            let response = client()?
                .get(format!("{BASE}/v1/models"))
                .bearer_auth(&keys.client)
                .send()
                .map_err(|_| "无法读取本地模型目录")?;
            if !response.status().is_success() {
                return Err("模型目录暂不可用，请重新启动或登录".into());
            }
            let models: Value = response.json().map_err(|_| "模型目录格式无效")?;
            view.models = models["data"]
                .as_array()
                .ok_or("模型目录格式无效")?
                .iter()
                .filter_map(|v| v["id"].as_str())
                .filter(|id| !id.is_empty() && id.len() <= 200)
                .map(str::to_owned)
                .collect();
            view.models.sort();
            view.models.dedup();
        }
        Ok(view)
    }
    fn cancel(&mut self, keys: &Keys) -> Result<(), String> {
        if let Some(session) = &self.session {
            let _ = api(
                keys,
                Method::DELETE,
                "oauth/session",
                &[("state", &session.state)],
            );
            // Termination closes callback listener and prevents a late save after cancellation.
            self.stop()?;
            self.start(keys)?;
        }
        Ok(())
    }
    fn submit_callback(&mut self, keys: &Keys, raw: &str) -> Result<(), String> {
        let session = self
            .session
            .as_ref()
            .ok_or("没有待完成的登录，请先点击登录")?;
        if !session.manual {
            return Err("当前为自动回调模式；请取消后选择手动粘贴重新登录".into());
        }
        if session.started.elapsed() > Duration::from_secs(300) {
            return Err("本次登录已超时，请重新登录".into());
        }
        let payload = callback_payload(raw, &session.state)?;
        if session.submitted {
            return Err("回调已提交，正在等待代理完成授权，请勿重复提交".into());
        }
        // Never navigate to a pasted URL; submit validated fields to our fixed loopback endpoint.
        let response = client()?
            .post(format!("{BASE}/v8/management/oauth/callback"))
            .bearer_auth(&keys.management)
            .json(&payload)
            .send()
            .map_err(|_| "回调提交结果未确认，请刷新状态，勿立即重复提交")?;
        if !response.status().is_success() {
            return Err(format!(
                "回调提交失败（HTTP {}），请刷新状态或重新登录",
                response.status().as_u16()
            ));
        }
        if let Some(session) = self.session.as_mut() {
            session.submitted = true;
        }
        self.connection_error = None;
        Ok(())
    }
    fn login(&mut self, keys: &Keys, manual: bool) -> Result<(), String> {
        self.start(keys)?;
        let state = self.status(keys)?;
        if state.account.is_some() {
            return Err("已登录账号；更换账号请先退出".into());
        }
        if self.session.is_some() {
            return Err("已有授权正在进行，请先完成或取消".into());
        }
        let listener = if manual {
            None
        } else {
            Some(callback::bind()?)
        };
        private_json(&self.root.join("login-pending"), &true)?;
        let reply = api(
            keys,
            Method::GET,
            "oauth/auth-url",
            &[("provider", "codex")],
        )?;
        let state = reply["state"]
            .as_str()
            .ok_or("代理未返回授权状态")?
            .to_owned();
        self.session = Some(Session {
            state: state.clone(),
            started: Instant::now(),
            manual,
            submitted: false,
        });
        self.callback = listener
            .map(|l| callback::Callback::start(l, state.clone()))
            .transpose()?;
        let result =
            auth_url(reply["url"].as_str().unwrap_or(""), &state).and_then(|url| open_login(&url));
        if result.is_err() {
            self.stop()?;
        }
        result
    }
}
fn add_catalog(
    w: &mut crate::workspace::Workspace,
    keys: &Keys,
    models: &[String],
    selected: Option<&str>,
) -> Result<(), String> {
    if models.is_empty() {
        return Err("已登录，但模型目录为空，请稍后点击接入 Droid 重试".into());
    }
    let provider = ProviderConnection {
        id: keys.provider_id,
        name: "ChatGPT 本地授权".into(),
        base_url: format!("{BASE}/v1"),
        protocol: Protocol::OpenAiResponses,
        credential: Credential {
            mode: CredentialMode::Direct,
            value: keys.client.clone(),
        },
        model_list_url: None,
        extra_headers: Default::default(),
    };
    if let Some(p) = w
        .catalog
        .providers
        .iter_mut()
        .find(|p| p.id == keys.provider_id)
    {
        *p = provider;
    } else {
        w.catalog.providers.push(provider);
    }
    for model in models {
        let api_id = ApiModelId::try_from(model.clone())?;
        if let Some(entry) = w
            .catalog
            .models
            .iter_mut()
            .find(|m| m.provider_id == keys.provider_id && m.api_model_id == api_id)
        {
            entry.enabled = true;
        } else {
            let mut entry = ModelEntry::new(keys.provider_id, api_id, format!("{model} (ChatGPT)"));
            entry.manufacturer = Some("openai".into());
            w.catalog.models.push(entry);
        }
    }
    let previous = w
        .catalog
        .models
        .iter()
        .find(|m| Some(m.id) == w.scheme.api.model_id && m.provider_id == keys.provider_id)
        .map(|m| String::from(m.api_model_id.clone()));
    let selected = selected
        .or(previous.as_deref())
        .filter(|id| models.iter().any(|m| m == id))
        .unwrap_or(&models[0]);
    let id = w
        .catalog
        .models
        .iter()
        .find(|m| {
            m.provider_id == keys.provider_id && String::from(m.api_model_id.clone()) == selected
        })
        .ok_or("默认模型不存在")?
        .id;
    w.scheme.source = Some(crate::switch::scheme::Source::Api);
    w.scheme.api.model_id = Some(id);
    w.scheme.api.reasoning = None;
    Ok(())
}
fn connect_droid(
    store: &Store,
    keys: &Keys,
    status: &Status,
    selected: Option<&str>,
) -> Result<(), String> {
    if status.account.is_none() || !status.running {
        return Err("请先登录并启动服务".into());
    }
    if selected.is_some_and(|s| !status.models.iter().any(|m| m == s)) {
        return Err("请选择当前目录中的模型".into());
    }
    crate::config::pipeline::apply_direct(store, |w| {
        add_catalog(w, keys, &status.models, selected)
    })?;
    Ok(())
}
/// Finish sign-in even if the user navigates away from the authorization page.
pub fn monitor(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let service = app.state::<CodexService>();
        let pending = service
            .inner
            .try_lock()
            .map(|r| r.session.is_some() || r.connect_after_login)
            .unwrap_or(false);
        if pending {
            let _ = tauri::async_runtime::block_on(codex_action(
                app.clone(),
                Action::Status,
                None,
                None,
                None,
            ));
        }
    });
}
#[tauri::command]
pub async fn codex_action(
    app: tauri::AppHandle,
    action: Action,
    model: Option<String>,
    revision: Option<u64>,
    callback_url: Option<String>,
) -> Result<Status, String> {
    let _ = revision;
    tauri::async_runtime::spawn_blocking(move || {
        let service = app.state::<CodexService>();
        let mut runtime = service.inner.lock().map_err(|_| "Codex 服务繁忙")?;
        let keys = runtime.keys()?;
        match action {
            Action::Status => {}
            Action::Start => {
                runtime.start(&keys)?;
                runtime.connect_after_login = true;
            }
            Action::Stop => runtime.stop()?,
            Action::Login | Action::LoginManual => {
                let manual = matches!(action, Action::LoginManual);
                if let Err(error) = runtime.login(&keys, manual) {
                    if runtime.root.join("login-pending").exists() {
                        runtime.stop()?;
                    }
                    return Err(error);
                }
                runtime.connect_after_login = true;
                runtime.connection_error = None;
            }
            Action::SubmitCallback => {
                runtime.submit_callback(&keys, callback_url.as_deref().ok_or("请粘贴回调地址")?)?;
            }
            Action::Cancel => runtime.cancel(&keys)?,
            Action::Logout => {
                runtime.stop()?;
                let auth = runtime.root.join("auth");
                if auth.exists() {
                    fs::remove_dir_all(&auth).map_err(|_| "本地授权移除失败，请重试")?;
                }
                private_dir(&auth)?;
            }
            Action::AddModel => {
                let status = runtime.status(&keys)?;
                connect_droid(&app.state::<Store>(), &keys, &status, model.as_deref())?;
                runtime.connection_error = None;
            }
        }
        let mut status = runtime.status(&keys)?;
        if runtime.connect_after_login && status.account.is_some() && !status.login_pending {
            runtime.connect_after_login = false;
            runtime.connection_error =
                connect_droid(&app.state::<Store>(), &keys, &status, None).err();
        }
        let w = app.state::<Store>().load()?;
        status.connected = w
            .applied_catalog
            .models
            .iter()
            .any(|m| m.provider_id == keys.provider_id && m.enabled);
        status.connection_error = runtime.connection_error.clone();
        Ok(status)
    })
    .await
    .map_err(|_| "Codex 操作未完成")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_manual_callback_validates_origin_state_and_ambiguous_parameters() {
        let good = "http://localhost:1455/auth/callback?state=current&code=synthetic";
        assert_eq!(
            callback_payload(good, "current").unwrap(),
            json!({
                "provider":"codex", "state":"current", "code":"synthetic"
            })
        );
        assert!(callback_payload(
            "http://127.0.0.1:18317/codex/callback?state=current&code=synthetic",
            "current"
        )
        .is_ok());
        for bad in [
            good.replace("localhost", "evil.test"),
            good.replace("1455", "9999"),
            good.replace("current", "previous"),
            good.replace("code=synthetic", "code="),
            format!("{good}&state=current"),
            format!("{good}&code=second"),
            format!("{good}&error=access_denied"),
            format!("{good}#fragment"),
            good.replace("localhost", "user@localhost"),
            good.replace("http:", "https:"),
        ] {
            let error = callback_payload(&bad, "current").unwrap_err();
            assert!(!error.contains("synthetic"));
        }
    }
    #[test]
    fn codex_background_failure_survives_status_refresh_without_upstream_secrets() {
        let root = tempfile::tempdir().unwrap();
        let service = CodexService::new(root.path().to_path_buf(), PathBuf::new());
        let mut r = service.inner.lock().unwrap();
        let keys = r.keys().unwrap();
        r.session = Some(Session {
            state: "test".into(),
            started: Instant::now(),
            manual: true,
            submitted: true,
        });
        r.connect_after_login = true;
        let message = oauth_failure(
            "Failed to exchange authorization code for tokens: code=SECRET access_token=SECRET",
        );
        let failure = r.authorization_failed(message.clone());
        assert_eq!(failure, message);
        assert!(!failure.contains("SECRET"));
        assert!(failure.contains("换取登录令牌失败"));
        for _ in 0..2 {
            let status = r.status(&keys).unwrap();
            assert_eq!(status.connection_error.as_deref(), Some(message.as_str()));
            assert!(!status.login_pending);
            assert!(!status.running);
        }
        assert!(!r.connect_after_login);
        assert!(r
            .submit_callback(
                &keys,
                "http://localhost:1455/auth/callback?state=test&code=synthetic"
            )
            .is_err());
    }
    #[test]
    fn codex_manual_callback_rejects_expired_and_repeated_submissions_before_network() {
        let root = tempfile::tempdir().unwrap();
        let service = CodexService::new(root.path().to_path_buf(), PathBuf::new());
        let mut r = service.inner.lock().unwrap();
        let keys = r.keys().unwrap();
        let callback = "http://localhost:1455/auth/callback?state=test&code=synthetic";
        r.session = Some(Session {
            state: "test".into(),
            started: Instant::now(),
            manual: true,
            submitted: true,
        });
        assert!(r
            .submit_callback(&keys, callback)
            .unwrap_err()
            .contains("请勿重复提交"));
        r.session.as_mut().unwrap().started = Instant::now() - Duration::from_secs(301);
        assert!(r
            .submit_callback(&keys, callback)
            .unwrap_err()
            .contains("超时"));
    }
    #[test]
    fn codex_direct_catalog_is_idempotent_and_sets_default_without_oauth_secrets() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path()).unwrap();
        let mut w = store.load().unwrap();
        let keys = Keys {
            client: "local-client".into(),
            management: "never-export-this".into(),
            provider_id: InternalId::new(),
        };
        let models = vec!["model-a".into(), "model-b".into()];
        add_catalog(&mut w, &keys, &models, None).unwrap();
        assert_eq!(w.catalog.models.len(), 2);
        let first_id = w.catalog.models[0].id;
        assert_eq!(w.scheme.api.model_id, Some(first_id));
        add_catalog(&mut w, &keys, &models, Some("model-b")).unwrap();
        assert_eq!(w.catalog.providers.len(), 1);
        assert_eq!(w.catalog.models.len(), 2);
        assert_eq!(w.catalog.models[0].id, first_id);
        assert_eq!(w.scheme.api.model_id, Some(w.catalog.models[1].id));
        let serialized = serde_json::to_string(&w).unwrap();
        assert!(!serialized.contains(&keys.management));
        assert!(serialized.contains("(ChatGPT)"));
    }
    #[test]
    #[ignore = "Requires loopback permission and the checksum-pinned macOS binary"]
    fn codex_real_sidecar_lifecycle() {
        let root = tempfile::tempdir().unwrap();
        let mut r = Runtime {
            root: root.path().to_path_buf(),
            binary: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/codex/cli-proxy-api"),
            child: None,
            session: None,
            callback: None,
            connect_after_login: false,
            connection_error: None,
        };
        let keys = r.keys().unwrap();
        let result = (|| -> Result<(), String> {
            r.start(&keys)?;
            assert!(r.status(&keys)?.running);
            assert!(r.status(&keys)?.account.is_none());
            let blocker = std::net::TcpListener::bind(("127.0.0.1", 1455))
                .map_err(|_| "callback port busy")?;
            assert!(r.login(&keys, false).unwrap_err().contains("1455"));
            drop(blocker);
            let listener = callback::bind()?;
            let login = api(
                &keys,
                Method::GET,
                "oauth/auth-url",
                &[("provider", "codex")],
            )?;
            let state = login["state"].as_str().ok_or("missing state")?.to_owned();
            assert!(auth_url(login["url"].as_str().unwrap(), &state).is_ok());
            assert_eq!(
                listener.local_addr().unwrap().ip(),
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            );
            r.callback = Some(callback::Callback::start(listener, state.clone())?);
            r.session = Some(Session {
                state,
                started: Instant::now(),
                manual: false,
                submitted: false,
            });
            assert!(r.status(&keys)?.login_pending);
            let rejected = client()?
                .get("http://127.0.0.1:1455/auth/callback?state=wrong&code=test")
                .send()
                .map_err(|_| "test callback failed")?;
            assert_eq!(rejected.status().as_u16(), 400);
            let mut callback_url =
                reqwest::Url::parse("http://localhost:1455/auth/callback").unwrap();
            callback_url
                .query_pairs_mut()
                .append_pair("state", &r.session.as_ref().unwrap().state)
                .append_pair("code", "test-only-not-exchanged");
            let redirect = client()?
                .get(callback_url)
                .send()
                .map_err(|_| "test relay failed")?;
            assert_eq!(redirect.status().as_u16(), 302);
            assert!(redirect.headers()["location"]
                .to_str()
                .unwrap()
                .starts_with("http://127.0.0.1:18317/codex/callback?"));
            private_json(&r.root.join("login-pending"), &true)?;

            r.cancel(&keys)?;
            assert!(!r.status(&keys)?.login_pending);
            assert!(!port_busy(1455));
            assert!(!r.saved_login());
            r.stop()?;
            assert!(!port_busy(PORT));
            r.start(&keys)?;
            r.stop()?;
            let listener =
                std::net::TcpListener::bind(("127.0.0.1", PORT)).map_err(|_| "test bind")?;
            assert!(r.start(&keys).unwrap_err().contains("占用"));
            drop(listener);
            Ok(())
        })();
        let _ = r.stop();
        result.unwrap();
    }
    #[test]
    fn codex_cancel_cleans_late_callback_credentials() {
        let root = tempfile::tempdir().unwrap();
        private_dir(&root.path().join("auth")).unwrap();
        fs::write(root.path().join("auth/late.json"), "{}").unwrap();
        let mut r = Runtime {
            root: root.path().to_path_buf(),
            binary: PathBuf::new(),
            child: None,
            callback: None,
            connect_after_login: false,
            connection_error: None,
            session: Some(Session {
                state: "test".into(),
                started: Instant::now(),
                manual: false,
                submitted: false,
            }),
        };
        r.stop().unwrap();
        assert!(!r.saved_login());
    }
    #[test]
    fn codex_rejects_untrusted_auth_urls() {
        assert!(auth_url("https://auth.openai.com/oauth/authorize?state=abc", "abc").is_ok());
        for url in [
            "http://auth.openai.com/oauth/authorize?state=abc",
            "https://auth.openai.com.evil.test/oauth/authorize?state=abc",
            "https://evil@auth.openai.com/oauth/authorize?state=abc",
            "https://auth.openai.com/oauth/authorize?state=wrong",
            "https://auth.openai.com/oauth/authorize?state=abc&state=abc",
        ] {
            assert!(auth_url(url, "abc").is_err());
        }
    }
    #[test]
    fn codex_private_config_and_keys() {
        let root = tempfile::tempdir().unwrap();
        let r = Runtime {
            root: root.path().join("codex"),
            binary: PathBuf::new(),
            child: None,
            session: None,
            callback: None,
            connect_after_login: false,
            connection_error: None,
        };
        let keys = r.keys().unwrap();
        assert_ne!(keys.client, keys.management);
        assert_eq!(keys.client, r.keys().unwrap().client);
        let config = proxy_config(&r.root, &keys);
        assert_eq!(config["server"]["host"], "127.0.0.1");
        assert_eq!(config["server"]["commercial-mode"], true);
        assert_eq!(config["management"]["allow-remote"], false);
        assert_eq!(config["routing"]["retry"]["max-retry-credentials"], 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(r.root.join("keys.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn codex_refuses_account_pools() {
        assert!(accounts(&json!({"files":[]})).unwrap().is_empty());
        assert!(accounts(&json!({"files":[{"provider":"codex"}]})).is_ok());
        assert!(accounts(&json!({"files":[{"provider":"codex"},{"provider":"codex"}]})).is_err());
        assert!(accounts(&json!({"files":[{"provider":"claude"}]})).is_err());
        let view = serde_json::to_string(&Status::default()).unwrap();
        assert!(!view.contains("token"));
        assert!(!view.contains("management"));
    }
}
