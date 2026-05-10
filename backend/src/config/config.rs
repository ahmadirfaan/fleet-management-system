// src/config/config.rs

pub struct AppConfig {
    pub database_url: String,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub server_port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://fleet:fleet_secret@localhost:5432/fleetdb".into()),
            mqtt_host: std::env::var("MQTT_HOST")
                .unwrap_or_else(|_| "localhost".into()),
            mqtt_port: std::env::var("MQTT_PORT")
                .unwrap_or_else(|_| "1883".into())
                .parse()
                .expect("MQTT_PORT must be a valid u16"),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .expect("SERVER_PORT must be a valid u16"),
        }
    }
}