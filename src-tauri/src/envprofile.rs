//! Ready-to-paste configuration for the frameworks people run on CHAMP.
//!
//! One set of facts (web URL, database, enabled add-ons) is rendered in each
//! framework's own convention: Laravel and CodeIgniter `.env` keys, Symfony
//! and CakePHP URL DSNs, WordPress `wp-config.php` constants, a generic DSN
//! block for Slim, Yii, Laminas, and plain PHP, and the `CHAMP_*` variables
//! PHP-FPM already receives. The UI and the agent connector both read this,
//! so they can never disagree.

use crate::addons::AddonEndpoints;
use serde::Serialize;

pub const PROFILE_IDS: [&str; 7] = [
    "laravel",
    "symfony",
    "wordpress",
    "codeigniter",
    "cakephp",
    "dsn",
    "champ",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngine {
    MySql,
    PostgreSql,
}

/// Facts every profile is rendered from.
#[derive(Debug, Clone)]
pub struct ProfileInput {
    pub web_url: String,
    pub engine: DbEngine,
    pub db_port: u16,
    pub database: String,
    /// Server version for Doctrine's `serverVersion` (`8.4.3`), when known.
    pub db_version: Option<String>,
    pub addons: AddonEndpoints,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvProfile {
    pub id: &'static str,
    pub label: &'static str,
    /// Where the lines go, relative to the project folder.
    pub target: &'static str,
    /// `dotenv` or `php`.
    pub format: &'static str,
    pub content: String,
    /// Packages to install or caveats, in English (agents read these too).
    pub notes: Vec<String>,
}

impl ProfileInput {
    fn user(&self) -> &'static str {
        match self.engine {
            DbEngine::MySql => "root",
            DbEngine::PostgreSql => "postgres",
        }
    }

    fn scheme(&self) -> &'static str {
        match self.engine {
            DbEngine::MySql => "mysql",
            DbEngine::PostgreSql => "postgresql",
        }
    }

    /// `mysql://root:@127.0.0.1:3306/app`
    fn database_url(&self) -> String {
        format!(
            "{}://{}:@127.0.0.1:{}/{}",
            self.scheme(),
            self.user(),
            self.db_port,
            self.database
        )
    }
}

fn redis_url(port: u16) -> String {
    format!("redis://127.0.0.1:{port}")
}

fn meili_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// S3 bucket names allow lowercase letters, digits, and hyphens.
fn bucket_name(database: &str) -> String {
    let name: String = database
        .trim_start_matches('_')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if name.len() < 3 {
        format!("{name}-files")
    } else {
        name
    }
}

fn s3_aws_lines(port: u16, database: &str) -> Vec<String> {
    use crate::addons::spec::{S3_ACCESS_KEY, S3_REGION, S3_SECRET_KEY};
    vec![
        format!("AWS_ACCESS_KEY_ID={S3_ACCESS_KEY}"),
        format!("AWS_SECRET_ACCESS_KEY={S3_SECRET_KEY}"),
        format!("AWS_DEFAULT_REGION={S3_REGION}"),
        format!("AWS_BUCKET={}", bucket_name(database)),
        format!("AWS_ENDPOINT=http://127.0.0.1:{port}"),
    ]
}

fn s3_generic_lines(port: u16, database: &str) -> Vec<String> {
    use crate::addons::spec::{S3_ACCESS_KEY, S3_REGION, S3_SECRET_KEY};
    vec![
        format!("S3_ENDPOINT=http://127.0.0.1:{port}"),
        format!("S3_ACCESS_KEY={S3_ACCESS_KEY}"),
        format!("S3_SECRET_KEY={S3_SECRET_KEY}"),
        format!("S3_REGION={S3_REGION}"),
        format!("S3_BUCKET={}", bucket_name(database)),
        "S3_USE_PATH_STYLE=true".to_string(),
    ]
}

fn centrifugo_lines(port: u16) -> Vec<String> {
    vec![
        format!("CENTRIFUGO_API_URL=http://127.0.0.1:{port}/api"),
        format!("CENTRIFUGO_WS_URL=ws://127.0.0.1:{port}/connection/websocket"),
    ]
}

fn laravel(input: &ProfileInput) -> EnvProfile {
    let mut lines = vec![
        format!("APP_URL={}", input.web_url.trim_end_matches('/')),
        format!(
            "DB_CONNECTION={}",
            match input.engine {
                DbEngine::MySql => "mysql",
                DbEngine::PostgreSql => "pgsql",
            }
        ),
        "DB_HOST=127.0.0.1".to_string(),
        format!("DB_PORT={}", input.db_port),
        format!("DB_DATABASE={}", input.database),
        format!("DB_USERNAME={}", input.user()),
        "DB_PASSWORD=".to_string(),
    ];
    let mut notes = Vec::new();
    if let Some(port) = input.addons.redis {
        lines.extend([
            "REDIS_CLIENT=predis".to_string(),
            "REDIS_HOST=127.0.0.1".to_string(),
            "REDIS_PASSWORD=null".to_string(),
            format!("REDIS_PORT={port}"),
            "CACHE_STORE=redis".to_string(),
            "QUEUE_CONNECTION=redis".to_string(),
        ]);
        notes.push(
            "Redis needs the predis/predis package (composer require predis/predis) or the phpredis extension.".to_string(),
        );
    }
    if let Some(mail) = input.addons.mail {
        lines.extend([
            "MAIL_MAILER=smtp".to_string(),
            "MAIL_HOST=127.0.0.1".to_string(),
            format!("MAIL_PORT={}", mail.smtp_port),
            "MAIL_USERNAME=null".to_string(),
            "MAIL_PASSWORD=null".to_string(),
            "MAIL_ENCRYPTION=null".to_string(),
        ]);
    }
    if let Some(port) = input.addons.meilisearch {
        lines.extend([
            "SCOUT_DRIVER=meilisearch".to_string(),
            format!("MEILISEARCH_HOST={}", meili_url(port)),
            // Development mode runs without a master key.
            "MEILISEARCH_KEY=".to_string(),
        ]);
        notes.push(
            "Laravel Scout needs laravel/scout, meilisearch/meilisearch-php, and http-interop/http-factory-guzzle.".to_string(),
        );
    }
    if let Some(port) = input.addons.s3 {
        lines.extend(s3_aws_lines(port, &input.database));
        lines.push("AWS_USE_PATH_STYLE_ENDPOINT=true".to_string());
        notes.push(format!(
            "The s3 disk needs league/flysystem-aws-s3-v3. Create the bucket `{}` in the RustFS console first; set FILESYSTEM_DISK=s3 to make it the default.",
            bucket_name(&input.database)
        ));
    }
    if let Some(port) = input.addons.centrifugo {
        lines.extend(centrifugo_lines(port));
        notes.push(
            "Broadcasting to Centrifugo needs a Centrifugo broadcaster package; it runs without tokens or an API key here.".to_string(),
        );
    }
    EnvProfile {
        id: "laravel",
        label: "Laravel",
        target: ".env",
        format: "dotenv",
        content: lines.join("\n"),
        notes,
    }
}

fn symfony(input: &ProfileInput) -> EnvProfile {
    let mut url = input.database_url();
    match (input.engine, input.db_version.as_deref()) {
        (DbEngine::MySql, Some(version)) => {
            url.push_str(&format!("?serverVersion={version}&charset=utf8mb4"))
        }
        (DbEngine::MySql, None) => url.push_str("?charset=utf8mb4"),
        (DbEngine::PostgreSql, Some(version)) => {
            url.push_str(&format!("?serverVersion={version}&charset=utf8"))
        }
        (DbEngine::PostgreSql, None) => url.push_str("?charset=utf8"),
    }
    let mut lines = vec![
        format!("DEFAULT_URI={}", input.web_url),
        format!("DATABASE_URL=\"{url}\""),
    ];
    let mut notes = Vec::new();
    if let Some(mail) = input.addons.mail {
        lines.push(format!("MAILER_DSN=smtp://127.0.0.1:{}", mail.smtp_port));
    }
    if let Some(port) = input.addons.redis {
        lines.push(format!("REDIS_URL={}", redis_url(port)));
        lines.push(format!(
            "MESSENGER_TRANSPORT_DSN={}/messages",
            redis_url(port)
        ));
        notes.push("Messenger over Redis needs symfony/redis-messenger.".to_string());
    }
    if let Some(port) = input.addons.meilisearch {
        lines.push(format!("MEILISEARCH_URL={}", meili_url(port)));
        lines.push("MEILISEARCH_API_KEY=".to_string());
        notes.push("Search needs meilisearch/search-bundle.".to_string());
    }
    if let Some(port) = input.addons.s3 {
        lines.extend(s3_generic_lines(port, &input.database));
        notes.push(
            "S3 storage works with league/flysystem-bundle and async-aws/s3 (path-style endpoint)."
                .to_string(),
        );
    }
    if let Some(port) = input.addons.centrifugo {
        lines.extend(centrifugo_lines(port));
    }
    EnvProfile {
        id: "symfony",
        label: "Symfony",
        target: ".env.local",
        format: "dotenv",
        content: lines.join("\n"),
        notes,
    }
}

fn wordpress(input: &ProfileInput) -> EnvProfile {
    let quote = |value: &str| value.replace('\\', "\\\\").replace('\'', "\\'");
    let url = quote(input.web_url.trim_end_matches('/'));
    let mut lines = vec![
        format!("define( 'DB_NAME', '{}' );", quote(&input.database)),
        format!("define( 'DB_USER', '{}' );", input.user()),
        "define( 'DB_PASSWORD', '' );".to_string(),
        format!("define( 'DB_HOST', '127.0.0.1:{}' );", input.db_port),
        "define( 'DB_CHARSET', 'utf8mb4' );".to_string(),
        format!("define( 'WP_HOME', '{url}' );"),
        format!("define( 'WP_SITEURL', '{url}' );"),
        "define( 'WP_ENVIRONMENT_TYPE', 'local' );".to_string(),
    ];
    let mut notes = Vec::new();
    if input.engine == DbEngine::PostgreSql {
        notes.push("WordPress needs MySQL: switch CHAMP to MySQL for this project.".to_string());
    }
    if let Some(port) = input.addons.redis {
        lines.push("define( 'WP_REDIS_HOST', '127.0.0.1' );".to_string());
        lines.push(format!("define( 'WP_REDIS_PORT', {port} );"));
        notes.push("Object caching needs the Redis Object Cache plugin.".to_string());
    }
    if input.addons.mail.is_some() {
        notes.push(
            "wp_mail() already reaches Mailpit: CHAMP routes PHP mail() there, no SMTP plugin needed.".to_string(),
        );
    }
    EnvProfile {
        id: "wordpress",
        label: "WordPress",
        target: "wp-config.php",
        format: "php",
        content: lines.join("\n"),
        notes,
    }
}

fn codeigniter(input: &ProfileInput) -> EnvProfile {
    let mut lines = vec![
        "CI_ENVIRONMENT = development".to_string(),
        format!("app.baseURL = '{}'", input.web_url),
        "database.default.hostname = 127.0.0.1".to_string(),
        format!("database.default.database = {}", input.database),
        format!("database.default.username = {}", input.user()),
        "database.default.password = ".to_string(),
        format!(
            "database.default.DBDriver = {}",
            match input.engine {
                DbEngine::MySql => "MySQLi",
                DbEngine::PostgreSql => "Postgre",
            }
        ),
        format!("database.default.port = {}", input.db_port),
    ];
    if let Some(mail) = input.addons.mail {
        lines.extend([
            "email.protocol = smtp".to_string(),
            "email.SMTPHost = 127.0.0.1".to_string(),
            format!("email.SMTPPort = {}", mail.smtp_port),
        ]);
    }
    if let Some(port) = input.addons.redis {
        lines.extend([
            "cache.handler = redis".to_string(),
            "cache.redis.host = 127.0.0.1".to_string(),
            format!("cache.redis.port = {port}"),
        ]);
    }
    EnvProfile {
        id: "codeigniter",
        label: "CodeIgniter 4",
        target: ".env",
        format: "dotenv",
        content: lines.join("\n"),
        notes: Vec::new(),
    }
}

fn cakephp(input: &ProfileInput) -> EnvProfile {
    let mut lines = vec![format!("export DATABASE_URL=\"{}\"", input.database_url())];
    if let Some(mail) = input.addons.mail {
        lines.push(format!(
            "export EMAIL_TRANSPORT_DEFAULT_URL=\"smtp://127.0.0.1:{}\"",
            mail.smtp_port
        ));
    }
    if let Some(port) = input.addons.redis {
        lines.push(format!("export CACHE_DEFAULT_URL=\"{}\"", redis_url(port)));
    }
    EnvProfile {
        id: "cakephp",
        label: "CakePHP",
        target: "config/.env",
        format: "dotenv",
        content: lines.join("\n"),
        notes: vec![
            "Uncomment the .env loader in config/bootstrap.php so CakePHP reads config/.env."
                .to_string(),
        ],
    }
}

fn dsn(input: &ProfileInput) -> EnvProfile {
    let mut lines = vec![
        format!("APP_URL={}", input.web_url.trim_end_matches('/')),
        format!("DATABASE_URL={}", input.database_url()),
        "DB_HOST=127.0.0.1".to_string(),
        format!("DB_PORT={}", input.db_port),
        format!("DB_DATABASE={}", input.database),
        format!("DB_USERNAME={}", input.user()),
        "DB_PASSWORD=".to_string(),
    ];
    if let Some(port) = input.addons.redis {
        lines.push(format!("REDIS_URL={}", redis_url(port)));
    }
    if let Some(mail) = input.addons.mail {
        lines.push(format!("MAILER_DSN=smtp://127.0.0.1:{}", mail.smtp_port));
        lines.push("SMTP_HOST=127.0.0.1".to_string());
        lines.push(format!("SMTP_PORT={}", mail.smtp_port));
    }
    if let Some(port) = input.addons.meilisearch {
        lines.push(format!("MEILISEARCH_URL={}", meili_url(port)));
    }
    if let Some(port) = input.addons.s3 {
        lines.extend(s3_generic_lines(port, &input.database));
    }
    if let Some(port) = input.addons.centrifugo {
        lines.extend(centrifugo_lines(port));
    }
    for (id, _, port) in &input.addons.custom {
        let key = id.to_ascii_uppercase().replace('-', "_");
        lines.push(format!("{key}_HOST=127.0.0.1"));
        lines.push(format!("{key}_PORT={port}"));
    }
    EnvProfile {
        id: "dsn",
        label: "Generic (Slim, Yii, Laminas, plain PHP)",
        target: ".env",
        format: "dotenv",
        content: lines.join("\n"),
        notes: vec![
            "Load it with vlucas/phpdotenv or symfony/dotenv; most database, cache, and mailer libraries accept these URLs.".to_string(),
        ],
    }
}

fn champ(input: &ProfileInput, php_env: &[(String, String)]) -> EnvProfile {
    let mut lines = vec![
        "<?php".to_string(),
        "// CHAMP sets these for every PHP request; no .env file is needed.".to_string(),
        match input.engine {
            DbEngine::MySql => format!(
                "$pdo = new PDO('mysql:host=' . getenv('CHAMP_MYSQL_HOST') . ';port=' . getenv('CHAMP_MYSQL_PORT') . ';dbname={};charset=utf8mb4', getenv('CHAMP_MYSQL_USER'), getenv('CHAMP_MYSQL_PASSWORD'));",
                input.database
            ),
            DbEngine::PostgreSql => format!(
                "$pdo = new PDO('pgsql:host=' . getenv('CHAMP_POSTGRES_HOST') . ';port=' . getenv('CHAMP_POSTGRES_PORT') . ';dbname={}', getenv('CHAMP_POSTGRES_USER'), getenv('CHAMP_POSTGRES_PASSWORD'));",
                input.database
            ),
        },
    ];
    if !php_env.is_empty() {
        lines.push("// Add-ons:".to_string());
        lines.extend(
            php_env
                .iter()
                .map(|(key, value)| format!("// getenv('{key}') === '{value}'")),
        );
    }
    EnvProfile {
        id: "champ",
        label: "CHAMP variables (getenv)",
        target: "any PHP file",
        format: "php",
        content: lines.join("\n"),
        notes: vec![
            "CHAMP_MYSQL_*, CHAMP_POSTGRES_*, CHAMP_WEB_PORT, and the add-on variables are also in $_SERVER.".to_string(),
        ],
    }
}

/// Every profile, in `PROFILE_IDS` order.
pub fn profiles(input: &ProfileInput, php_env: &[(String, String)]) -> Vec<EnvProfile> {
    vec![
        laravel(input),
        symfony(input),
        wordpress(input),
        codeigniter(input),
        cakephp(input),
        dsn(input),
        champ(input, php_env),
    ]
}

/// The profile for a detected framework (`slim`, `yii`, ... use `dsn`).
pub fn profile_for_framework(framework: Option<&str>) -> &'static str {
    match framework {
        Some(id @ ("laravel" | "symfony" | "wordpress" | "codeigniter" | "cakephp")) => PROFILE_IDS
            .iter()
            .find(|known| **known == id)
            .copied()
            .unwrap_or("dsn"),
        Some(_) => "dsn",
        None => "champ",
    }
}

/// A database name derived from a project folder: `my-shop` -> `my_shop`.
pub fn database_name_for(folder: &str) -> String {
    let mut name: String = folder
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(63)
        .collect();
    if name.is_empty() || name.starts_with(|ch: char| ch.is_ascii_digit()) {
        name.insert(0, '_');
        name.truncate(63);
    }
    name
}

/// Facts from the current settings, for the UI and agents.
pub fn current_input(
    web_url: String,
    database: Option<&str>,
    db_version: Option<String>,
) -> ProfileInput {
    let settings = crate::config::AppSettings::load();
    let postgresql = settings.package_selection.is_postgresql();
    ProfileInput {
        web_url,
        engine: if postgresql {
            DbEngine::PostgreSql
        } else {
            DbEngine::MySql
        },
        db_port: if postgresql {
            settings.postgresql_port
        } else {
            settings.mysql_port
        },
        database: database.unwrap_or("app").to_string(),
        db_version,
        addons: AddonEndpoints::from_settings(&settings.addons),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvProfilesReply {
    pub project: Option<String>,
    pub framework: Option<String>,
    pub recommended: &'static str,
    pub profiles: Vec<EnvProfile>,
}

/// Profiles for one project (its domain URL, database named after it, and
/// the detected framework recommended) or for the stack in general.
pub fn profiles_for_project(project: Option<&str>) -> Result<EnvProfilesReply, String> {
    let settings = crate::config::AppSettings::load();
    let web_port = settings.web_port;
    let (web_url, database, framework) = match project {
        Some(name) => {
            let info = crate::projects::detect(name)?;
            let url = crate::projects::domain_url(name, web_port)
                .unwrap_or_else(|| format!("http://localhost:{web_port}/{name}/"));
            (
                url,
                Some(database_name_for(name)),
                info.framework.map(str::to_string),
            )
        }
        None => (format!("http://localhost:{web_port}/"), None, None),
    };
    let input = current_input(web_url, database.as_deref(), None);
    let php_env = crate::addons::env_pairs(&settings.addons);
    Ok(EnvProfilesReply {
        project: project.map(str::to_string),
        recommended: profile_for_framework(framework.as_deref()),
        framework,
        profiles: profiles(&input, &php_env),
    })
}

#[tauri::command]
pub async fn get_env_profiles(project: Option<String>) -> Result<EnvProfilesReply, String> {
    tauri::async_runtime::spawn_blocking(move || profiles_for_project(project.as_deref()))
        .await
        .map_err(|e| format!("Env profile task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addons::MailEndpoint;

    fn input(engine: DbEngine, all_addons: bool) -> ProfileInput {
        ProfileInput {
            web_url: "http://shop.localhost:8080/".to_string(),
            engine,
            db_port: if engine == DbEngine::MySql {
                3306
            } else {
                5432
            },
            database: "shop".to_string(),
            db_version: None,
            addons: if all_addons {
                AddonEndpoints {
                    redis: Some(6390),
                    mail: Some(MailEndpoint {
                        smtp_port: 1025,
                        web_port: 8025,
                    }),
                    meilisearch: Some(7700),
                    centrifugo: Some(8000),
                    s3: Some(9100),
                    custom: vec![("my-s3".to_string(), "S3".to_string(), 9200)],
                }
            } else {
                AddonEndpoints::default()
            },
        }
    }

    #[test]
    fn every_profile_id_is_rendered_once_in_order() {
        let list = profiles(&input(DbEngine::MySql, true), &[]);
        let ids: Vec<&str> = list.iter().map(|profile| profile.id).collect();
        assert_eq!(ids, PROFILE_IDS);
    }

    #[test]
    fn laravel_keeps_its_keys_and_only_lists_enabled_addons() {
        let bare = laravel(&input(DbEngine::MySql, false));
        assert!(bare.content.contains("DB_CONNECTION=mysql"));
        assert!(bare.content.contains("APP_URL=http://shop.localhost:8080"));
        assert!(!bare.content.contains("REDIS"));
        assert!(bare.notes.is_empty());

        let full = laravel(&input(DbEngine::PostgreSql, true));
        assert!(full.content.contains("DB_CONNECTION=pgsql"));
        assert!(full.content.contains("REDIS_PORT=6390"));
        assert_eq!(full.content.matches("CACHE_STORE=").count(), 1);
        assert!(full.content.contains("MAIL_PORT=1025"));
        assert!(full.content.contains("SCOUT_DRIVER=meilisearch"));
    }

    #[test]
    fn symfony_and_cakephp_use_url_dsns() {
        let mut facts = input(DbEngine::MySql, true);
        facts.db_version = Some("8.4.3".to_string());
        let symfony = symfony(&facts);
        assert!(symfony.content.contains(
            "DATABASE_URL=\"mysql://root:@127.0.0.1:3306/shop?serverVersion=8.4.3&charset=utf8mb4\""
        ));
        assert!(symfony.content.contains("MAILER_DSN=smtp://127.0.0.1:1025"));
        assert!(symfony
            .content
            .contains("MESSENGER_TRANSPORT_DSN=redis://127.0.0.1:6390/messages"));

        let cake = cakephp(&input(DbEngine::PostgreSql, true));
        assert!(cake
            .content
            .contains("postgresql://postgres:@127.0.0.1:5432/shop"));
        assert!(cake
            .content
            .contains("CACHE_DEFAULT_URL=\"redis://127.0.0.1:6390\""));
    }

    #[test]
    fn wordpress_warns_about_postgresql_and_quotes_values() {
        let mut facts = input(DbEngine::PostgreSql, true);
        facts.database = "it's".to_string();
        let wp = wordpress(&facts);
        assert!(wp.content.contains("define( 'DB_NAME', 'it\\'s' );"));
        assert!(wp.notes.iter().any(|note| note.contains("MySQL")));
        assert!(wp.notes.iter().any(|note| note.contains("Mailpit")));
    }

    #[test]
    fn generic_and_champ_profiles_include_custom_addons_and_php_env() {
        let facts = input(DbEngine::MySql, true);
        assert!(dsn(&facts).content.contains("MY_S3_PORT=9200"));
        let champ = champ(
            &facts,
            &[("CHAMP_REDIS_PORT".to_string(), "6390".to_string())],
        );
        assert!(champ.content.contains("getenv('CHAMP_MYSQL_HOST')"));
        assert!(champ.content.contains("CHAMP_REDIS_PORT"));
    }

    #[test]
    fn frameworks_map_to_profiles_and_folders_to_database_names() {
        assert_eq!(profile_for_framework(Some("laravel")), "laravel");
        assert_eq!(profile_for_framework(Some("slim")), "dsn");
        assert_eq!(profile_for_framework(None), "champ");
        assert_eq!(database_name_for("My-Shop"), "my_shop");
        assert_eq!(database_name_for("2024site"), "_2024site");
    }
}
