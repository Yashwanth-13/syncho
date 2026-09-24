
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaEvent {
    Playing,
    Paused,
    Stopped,
    TrackChanged { title: String, artist: String },
}


#[cfg(windows)]
mod win {
    use super::*;
    use windows::{
        Foundation::TypedEventHandler,
        Media::Control::{
            GlobalSystemMediaTransportControlsSession,
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
    };
    use windows::core::Ref;
    use std::sync::{Arc, Mutex};

    fn emit_status<F: Fn(MediaEvent) + Send + Sync + 'static>(
        session: &GlobalSystemMediaTransportControlsSession,
        cb: &Arc<F>,
    ) {
        if let Ok(info) = session.GetPlaybackInfo() {
            if let Ok(status) = info.PlaybackStatus() {
                let event = match status {
                    GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => {
                        Some(MediaEvent::Playing)
                    }
                    GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => {
                        Some(MediaEvent::Paused)
                    }
                    GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => {
                        Some(MediaEvent::Stopped)
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    cb(event);
                }
            }
        }
    }

    fn emit_track<F: Fn(MediaEvent) + Send + Sync + 'static>(
        session: &GlobalSystemMediaTransportControlsSession,
        cb: &Arc<F>,
        last_track: &Arc<Mutex<Option<(String, String)>>>,
    ) {
        if let Ok(op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(props) = op.get() {
                let title = props.Title().unwrap_or_default().to_string();
                let artist = props.Artist().unwrap_or_default().to_string();
                let track = (title, artist);

                let mut last = last_track.lock().unwrap();
                // GSMTC can raise MediaPropertiesChanged twice for the same
                // track, so dedupe against the last one emitted.
                if last.as_ref() != Some(&track) {
                    cb(MediaEvent::TrackChanged {
                        title: track.0.clone(),
                        artist: track.1.clone(),
                    });
                    *last = Some(track);
                }
            }
        }
    }

    fn watch_session<F: Fn(MediaEvent) + Send + Sync + 'static>(
        session: &GlobalSystemMediaTransportControlsSession,
        cb: &Arc<F>,
        last_track: &Arc<Mutex<Option<(String, String)>>>,
    ) -> windows::core::Result<()> {
        emit_status(session, cb);
        emit_track(session, cb, last_track);

        let cb_status = cb.clone();
        let status_handler = TypedEventHandler::new(
            move |session: Ref<'_, GlobalSystemMediaTransportControlsSession>, _args| {
                if let Ok(session) = session.ok() {
                    emit_status(session, &cb_status);
                }
                Ok(())
            },
        );
        session.PlaybackInfoChanged(&status_handler)?;

        let cb_track = cb.clone();
        let last_track_clone = last_track.clone();
        let track_handler = TypedEventHandler::new(
            move |session: Ref<'_, GlobalSystemMediaTransportControlsSession>, _args| {
                if let Ok(session) = session.ok() {
                    emit_track(session, &cb_track, &last_track_clone);
                }
                Ok(())
            },
        );
        session.MediaPropertiesChanged(&track_handler)?;

        Ok(())
    }

    pub async fn listen<F>(callback: F) -> Result<()>
    where
        F: Fn(MediaEvent) + Send + Sync + 'static,
    {
        let cb = Arc::new(callback);
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;

        // Session object must stay alive for the program's lifetime, or its
        // event registration gets dropped and PlaybackInfoChanged stops firing.
        let current_session: Arc<Mutex<Option<GlobalSystemMediaTransportControlsSession>>> =
            Arc::new(Mutex::new(None));
        let last_track: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));

        // Tracks which app we last registered handlers for, so a spurious
        // CurrentSessionChanged firing for the *same* session (WinRT raises
        // one right after you subscribe) doesn't register a second, leaked
        // set of handlers and double every event.
        let current_app: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        if let Ok(session) = manager.GetCurrentSession() {
            let aumid = session.SourceAppUserModelId().ok().map(|s| s.to_string());
            watch_session(&session, &cb, &last_track)?;
            *current_app.lock().unwrap() = aumid;
            *current_session.lock().unwrap() = Some(session);
        }

        let session_slot = current_session.clone();
        let app_slot = current_app.clone();
        let last_track_slot = last_track.clone();
        let cb_slot = cb.clone();
        let manager_clone = manager.clone();

        let session_handler = TypedEventHandler::new(
            move |_manager: Ref<'_, GlobalSystemMediaTransportControlsSessionManager>, _args| {
                if let Ok(session) = manager_clone.GetCurrentSession() {
                    let aumid = session.SourceAppUserModelId().ok().map(|s| s.to_string());

                    let mut last_app = app_slot.lock().unwrap();
                    if *last_app != aumid {
                        let _ = watch_session(&session, &cb_slot, &last_track_slot);
                        *last_app = aumid;
                    }

                    *session_slot.lock().unwrap() = Some(session);
                } else {
                    *app_slot.lock().unwrap() = None;
                    *session_slot.lock().unwrap() = None;
                }
                Ok(())
            },
        );
        manager.CurrentSessionChanged(&session_handler)?;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use futures_lite::stream::StreamExt;
    use zbus::Connection;
    use zbus::zvariant::OwnedValue;
    use std::collections::HashMap;

    pub async fn listen<F>(callback: F) -> Result<()>
    where
        F: Fn(MediaEvent) + Send + Sync + 'static,
    {
        let connection = Connection::session().await?;

        // PropertiesChanged lives on org.freedesktop.DBus.Properties, not on
        // org.mpris.MediaPlayer2.Player — the proxy's interface has to match
        // the signal's actual interface or zbus silently matches nothing.
        let props = zbus::Proxy::new(
            &connection,
            "org.mpris.MediaPlayer2.spotify",
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
        )
        .await?;

        let mut stream = props.receive_signal("PropertiesChanged").await?;
        let mut last_track: Option<(String, String)> = None;

        while let Some(signal) = stream.next().await {
            // Read PlaybackStatus straight out of the signal payload instead
            // of opening a fresh proxy and re-querying on every event.
            let (interface, changed, _invalidated): (
                String,
                HashMap<String, OwnedValue>,
                Vec<String>,
            ) = signal.body().deserialize()?;

            if interface != "org.mpris.MediaPlayer2.Player" {
                continue;
            }

            if let Some(value) = changed.get("PlaybackStatus") {
                if let Ok(status) = String::try_from(value.clone()) {
                    let event = match status.as_str() {
                        "Playing" => Some(MediaEvent::Playing),
                        "Paused" => Some(MediaEvent::Paused),
                        "Stopped" => Some(MediaEvent::Stopped),
                        _ => None,
                    };
                    if let Some(event) = event {
                        callback(event);
                    }
                }
            }

            // Fires on track change.
            if let Some(value) = changed.get("Metadata") {
                if let Ok(metadata) = HashMap::<String, OwnedValue>::try_from(value.clone()) {
                    let title = metadata
                        .get("xesam:title")
                        .and_then(|v| String::try_from(v.clone()).ok())
                        .unwrap_or_default();

                    let artist = metadata
                        .get("xesam:artist")
                        .and_then(|v| <Vec<String>>::try_from(v.clone()).ok())
                        .map(|a| a.join(", "))
                        .unwrap_or_default();

                    let track = (title, artist);
                    if last_track.as_ref() != Some(&track) {
                        callback(MediaEvent::TrackChanged {
                            title: track.0.clone(),
                            artist: track.1.clone(),
                        });
                        last_track = Some(track);
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(windows)]
pub use win::listen;

#[cfg(unix)]
pub use unix::listen;
