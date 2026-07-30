//! On-disk vault format and session state.
//!
//! Layout under `%APPDATA%/SecretSticky/`:
//! - `vault.json` — header + encrypted note blobs (no plaintext)
//!
//! # Invariant: updates must NEVER corrupt saved stickies
//!
//! This is a **hard product rule** (see repo `SECURITY.md` / `CONTRIBUTING.md`):
//!
//! - Installers and auto-update replace the **app binary only**. They must never
//!   delete or rewrite `vault.json` under the user's AppData directory.
//! - Format changes are **backward-compatible**: always be able to **read** vaults
//!   written by prior supported 0.1.x builds. Prefer optional fields with defaults;
//!   never ship a destructive migration that drops notes or re-keys without the user.
//! - Saves use temp file + replace so a crash mid-write does not leave a half-written
//!   vault as the only copy.
//! - IPC/ACL/UI bugs that block displaying a note are critical regressions; they still
//!   must not wipe or rewrite ciphertext. Prefer fail-closed errors over empty vaults.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::crypto::{
    self, derive_key, encrypt, generate_recovery_key, normalize_recovery_key, MasterKey,
    ARGON2_M_KIB, ARGON2_P, ARGON2_T, SALT_LEN,
};
use crate::error::{AppError, AppResult};

/// On-disk vault format version.
///
/// Bump only with a **backward-compatible** reader path for every older version we
/// still support. Never ship a version that cannot open vaults from prior 0.1.x
/// builds — updates must not corrupt or strand saved stickies.
pub const VAULT_VERSION: u32 = 1;
pub const DEFAULT_IDLE_LOCK_SECS: u64 = 15 * 60; // 15 minutes

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum NoteColor {
    #[default]
    Yellow,
    Green,
    Pink,
    Blue,
    Purple,
    Gray,
    Black,
    DarkGreen,
}

/// Sticky geometry floors/ceilings — keep in sync with commands window builder.
pub const NOTE_MIN_WIDTH: f64 = 345.0;
pub const NOTE_MIN_HEIGHT: f64 = 250.0;
pub const NOTE_MAX_SIZE: f64 = 900.0;
/// Default height for a brand-new sticky (slightly taller than min for typing room).
pub const NOTE_DEFAULT_HEIGHT: f64 = 280.0;

/// Clamp sticky width/height. Non-finite values (NaN/±inf) fall back to `default`
/// so a bad IPC payload cannot poison vault geometry or window builder sizes.
pub(crate) fn sanitize_size(value: f64, min: f64, max: f64, default: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

/// Accept finite positions only; drop NaN/±inf so layout stays on-screen-ish.
pub(crate) fn sanitize_position(value: f64, default: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        default
    }
}

impl NoteColor {
    pub fn as_css(&self) -> &'static str {
        match self {
            // Keep in sync with src/types.ts COLORS
            Self::Yellow => "#ffe566",
            Self::Green => "#b8e08a",
            Self::Pink => "#f5a8c0",
            Self::Blue => "#7ec4f5",
            Self::Purple => "#c79be0",
            Self::Gray => "#d4d4d8",
            Self::Black => "#121212",
            Self::DarkGreen => "#163d2c",
        }
    }

    /// Foreground suited to the sticky background (light notes → dark text).
    pub fn text_css(&self) -> &'static str {
        match self {
            Self::Black => "#fafafa",
            Self::DarkGreen => "#ecfdf5",
            Self::Yellow => "#1a1508",
            Self::Green => "#14210f",
            Self::Pink => "#2a0f18",
            Self::Blue => "#0c1a28",
            Self::Purple => "#1c0f24",
            Self::Gray => "#18181b",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub id: String,
    pub color: NoteColor,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub always_on_top: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotePlain {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedNote {
    meta: NoteMeta,
    /// base64(nonce || ciphertext) of NotePlain JSON; AAD = note id
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultHeader {
    version: u32,
    salt_b64: String,
    /// Verifier: encrypt fixed string with *password-derived* key; AAD = "secretsticky-verifier"
    verifier_b64: String,
    /// Optional: encrypt fixed string with recovery-derived key (proves recovery key)
    recovery_verifier_b64: Option<String>,
    /// Content/master key bytes wrapped with recovery-derived key (AAD = "secretsticky-wrap")
    wrapped_master_b64: Option<String>,
    /// When set, password-derived key unwraps the stable content key (AAD = "secretsticky-pw-wrap").
    /// Absent on legacy vaults where the password-derived key *is* the content key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password_wrapped_key_b64: Option<String>,
    argon2_m_kib: u32,
    argon2_t: u32,
    argon2_p: u32,
    idle_lock_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultFile {
    header: VaultHeader,
    notes: Vec<EncryptedNote>,
}

/// Public status for UI (never includes secrets).
#[derive(Debug, Clone, Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
    pub note_count: usize,
    pub idle_lock_secs: u64,
    pub has_recovery_key: bool,
}

/// Note list item when unlocked (includes body — only for the owning note window).
#[derive(Debug, Clone, Serialize)]
pub struct NoteDto {
    pub id: String,
    pub title: String,
    pub body: String,
    pub color: NoteColor,
    pub color_css: String,
    pub color_text_css: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub always_on_top: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manager list item — title + chrome only (no body).
#[derive(Debug, Clone, Serialize)]
pub struct NotePreviewDto {
    pub id: String,
    pub title: String,
    pub color: NoteColor,
    pub color_css: String,
    pub color_text_css: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub always_on_top: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

struct Session {
    key: MasterKey,
    notes: HashMap<String, (NoteMeta, NotePlain)>,
    idle_lock_secs: u64,
    last_activity: Instant,
    has_recovery_key: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        // MasterKey zeroizes on drop; clear plaintext notes
        for (_, (_, plain)) in self.notes.drain() {
            let mut t = plain.title;
            let mut b = plain.body;
            t.zeroize();
            b.zeroize();
        }
    }
}

pub struct Vault {
    path: PathBuf,
    file: Option<VaultFile>,
    session: Option<Session>,
}

impl Vault {
    pub fn open_default() -> AppResult<Self> {
        let dir = dirs::data_dir()
            .ok_or_else(|| AppError::Message("cannot resolve app data dir".into()))?
            .join("SecretSticky");
        fs::create_dir_all(&dir)?;
        let path = dir.join("vault.json");
        Self::open_path(path)
    }

    pub fn open_path(path: PathBuf) -> AppResult<Self> {
        let file = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            Some(serde_json::from_str(&raw)?)
        } else {
            None
        };
        Ok(Self {
            path,
            file,
            session: None,
        })
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self) -> VaultStatus {
        let (unlocked, note_count, idle, has_rec) = if let Some(s) = &self.session {
            (true, s.notes.len(), s.idle_lock_secs, s.has_recovery_key)
        } else if let Some(f) = &self.file {
            (
                false,
                f.notes.len(),
                f.header.idle_lock_secs,
                f.header.recovery_verifier_b64.is_some(),
            )
        } else {
            (false, 0, DEFAULT_IDLE_LOCK_SECS, false)
        };
        VaultStatus {
            initialized: self.file.is_some(),
            unlocked,
            note_count,
            idle_lock_secs: idle,
            has_recovery_key: has_rec,
        }
    }

    pub fn touch(&mut self) {
        if let Some(s) = &mut self.session {
            s.last_activity = Instant::now();
        }
    }

    /// Returns true if session was locked due to idle.
    pub fn check_idle_lock(&mut self) -> bool {
        let should_lock = self
            .session
            .as_ref()
            .map(|s| {
                s.idle_lock_secs > 0
                    && s.last_activity.elapsed() >= Duration::from_secs(s.idle_lock_secs)
            })
            .unwrap_or(false);
        if should_lock {
            self.lock();
            true
        } else {
            false
        }
    }

    pub fn lock(&mut self) {
        self.session = None;
    }

    /// Write vault to disk via temp + replace.
    ///
    /// Never truncates the live `vault.json` in place: a failed write must not
    /// destroy the previous good file (update / crash safety for saved stickies).
    fn persist(&self) -> AppResult<()> {
        let file = self.file.as_ref().ok_or(AppError::NotInitialized)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(file)?;
        fs::write(&tmp, data)?;
        // Windows cannot rename over an existing file — replace atomically-ish.
        replace_file(&tmp, &self.path)?;
        Ok(())
    }

    fn encrypt_note_blob(key: &MasterKey, id: &str, plain: &NotePlain) -> AppResult<String> {
        let pt = serde_json::to_vec(plain)?;
        let blob = encrypt(key, &pt, id.as_bytes())?;
        Ok(B64.encode(blob))
    }

    #[allow(dead_code)]
    fn rebuild_file_notes(&mut self) -> AppResult<()> {
        let session = self.session.as_ref().ok_or(AppError::Locked)?;
        let file = self.file.as_mut().ok_or(AppError::NotInitialized)?;
        let mut enc_notes = Vec::with_capacity(session.notes.len());
        for (id, (meta, plain)) in &session.notes {
            enc_notes.push(EncryptedNote {
                meta: meta.clone(),
                ciphertext_b64: Self::encrypt_note_blob(&session.key, id, plain)?,
            });
        }
        // stable order by updated_at desc
        enc_notes.sort_by_key(|n| std::cmp::Reverse(n.meta.updated_at));
        file.notes = enc_notes;
        Ok(())
    }

    /// Persist a single note's ciphertext + meta without re-encrypting the whole vault.
    fn persist_one_note(&mut self, id: &str) -> AppResult<()> {
        let session = self.session.as_ref().ok_or(AppError::Locked)?;
        let (meta, plain) = session.notes.get(id).ok_or(AppError::NoteNotFound)?;
        let enc = EncryptedNote {
            meta: meta.clone(),
            ciphertext_b64: Self::encrypt_note_blob(&session.key, id, plain)?,
        };
        let file = self.file.as_mut().ok_or(AppError::NotInitialized)?;
        if let Some(slot) = file.notes.iter_mut().find(|n| n.meta.id == id) {
            *slot = enc;
        } else {
            file.notes.push(enc);
        }
        file.notes
            .sort_by_key(|n| std::cmp::Reverse(n.meta.updated_at));
        self.persist()?;
        self.touch();
        Ok(())
    }

    fn remove_note_from_file(&mut self, id: &str) -> AppResult<()> {
        let file = self.file.as_mut().ok_or(AppError::NotInitialized)?;
        file.notes.retain(|n| n.meta.id != id);
        self.persist()?;
        self.touch();
        Ok(())
    }

    #[allow(dead_code)]
    fn save_unlocked(&mut self) -> AppResult<()> {
        self.rebuild_file_notes()?;
        self.persist()?;
        self.touch();
        Ok(())
    }

    /// First-run setup. Returns recovery key (show once).
    pub fn setup(&mut self, password: &str) -> AppResult<String> {
        if self.file.is_some() {
            return Err(AppError::AlreadyInitialized);
        }
        if password.chars().count() < 12 {
            return Err(AppError::Message(
                "password must be at least 12 characters".into(),
            ));
        }

        let salt = crypto::random_array::<SALT_LEN>();
        // Stable content key encrypts notes; password and recovery only wrap it.
        let content_key = MasterKey::from_bytes(crypto::random_array::<{ crypto::KEY_LEN }>());
        let pw_key = derive_key(password, &salt, ARGON2_M_KIB, ARGON2_T, ARGON2_P)?;
        let verifier = encrypt(&pw_key, b"secretsticky-ok", b"secretsticky-verifier")?;
        let password_wrapped = encrypt(&pw_key, content_key.as_bytes(), b"secretsticky-pw-wrap")?;

        let recovery = generate_recovery_key();
        let recovery_norm = normalize_recovery_key(&recovery);
        let recovery_key = derive_key(&recovery_norm, &salt, ARGON2_M_KIB, ARGON2_T, ARGON2_P)?;
        let recovery_verifier = encrypt(
            &recovery_key,
            b"secretsticky-recovery-ok",
            b"secretsticky-recovery",
        )?;
        let wrapped_master = encrypt(&recovery_key, content_key.as_bytes(), b"secretsticky-wrap")?;

        let header = VaultHeader {
            version: VAULT_VERSION,
            salt_b64: B64.encode(salt),
            verifier_b64: B64.encode(verifier),
            recovery_verifier_b64: Some(B64.encode(recovery_verifier)),
            wrapped_master_b64: Some(B64.encode(wrapped_master)),
            password_wrapped_key_b64: Some(B64.encode(password_wrapped)),
            argon2_m_kib: ARGON2_M_KIB,
            argon2_t: ARGON2_T,
            argon2_p: ARGON2_P,
            idle_lock_secs: DEFAULT_IDLE_LOCK_SECS,
        };

        self.file = Some(VaultFile {
            header,
            notes: vec![],
        });
        self.session = Some(Session {
            key: content_key,
            notes: HashMap::new(),
            idle_lock_secs: DEFAULT_IDLE_LOCK_SECS,
            last_activity: Instant::now(),
            has_recovery_key: true,
        });
        self.persist()?;
        Ok(recovery)
    }

    pub fn unlock(&mut self, password: &str) -> AppResult<()> {
        if self.session.is_some() {
            return Err(AppError::AlreadyUnlocked);
        }
        let file = self.file.as_ref().ok_or(AppError::NotInitialized)?;
        let salt = B64
            .decode(&file.header.salt_b64)
            .map_err(|e| AppError::Crypto(format!("salt: {e}")))?;
        let pw_key = derive_key(
            password,
            &salt,
            file.header.argon2_m_kib,
            file.header.argon2_t,
            file.header.argon2_p,
        )?;
        let verifier = B64
            .decode(&file.header.verifier_b64)
            .map_err(|e| AppError::Crypto(format!("verifier: {e}")))?;
        let _ = crypto::decrypt(&pw_key, &verifier, b"secretsticky-verifier")?;

        let content_key = if let Some(wrap_b64) = &file.header.password_wrapped_key_b64 {
            let wrapped = B64
                .decode(wrap_b64)
                .map_err(|e| AppError::Crypto(format!("pw wrap: {e}")))?;
            let bytes = crypto::decrypt(&pw_key, &wrapped, b"secretsticky-pw-wrap")?;
            MasterKey::from_slice(&bytes)?
        } else {
            // Legacy vault: password-derived key is the content key.
            pw_key
        };
        self.load_session(content_key, false)?;
        Ok(())
    }

    /// Unlock using the recovery key (unwraps master key).
    pub fn unlock_with_recovery(&mut self, recovery_key: &str) -> AppResult<()> {
        if self.session.is_some() {
            return Err(AppError::AlreadyUnlocked);
        }
        let file = self.file.clone().ok_or(AppError::NotInitialized)?;
        let wrapped_b64 = file
            .header
            .wrapped_master_b64
            .as_ref()
            .ok_or_else(|| AppError::Message("no recovery key configured".into()))?;

        let salt = B64
            .decode(&file.header.salt_b64)
            .map_err(|e| AppError::Crypto(format!("salt: {e}")))?;
        let norm = normalize_recovery_key(recovery_key);
        if norm.len() != 64 {
            return Err(AppError::BadPassword);
        }
        let rkey = derive_key(
            &norm,
            &salt,
            file.header.argon2_m_kib,
            file.header.argon2_t,
            file.header.argon2_p,
        )?;
        if let Some(ver_b64) = &file.header.recovery_verifier_b64 {
            let verifier = B64
                .decode(ver_b64)
                .map_err(|e| AppError::Crypto(format!("recovery verifier: {e}")))?;
            let _ = crypto::decrypt(&rkey, &verifier, b"secretsticky-recovery")?;
        }
        let wrapped = B64
            .decode(wrapped_b64)
            .map_err(|e| AppError::Crypto(format!("wrapped master: {e}")))?;
        let master_bytes = crypto::decrypt(&rkey, &wrapped, b"secretsticky-wrap")?;
        if master_bytes.len() != crypto::KEY_LEN {
            return Err(AppError::Crypto("bad wrapped master length".into()));
        }
        self.load_session(MasterKey::from_slice(&master_bytes)?, true)?;
        Ok(())
    }

    fn load_session(&mut self, key: MasterKey, has_recovery_override: bool) -> AppResult<()> {
        let file = self.file.as_ref().ok_or(AppError::NotInitialized)?;
        let mut notes = HashMap::new();
        for enc in &file.notes {
            let blob = B64
                .decode(&enc.ciphertext_b64)
                .map_err(|e| AppError::Crypto(format!("note b64: {e}")))?;
            let pt = crypto::decrypt(&key, &blob, enc.meta.id.as_bytes())?;
            let plain: NotePlain = serde_json::from_slice(&pt)?;
            notes.insert(enc.meta.id.clone(), (enc.meta.clone(), plain));
        }
        let has_recovery_key = has_recovery_override || file.header.recovery_verifier_b64.is_some();
        self.session = Some(Session {
            key,
            notes,
            idle_lock_secs: file.header.idle_lock_secs,
            last_activity: Instant::now(),
            has_recovery_key,
        });
        Ok(())
    }

    fn require_session(&mut self) -> AppResult<&mut Session> {
        self.check_idle_lock();
        self.session.as_mut().ok_or(AppError::Locked)
    }

    pub fn list_notes(&mut self) -> AppResult<Vec<NoteDto>> {
        let session = self.require_session()?;
        session.last_activity = Instant::now();
        let mut out: Vec<NoteDto> = session
            .notes
            .values()
            .map(|(meta, plain)| NoteDto {
                id: meta.id.clone(),
                title: plain.title.clone(),
                body: plain.body.clone(),
                color: meta.color.clone(),
                color_css: meta.color.as_css().to_string(),
                color_text_css: meta.color.text_css().to_string(),
                x: meta.x,
                y: meta.y,
                width: meta.width,
                height: meta.height,
                always_on_top: meta.always_on_top,
                created_at: meta.created_at,
                updated_at: meta.updated_at,
            })
            .collect();
        out.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
        Ok(out)
    }

    pub fn get_note(&mut self, id: &str) -> AppResult<NoteDto> {
        let session = self.require_session()?;
        session.last_activity = Instant::now();
        let (meta, plain) = session.notes.get(id).ok_or(AppError::NoteNotFound)?;
        Ok(NoteDto {
            id: meta.id.clone(),
            title: plain.title.clone(),
            body: plain.body.clone(),
            color: meta.color.clone(),
            color_css: meta.color.as_css().to_string(),
            color_text_css: meta.color.text_css().to_string(),
            x: meta.x,
            y: meta.y,
            width: meta.width,
            height: meta.height,
            always_on_top: meta.always_on_top,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        })
    }

    pub fn create_note(&mut self, color: Option<NoteColor>) -> AppResult<NoteDto> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let meta = NoteMeta {
            id: id.clone(),
            color: color.unwrap_or_default(),
            x: 120.0,
            y: 120.0,
            // Match sticky min floor; slightly taller default for typing room.
            width: NOTE_MIN_WIDTH,
            height: NOTE_DEFAULT_HEIGHT,
            always_on_top: true,
            created_at: now,
            updated_at: now,
        };
        let plain = NotePlain {
            title: String::new(),
            body: String::new(),
        };
        {
            let session = self.require_session()?;
            session.notes.insert(id.clone(), (meta, plain));
            session.last_activity = Instant::now();
        }
        self.persist_one_note(&id)?;
        self.get_note(&id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_note(
        &mut self,
        id: &str,
        title: Option<String>,
        body: Option<String>,
        color: Option<NoteColor>,
        x: Option<f64>,
        y: Option<f64>,
        width: Option<f64>,
        height: Option<f64>,
        always_on_top: Option<bool>,
    ) -> AppResult<NoteDto> {
        {
            let session = self.require_session()?;
            let (meta, plain) = session.notes.get_mut(id).ok_or(AppError::NoteNotFound)?;
            if let Some(t) = title {
                plain.title = t;
            }
            if let Some(b) = body {
                plain.body = b;
            }
            if let Some(c) = color {
                meta.color = c;
            }
            if let Some(v) = x {
                meta.x = sanitize_position(v, meta.x);
            }
            if let Some(v) = y {
                meta.y = sanitize_position(v, meta.y);
            }
            if let Some(v) = width {
                // Keep vault geometry at/above sticky min; reject NaN/inf.
                meta.width = sanitize_size(v, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, meta.width);
            }
            if let Some(v) = height {
                meta.height = sanitize_size(v, NOTE_MIN_HEIGHT, NOTE_MAX_SIZE, meta.height);
            }
            if let Some(v) = always_on_top {
                meta.always_on_top = v;
            }
            meta.updated_at = Utc::now();
            session.last_activity = Instant::now();
        }
        self.persist_one_note(id)?;
        self.get_note(id)
    }

    pub fn delete_note(&mut self, id: &str) -> AppResult<()> {
        {
            let session = self.require_session()?;
            let removed = session.notes.remove(id);
            if removed.is_none() {
                return Err(AppError::NoteNotFound);
            }
            if let Some((_, mut plain)) = removed {
                plain.title.zeroize();
                plain.body.zeroize();
            }
            session.last_activity = Instant::now();
        }
        self.remove_note_from_file(id)
    }

    pub fn set_idle_lock_secs(&mut self, secs: u64) -> AppResult<()> {
        {
            let session = self.require_session()?;
            session.idle_lock_secs = secs;
            session.last_activity = Instant::now();
        }
        if let Some(f) = &mut self.file {
            f.header.idle_lock_secs = secs;
        }
        self.persist()
    }

    pub fn change_password(&mut self, current: &str, new_password: &str) -> AppResult<()> {
        if new_password.chars().count() < 12 {
            return Err(AppError::Message(
                "password must be at least 12 characters".into(),
            ));
        }
        // Must already be unlocked so we have the stable content key in session.
        if self.session.is_none() {
            return Err(AppError::Locked);
        }

        let file = self.file.as_ref().ok_or(AppError::NotInitialized)?.clone();
        let salt = B64
            .decode(&file.header.salt_b64)
            .map_err(|e| AppError::Crypto(format!("salt: {e}")))?;
        let cur_pw_key = derive_key(
            current,
            &salt,
            file.header.argon2_m_kib,
            file.header.argon2_t,
            file.header.argon2_p,
        )?;
        let verifier = B64
            .decode(&file.header.verifier_b64)
            .map_err(|e| AppError::Crypto(format!("verifier: {e}")))?;
        let _ = crypto::decrypt(&cur_pw_key, &verifier, b"secretsticky-verifier")
            .map_err(|_| AppError::BadPassword)?;

        // Confirm current password unwraps the same content key the session holds
        // (or is the content key on legacy vaults).
        let content_from_pw = if let Some(wrap_b64) = &file.header.password_wrapped_key_b64 {
            let wrapped = B64
                .decode(wrap_b64)
                .map_err(|e| AppError::Crypto(format!("pw wrap: {e}")))?;
            let bytes = crypto::decrypt(&cur_pw_key, &wrapped, b"secretsticky-pw-wrap")
                .map_err(|_| AppError::BadPassword)?;
            MasterKey::from_slice(&bytes)?
        } else {
            cur_pw_key
        };
        {
            let session = self.session.as_ref().ok_or(AppError::Locked)?;
            if session.key.as_ref() != content_from_pw.as_ref() {
                return Err(AppError::BadPassword);
            }
        }

        let content_key = {
            let session = self.session.as_ref().ok_or(AppError::Locked)?;
            session.key.clone()
        };

        // Rotate password salt/KEK only. Content key + recovery wrap stay put.
        // Recovery KEK is bound to the *old* salt — must keep the same salt for
        // recovery, OR re-wrap recovery under new salt (needs recovery secret).
        // Keep salt stable so recovery key continues to work; only rotate the
        // password-derived wrap. (Salt is not secret; password strength is.)
        let new_pw_key = derive_key(new_password, &salt, ARGON2_M_KIB, ARGON2_T, ARGON2_P)?;
        let wrapped_by_password =
            encrypt(&new_pw_key, content_key.as_bytes(), b"secretsticky-pw-wrap")?;
        let new_verifier = encrypt(&new_pw_key, b"secretsticky-ok", b"secretsticky-verifier")?;

        // Legacy vaults may lack recovery wrap of content key if content key was
        // the old password key — recovery still holds that same key bytes.
        {
            let f = self.file.as_mut().ok_or(AppError::NotInitialized)?;
            f.header.verifier_b64 = B64.encode(new_verifier);
            f.header.password_wrapped_key_b64 = Some(B64.encode(wrapped_by_password));
            f.header.argon2_m_kib = ARGON2_M_KIB;
            f.header.argon2_t = ARGON2_T;
            f.header.argon2_p = ARGON2_P;
            // salt_b64 unchanged — recovery derive_key(recovery, salt) still works.
            // wrapped_master_b64 / recovery_verifier_b64 unchanged.
        }
        {
            let session = self.session.as_mut().ok_or(AppError::Locked)?;
            session.last_activity = Instant::now();
        }
        self.persist()?;
        self.touch();
        Ok(())
    }

    /// Title-only list for manager (no body plaintext over IPC).
    pub fn list_note_previews(&mut self) -> AppResult<Vec<NotePreviewDto>> {
        let session = self.require_session()?;
        session.last_activity = Instant::now();
        let mut out: Vec<NotePreviewDto> = session
            .notes
            .values()
            .map(|(meta, plain)| NotePreviewDto {
                id: meta.id.clone(),
                title: plain.title.clone(),
                color: meta.color.clone(),
                color_css: meta.color.as_css().to_string(),
                color_text_css: meta.color.text_css().to_string(),
                x: meta.x,
                y: meta.y,
                width: meta.width,
                height: meta.height,
                always_on_top: meta.always_on_top,
                created_at: meta.created_at,
                updated_at: meta.updated_at,
            })
            .collect();
        out.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
        Ok(out)
    }
}

/// Replace `from` → `to`. On Windows, `rename` fails if `to` exists.
fn replace_file(from: &Path, to: &Path) -> AppResult<()> {
    #[cfg(windows)]
    {
        if to.exists() {
            fs::remove_file(to)?;
        }
        fs::rename(from, to)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(from, to)?;
        Ok(())
    }
}

/// Brute-force protection.
///
/// After FAIL_THRESHOLD failures within WINDOW_SECS,
/// the unlock is blocked for escalating cooldown periods
/// (COOLDOWN_1 → COOLDOWN_2 → COOLDOWN_3 → MAX_COOLDOWN).
#[derive(Debug)]
pub struct UnlockThrottle {
    failures: Vec<Instant>,
    blocked_until: Option<Instant>,
    /// How many times we've entered a cooldown (drives escalation).
    /// Reset only on successful unlock — not on window expiry alone.
    strikes: u32,
}

impl UnlockThrottle {
    const FAIL_THRESHOLD: usize = 5;
    const WINDOW_SECS: u64 = 60;
    /// Escalating cooldowns after each threshold breach (seconds).
    const COOLDOWN_STEPS: &'static [u64] = &[10, 30, 60, 300];

    pub fn new() -> Self {
        Self {
            failures: Vec::new(),
            blocked_until: None,
            strikes: 0,
        }
    }

    /// Check whether the caller is allowed to attempt an unlock.
    /// Returns an error with a human-readable retry-after message if blocked.
    pub fn check(&mut self) -> AppResult<()> {
        self.evict_stale();
        if let Some(deadline) = self.blocked_until {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                return Err(crate::error::AppError::Message(format!(
                    "Too many failed attempts. Retry in {:.0}s.",
                    remaining.as_secs_f64().ceil()
                )));
            }
            self.blocked_until = None; // cooldown expired
        }
        Ok(())
    }

    /// Record a failed unlock attempt.  Returns the (new) total failures
    /// within the window so callers can log if they want.
    pub fn record_failure(&mut self) -> usize {
        self.failures.push(Instant::now());
        self.evict_stale();

        let count = self.failures.len();
        if count >= Self::FAIL_THRESHOLD {
            // Escalate: 1st block → 10s, 2nd → 30s, 3rd → 60s, 4th+ → 300s.
            // Previously failures were cleared at threshold while matching on
            // count 8..=12 — those arms were dead code. Strikes fix that.
            self.strikes = self.strikes.saturating_add(1);
            let tier = (self.strikes as usize - 1).min(Self::COOLDOWN_STEPS.len() - 1);
            let cooldown = Duration::from_secs(Self::COOLDOWN_STEPS[tier]);
            self.blocked_until = Some(Instant::now() + cooldown);
            self.failures.clear();
        }
        count
    }

    /// Record a successful unlock — clear all failure history and strikes.
    pub fn record_success(&mut self) {
        self.failures.clear();
        self.blocked_until = None;
        self.strikes = 0;
    }

    fn evict_stale(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(Self::WINDOW_SECS);
        self.failures.retain(|t| *t > cutoff);
    }
}

/// Process-wide vault behind a mutex, with unlock throttle.
pub struct VaultState {
    pub vault: Mutex<Vault>,
    pub unlock_throttle: Mutex<UnlockThrottle>,
}

impl VaultState {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            vault: Mutex::new(Vault::open_default()?),
            unlock_throttle: Mutex::new(UnlockThrottle::new()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        let v = Vault::open_path(path).unwrap();
        (dir, v)
    }

    #[test]
    fn setup_unlock_crud() {
        let (_dir, mut v) = test_vault();
        assert!(!v.status().initialized);
        let recovery = v.setup("password1234").unwrap();
        assert!(!recovery.is_empty());
        assert!(v.status().unlocked);

        let n = v.create_note(Some(NoteColor::Pink)).unwrap();
        assert_eq!(n.color, NoteColor::Pink);
        v.update_note(
            &n.id,
            Some("API".into()),
            Some("sk-test-key".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        v.lock();
        assert!(!v.status().unlocked);
        assert!(v.unlock("wrong-password").is_err());
        v.unlock("password1234").unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.title, "API");
        assert_eq!(got.body, "sk-test-key");

        v.delete_note(&n.id).unwrap();
        assert!(v.get_note(&n.id).is_err());
    }

    #[test]
    fn locked_ops_fail() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.lock();
        assert!(v.list_notes().is_err());
        assert!(v.create_note(None).is_err());
    }

    #[test]
    fn recovery_unlock_works() {
        let (_dir, mut v) = test_vault();
        let recovery = v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        v.update_note(
            &n.id,
            Some("t".into()),
            Some("secret-body".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        v.lock();
        v.unlock_with_recovery(&recovery).unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.body, "secret-body");
    }

    #[test]
    fn change_password_keeps_notes_and_recovery() {
        let (_dir, mut v) = test_vault();
        let recovery = v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        v.update_note(
            &n.id,
            Some("API".into()),
            Some("sk-keep-me".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        v.change_password("password1234", "new-password-456")
            .unwrap();
        assert!(v.status().unlocked);
        assert!(v.status().has_recovery_key);

        // Old password fails; new works; recovery still works.
        v.lock();
        assert!(v.unlock("password1234").is_err());
        v.unlock("new-password-456").unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.body, "sk-keep-me");

        v.lock();
        v.unlock_with_recovery(&recovery).unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.title, "API");
        assert_eq!(got.body, "sk-keep-me");
    }

    #[test]
    fn list_previews_omit_body() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        v.update_note(
            &n.id,
            Some("Title only".into()),
            Some("super-secret-body".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let previews = v.list_note_previews().unwrap();
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].title, "Title only");
        // Preview type has no body field — full get still has it.
        assert_eq!(v.get_note(&n.id).unwrap().body, "super-secret-body");
    }

    /// Stand-in for "app update then reopen": process drops Vault, opens same
    /// vault.json path, unlocks with same password — note bodies must be intact.
    /// Invariant: updates must NEVER corrupt saved stickies (SECURITY.md).
    #[test]
    fn single_note_persist_survives_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        let id = {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(Some(NoteColor::Black)).unwrap();
            v.update_note(
                &n.id,
                Some("one".into()),
                Some("body-one".into()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            n.id
        };
        // "Upgrade": new process loads existing AppData vault only — never wipe.
        assert!(path.exists(), "vault.json must remain on disk across restarts");
        let mut v2 = Vault::open_path(path).unwrap();
        v2.unlock("password1234").unwrap();
        let got = v2.get_note(&id).unwrap();
        assert_eq!(got.body, "body-one");
        assert_eq!(got.color, NoteColor::Black);
    }

    /// Format version is intentionally stable; bump only with a reader for older files.
    #[test]
    fn vault_format_version_is_backward_compatible_baseline() {
        assert_eq!(
            VAULT_VERSION, 1,
            "bumping VAULT_VERSION requires a compatible reader for prior on-disk vaults; \
             updates must never strand saved stickies"
        );
    }

    #[test]
    fn persist_replace_overwrites_existing_vault_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        let id = {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(None).unwrap();
            // Second write must replace vault.json (Windows rename-over fails without replace).
            v.update_note(
                &n.id,
                Some("t2".into()),
                Some("body-2".into()),
                None,
                Some(200.0),
                Some(240.0),
                None,
                None,
                None,
            )
            .unwrap();
            n.id
        };
        assert!(path.exists());
        let mut v2 = Vault::open_path(path).unwrap();
        v2.unlock("password1234").unwrap();
        let got = v2.get_note(&id).unwrap();
        assert_eq!(got.title, "t2");
        assert_eq!(got.body, "body-2");
        assert_eq!(got.x, 200.0);
        assert_eq!(got.y, 240.0);
    }

    #[test]
    fn setup_rejects_short_password() {
        let (_dir, mut v) = test_vault();
        let err = v.setup("short").unwrap_err();
        assert!(err.to_string().contains("12"));
        assert!(!v.status().initialized);
    }

    #[test]
    fn double_setup_fails() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(matches!(
            v.setup("password456"),
            Err(AppError::AlreadyInitialized)
        ));
    }

    #[test]
    fn double_unlock_fails() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(matches!(
            v.unlock("password1234"),
            Err(AppError::AlreadyUnlocked)
        ));
    }

    #[test]
    fn bad_recovery_key_fails() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.lock();
        assert!(v.unlock_with_recovery("not-a-real-key").is_err());
        assert!(v
            .unlock_with_recovery(
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
            .is_err());
    }

    #[test]
    fn change_password_rejects_wrong_current() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(v
            .change_password("wrong-current", "new-password-456")
            .is_err());
        // still unlocked with original
        assert!(v.status().unlocked);
        v.lock();
        v.unlock("password1234").unwrap();
    }

    #[test]
    fn change_password_rejects_short_new() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(v.change_password("password1234", "short").is_err());
    }

    #[test]
    fn idle_lock_zero_never_auto_locks() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.set_idle_lock_secs(0).unwrap();
        assert!(!v.check_idle_lock());
        assert!(v.status().unlocked);
    }

    #[test]
    fn idle_lock_triggers_after_elapsed() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.set_idle_lock_secs(1).unwrap();
        // Force last_activity into the past.
        if let Some(s) = v.session.as_mut() {
            s.last_activity = Instant::now() - Duration::from_secs(5);
        }
        assert!(v.check_idle_lock());
        assert!(!v.status().unlocked);
    }

    #[test]
    fn tampered_ciphertext_fails_unlock_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(None).unwrap();
            v.update_note(
                &n.id,
                Some("t".into()),
                Some("body".into()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        // Flip a byte in the on-disk ciphertext.
        let raw = fs::read_to_string(&path).unwrap();
        let mut file: VaultFile = serde_json::from_str(&raw).unwrap();
        assert!(!file.notes.is_empty());
        let mut bytes = B64.decode(&file.notes[0].ciphertext_b64).unwrap();
        let idx = bytes.len() - 1;
        bytes[idx] ^= 0xff;
        file.notes[0].ciphertext_b64 = B64.encode(&bytes);
        fs::write(&path, serde_json::to_string_pretty(&file).unwrap()).unwrap();

        let mut v2 = Vault::open_path(path).unwrap();
        // Password verifier still ok, but note decrypt must fail during load.
        assert!(v2.unlock("password1234").is_err());
    }

    #[test]
    fn color_css_pairs_are_nonempty() {
        for c in [
            NoteColor::Yellow,
            NoteColor::Green,
            NoteColor::Pink,
            NoteColor::Blue,
            NoteColor::Purple,
            NoteColor::Gray,
            NoteColor::Black,
            NoteColor::DarkGreen,
        ] {
            assert!(c.as_css().starts_with('#'));
            assert!(c.text_css().starts_with('#'));
        }
    }

    #[test]
    fn status_reports_counts_when_locked() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.create_note(None).unwrap();
        v.create_note(Some(NoteColor::Blue)).unwrap();
        v.lock();
        let s = v.status();
        assert!(s.initialized);
        assert!(!s.unlocked);
        assert_eq!(s.note_count, 2);
        assert!(s.has_recovery_key);
    }

    #[test]
    fn vault_file_contains_no_plaintext_body() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(None).unwrap();
            v.update_note(
                &n.id,
                Some("SecretTitleXYZ".into()),
                Some("SuperSecretBodyXYZ".into()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        let disk = fs::read_to_string(&path).unwrap();
        assert!(
            !disk.contains("SuperSecretBodyXYZ"),
            "body must not appear in vault.json"
        );
        assert!(
            !disk.contains("SecretTitleXYZ"),
            "title must not appear in vault.json"
        );
        assert!(disk.contains("ciphertext_b64"));
    }

    #[test]
    fn unlock_throttle_allows_under_threshold() {
        let mut t = UnlockThrottle::new();
        for _ in 0..4 {
            assert!(t.check().is_ok());
            t.record_failure();
        }
        // 4 failures — still allowed
        assert!(t.check().is_ok());
    }

    #[test]
    fn unlock_throttle_blocks_after_threshold() {
        let mut t = UnlockThrottle::new();
        for _ in 0..5 {
            let _ = t.check();
            t.record_failure();
        }
        let err = t.check().unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("too many") || err.to_lowercase().contains("retry"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn unlock_throttle_success_clears_failures() {
        let mut t = UnlockThrottle::new();
        for _ in 0..4 {
            t.record_failure();
        }
        t.record_success();
        assert!(t.check().is_ok());
        // Can fail again without immediate block
        for _ in 0..4 {
            t.record_failure();
        }
        assert!(t.check().is_ok());
    }

    #[test]
    fn unlock_throttle_escalates_cooldown() {
        let mut t = UnlockThrottle::new();
        // First block at 5 failures → 10s tier (strike 1)
        for _ in 0..5 {
            t.record_failure();
        }
        let err1 = t.check().unwrap_err().to_string();
        assert!(
            err1.contains("10"),
            "first cooldown should be ~10s, got: {err1}"
        );

        // Expire cooldown; failures already cleared by record_failure
        t.blocked_until = Some(Instant::now() - Duration::from_secs(1));
        assert!(t.check().is_ok());

        // Second block → 30s tier (strike 2)
        for _ in 0..5 {
            t.record_failure();
        }
        let err2 = t.check().unwrap_err().to_string();
        assert!(
            err2.contains("30"),
            "second cooldown should escalate to ~30s, got: {err2}"
        );

        // Success resets strikes
        t.record_success();
        for _ in 0..5 {
            t.record_failure();
        }
        let err3 = t.check().unwrap_err().to_string();
        assert!(
            err3.contains("10"),
            "after success, cooldown should reset to 10s, got: {err3}"
        );
    }

    #[test]
    fn get_missing_note_errors() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(matches!(
            v.get_note("00000000-0000-0000-0000-000000000000"),
            Err(AppError::NoteNotFound)
        ));
    }

    #[test]
    fn delete_missing_note_errors() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        assert!(matches!(
            v.delete_note("00000000-0000-0000-0000-000000000000"),
            Err(AppError::NoteNotFound)
        ));
    }

    #[test]
    fn update_geometry_and_always_on_top() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(Some(NoteColor::Blue)).unwrap();
        v.update_note(
            &n.id,
            None,
            None,
            Some(NoteColor::Purple),
            Some(10.0),
            Some(20.0),
            Some(400.0),
            Some(300.0),
            Some(true),
        )
        .unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.color, NoteColor::Purple);
        assert_eq!(got.x, 10.0);
        assert_eq!(got.y, 20.0);
        assert_eq!(got.width, 400.0);
        assert_eq!(got.height, 300.0);
        assert!(got.always_on_top);
    }

    #[test]
    fn list_notes_sorted_by_updated() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let a = v.create_note(None).unwrap();
        let b = v.create_note(None).unwrap();
        // Touch a so it becomes most recently updated
        v.update_note(
            &a.id,
            Some("later".into()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let list = v.list_notes().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, a.id, "most recently updated first");
        assert_eq!(list[1].id, b.id);
    }

    #[test]
    fn update_clamps_geometry_to_min_max() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        // Below min → floor
        v.update_note(
            &n.id,
            None,
            None,
            None,
            None,
            None,
            Some(10.0),
            Some(10.0),
            None,
        )
        .unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.width, NOTE_MIN_WIDTH);
        assert_eq!(got.height, NOTE_MIN_HEIGHT);

        // Above max → ceiling
        v.update_note(
            &n.id,
            None,
            None,
            None,
            None,
            None,
            Some(5000.0),
            Some(5000.0),
            None,
        )
        .unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.width, NOTE_MAX_SIZE);
        assert_eq!(got.height, NOTE_MAX_SIZE);
    }

    #[test]
    fn create_note_default_geometry_and_always_on_top() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        assert_eq!(n.width, NOTE_MIN_WIDTH);
        assert_eq!(n.height, NOTE_DEFAULT_HEIGHT);
        assert!(n.always_on_top);
        assert_eq!(n.color, NoteColor::Yellow);
        assert_eq!(n.x, 120.0);
        assert_eq!(n.y, 120.0);
    }

    #[test]
    fn set_idle_lock_secs_persists_across_lock() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.set_idle_lock_secs(42).unwrap();
        assert_eq!(v.status().idle_lock_secs, 42);
        v.lock();
        assert_eq!(v.status().idle_lock_secs, 42);
        v.unlock("password1234").unwrap();
        assert_eq!(v.status().idle_lock_secs, 42);
    }

    #[test]
    fn set_idle_lock_secs_requires_unlock() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.lock();
        assert!(matches!(v.set_idle_lock_secs(10), Err(AppError::Locked)));
    }

    #[test]
    fn lock_then_wrong_password_keeps_locked() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.lock();
        assert!(v.unlock("definitely-wrong").is_err());
        assert!(!v.status().unlocked);
        // Correct password still works after a failed attempt
        v.unlock("password1234").unwrap();
        assert!(v.status().unlocked);
    }

    #[test]
    fn empty_title_and_body_roundtrip() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        assert_eq!(n.title, "");
        assert_eq!(n.body, "");
        v.update_note(
            &n.id,
            Some("".into()),
            Some("".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.title, "");
        assert_eq!(got.body, "");
    }

    #[test]
    fn color_css_matches_frontend_palette() {
        // Must stay in lockstep with src/types.ts COLORS
        let expected: &[(&str, &str, &str)] = &[
            ("yellow", "#ffe566", "#1a1508"),
            ("green", "#b8e08a", "#14210f"),
            ("pink", "#f5a8c0", "#2a0f18"),
            ("blue", "#7ec4f5", "#0c1a28"),
            ("purple", "#c79be0", "#1c0f24"),
            ("gray", "#d4d4d8", "#18181b"),
            ("black", "#121212", "#fafafa"),
            ("darkgreen", "#163d2c", "#ecfdf5"),
        ];
        let colors = [
            NoteColor::Yellow,
            NoteColor::Green,
            NoteColor::Pink,
            NoteColor::Blue,
            NoteColor::Purple,
            NoteColor::Gray,
            NoteColor::Black,
            NoteColor::DarkGreen,
        ];
        for (c, (name, bg, fg)) in colors.iter().zip(expected.iter()) {
            assert_eq!(c.as_css(), *bg, "{name} bg");
            assert_eq!(c.text_css(), *fg, "{name} fg");
        }
    }

    #[test]
    fn unicode_password_and_note_content() {
        let (_dir, mut v) = test_vault();
        // 12+ unicode scalar values
        let pw = "пароль🔐ok-extra"; // cyrillic + emoji
        assert!(pw.chars().count() >= 12);
        v.setup(pw).unwrap();
        let n = v.create_note(None).unwrap();
        v.update_note(
            &n.id,
            Some("标题 🔑".into()),
            Some("こんにちは — café".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        v.lock();
        v.unlock(pw).unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.title, "标题 🔑");
        assert_eq!(got.body, "こんにちは — café");
    }

    #[test]
    fn wrong_password_is_bad_password_not_crypto() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.lock();
        let err = v.unlock("wrong-password").unwrap_err();
        assert!(
            matches!(err, AppError::BadPassword),
            "wrong password must surface as BadPassword, got: {err:?}"
        );
        assert!(!v.status().unlocked);
    }

    #[test]
    fn unlock_not_initialized_errors() {
        let (_dir, mut v) = test_vault();
        assert!(matches!(
            v.unlock("password1234"),
            Err(AppError::NotInitialized)
        ));
        assert!(matches!(
            v.unlock_with_recovery("abcd"),
            Err(AppError::NotInitialized)
        ));
    }

    #[test]
    fn operations_require_unlock() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let id = v.create_note(None).unwrap().id;
        v.lock();
        assert!(matches!(v.list_notes(), Err(AppError::Locked)));
        assert!(matches!(v.list_note_previews(), Err(AppError::Locked)));
        assert!(matches!(v.get_note(&id), Err(AppError::Locked)));
        assert!(matches!(v.create_note(None), Err(AppError::Locked)));
        assert!(matches!(
            v.update_note(
                &id,
                Some("x".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None
            ),
            Err(AppError::Locked)
        ));
        assert!(matches!(v.delete_note(&id), Err(AppError::Locked)));
    }

    #[test]
    fn delete_note_removes_from_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        let id = {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(None).unwrap();
            v.update_note(
                &n.id,
                Some("gone".into()),
                Some("bye".into()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            n.id
        };
        {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.unlock("password1234").unwrap();
            v.delete_note(&id).unwrap();
            assert!(matches!(v.get_note(&id), Err(AppError::NoteNotFound)));
            assert_eq!(v.list_notes().unwrap().len(), 0);
        }
        let mut v2 = Vault::open_path(path).unwrap();
        v2.unlock("password1234").unwrap();
        assert_eq!(v2.list_notes().unwrap().len(), 0);
        assert_eq!(v2.status().note_count, 0);
    }

    #[test]
    fn recovery_key_accepts_spaced_and_dashed_input() {
        let (_dir, mut v) = test_vault();
        let recovery = v.setup("password1234").unwrap();
        v.lock();
        // Insert extra spaces / mixed case — normalize_recovery_key must accept.
        let spaced = recovery
            .chars()
            .flat_map(|c| [c, ' '])
            .collect::<String>()
            .to_lowercase();
        v.unlock_with_recovery(&spaced).unwrap();
        assert!(v.status().unlocked);
        v.lock();
        v.unlock_with_recovery(&recovery.to_lowercase()).unwrap();
        assert!(v.status().unlocked);
    }

    #[test]
    fn touch_resets_idle_timer() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        v.set_idle_lock_secs(2).unwrap();
        if let Some(s) = v.session.as_mut() {
            s.last_activity = Instant::now() - Duration::from_secs(5);
        }
        // Without touch, idle would lock; touch refreshes activity.
        v.touch();
        assert!(!v.check_idle_lock());
        assert!(v.status().unlocked);
    }

    #[test]
    fn update_rejects_non_finite_geometry() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let n = v.create_note(None).unwrap();
        let before = v.get_note(&n.id).unwrap();

        v.update_note(
            &n.id,
            None,
            None,
            None,
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NAN),
            Some(f64::NEG_INFINITY),
            None,
        )
        .unwrap();
        let got = v.get_note(&n.id).unwrap();
        assert_eq!(got.x, before.x, "NaN x must keep previous");
        assert_eq!(got.y, before.y, "inf y must keep previous");
        assert_eq!(got.width, before.width, "NaN width must keep previous");
        assert_eq!(got.height, before.height, "inf height must keep previous");
        assert!(got.width.is_finite() && got.height.is_finite());
    }

    #[test]
    fn sanitize_size_and_position_helpers() {
        assert_eq!(
            sanitize_size(10.0, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, 400.0),
            NOTE_MIN_WIDTH
        );
        assert_eq!(
            sanitize_size(5000.0, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, 400.0),
            NOTE_MAX_SIZE
        );
        assert_eq!(
            sanitize_size(f64::NAN, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, 400.0),
            400.0
        );
        assert_eq!(
            sanitize_size(f64::INFINITY, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, 400.0),
            400.0
        );
        assert_eq!(sanitize_position(12.5, 0.0), 12.5);
        assert_eq!(sanitize_position(f64::NAN, 99.0), 99.0);
        assert_eq!(sanitize_position(f64::NEG_INFINITY, 99.0), 99.0);
    }

    #[test]
    fn list_previews_sorted_and_omit_secret_fields() {
        let (_dir, mut v) = test_vault();
        v.setup("password1234").unwrap();
        let a = v.create_note(None).unwrap();
        let b = v.create_note(Some(NoteColor::Pink)).unwrap();
        v.update_note(
            &a.id,
            Some("alpha-secret-title".into()),
            Some("alpha-body-should-not-leak".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // Touch b last so it sorts first
        v.update_note(
            &b.id,
            Some("beta".into()),
            Some("beta-body".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let previews = v.list_note_previews().unwrap();
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].id, b.id);
        assert_eq!(previews[1].id, a.id);
        assert_eq!(previews[1].title, "alpha-secret-title");
        // Compile-time: NotePreviewDto has no body — runtime JSON must not either.
        let json = serde_json::to_string(&previews).unwrap();
        assert!(!json.contains("alpha-body-should-not-leak"));
        assert!(!json.contains("beta-body"));
        assert!(!json.contains("\"body\""));
    }

    #[test]
    fn legacy_vault_without_password_wrap_still_unlocks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.json");
        let note_id = {
            let mut v = Vault::open_path(path.clone()).unwrap();
            v.setup("password1234").unwrap();
            let n = v.create_note(None).unwrap();
            v.update_note(
                &n.id,
                Some("legacy".into()),
                Some("body".into()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            n.id
        };
        // Strip password_wrapped_key_b64 and re-encrypt notes under the
        // password-derived key (true pre-wrap layout).
        let raw = fs::read_to_string(&path).unwrap();
        let mut file: VaultFile = serde_json::from_str(&raw).unwrap();
        file.header.password_wrapped_key_b64 = None;
        let salt = B64.decode(&file.header.salt_b64).unwrap();
        let pw_key = derive_key(
            "password1234",
            &salt,
            file.header.argon2_m_kib,
            file.header.argon2_t,
            file.header.argon2_p,
        )
        .unwrap();
        for note in &mut file.notes {
            let plain = NotePlain {
                title: "legacy".into(),
                body: "body".into(),
            };
            note.ciphertext_b64 = Vault::encrypt_note_blob(&pw_key, &note.meta.id, &plain).unwrap();
        }
        fs::write(&path, serde_json::to_string_pretty(&file).unwrap()).unwrap();

        let mut v = Vault::open_path(path).unwrap();
        v.unlock("password1234").unwrap();
        assert!(v.status().unlocked);
        let got = v.get_note(&note_id).unwrap();
        assert_eq!(got.title, "legacy");
        assert_eq!(got.body, "body");
    }

    #[test]
    fn empty_password_rejected_on_setup_and_change() {
        let (_dir, mut v) = test_vault();
        assert!(matches!(v.setup(""), Err(AppError::Message(_))));
        assert!(matches!(v.setup("short"), Err(AppError::Message(_))));
        v.setup("password1234").unwrap();
        assert!(matches!(
            v.change_password("password1234", "tiny"),
            Err(AppError::Message(_))
        ));
    }
}
