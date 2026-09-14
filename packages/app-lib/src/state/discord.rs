use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, Assets, Button, Timestamps},
};
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::plus::discord::{
    ApplicationSource, PlayingInstance, PresenceContent, PresenceProblem,
    application_id, application_known, presence_content,
};
use crate::plus::settings::PlusSettings;
use crate::state::{ProcessManager, ProcessMetadata};

/// Discord usually answers in a fraction of a second, but handshakes have been seen taking
/// over six seconds, and half a minute while it's throttling. Giving up and starting over
/// would never finish during a slow spell, such as just after Discord starts.
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);
const HANDSHAKE_OPCODE: u8 = 0;
const CLOSE_OPCODE: u32 = 2;
const INVALID_CLIENT_ID: u32 = 4000;
const MAX_REPLIES_READ: usize = 8;

struct Connection {
    application_id: String,
    client: DiscordIpcClient,
    connected: bool,
}

pub struct DiscordGuard {
    connection: Mutex<Option<Connection>>,
    reachable: AtomicBool,
    problem: StdMutex<Option<PresenceProblem>>,
    known_application: Mutex<Option<String>>,
}

enum SendFailure {
    Lost,
    Refused(String),
}

/// Development builds can write what would be sent to a file, so testing never
/// shows anything on a real Discord profile.
#[cfg(debug_assertions)]
fn capture(entry: serde_json::Value) -> bool {
    use std::io::Write;

    let Some(path) = std::env::var_os("PAWRINTH_DISCORD_CAPTURE") else {
        return false;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(format!("{entry}\n").as_bytes());
    }
    true
}

fn problem_from(failure: SendFailure) -> PresenceProblem {
    match failure {
        SendFailure::Lost => PresenceProblem::NoReply,
        SendFailure::Refused(message) => {
            PresenceProblem::ActivityRefused { message }
        }
    }
}

fn activity_from(content: &PresenceContent) -> Activity<'_> {
    let mut assets = Assets::new()
        .large_image(content.logo_image.as_str())
        .large_text(content.logo_text.as_str());
    if let Some(badge) = &content.badge {
        assets = assets
            .small_image(badge.image.as_str())
            .small_text(badge.text.as_str());
    }
    let mut activity = Activity::new()
        .details(content.details.as_str())
        .assets(assets);
    if let Some(state) = &content.state {
        activity = activity.state(state.as_str());
    }
    if let Some(started_at) = content.started_at {
        activity = activity.timestamps(Timestamps::new().start(started_at));
    }
    if !content.buttons.is_empty() {
        activity = activity.buttons(
            content
                .buttons
                .iter()
                .map(|button| {
                    Button::new(button.label.as_str(), button.url.as_str())
                })
                .collect(),
        );
    }
    activity
}

/// Windows refuses a normal app when Discord runs as administrator, which looks the same
/// as Discord being closed unless the pipes are checked for it.
#[cfg(windows)]
fn discord_is_blocked() -> bool {
    use std::os::windows::fs::OpenOptionsExt;

    (0..10).any(|index| {
        std::fs::OpenOptions::new()
            .access_mode(0x3)
            .open(format!(r"\\?\pipe\discord-ipc-{index}"))
            .is_err_and(|error| {
                error.kind() == std::io::ErrorKind::PermissionDenied
            })
    })
}

#[cfg(not(windows))]
fn discord_is_blocked() -> bool {
    false
}

impl Connection {
    fn new(application_id: &str) -> Self {
        Self {
            application_id: application_id.to_string(),
            client: DiscordIpcClient::new(application_id),
            connected: false,
        }
    }

    /// Discord says it refuses an application in its reply to the handshake, which the client
    /// library's `connect` discards. Anything else that ends a connection is normal while
    /// Discord starts or closes, so it's worth trying again.
    fn open(&mut self) -> Result<(), PresenceProblem> {
        self.client = DiscordIpcClient::new(&self.application_id);
        self.connected = false;
        if self.client.connect_ipc().is_err() {
            return Err(if discord_is_blocked() {
                PresenceProblem::Blocked
            } else {
                PresenceProblem::NotRunning
            });
        }
        let handshake =
            serde_json::json!({ "v": 1, "client_id": self.application_id });
        let reply = self
            .client
            .send(handshake, HANDSHAKE_OPCODE)
            .and_then(|()| self.client.recv());
        match reply {
            Ok((CLOSE_OPCODE, reply)) if reply["code"] == INVALID_CLIENT_ID => {
                Err(PresenceProblem::ApplicationRefused)
            }
            Ok((CLOSE_OPCODE, _)) | Err(_) => Err(PresenceProblem::NoReply),
            Ok(_) => {
                self.connected = true;
                Ok(())
            }
        }
    }

    fn set_activity(
        &mut self,
        content: &PresenceContent,
    ) -> Result<(), SendFailure> {
        self.client
            .set_activity(activity_from(content))
            .map_err(|_| SendFailure::Lost)?;
        for _ in 0..MAX_REPLIES_READ {
            let (opcode, reply) =
                self.client.recv().map_err(|_| SendFailure::Lost)?;
            if opcode == CLOSE_OPCODE {
                return Err(SendFailure::Lost);
            }
            if reply["cmd"] != "SET_ACTIVITY" {
                continue;
            }
            if reply["evt"] == "ERROR" {
                let message =
                    reply["data"]["message"].as_str().unwrap_or_default();
                return Err(SendFailure::Refused(message.to_string()));
            }
            return Ok(());
        }
        Err(SendFailure::Lost)
    }

    /// A connection that has gone stale gets one fresh try.
    fn deliver(
        &mut self,
        content: &PresenceContent,
    ) -> Result<(), PresenceProblem> {
        if self.connected {
            match self.set_activity(content) {
                Err(SendFailure::Lost) => self.connected = false,
                outcome => return outcome.map_err(problem_from),
            }
        }
        self.open()?;
        self.set_activity(content).map_err(|failure| {
            self.connected = false;
            problem_from(failure)
        })
    }

    fn close(mut self) {
        if self.connected {
            let _ = self.client.clear_activity();
            let _ = self.client.close();
        }
    }
}

impl DiscordGuard {
    pub fn init() -> crate::Result<DiscordGuard> {
        Ok(DiscordGuard {
            connection: Mutex::new(None),
            reachable: AtomicBool::new(false),
            problem: StdMutex::new(None),
            known_application: Mutex::new(None),
        })
    }

    /// Whether Discord took the last thing sent to it.
    pub fn is_connected(&self) -> bool {
        self.reachable.load(Ordering::Relaxed)
    }

    /// Why the last attempt failed, if it did.
    pub fn problem(&self) -> Option<PresenceProblem> {
        self.problem.lock().ok()?.clone()
    }

    fn record(&self, outcome: Result<(), PresenceProblem>) {
        self.reachable.store(outcome.is_ok(), Ordering::Relaxed);
        if let Ok(mut problem) = self.problem.lock() {
            *problem = outcome.err();
        }
    }

    /// Shows an instance that's starting, which isn't a running process yet.
    pub async fn show_playing(
        &self,
        instance: PlayingInstance,
        pool: &SqlitePool,
    ) -> crate::Result<()> {
        self.show(Some(&instance), pool).await
    }

    /// Shows the running instance if there is one, or else the idle status.
    pub async fn clear_to_default(
        &self,
        pool: &SqlitePool,
        process_manager: &ProcessManager,
    ) -> crate::Result<()> {
        let running = process_manager.get_all();
        let playing = match running.first() {
            Some(process) => Some(playing_from(process, pool).await),
            None => None,
        };
        self.show(playing.as_ref(), pool).await
    }

    /// Sending the status again is what notices Discord closing or coming back. A refused
    /// application only changes when the settings do, and asking again would only be throttled.
    pub async fn keep_alive(
        &self,
        pool: &SqlitePool,
        process_manager: &ProcessManager,
    ) -> crate::Result<()> {
        #[cfg(debug_assertions)]
        if std::env::var_os("PAWRINTH_DISCORD_CAPTURE").is_some() {
            return Ok(());
        }
        if self.problem() == Some(PresenceProblem::ApplicationRefused)
            || self.connection.try_lock().is_err()
        {
            return Ok(());
        }
        self.clear_to_default(pool, process_manager).await
    }

    /// Discord answers a handshake with an unknown ID slowly, up to half a minute once it
    /// starts throttling, while its website answers at once. If the website can't be asked,
    /// Discord still gets to decide.
    async fn application_accepted(&self, application: &str) -> bool {
        #[cfg(debug_assertions)]
        if std::env::var_os("PAWRINTH_DISCORD_CAPTURE").is_some() {
            return true;
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("PAWRINTH_DISCORD_SKIP_LOOKUP").is_some() {
            return true;
        }
        let mut known = self.known_application.lock().await;
        if known.as_deref() == Some(application) {
            return true;
        }
        let accepted = application_known(application).await != Some(false);
        if accepted {
            *known = Some(application.to_string());
        }
        accepted
    }

    async fn show(
        &self,
        playing: Option<&PlayingInstance>,
        pool: &SqlitePool,
    ) -> crate::Result<()> {
        let settings = crate::state::Settings::get(pool).await?;
        if !settings.discord_rpc {
            return self.disconnect().await;
        }
        let plus = PlusSettings::read(pool).await?;
        let (source, application) = application_id(&plus.discord);
        if source == ApplicationSource::Custom
            && !self.application_accepted(&application).await
        {
            self.disconnect().await?;
            self.record(Err(PresenceProblem::ApplicationRefused));
            return Ok(());
        }
        if self.problem() == Some(PresenceProblem::ApplicationRefused)
            && let Ok(mut problem) = self.problem.lock()
        {
            *problem = None;
        }
        let content = presence_content(&plus.discord, &plus.privacy, playing);
        self.send(&application, content).await
    }

    async fn send(
        &self,
        application: &str,
        content: PresenceContent,
    ) -> crate::Result<()> {
        #[cfg(debug_assertions)]
        if capture(serde_json::json!({
            "application_id": application,
            "activity": activity_from(&content),
        })) {
            self.record(Ok(()));
            return Ok(());
        }

        // Tests start the whole app, and must never reach a real Discord profile.
        if cfg!(test) {
            return Ok(());
        }

        let mut current = self.connection.lock().await;
        let mut connection = match current.take() {
            Some(existing) if existing.application_id == application => {
                existing
            }
            previous => {
                if let Some(previous) = previous {
                    previous.close();
                }
                Connection::new(application)
            }
        };
        let delivery = tokio::task::spawn_blocking(move || {
            let outcome = connection.deliver(&content);
            (connection, outcome)
        });
        match tokio::time::timeout(REPLY_TIMEOUT, delivery).await {
            Ok(Ok((connection, outcome))) => {
                *current = Some(connection);
                self.record(outcome);
            }
            _ => self.record(Err(PresenceProblem::NoReply)),
        }
        Ok(())
    }

    /// Takes Pawrinth off the profile and stops talking to Discord.
    async fn disconnect(&self) -> crate::Result<()> {
        #[cfg(debug_assertions)]
        if capture(serde_json::json!({ "cleared": true })) {
            self.record(Ok(()));
            self.reachable.store(false, Ordering::Relaxed);
            return Ok(());
        }

        if let Some(connection) = self.connection.lock().await.take() {
            connection.close();
        }
        self.record(Ok(()));
        self.reachable.store(false, Ordering::Relaxed);
        Ok(())
    }
}

async fn playing_from(
    process: &ProcessMetadata,
    pool: &SqlitePool,
) -> PlayingInstance {
    let content_set =
        crate::state::instances::commands::get_instance_launch_context(
            &process.instance_id,
            pool,
        )
        .await
        .ok()
        .flatten()
        .map(|context| context.applied_content_set);
    PlayingInstance {
        name: process.instance_name.clone(),
        game_version: content_set.as_ref().map(|set| set.game_version.clone()),
        loader: content_set.map(|set| set.loader),
        started_at: Some(process.start_time.timestamp()),
    }
}
