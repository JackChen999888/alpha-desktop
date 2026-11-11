//! Additional read-only mail connections. Credentials never enter JSON config.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
const SCOPE: &str = "https://graph.microsoft.com/Mail.Read offline_access";
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum Kind {
    Outlook,
    Proton,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub client_id: String,
    pub username: String,
    pub port: u16,
    pub certificate: String,
    pub starttls: bool,
}
impl Account {
    fn new(kind: Kind) -> Self {
        Self {
            id: chrono::Utc::now().timestamp_micros().to_string(),
            label: if kind == Kind::Outlook {
                "Outlook"
            } else {
                "Proton Bridge"
            }
            .into(),
            kind,
            client_id: String::new(),
            username: String::new(),
            port: 1143,
            certificate: String::new(),
            starttls: true,
        }
    }
}
#[derive(Clone, Default)]
pub struct Message {
    pub subject: String,
    pub from: String,
    pub date: String,
    pub preview: String,
    pub unread: bool,
}
enum Update {
    Hint(String),
    Checked(Result<(), String>),
    Done(Result<Vec<Message>, String>),
}
struct Check {
    account: Account,
    at: i64,
    error: Option<String>,
}
pub struct Mail {
    accounts: Vec<Account>,
    selected: usize,
    password: String,
    rx: Option<mpsc::Receiver<Update>>,
    cancel: Arc<AtomicBool>,
    messages: Vec<Message>,
    status: String,
    search: String,
    active: Option<Account>,
    checks: std::collections::HashMap<String,Check>,
    pending_delete: Option<String>,
    visible_kind: Option<Kind>,
}
impl Default for Mail {
    fn default() -> Self {
        Self {
            accounts: super::load(&super::root().join("mail.json")),
            selected: 0,
            password: String::new(),
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            messages: Vec::new(),
            status: String::new(),
            search: String::new(),
            active: None,
            checks: Default::default(),
            pending_delete: None,
            visible_kind: None,
        }
    }
}
impl Drop for Mail {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn credential(a: &Account) -> Result<keyring::Entry, String> {
    keyring::Entry::new("DesktopIslandPro.MailExtra", &a.id)
        .map_err(|_| "无法打开凭据管理器".into())
}
fn validate_config(a: &Account) -> Result<(), String> {
    if a.kind == Kind::Outlook {
        let id = a.client_id.trim().as_bytes();
        if id.len() != 36 || !id.iter().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { *b == b'-' } else { b.is_ascii_hexdigit() }
        }) {
            return Err("Client ID 格式不正确：请复制 Microsoft 应用的完整 GUID（8-4-4-4-12），不是邮箱地址或密码".into());
        }
    } else {
        if a.port == 0 || a.username.trim().is_empty() {
            return Err("请填写 Bridge 提供的用户名和 IMAP 端口".into());
        }
        let path = std::path::Path::new(a.certificate.trim());
        if !path.is_absolute() || !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= 100_000) {
            return Err("请选择从 Proton Bridge 导出的 PEM 证书完整路径".into());
        }
        let bytes = std::fs::read(path).map_err(|_| "无法读取 Bridge 证书")?;
        native_tls::Certificate::from_pem(&bytes).map_err(|_| "Bridge 证书不是有效的 PEM 文件")?;
    }
    Ok(())
}
fn graph_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(401, _) => "Outlook 授权已失效，请重新登录 Outlook".into(),
        ureq::Error::Status(403, _) => "Outlook 拒绝访问（403）：请确认 Mail.Read 委托权限已授权；组织账号还可能受管理员策略限制".into(),
        ureq::Error::Status(429, _) => "Outlook 请求过于频繁（429），请稍后重试".into(),
        ureq::Error::Status(c, _) => format!("Outlook 服务返回 HTTP {c}，请稍后重试"),
        _ => "无法连接 Outlook，请检查网络或代理后重试".into(),
    }
}
fn oauth_hint(code: &str) -> String {
    match code {
        "invalid_grant" | "interaction_required" | "consent_required" => "Microsoft 授权已过期、被撤销或需要重新确认，请点击登录 Outlook 重新授权。",
        "invalid_client" | "unauthorized_client" => "Microsoft 应用配置不匹配，请核对 Client ID、个人账号支持及公共客户端流设置。",
        "authorization_declined" | "access_denied" => "本次 Microsoft 授权未获同意，可点击登录 Outlook 重新开始。",
        "expired_token" => "Microsoft 登录码已过期，请点击登录 Outlook 获取新代码。",
        "temporarily_unavailable" | "server_error" => "Microsoft 授权服务暂不可用，请稍后重试。",
        _ => "Microsoft 授权未完成，请检查网络或重新登录 Outlook。",
    }.into()
}
fn client() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(12))
        .timeout_read(Duration::from_secs(25))
        .build()
}
fn decode(r: ureq::Response) -> Result<Value, String> {
    let mut body = String::new();
    r.into_reader()
        .take(2_000_000)
        .read_to_string(&mut body)
        .map_err(|_| "读取响应失败")?;
    serde_json::from_str(&body).map_err(|_| "服务器响应格式无效".into())
}
fn token_request(fields: &[(&str, &str)]) -> Result<Value, String> {
    match client()
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .send_form(fields)
    {
        Ok(r) => decode(r),
        Err(ureq::Error::Status(_, r)) => {
            let e = decode(r)?;
            Err(e["error"].as_str().unwrap_or("授权失败").to_string())
        }
        Err(_) => Err("连接微软登录服务失败".into()),
    }
}
fn save_token(a: &Account, mut v: Value, old_refresh: Option<&str>) -> Result<String, String> {
    let access = v["access_token"]
        .as_str()
        .ok_or("缺少访问令牌")?
        .to_string();
    if v["refresh_token"].as_str().is_none() {
        if let Some(r) = old_refresh {
            v["refresh_token"] = json!(r);
        }
    }
    v["expires_at"] =
        json!(chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600));
    credential(a)?
        .set_password(&v.to_string())
        .map_err(|_| "保存令牌失败")?;
    Ok(access)
}
fn access(a: &Account) -> Result<String, String> {
    let raw = credential(a)?
        .get_password()
        .map_err(|_| "请先登录 Outlook")?;
    let v: Value = serde_json::from_str(&raw).map_err(|_| "令牌无效，请重新登录")?;
    if v["expires_at"].as_i64().unwrap_or(0) > chrono::Utc::now().timestamp() + 90 {
        return v["access_token"]
            .as_str()
            .map(str::to_string)
            .ok_or("请重新登录".into());
    }
    let refresh = v["refresh_token"]
        .as_str()
        .ok_or("登录已过期，请重新登录")?;
    let next = token_request(&[
        ("client_id", &a.client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("scope", SCOPE),
    ]).map_err(|e|oauth_hint(&e))?;
    save_token(a, next, Some(refresh))
}
fn login(a: &Account, tx: &mpsc::Sender<Update>, cancel: &AtomicBool) -> Result<(), String> {
    validate_config(a)?;
    let r = client()
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/devicecode")
        .send_form(&[("client_id", a.client_id.as_str()), ("scope", SCOPE)])
        .map_err(|_| "无法申请登录码，请检查 Client ID 和公共客户端配置")?;
    let v = decode(r)?;
    let code = v["device_code"].as_str().ok_or("未返回登录码")?;
    let user = v["user_code"].as_str().ok_or("未返回用户代码")?;
    let _=tx.send(Update::Hint(format!("打开 https://microsoft.com/devicelogin ，输入代码 {user} 并授权只读邮件。请核对应用名称是你注册的Alpha。")));
    let start = std::time::Instant::now();
    let mut interval = v["interval"].as_u64().unwrap_or(5).max(5);
    let expires = v["expires_in"].as_u64().unwrap_or(900).min(900);
    while start.elapsed().as_secs() < expires {
        for _ in 0..interval * 4 {
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消登录".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        match token_request(&[
            ("client_id", &a.client_id),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", code),
        ]) {
            Ok(v) => {
                if cancel.load(Ordering::Relaxed) { return Err("已取消登录".into()); }
                save_token(a, v, None)?;
                return Ok(());
            }
            Err(e) if e == "authorization_pending" => {}
            Err(e) if e == "slow_down" => interval = (interval + 5).min(30),
            Err(e) => return Err(oauth_hint(&e)),
        }
    }
    Err("登录码已过期，请重新登录".into())
}
pub fn parse_graph(v: &Value) -> Result<Vec<Message>, String> {
    let rows = v["value"].as_array().ok_or("邮件响应缺少列表")?;
    Ok(rows
        .iter()
        .take(30)
        .map(|m| Message {
            subject: m["subject"].as_str().unwrap_or("（无主题）").into(),
            from: m["from"]["emailAddress"]["address"]
                .as_str()
                .unwrap_or("")
                .into(),
            date: m["receivedDateTime"].as_str().unwrap_or("").into(),
            preview: m["bodyPreview"].as_str().unwrap_or("").into(),
            unread: !m["isRead"].as_bool().unwrap_or(true),
        })
        .collect())
}
fn outlook(a: &Account) -> Result<Vec<Message>, String> {
    let token = access(a)?;
    let r = client()
        .get("https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages")
        .query("$top", "30")
        .query(
            "$select",
            "subject,from,receivedDateTime,bodyPreview,isRead",
        )
        .query("$orderby", "receivedDateTime desc")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(graph_error)?;
    parse_graph(&decode(r)?)
}
fn proton(a: &Account, test_only: bool) -> Result<Vec<Message>, String> {
    use std::net::{SocketAddr, TcpStream};
    let password = credential(a)?
        .get_password()
        .map_err(|_| "请先保存 Bridge 专用密码")?;
    if a.port == 0 || a.username.trim().is_empty() {
        return Err("请填写 Bridge 用户名和端口".into());
    }
    let path = std::path::Path::new(&a.certificate);
    if !path.is_absolute()
        || std::fs::metadata(path)
            .map(|m| m.len() > 100_000)
            .unwrap_or(true)
    {
        return Err("请选择从 Proton Bridge 导出的 PEM 证书完整路径".into());
    }
    let cert = std::fs::read(path).map_err(|_| "无法读取证书")?;
    let cert = native_tls::Certificate::from_pem(&cert).map_err(|_| "证书必须为 PEM 格式")?;
    let tls = native_tls::TlsConnector::builder()
        .add_root_certificate(cert)
        .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
        .build()
        .map_err(|_| "TLS 初始化失败")?;
    let address = SocketAddr::from(([127, 0, 0, 1], a.port));
    let tcp = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|_| "无法连接本机 Proton Bridge，请启动 Bridge 并核对端口")?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(20)));
    let mut connection = if a.starttls {
        let mut c = imap::Client::new(tcp);
        c.read_greeting().map_err(|_| "Bridge 问候失败")?;
        let mut c = c
            .secure("127.0.0.1", &tls)
            .map_err(|_| "Bridge STARTTLS 或证书校验失败，请重新导出证书并核对模式")?;
        c.greeting_read = true;
        c
    } else {
        let stream = tls
            .connect("127.0.0.1", tcp)
            .map_err(|_| "Bridge TLS 证书校验失败，请核对证书和连接模式")?;
        let mut c = imap::Client::new(stream);
        c.read_greeting().map_err(|_| "Bridge 问候失败")?;
        c
    };
    connection.debug = false;
    let mut session = connection
        .login(&a.username, password)
        .map_err(|_| "Bridge 登录失败，请使用 Bridge 显示的专用密码")?;
    let inbox = session.examine("INBOX").map_err(|_| "无法打开收件箱")?;
    let mut result = Vec::new();
    if !test_only && inbox.exists > 0 {
        let range = format!(
            "{}:{}",
            inbox.exists.saturating_sub(29).max(1),
            inbox.exists
        );
        let messages = session
            .fetch(
                range,
                "(FLAGS BODY.PEEK[HEADER.FIELDS (SUBJECT FROM DATE)])",
            )
            .map_err(|_| "读取邮件列表失败")?;
        for m in messages.iter().rev() {
            if let Some(raw) = m.header() {
                let parsed = crate::mail::parse_message(raw);
                result.push(Message {
                    subject: parsed.subject,
                    from: parsed.from,
                    date: String::new(),
                    preview: "Proton 邮件在此显示主题与发件人，正文请在 Proton 中阅读。".into(),
                    unread: !m
                        .flags()
                        .iter()
                        .any(|f| matches!(f, imap::types::Flag::Seen)),
                });
            }
        }
    }
    let _ = session.logout();
    Ok(result)
}
impl Mail {
    fn remove_account_with(
        &mut self,
        id: &str,
        path: &std::path::Path,
        erase_credential: impl FnOnce(&Account) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.rx.is_some() { return Err("请先等待连接完成或取消连接，再移除账号".into()); }
        let index = self.accounts.iter().position(|a| a.id == id).ok_or("此账号已被移除")?;
        let account = &self.accounts[index];
        let kind = account.kind.clone();
        let selected_id = self.accounts.get(self.selected).map(|a| a.id.clone());
        let remaining: Vec<_> = self.accounts.iter().filter(|a| a.id != id).cloned().collect();
        // Never report removal if credential cleanup or atomic config persistence fails.
        erase_credential(account)?;
        self.checks.remove(id);
        super::save(path, &remaining).map_err(|e| format!("本机凭据已清除，但账号配置未能移除，请重试：{e}"))?;
        self.accounts = remaining;
        self.selected = selected_id.as_ref().filter(|selected| selected.as_str() != id)
            .and_then(|selected| self.accounts.iter().position(|a| &a.id == selected))
            .or_else(|| self.accounts.iter().position(|a| a.kind == kind))
            .unwrap_or(usize::MAX);
        if selected_id.as_deref() == Some(id) {
            self.messages.clear(); self.password.clear(); self.search.clear(); self.active = None;
        }
        self.pending_delete = None;
        self.status = "已从 Alpha 移除账号及本机凭据，邮箱服务端的账号和邮件不受影响".into();
        Ok(())
    }

    pub fn select_provider(&mut self, kind: Kind) {
        if self.selected == usize::MAX && self.visible_kind.as_ref() == Some(&kind) { return; }
        self.visible_kind = Some(kind.clone());
        if self
                .accounts
                .get(self.selected)
                .is_some_and(|a| a.kind == kind)
        {
            return;
        }
        if let Some(i) = self.accounts.iter().position(|a| a.kind == kind) {
            self.selected = i;
        } else {
            self.selected = usize::MAX;
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.rx=None;
        self.active=None;
        self.status.clear();
        self.search.clear();
        self.messages.clear();
        self.password.clear();
        self.pending_delete = None;
    }
    fn start(&mut self, auth: bool, test_only: bool) {
        if self.rx.is_some() {
            return;
        }
        let Some(a) = self.accounts.get(self.selected).cloned() else {
            return;
        };
        if let Err(e) = validate_config(&a) { self.status=e; return; }
        self.active=Some(a.clone());
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.status = if test_only {"正在测试连接与收件箱权限…"} else {"正在连接…"}.into();
        std::thread::spawn(move || {
            let result = (|| {
                if auth {
                    login(&a, &tx, &cancel)?;
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err("已取消".into());
                }
                if a.kind == Kind::Outlook {
                    if test_only {
                        let token=access(&a)?;
                        client().get("https://graph.microsoft.com/v1.0/me/mailFolders/inbox")
                            .query("$select", "id")
                            .set("Authorization", &format!("Bearer {token}"))
                            .call().map_err(graph_error)?;
                        Ok(vec![])
                    } else { outlook(&a) }
                } else {
                    proton(&a, test_only)
                }
            })();
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send(if test_only {Update::Checked(result.map(|_|()))} else {Update::Done(result)});
            }
        });
    }
    pub fn tick(&mut self) {
        if let Some(rx) = &self.rx {
            let mut done = None;
            let mut checked = None;
            let mut disconnected=false;
            loop {
                let v=match rx.try_recv(){
                    Ok(v)=>v,
                    Err(mpsc::TryRecvError::Empty)=>break,
                    Err(mpsc::TryRecvError::Disconnected)=>{disconnected=true;break;},
                };
                match v {
                    Update::Hint(h) => self.status = h,
                    Update::Checked(r) => checked = Some(r),
                    Update::Done(r) => done = Some(r),
                }
            }
            if let Some(error)=checked.as_ref().map(|r|r.as_ref().err().cloned()).or_else(||done.as_ref().map(|r|r.as_ref().err().cloned())) {
                if let Some(account)=self.active.take(){self.checks.insert(account.id.clone(),Check{account,at:chrono::Utc::now().timestamp(),error});}
            }
            if checked.is_none() && done.is_none() && disconnected {
                self.rx=None;self.active=None;
                self.status="连接任务意外结束，请重试；已读取的邮件仍保留。".into();
            }
            if let Some(r) = checked {
                self.rx = None;
                self.status = match r { Ok(())=>"连接测试通过 · 已验证身份和收件箱权限，未读取邮件内容".into(), Err(e)=>e };
            }
            if let Some(r) = done {
                self.rx = None;
                match r {
                    Ok(messages) => {
                        self.status = format!("已读取 {} 封最近邮件 · 只读", messages.len());
                        self.messages = messages;
                    }
                    Err(e) => self.status = e,
                }
            }
        }
    }
    pub fn ui(&mut self, ui: &mut eframe::egui::Ui, kind: Kind) {
        use eframe::egui;
        self.tick();
        crate::extras::settings_hint(ui,"只读收件箱 · 邮件不进入 Bot 共享");
        crate::extras::settings_card(ui,|ui| {
            if kind==Kind::Outlook {
                ui.strong("使用 Microsoft 官方登录授权");
                ui.label("1. 展开下方账号配置，填写自己的 Microsoft 应用 Client ID。
2. 点击登录 Outlook，按提示打开微软页面并输入设备代码。
3. 授权后点击刷新，读取最近邮件。");
                crate::extras::settings_hint(ui,"当前 Alpha 自用版尚无统一 Client ID。注册应用时允许个人账号及公共客户端流，添加 Mail.Read 委托权限；不需要 Client Secret 或邮箱登录密码。");
                ui.hyperlink_to("打开 Microsoft 应用注册","https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade");
            } else {
                ui.strong("使用 Proton Mail Bridge 专用密码");
                ui.label("1. 安装并登录 Proton Mail Bridge（需支持它的付费套餐）。
2. 将 Bridge 生成的用户名、专用密码、IMAP 端口填入下方。
3. 导出 Bridge 证书并填写路径，保存后先测试连接，再刷新邮件。");
                crate::extras::settings_hint(ui,"使用 Bridge 专用密码，不是 Proton 网页密码。连接限本机 127.0.0.1，证书用于核验此连接。");
                ui.hyperlink_to("查看 Proton Bridge 官方说明","https://proton.me/mail/bridge");
            }
        });
        ui.add_enabled_ui(self.rx.is_none(), |ui| {
            if ui.button(if kind==Kind::Outlook {"添加 Outlook 账号"}else{"添加 Proton 账号"}).clicked() {
                self.accounts.push(Account::new(kind.clone()));self.selected=self.accounts.len()-1;
                let _=super::save(&super::root().join("mail.json"),&self.accounts);
                self.messages.clear();self.password.clear();self.status.clear();self.pending_delete=None;
            }
            ui.horizontal_wrapped(|ui| {
                for (i,a) in self.accounts.iter().enumerate().filter(|(_,a)|a.kind==kind) {
                    ui.push_id(&a.id, |ui| {
                        let label_width = ui.fonts(|f| f.layout_no_wrap(a.label.clone(), egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color()).size().x);
                        let width = label_width + ui.spacing().button_padding.x * 2.0 + 28.0;
                        ui.allocate_ui_with_layout(egui::vec2(width, 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if ui.selectable_value(&mut self.selected,i,&a.label).clicked() {
                                self.messages.clear();self.password.clear();self.status.clear();self.pending_delete=None;
                            }
                            if ui.add_sized([26.0, 26.0], egui::Button::new("×").frame(false))
                                .on_hover_text(format!("从 Alpha 移除 {}", a.label)).clicked() {
                                self.pending_delete = Some(a.id.clone());
                            }
                        });
                    });
                }
            });
        });
        if let Some(account) = self.pending_delete.as_ref()
            .and_then(|id| self.accounts.iter().find(|a| &a.id == id && a.kind == kind)).cloned() {
            let mut remove = false;
            crate::extras::settings_card(ui, |ui| {
                ui.strong(format!("移除账号「{}」？", account.label));
                ui.label("将删除此账号在 Alpha 中的配置和本机登录凭据，不会删除邮箱服务端的账号或邮件。");
                ui.horizontal(|ui| {
                    remove = ui.add_enabled(self.rx.is_none(), egui::Button::new("确认移除")).clicked();
                    if ui.button("取消移除").clicked() { self.pending_delete = None; }
                });
            });
            if remove {
                if let Err(e) = self.remove_account_with(&account.id, &super::root().join("mail.json"), |a| {
                    match credential(a)?.delete_credential() {
                        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                        Err(_) => Err("无法移除本机登录凭据，账号已保留，请重试".into()),
                    }
                }) { self.status = e; }
            }
        }
        if !self.accounts.iter().any(|a| a.kind == kind) {
            crate::extras::settings_hint(ui, "尚未添加账号，点击上方按钮添加。");
        }
        let mut auth = false;
        let mut fetch = false;
        let mut test = false;
        let mut save = false;
        if let Some(a) = self.accounts.get_mut(self.selected).filter(|a| a.kind == kind) {
            ui.add_enabled_ui(self.rx.is_none(),|ui|{egui::CollapsingHeader::new("账号配置").default_open(true).show(ui,|ui|{
            ui.horizontal(|ui|{ui.label("显示名称");ui.text_edit_singleline(&mut a.label);});
            if a.kind==Kind::Outlook{ui.label("需自己注册 Microsoft 应用：支持个人 Microsoft 账户，允许公共客户端流，Microsoft Graph 委托权限 Mail.Read。无需 Client Secret。");ui.hyperlink_to("打开 Microsoft 应用注册","https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade");ui.horizontal(|ui|{ui.label("Client ID");ui.text_edit_singleline(&mut a.client_id);});}
            else{ui.label("需运行 Proton Bridge（通常需要付费计划）。将 Bridge 导出的 PEM 证书填在下方，仅信任此连接；连接地址固定为 127.0.0.1。");ui.horizontal(|ui|{ui.label("Bridge 用户名");ui.text_edit_singleline(&mut a.username);});ui.horizontal(|ui|{ui.label("IMAP 端口");ui.add(egui::DragValue::new(&mut a.port).range(1..=65535));ui.checkbox(&mut a.starttls,"STARTTLS（取消为 SSL）");});ui.horizontal(|ui|{ui.label("证书路径");ui.text_edit_singleline(&mut a.certificate);if ui.button("选择证书…").clicked(){
                #[cfg(windows)] match crate::system::pick_files_for("选择 Proton Bridge 导出的 PEM 证书",false){Ok(files)=>if let Some(path)=files.first(){a.certificate=path.to_string_lossy().into_owned();},Err(_)=>self.status="未能选择证书文件".into()}
            }});ui.add(egui::TextEdit::singleline(&mut self.password).password(true).hint_text("Bridge 专用密码"));}
            if ui.button("保存配置 / 密码").clicked(){save=true;self.status="配置已保存 · 尚未测试连接".into();if !self.password.is_empty(){self.checks.remove(&a.id);self.status=match credential(a).and_then(|c|c.set_password(&self.password).map_err(|_|"保存失败".into())){Ok(())=>{self.password.clear();"密码已存入 Windows 凭据管理器".into()},Err(e)=>e};}}
        });ui.horizontal_wrapped(|ui|{if a.kind==Kind::Outlook{auth=ui.button("登录 Outlook").clicked();}test=ui.button("测试连接").clicked();fetch=ui.button("刷新最近 30 封").clicked();if ui.button("断开账号授权").clicked(){self.checks.remove(&a.id);self.status=match credential(a).and_then(|c|c.delete_credential().map_err(|_|"没有可删除的凭据".into())){Ok(())=>"本机凭据已移除".into(),Err(e)=>e};self.messages.clear();}});});
            if let Some(check)=self.checks.get(&a.id){
                if check.account!=*a || !self.password.is_empty(){
                    crate::extras::settings_hint(ui,"账号配置已改动 · 请保存并重新测试连接");
                }else{
                    let time=chrono::DateTime::from_timestamp(check.at,0).map(|at|at.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string()).unwrap_or_default();
                    ui.label(format!("上次验证 {time} · {}",if check.error.is_some(){"连接未通过"}else{"已通过（不代表持续在线）"}));
                }
            }else{crate::extras::settings_hint(ui,"尚未验证此账号 · 保存配置后点击测试连接");}
        }
        if save || auth || fetch || test {
            if let Some(a)=self.accounts.get_mut(self.selected){a.client_id=a.client_id.trim().to_owned();a.username=a.username.trim().to_owned();a.certificate=a.certificate.trim().to_owned();}
            if (auth || fetch || test) && !self.password.is_empty(){self.status="请先保存刚输入的 Bridge 专用密码".into();return;}
            if let Err(e) = super::save(&super::root().join("mail.json"), &self.accounts) {
                self.status = e;
                return;
            }
        }
        if auth {
            self.start(true, false);
        } else if fetch {
            self.start(false, false);
        }
        if test { self.start(false, true); }
        if self.rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                if ui.button("取消").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.rx=None;self.status="已取消，未读取新的邮件结果".into();
                    self.active=None;
                }
            });
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        ui.label(&self.status);
        if self.status.contains("devicelogin") {
            ui.hyperlink_to("打开微软设备登录页面", "https://microsoft.com/devicelogin");
        }
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索本次读取的邮件"));
        let q = self.search.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for m in &self.messages {
                if !format!("{} {} {}", m.subject, m.from, m.preview)
                    .to_lowercase()
                    .contains(&q)
                {
                    continue;
                }
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.strong(format!("{}{}", if m.unread { "● " } else { "" }, m.subject));
                    ui.small(format!("{} · {}", m.from, m.date));
                    ui.label(&m.preview);
                });
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn removal_fixture() -> Mail {
        let accounts = [Kind::Outlook, Kind::Proton, Kind::Outlook].into_iter().enumerate().map(|(i, kind)| {
            let mut a = Account::new(kind);
            a.id = format!("alpha-removal-test-{}-{i}", std::process::id());
            a
        }).collect();
        Mail { accounts, selected:2, password:"draft".into(), rx:None,
            cancel:Arc::new(AtomicBool::new(false)), messages:vec![Message{subject:"selected mailbox".into(),..Default::default()}],
            status:String::new(), search:"query".into(), active:None, checks:Default::default(), pending_delete:None, visible_kind:None }
    }
    #[test]
    fn remove_account_preserves_selection_persists_and_never_recreates_last_account() {
        let mut mail=removal_fixture();
        let dir=std::env::temp_dir().join(format!("alpha-mail-removal-{}-{}", std::process::id(), chrono::Utc::now().timestamp_micros()));
        let path=dir.join("mail.json");
        let first=mail.accounts[0].id.clone();let selected=mail.accounts[2].id.clone();let proton=mail.accounts[1].id.clone();
        mail.remove_account_with(&first,&path,|a| {assert_eq!(a.id,first);Ok(())}).unwrap();
        assert_eq!(mail.accounts[mail.selected].id,selected);assert_eq!(mail.messages.len(),1);assert_eq!(mail.password,"draft");
        mail.remove_account_with(&selected,&path,|_|Ok(())).unwrap();
        assert_eq!(mail.selected,usize::MAX);assert!(mail.messages.is_empty());assert!(mail.password.is_empty());assert!(mail.search.is_empty());
        for _ in 0..3 {mail.select_provider(Kind::Outlook);}
        let saved:Vec<Account>=super::super::load(&path);assert_eq!(saved.len(),1);assert_eq!(saved[0].id,proton);
        assert_eq!(mail.accounts.len(),1);assert_eq!(mail.selected,usize::MAX);
        mail.select_provider(Kind::Proton);assert_eq!(mail.accounts[mail.selected].id,proton);
        mail.remove_account_with(&proton,&path,|_|Ok(())).unwrap();
        mail.select_provider(Kind::Outlook);mail.select_provider(Kind::Proton);assert!(mail.accounts.is_empty());
        let saved:Vec<Account>=super::super::load(&path);assert!(saved.is_empty());
    }
    #[test]
    fn failed_removal_or_active_connection_keeps_account() {
        let mut mail=removal_fixture();let id=mail.accounts[2].id.clone();
        let path=std::env::temp_dir().join(format!("alpha-mail-removal-failure-{}-{}",std::process::id(),chrono::Utc::now().timestamp_micros()));
        std::fs::create_dir_all(&path).unwrap();
        assert!(mail.remove_account_with(&id,&path,|_|Err("credential unavailable".into())).is_err());assert_eq!(mail.accounts.len(),3);
        assert!(mail.remove_account_with(&id,&path,|_|Ok(())).unwrap_err().contains("配置未能移除"));assert_eq!(mail.accounts.len(),3);
        let (_tx,rx)=mpsc::channel();mail.rx=Some(rx);
        assert!(mail.remove_account_with(&id,&path,|_|panic!("must not erase while busy")).is_err());assert_eq!(mail.accounts.len(),3);
    }
    #[test]
    fn account_close_buttons_target_same_named_accounts_and_cancel_safely() {
        use eframe::egui;
        fn frame(ctx:&egui::Context,mail:&mut Mail,events:Vec<egui::Event>)->egui::FullOutput {
            ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(900.0,900.0))),events,..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui|mail.ui(ui,Kind::Outlook));
            })
        }
        fn text_positions(output:&egui::FullOutput,label:&str)->Vec<egui::Pos2> {
            output.shapes.iter().filter_map(|s|if let egui::Shape::Text(t)=&s.shape {
                (t.galley.text()==label).then(|| t.pos+t.galley.size()*0.5)
            }else{None}).collect()
        }
        fn click(ctx:&egui::Context,mail:&mut Mail,pos:egui::Pos2) {
            frame(ctx,mail,vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true,false] {frame(ctx,mail,vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
        }
        let mut mail=removal_fixture();let ctx=egui::Context::default();
        let output=frame(&ctx,&mut mail,vec![]);let close=text_positions(&output,"×");
        assert_eq!(close.len(),2);assert!((close[0].y-close[1].y).abs()<1.0,"account labels must share a row when space allows");
        click(&ctx,&mut mail,close[0]);assert_eq!(mail.pending_delete.as_deref(),Some(mail.accounts[0].id.as_str()));
        assert_eq!(mail.selected,2);assert_eq!(mail.accounts.len(),3);
        let output=frame(&ctx,&mut mail,vec![]);let cancel=text_positions(&output,"取消移除")[0];
        click(&ctx,&mut mail,cancel);assert!(mail.pending_delete.is_none());assert_eq!(mail.accounts.len(),3);
        let output=frame(&ctx,&mut mail,vec![]);click(&ctx,&mut mail,text_positions(&output,"×")[1]);
        assert_eq!(mail.pending_delete.as_deref(),Some(mail.accounts[2].id.as_str()));
        mail.select_provider(Kind::Proton);assert!(mail.pending_delete.is_none());
    }
    #[test]
    fn connection_validation_and_http_errors_are_actionable() {
        let mut a=Account::new(Kind::Outlook);
        for bad in ["------------------------------------", "012345678901234567890123456789012345", "user@example.com"] {
            a.client_id=bad.into();assert!(validate_config(&a).is_err());
        }
        a.client_id="01234567-89ab-cdef-0123-456789abcdef".into();assert!(validate_config(&a).is_ok());
        let a=Account::new(Kind::Proton);assert!(validate_config(&a).unwrap_err().contains("用户名"));
        for (code,expected) in [(401,"重新登录"),(403,"Mail.Read"),(429,"稍后重试")] {
            let r=ureq::Response::new(code,"Error","PRIVATE_RESPONSE").unwrap();
            let message=graph_error(ureq::Error::Status(code,r));
            assert!(message.contains(expected));assert!(!message.contains("PRIVATE_RESPONSE"));
        }
    }
    #[test]
    fn connection_test_result_keeps_mail_and_provider_switch_discards_it() {
        let mut mail=Mail {
            accounts:vec![Account::new(Kind::Outlook),Account::new(Kind::Proton)],selected:0,
            password:String::new(),rx:None,cancel:Arc::new(AtomicBool::new(false)),
            messages:vec![Message{subject:"existing mail".into(),..Default::default()}],status:String::new(),search:String::new(),active:None,checks:Default::default(),pending_delete:None,visible_kind:None,
        };
        let id=mail.accounts[0].id.clone();mail.active=Some(mail.accounts[0].clone());
        let (tx,rx)=mpsc::channel();mail.rx=Some(rx);tx.send(Update::Checked(Ok(()))).unwrap();
        let ctx=eframe::egui::Context::default();
        let _=ctx.run(Default::default(),|ctx|{eframe::egui::CentralPanel::default().show(ctx,|ui|mail.ui(ui,Kind::Outlook));});
        assert!(mail.rx.is_none());assert!(mail.status.contains("测试通过"));assert_eq!(mail.messages[0].subject,"existing mail");
        assert!(mail.checks[&id].error.is_none());assert!(mail.checks[&id].account==mail.accounts[0]);
        mail.accounts[0].client_id="changed".into();assert!(mail.checks[&id].account!=mail.accounts[0]);
        let (tx,rx)=mpsc::channel();mail.rx=Some(rx);drop(tx);mail.tick();
        assert!(mail.rx.is_none());assert!(mail.status.contains("意外结束"));assert_eq!(mail.messages.len(),1);
        let (tx,rx)=mpsc::channel();mail.rx=Some(rx);mail.select_provider(Kind::Proton);
        assert!(tx.send(Update::Checked(Ok(()))).is_err());assert!(mail.status.is_empty());
    }
    #[test]
    fn oauth_failure_guidance_does_not_echo_server_payloads() {
        assert!(oauth_hint("invalid_grant").contains("重新授权"));
        assert!(oauth_hint("invalid_client").contains("公共客户端"));
        assert!(oauth_hint("expired_token").contains("新代码"));
        assert!(!oauth_hint("PRIVATE_RESPONSE").contains("PRIVATE_RESPONSE"));
    }
    #[test]
    fn provider_view_filters_accounts_instructions_and_discards_late_results() {
        let mut mail=Mail {
            accounts:vec![Account::new(Kind::Outlook),Account::new(Kind::Proton)],selected:0,
            password:String::new(),rx:None,cancel:Arc::new(AtomicBool::new(false)),
            messages:vec![],status:String::new(),search:String::new(),active:None,checks:Default::default(),pending_delete:None,visible_kind:None,
        };
        let ctx=eframe::egui::Context::default();
        for (kind,expected,absent) in [(Kind::Outlook,"Microsoft","Proton"),(Kind::Proton,"Proton","Outlook")] {
            mail.select_provider(kind.clone());
            let output=ctx.run(Default::default(),|ctx| {eframe::egui::CentralPanel::default().show(ctx,|ui|mail.ui(ui,kind.clone()));});
            let text=output.shapes.iter().filter_map(|s|if let eframe::egui::Shape::Text(t)=&s.shape {Some(t.galley.text().to_owned())}else{None}).collect::<Vec<_>>().join("|");
            assert!(text.contains(expected),"{text}");assert!(!text.contains(absent),"{text}");
        }
        let (tx,rx)=mpsc::channel();mail.rx=Some(rx);mail.status="old Proton result".into();mail.password="draft".into();
        let old_cancel=mail.cancel.clone();mail.select_provider(Kind::Outlook);
        assert!(old_cancel.load(Ordering::Relaxed));assert!(mail.rx.is_none());assert!(mail.status.is_empty());assert!(mail.password.is_empty());
        assert!(tx.send(Update::Done(Ok(vec![]))).is_err());
        assert!(mail.accounts[mail.selected].kind==Kind::Outlook);
    }
    #[test]
    fn parse_graph_missing_fields_and_unread() {
        let m = parse_graph(&json!({"value":[{"subject":"Test","isRead":false}]})).unwrap();
        assert_eq!(m.len(), 1);
        assert!(m[0].unread);
        assert!(parse_graph(&json!({"error":{}})).is_err());
    }
}
