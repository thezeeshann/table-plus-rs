//! Local persistence: connections.json (mirrors TablePlus' Connections.plist
//! fields), settings.json, favorites.json and per-connection history files.
//! Passwords live in connections.json (mode 0600). The OS keychain is deliberately
//! not used: unsigned builds re-prompt for the login password on every rebuild.

use crate::error::{AppError, AppResult};
use crate::model::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Storage {
    root: PathBuf,
    lock: Mutex<()>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ConnectionsFile {
    #[serde(default)]
    connections: Vec<ConnectionConfig>,
    #[serde(default)]
    groups: Vec<ConnectionGroup>,
    /// Connection id -> password.
    #[serde(default)]
    passwords: std::collections::BTreeMap<String, String>,
}

impl Storage {
    pub fn new(root: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(root.join("history"))?;
        Ok(Self { root, lock: Mutex::new(()) })
    }

    fn connections_path(&self) -> PathBuf { self.root.join("connections.json") }
    fn settings_path(&self) -> PathBuf { self.root.join("settings.json") }
    fn favorites_path(&self) -> PathBuf { self.root.join("favorites.json") }
    fn history_path(&self, connection_id: &str) -> PathBuf {
        self.root.join("history").join(format!("{connection_id}.jsonl"))
    }

    fn read_connections(&self) -> AppResult<ConnectionsFile> {
        match fs::read(self.connections_path()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConnectionsFile::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn write_connections(&self, f: &ConnectionsFile) -> AppResult<()> {
        let tmp = self.connections_path().with_extension("json.tmp");
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600); // owner-only from creation: the file holds passwords
        }
        opts.open(&tmp)?.write_all(&serde_json::to_vec_pretty(f)?)?;
        fs::rename(tmp, self.connections_path())?;
        Ok(())
    }

    pub fn list_connections(&self) -> AppResult<(Vec<ConnectionConfig>, Vec<ConnectionGroup>)> {
        let _g = self.lock.lock().unwrap();
        let f = self.read_connections()?;
        Ok((f.connections, f.groups))
    }

    pub fn get_connection(&self, id: &str) -> AppResult<ConnectionConfig> {
        let _g = self.lock.lock().unwrap();
        let f = self.read_connections()?;
        let mut c = f
            .connections
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| AppError::ConnectionNotFound(id.to_string()))?;
        c.password = f.passwords.get(id).cloned();
        Ok(c)
    }

    pub fn save_connection(&self, mut cfg: ConnectionConfig) -> AppResult<ConnectionConfig> {
        let _g = self.lock.lock().unwrap();
        let mut f = self.read_connections()?;
        if cfg.id.is_empty() {
            cfg.id = uuid::Uuid::new_v4().to_string();
        }
        if cfg.created_at.is_none() {
            cfg.created_at = Some(chrono::Utc::now().to_rfc3339());
        }
        if let Some(pw) = cfg.password.take() {
            f.passwords.insert(cfg.id.clone(), pw);
        }
        match f.connections.iter_mut().find(|c| c.id == cfg.id) {
            Some(existing) => *existing = cfg.clone(),
            None => f.connections.push(cfg.clone()),
        }
        self.write_connections(&f)?;
        Ok(cfg)
    }

    pub fn delete_connection(&self, id: &str) -> AppResult<()> {
        let _g = self.lock.lock().unwrap();
        let mut f = self.read_connections()?;
        f.connections.retain(|c| c.id != id);
        f.passwords.remove(id);
        self.write_connections(&f)?;
        let _ = fs::remove_file(self.history_path(id));
        Ok(())
    }

    pub fn save_group(&self, mut g: ConnectionGroup) -> AppResult<ConnectionGroup> {
        let _g = self.lock.lock().unwrap();
        let mut f = self.read_connections()?;
        if g.id.is_empty() {
            g.id = uuid::Uuid::new_v4().to_string();
        }
        match f.groups.iter_mut().find(|x| x.id == g.id) {
            Some(existing) => *existing = g.clone(),
            None => f.groups.push(g.clone()),
        }
        self.write_connections(&f)?;
        Ok(g)
    }

    pub fn delete_group(&self, id: &str) -> AppResult<()> {
        let _g = self.lock.lock().unwrap();
        let mut f = self.read_connections()?;
        f.groups.retain(|x| x.id != id);
        for c in f.connections.iter_mut() {
            if c.group_id.as_deref() == Some(id) {
                c.group_id = None;
            }
        }
        self.write_connections(&f)
    }

    pub fn settings(&self) -> AppResult<AppSettings> {
        match fs::read(self.settings_path()) {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_settings(&self, s: &AppSettings) -> AppResult<()> {
        fs::write(self.settings_path(), serde_json::to_vec_pretty(s)?)?;
        Ok(())
    }

    pub fn favorites(&self) -> AppResult<Vec<FavoriteQuery>> {
        match fs::read(self.favorites_path()) {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_favorite(&self, mut fav: FavoriteQuery) -> AppResult<FavoriteQuery> {
        let _g = self.lock.lock().unwrap();
        let mut list = self.favorites()?;
        if fav.id.is_empty() {
            fav.id = uuid::Uuid::new_v4().to_string();
            fav.created_at = chrono::Utc::now().to_rfc3339();
        }
        match list.iter_mut().find(|f| f.id == fav.id) {
            Some(e) => *e = fav.clone(),
            None => list.push(fav.clone()),
        }
        fs::write(self.favorites_path(), serde_json::to_vec_pretty(&list)?)?;
        Ok(fav)
    }

    pub fn delete_favorite(&self, id: &str) -> AppResult<()> {
        let _g = self.lock.lock().unwrap();
        let mut list = self.favorites()?;
        list.retain(|f| f.id != id);
        fs::write(self.favorites_path(), serde_json::to_vec_pretty(&list)?)?;
        Ok(())
    }

    pub fn append_history(&self, entry: &HistoryEntry) -> AppResult<()> {
        let _g = self.lock.lock().unwrap();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.history_path(&entry.connection_id))?;
        f.write_all(serde_json::to_string(entry)?.as_bytes())?;
        f.write_all(b"\n")?;
        Ok(())
    }

    pub fn history(&self, connection_id: &str, limit: usize, search: Option<&str>) -> AppResult<Vec<HistoryEntry>> {
        let text = match fs::read_to_string(self.history_path(connection_id)) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let needle = search.map(|s| s.to_lowercase()).filter(|s| !s.is_empty());
        let mut out: Vec<HistoryEntry> = text
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str::<HistoryEntry>(l).ok())
            .filter(|h| needle.as_ref().map(|n| h.sql.to_lowercase().contains(n)).unwrap_or(true))
            .take(limit)
            .collect();
        out.shrink_to_fit();
        Ok(out)
    }

    pub fn clear_history(&self, connection_id: &str) -> AppResult<()> {
        let _g = self.lock.lock().unwrap();
        let _ = fs::remove_file(self.history_path(connection_id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(password: Option<&str>) -> ConnectionConfig {
        let mut c: ConnectionConfig = serde_json::from_value(serde_json::json!({ "id": "", "name": "t" })).unwrap();
        c.password = password.map(String::from);
        c
    }

    #[test]
    fn password_round_trips_through_the_file_without_the_keychain() {
        let dir = std::env::temp_dir().join(format!("table-plus-rs-storage-{}", uuid::Uuid::new_v4()));
        let s = Storage::new(dir.clone()).unwrap();
        let saved = s.save_connection(cfg(Some("hunter2"))).unwrap();
        assert_eq!(saved.password, None, "never echoed back in the saved config");
        assert_eq!(s.get_connection(&saved.id).unwrap().password.as_deref(), Some("hunter2"));
        // Editing without retyping the password keeps it.
        let again = s.save_connection(ConnectionConfig { name: "renamed".into(), ..saved.clone() }).unwrap();
        assert_eq!(s.get_connection(&again.id).unwrap().password.as_deref(), Some("hunter2"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("connections.json")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        s.delete_connection(&saved.id).unwrap();
        assert!(s.get_connection(&saved.id).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
