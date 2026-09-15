use std::{env, fmt::Display, fs::File, io::Read, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Clone)]
pub struct PgCredentials {
    hostname: String,
    port: usize,
    database: String,
    user: String,
    password: String,
}

impl PgCredentials {
    pub fn to_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.hostname, self.port, self.database
        )
    }
}

impl Display for PgCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = format!(
            "postgres://{}:[ HIDDEN ]@{}:{}/{}",
            self.user, self.hostname, self.port, self.database
        );
        write!(f, "{}", output)
    }
}

pub struct PgPass {
    credentials: Vec<PgCredentials>,
}

impl PgPass {
    fn get_pgpass_file() -> Result<File> {
        let home_dir = env::home_dir().context("Failed to find home directory")?;
        let config_file = home_dir.join(PathBuf::from(".pgpass"));
        File::open(config_file).context("No .pgpass file found")
    }

    fn parse_pgpass_file(mut file: File) -> Result<Vec<PgCredentials>> {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut credentials = Vec::new();

        for line in contents.lines() {
            let parts: Vec<&str> = line.split(":").collect();

            if parts.len() != 5 {
                continue;
            }

            credentials.push(PgCredentials {
                hostname: parts[0].to_string(),
                port: parts[1].parse()?,
                database: parts[2].to_string(),
                user: parts[3].to_string(),
                password: parts[4].to_string(),
            });
        }

        Ok(credentials)
    }

    pub fn load() -> Result<Self> {
        let file = Self::get_pgpass_file()?;
        let credentials = Self::parse_pgpass_file(file)?;

        Ok(Self { credentials })
    }

    pub fn all(&self) -> &[PgCredentials] {
        &self.credentials
    }
}
