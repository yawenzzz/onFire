#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityMode {
    LiveListen,
    ShadowPoll,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportAdapterKind {
    LiveListen,
    ShadowPoll,
    Replay,
}

impl TransportAdapterKind {
    pub const fn activity_mode(self) -> ActivityMode {
        match self {
            Self::LiveListen => ActivityMode::LiveListen,
            Self::ShadowPoll => ActivityMode::ShadowPoll,
            Self::Replay => ActivityMode::Replay,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportBoundaryConfig {
    pub activity: TransportAdapterKind,
    pub positions: TransportAdapterKind,
    pub market: TransportAdapterKind,
    pub verification: TransportAdapterKind,
}

impl TransportBoundaryConfig {
    pub const fn new(
        activity: TransportAdapterKind,
        positions: TransportAdapterKind,
        market: TransportAdapterKind,
        verification: TransportAdapterKind,
    ) -> Self {
        Self {
            activity,
            positions,
            market,
            verification,
        }
    }

    pub const fn for_mode(mode: ActivityMode) -> Self {
        let kind = match mode {
            ActivityMode::LiveListen => TransportAdapterKind::LiveListen,
            ActivityMode::ShadowPoll => TransportAdapterKind::ShadowPoll,
            ActivityMode::Replay => TransportAdapterKind::Replay,
        };

        Self::new(kind, kind, kind, kind)
    }

    pub fn requested_mode(&self) -> Result<ActivityMode, String> {
        if self.positions != self.activity {
            return Err("positions_transport_mode_mismatch".to_string());
        }
        if self.market != self.activity {
            return Err("market_transport_mode_mismatch".to_string());
        }
        if self.verification != self.activity {
            return Err("verification_transport_mode_mismatch".to_string());
        }

        Ok(self.activity.activity_mode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandAdapterConfig {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandAdapterConfig {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn repo_local_order_sign_helper(program: impl Into<String>) -> Self {
        Self::new(program).with_args(vec!["scripts/sign_order.py".into(), "--json".into()])
    }

    pub fn repo_local_l2_header_helper(program: impl Into<String>) -> Self {
        Self::new(program).with_args(vec!["scripts/sign_l2.py".into(), "--json".into()])
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn configured(&self) -> bool {
        !self.program.trim().is_empty()
    }

    pub fn from_env(program: impl Into<String>, args: Option<String>) -> Self {
        let mut command = Self::new(program);
        if let Some(args) = args {
            command = command.with_args(
                args.split_whitespace()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            );
        }
        command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveExecutionWiring {
    pub signing: CommandAdapterConfig,
    pub submit: CommandAdapterConfig,
    pub submit_base_url: String,
    pub submit_connect_timeout_ms: u64,
    pub submit_max_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootEnvLoadError {
    Io { path: PathBuf, error: String },
    MissingField(String),
    InvalidNumber { field: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningAdapterConfig {
    ReplayStub,
    Command { command: CommandAdapterConfig },
}

impl SigningAdapterConfig {
    pub const fn replay_stub() -> Self {
        Self::ReplayStub
    }

    pub fn command(program: impl Into<String>) -> Self {
        Self::Command {
            command: CommandAdapterConfig::new(program),
        }
    }

    pub fn command_with_args(program: impl Into<String>, args: Vec<String>) -> Self {
        Self::Command {
            command: CommandAdapterConfig::new(program).with_args(args),
        }
    }

    pub const fn mode_label(&self) -> &'static str {
        match self {
            Self::ReplayStub => "replay_stub",
            Self::Command { .. } => "command",
        }
    }

    pub fn command_config(&self) -> Option<&CommandAdapterConfig> {
        match self {
            Self::ReplayStub => None,
            Self::Command { command } => Some(command),
        }
    }

    pub fn live_ready(&self) -> bool {
        self.command_config()
            .is_some_and(CommandAdapterConfig::configured)
    }

    pub fn repo_local_l2_header_helper(&self) -> Option<CommandAdapterConfig> {
        self.command_config()
            .map(|command| CommandAdapterConfig::repo_local_l2_header_helper(&command.program))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitAdapterConfig {
    Replay,
    Http {
        base_url: String,
        command: CommandAdapterConfig,
        connect_timeout_ms: u64,
        max_time_ms: u64,
    },
}

impl SubmitAdapterConfig {
    pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 50;
    pub const DEFAULT_MAX_TIME_MS: u64 = 200;

    pub const fn replay() -> Self {
        Self::Replay
    }

    pub fn http(base_url: impl Into<String>, command_program: impl Into<String>) -> Self {
        Self::http_with_command(base_url, CommandAdapterConfig::new(command_program))
    }

    pub fn http_with_command(base_url: impl Into<String>, command: CommandAdapterConfig) -> Self {
        Self::Http {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            command,
            connect_timeout_ms: Self::DEFAULT_CONNECT_TIMEOUT_MS,
            max_time_ms: Self::DEFAULT_MAX_TIME_MS,
        }
    }

    pub fn with_connect_timeout_ms(self, connect_timeout_ms: u64) -> Self {
        match self {
            Self::Http {
                base_url,
                command,
                max_time_ms,
                ..
            } => Self::Http {
                base_url,
                command,
                connect_timeout_ms: connect_timeout_ms.max(1),
                max_time_ms,
            },
            Self::Replay => Self::Replay,
        }
    }

    pub fn with_max_time_ms(self, max_time_ms: u64) -> Self {
        match self {
            Self::Http {
                base_url,
                command,
                connect_timeout_ms,
                ..
            } => Self::Http {
                base_url,
                command,
                connect_timeout_ms,
                max_time_ms: max_time_ms.max(1),
            },
            Self::Replay => Self::Replay,
        }
    }

    pub const fn mode_label(&self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::Http { .. } => "http_command",
        }
    }

    pub fn command_config(&self) -> Option<&CommandAdapterConfig> {
        match self {
            Self::Replay => None,
            Self::Http { command, .. } => Some(command),
        }
    }

    pub fn base_url(&self) -> Option<&str> {
        match self {
            Self::Replay => None,
            Self::Http { base_url, .. } => Some(base_url.as_str()),
        }
    }

    pub const fn connect_timeout_ms(&self) -> Option<u64> {
        match self {
            Self::Replay => None,
            Self::Http {
                connect_timeout_ms, ..
            } => Some(*connect_timeout_ms),
        }
    }

    pub const fn max_time_ms(&self) -> Option<u64> {
        match self {
            Self::Replay => None,
            Self::Http { max_time_ms, .. } => Some(*max_time_ms),
        }
    }

    pub fn live_ready(&self) -> bool {
        self.command_config()
            .is_some_and(CommandAdapterConfig::configured)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionAdapterConfig {
    pub signing: SigningAdapterConfig,
    pub submit: SubmitAdapterConfig,
}

impl ExecutionAdapterConfig {
    pub const fn scaffold() -> Self {
        Self {
            signing: SigningAdapterConfig::ReplayStub,
            submit: SubmitAdapterConfig::Replay,
        }
    }

    pub fn live_command_http(
        signing_program: impl Into<String>,
        base_url: impl Into<String>,
        submit_command_program: impl Into<String>,
    ) -> Self {
        Self {
            signing: SigningAdapterConfig::command(signing_program),
            submit: SubmitAdapterConfig::http(base_url, submit_command_program),
        }
    }

    pub fn live_command_helper_http(
        signing_program: impl Into<String>,
        base_url: impl Into<String>,
        submit_command_program: impl Into<String>,
    ) -> Self {
        Self {
            signing: SigningAdapterConfig::Command {
                command: CommandAdapterConfig::repo_local_order_sign_helper(signing_program),
            },
            submit: SubmitAdapterConfig::http(base_url, submit_command_program),
        }
    }

    pub fn from_root(root: impl AsRef<Path>) -> Result<Self, RootEnvLoadError> {
        let env = merged_root_env(root)?;
        Self::from_env_map(&env)
    }

    pub fn from_env_map(env: &BTreeMap<String, String>) -> Result<Self, RootEnvLoadError> {
        let signing_program = env_value(env, &["RUST_COPYTRADER_SIGNING_PROGRAM"]);
        let signing_args = env_value(env, &["RUST_COPYTRADER_SIGNING_ARGS"]);
        let submit_program = env_value(env, &["RUST_COPYTRADER_SUBMIT_PROGRAM"]);
        let submit_args = env_value(env, &["RUST_COPYTRADER_SUBMIT_ARGS"]);
        let submit_base_url = env_value(env, &["CLOB_BASE_URL", "CLOB_HOST"]);
        let connect_timeout_value = env_value(env, &["RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS"]);
        let max_time_value = env_value(env, &["RUST_COPYTRADER_SUBMIT_MAX_TIME_MS"]);
        let helper_auth_present = env_value(env, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"]).is_some()
            || env_value(env, &["CLOB_API_KEY", "POLY_API_KEY"]).is_some()
            || env_value(env, &["CLOB_PASS_PHRASE", "POLY_PASSPHRASE"]).is_some();
        let live_requested = signing_program.is_some()
            || submit_program.is_some()
            || submit_base_url.is_some()
            || connect_timeout_value.is_some()
            || max_time_value.is_some()
            || helper_auth_present;

        if !live_requested {
            return Ok(Self::default());
        }

        let default_helper_mode = helper_auth_present
            && signing_program.is_none()
            && submit_program.is_none()
            && signing_args.is_none()
            && submit_args.is_none();
        let signing_program = signing_program.unwrap_or_else(|| {
            if default_helper_mode {
                "python3".to_string()
            } else {
                String::new()
            }
        });
        if signing_program.trim().is_empty() {
            return Err(RootEnvLoadError::MissingField(
                "RUST_COPYTRADER_SIGNING_PROGRAM".into(),
            ));
        }
        let submit_program = submit_program.unwrap_or_else(|| {
            if default_helper_mode {
                "python3".to_string()
            } else {
                String::new()
            }
        });
        if submit_program.trim().is_empty() {
            return Err(RootEnvLoadError::MissingField(
                "RUST_COPYTRADER_SUBMIT_PROGRAM".into(),
            ));
        }
        let submit_base_url =
            submit_base_url.unwrap_or_else(|| "https://clob.polymarket.com".to_string());
        let connect_timeout_ms = parse_u64_field(
            connect_timeout_value.as_deref(),
            "RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS",
            SubmitAdapterConfig::DEFAULT_CONNECT_TIMEOUT_MS,
        )?;
        let max_time_ms = parse_u64_field(
            max_time_value.as_deref(),
            "RUST_COPYTRADER_SUBMIT_MAX_TIME_MS",
            SubmitAdapterConfig::DEFAULT_MAX_TIME_MS,
        )?;

        Ok(Self {
            signing: SigningAdapterConfig::Command {
                command: if let Some(signing_args) = signing_args {
                    CommandAdapterConfig::from_env(signing_program, Some(signing_args))
                } else {
                    CommandAdapterConfig::repo_local_order_sign_helper(signing_program)
                },
            },
            submit: SubmitAdapterConfig::http_with_command(
                submit_base_url,
                if default_helper_mode {
                    CommandAdapterConfig::from_env(
                        submit_program,
                        Some("scripts/submit_helper.py --json --curl-bin curl".to_string()),
                    )
                } else {
                    CommandAdapterConfig::from_env(submit_program, submit_args)
                },
            )
            .with_connect_timeout_ms(connect_timeout_ms)
            .with_max_time_ms(max_time_ms),
        })
    }

    pub fn live_execution_wiring(&self) -> Option<LiveExecutionWiring> {
        let signing = self.signing.command_config()?.clone();
        let submit = self.submit.command_config()?.clone();
        let submit_base_url = self.submit.base_url()?.to_string();
        let submit_connect_timeout_ms = self.submit.connect_timeout_ms()?;
        let submit_max_time_ms = self.submit.max_time_ms()?;

        if !signing.configured() || !submit.configured() {
            return None;
        }

        Some(LiveExecutionWiring {
            signing,
            submit,
            submit_base_url,
            submit_connect_timeout_ms,
            submit_max_time_ms,
        })
    }

    pub fn live_ready(&self) -> bool {
        self.live_execution_wiring().is_some()
    }

    pub fn live_l2_header_helper(&self) -> Option<CommandAdapterConfig> {
        if !self.submit.live_ready() {
            return None;
        }

        self.signing.repo_local_l2_header_helper()
    }
}

impl Default for ExecutionAdapterConfig {
    fn default() -> Self {
        Self::scaffold()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveModeGate {
    mode: ActivityMode,
    pub activity_source_verified: bool,
    pub activity_source_under_budget: bool,
    pub activity_capability_detected: bool,
    pub positions_under_budget: bool,
    pub execution_surface_ready: bool,
}

impl LiveModeGate {
    pub const fn for_mode(mode: ActivityMode) -> Self {
        Self {
            mode,
            activity_source_verified: false,
            activity_source_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            execution_surface_ready: false,
        }
    }

    pub fn blocked_reason(&self) -> Option<String> {
        match self.mode {
            ActivityMode::LiveListen => {
                if !self.activity_source_verified {
                    Some("activity_source_unverified".to_string())
                } else if !self.activity_source_under_budget {
                    Some("activity_source_over_budget".to_string())
                } else if !self.activity_capability_detected {
                    Some("activity_capability_missing".to_string())
                } else if !self.positions_under_budget {
                    Some("positions_over_budget".to_string())
                } else if !self.execution_surface_ready {
                    Some("execution_surface_not_ready".to_string())
                } else {
                    None
                }
            }
            ActivityMode::ShadowPoll | ActivityMode::Replay => None,
        }
    }

    pub fn unlocked(&self) -> bool {
        self.blocked_reason().is_none()
    }
}

pub(crate) fn merged_root_env(
    root: impl AsRef<Path>,
) -> Result<BTreeMap<String, String>, RootEnvLoadError> {
    let mut env = std::env::vars().collect::<BTreeMap<_, _>>();
    merge_env_file(&mut env, &root.as_ref().join(".env"))?;
    merge_env_file(&mut env, &root.as_ref().join(".env.local"))?;
    Ok(env)
}

fn merge_env_file(env: &mut BTreeMap<String, String>, path: &Path) -> Result<(), RootEnvLoadError> {
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(path).map_err(|err| RootEnvLoadError::Io {
        path: path.to_path_buf(),
        error: err.to_string(),
    })?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        env.insert(key.trim().to_string(), value.trim().to_string());
    }

    Ok(())
}

fn env_value(env: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env.get(*key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_u64_field(
    value: Option<&str>,
    field: &str,
    default: u64,
) -> Result<u64, RootEnvLoadError> {
    match value {
        Some(raw) => raw.parse::<u64>().map(|parsed| parsed.max(1)).map_err(|_| {
            RootEnvLoadError::InvalidNumber {
                field: field.into(),
                value: raw.to_string(),
            }
        }),
        None => Ok(default),
    }
}
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
